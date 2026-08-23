//! Parakeet TDT ONNX inference (desktop) — FastConformer encoder + Token-Duration
//! Transducer greedy decoder.
//!
//! Adapted from `parakeet-rs` (MIT/Apache-2.0, © altunenes), derived from NVIDIA
//! NeMo, trimmed to the TDT path and ported to the workspace `ort = 2.0.0-rc.10`
//! API (direct `shape[i]` indexing on `try_extract_tensor`, same as
//! `supertonic.rs`). Model files come from `istupakov/parakeet-tdt-0.6b-v3-onnx`:
//!   - `encoder-model.onnx` (+ `.onnx.data`) or `encoder-model.int8.onnx`
//!   - `decoder_joint-model.onnx` or `decoder_joint-model.int8.onnx`
//!   - `vocab.txt`

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use ort::value::Value;

use super::audio::{extract_features, MelConfig};
use super::vocab::Vocabulary;
use super::{TimedSegment, TimestampMode, TranscribeOptions, Transcriber, TranscriptResult};

/// Encoder subsampling factor (FastConformer = 8×). Used to map decoded frame
/// indices back to wall-clock time for timestamps.
const ENCODER_STRIDE: usize = 8;
/// LSTM prediction-network state shape: (num_layers, batch, hidden).
const PRED_LAYERS: usize = 2;
const PRED_HIDDEN: usize = 640;
/// Safety cap on tokens emitted at a single encoder frame.
const MAX_TOKENS_PER_STEP: usize = 10;

pub struct ParakeetAsr {
    encoder: Session,
    decoder_joint: Session,
    vocab: Vocabulary,
    mel: MelConfig,
}

impl ParakeetAsr {
    /// Load the Parakeet TDT model from a directory containing the encoder +
    /// decoder_joint ONNX files and `vocab.txt`.
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Err(anyhow!("ASR model dir does not exist: {}", dir.display()));
        }
        let vocab = Vocabulary::from_file(dir.join("vocab.txt"))?;
        let encoder = build_session(&find_one(
            dir,
            &["encoder-model.onnx", "encoder.onnx", "encoder-model.int8.onnx"],
            "encoder",
        )?)?;
        let decoder_joint = build_session(&find_one(
            dir,
            &[
                "decoder_joint-model.onnx",
                "decoder_joint-model.int8.onnx",
                "decoder_joint.onnx",
            ],
            "decoder_joint",
        )?)?;
        Ok(Self {
            encoder,
            decoder_joint,
            vocab,
            mel: MelConfig::default(),
        })
    }

    fn blank_id(&self) -> usize {
        self.vocab.size().saturating_sub(1)
    }

    /// Run the encoder. Returns `(encoder_out[batch, encoder_dim, time], len)`.
    fn run_encoder(&mut self, features: &Array2<f32>) -> Result<Array3<f32>> {
        let time_steps = features.shape()[0];
        let feature_size = features.shape()[1];

        // Encoder expects (batch, features, time).
        let input = features
            .t()
            .to_shape((1, feature_size, time_steps))
            .map_err(|e| anyhow!("reshape encoder input: {e}"))?
            .to_owned();
        let input_length = Array1::from_vec(vec![time_steps as i64]);

        let input_value = Value::from_array(input)?;
        let length_value = Value::from_array(input_length)?;
        let outputs = self.encoder.run(ort::inputs! {
            "audio_signal" => input_value,
            "length" => length_value,
        })?;

        let (shape, data) = outputs["outputs"].try_extract_tensor::<f32>()?;
        if shape.len() != 3 {
            return Err(anyhow!("expected 3D encoder output, got {shape:?}"));
        }
        let arr = Array3::from_shape_vec(
            (shape[0] as usize, shape[1] as usize, shape[2] as usize),
            data.to_vec(),
        )?;
        Ok(arr)
    }

    /// Frame-by-frame TDT greedy decode. Returns the emitted `(token_id, frame)`s.
    fn greedy_decode(&mut self, encoder_out: &Array3<f32>) -> Result<Vec<(usize, usize)>> {
        let encoder_dim = encoder_out.shape()[1];
        let time_steps = encoder_out.shape()[2];
        let vocab_size = self.vocab.size();
        let blank_id = self.blank_id();

        let mut state_h = Array3::<f32>::zeros((PRED_LAYERS, 1, PRED_HIDDEN));
        let mut state_c = Array3::<f32>::zeros((PRED_LAYERS, 1, PRED_HIDDEN));

        let mut emitted: Vec<(usize, usize)> = Vec::new();
        let mut t = 0usize;
        let mut tokens_at_step = 0usize;
        let mut last_token = blank_id as i32;

        while t < time_steps {
            let frame = encoder_out.slice(ndarray::s![0, .., t]).to_owned();
            let frame_reshaped = frame
                .to_shape((1, encoder_dim, 1))
                .map_err(|e| anyhow!("reshape frame: {e}"))?
                .to_owned();
            let targets = Array2::from_shape_vec((1, 1), vec![last_token])
                .map_err(|e| anyhow!("targets: {e}"))?;

            let outputs = self.decoder_joint.run(ort::inputs! {
                "encoder_outputs" => Value::from_array(frame_reshaped)?,
                "targets" => Value::from_array(targets)?,
                "target_length" => Value::from_array(Array1::from_vec(vec![1i32]))?,
                "input_states_1" => Value::from_array(state_h.clone())?,
                "input_states_2" => Value::from_array(state_c.clone())?,
            })?;

            let (_, logits) = outputs["outputs"].try_extract_tensor::<f32>()?;
            // TDT joint output = [vocab_size token logits | duration logits].
            let token_id = argmax(&logits[..vocab_size.min(logits.len())]).unwrap_or(blank_id);
            let duration_step = if logits.len() > vocab_size {
                argmax(&logits[vocab_size..]).unwrap_or(0)
            } else {
                0
            };

            if token_id != blank_id {
                // Advance the prediction-network state only when emitting.
                if let Ok((hs, hd)) = outputs["output_states_1"].try_extract_tensor::<f32>() {
                    state_h = Array3::from_shape_vec(
                        (hs[0] as usize, hs[1] as usize, hs[2] as usize),
                        hd.to_vec(),
                    )?;
                }
                if let Ok((cs, cd)) = outputs["output_states_2"].try_extract_tensor::<f32>() {
                    state_c = Array3::from_shape_vec(
                        (cs[0] as usize, cs[1] as usize, cs[2] as usize),
                        cd.to_vec(),
                    )?;
                }
                emitted.push((token_id, t));
                last_token = token_id as i32;
                tokens_at_step += 1;
            }

            if duration_step > 0 {
                t += duration_step;
                tokens_at_step = 0;
            } else if token_id == blank_id || tokens_at_step >= MAX_TOKENS_PER_STEP {
                t += 1;
                tokens_at_step = 0;
            }
        }

        Ok(emitted)
    }

    /// Convert emitted `(token_id, frame)`s into timed text segments.
    fn detokenize(&self, emitted: &[(usize, usize)]) -> (String, Vec<TimedSegment>) {
        let hop = self.mel.hop_length;
        let sr = self.mel.sampling_rate;
        let frame_secs = |frame: usize| (frame * ENCODER_STRIDE * hop) as f32 / sr as f32;

        let mut full = String::new();
        let mut segments: Vec<TimedSegment> = Vec::new();

        for (i, &(token_id, frame)) in emitted.iter().enumerate() {
            let Some(tok) = self.vocab.id_to_text(token_id) else {
                continue;
            };
            // Skip special tokens like <blk>, <pad> (but keep <unk>).
            if tok.starts_with('<') && tok.ends_with('>') && tok != "<unk>" {
                continue;
            }
            let start = frame_secs(frame);
            let end = emitted
                .get(i + 1)
                .map(|&(_, f)| frame_secs(f))
                .unwrap_or(start + 0.01);

            // SentencePiece: ▁ marks a leading space.
            let mut text = tok.replace('▁', " ");
            // Heuristic: re-space bare digit tokens that follow a word
            // ("at60" → "at 60"); skip single uppercase (A4) but allow "a".
            if !full.is_empty()
                && !text.starts_with(' ')
                && text.chars().all(|c| c.is_ascii_digit())
            {
                let trailing_letters = full
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphabetic())
                    .count();
                let is_article_a = trailing_letters == 1 && full.chars().last() == Some('a');
                if trailing_letters > 1 || is_article_a {
                    text.insert(0, ' ');
                }
            }

            full.push_str(&text);
            segments.push(TimedSegment { text, start, end });
        }

        (full.trim().to_string(), segments)
    }

    /// Merge token segments into word segments (split on leading spaces).
    fn words_from_tokens(segments: &[TimedSegment]) -> Vec<TimedSegment> {
        let mut words: Vec<TimedSegment> = Vec::new();
        for seg in segments {
            let starts_word = seg.text.starts_with(' ') || words.is_empty();
            let trimmed = seg.text.trim_start();
            if starts_word {
                words.push(TimedSegment {
                    text: trimmed.to_string(),
                    start: seg.start,
                    end: seg.end,
                });
            } else if let Some(last) = words.last_mut() {
                last.text.push_str(trimmed);
                last.end = seg.end;
            }
        }
        words.retain(|w| !w.text.is_empty());
        words
    }
}

impl Transcriber for ParakeetAsr {
    fn transcribe_mono16k(
        &mut self,
        samples: &[f32],
        opts: &TranscribeOptions,
    ) -> Result<TranscriptResult> {
        let audio_secs = samples.len() as f32 / self.mel.sampling_rate as f32;
        let features = extract_features(samples, &self.mel).context("mel front-end")?;
        let encoder_out = self.run_encoder(&features).context("encoder")?;
        let emitted = self.greedy_decode(&encoder_out).context("tdt decode")?;
        let (text, token_segments) = self.detokenize(&emitted);

        let segments = match opts.timestamps {
            TimestampMode::Tokens => token_segments,
            TimestampMode::Words => Self::words_from_tokens(&token_segments),
        };

        Ok(TranscriptResult {
            text,
            segments,
            language: opts.language.clone(),
            audio_secs,
        })
    }
}

fn argmax(slice: &[f32]) -> Option<usize> {
    slice
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
}

fn find_one(dir: &Path, candidates: &[&str], what: &str) -> Result<PathBuf> {
    for c in candidates {
        let p = dir.join(c);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(anyhow!(
        "no {what} ONNX file in {} (looked for {candidates:?})",
        dir.display()
    ))
}

/// Build an ONNX session. Execution providers are registered ONCE globally at
/// the ORT environment level by the host app (`lib.rs`: CoreML on macOS, DirectML
/// on Windows, XNNPACK/CUDA on Linux) and sessions inherit them. We must NOT
/// re-register an EP per-session — doing so errors with "Provider ... already
/// registered" on platforms where the env already has it (see supertonic.rs).
fn build_session(path: &Path) -> Result<Session> {
    Session::builder()?
        .commit_from_file(path)
        .with_context(|| format!("loading ONNX session {}", path.display()))
}
