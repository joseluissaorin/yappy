//! Supertonic 3 ONNX inference, vendored from supertone-inc/supertonic
//! (MIT, Copyright (c) 2025 Supertone Inc.) and adapted for streaming usage.
//!
//! The model has four ONNX components run in sequence per chunk of text:
//!   1. `duration_predictor` — predicts duration per text item, given the DP style
//!   2. `text_encoder`        — encodes the text into a latent
//!   3. `vector_estimator`    — denoising loop (8 steps by default)
//!   4. `vocoder`             — turns the latent into 44.1kHz PCM
//!
//! All four sessions are held inside `TextToSpeech` and reused across calls.

use anyhow::{bail, Context, Result};
use ndarray::{Array, Array3};
use ort::{session::Session, value::Value};
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub const AVAILABLE_LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr", "hu",
    "id", "it", "lt", "lv", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi", "na",
];

pub fn is_valid_lang(lang: &str) -> bool {
    AVAILABLE_LANGS.contains(&lang)
}

// ---------- configuration ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ae: AEConfig,
    pub ttl: TTLConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AEConfig {
    pub sample_rate: i32,
    pub base_chunk_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTLConfig {
    pub chunk_compress_factor: i32,
    pub latent_dim: i32,
}

pub fn load_cfg<P: AsRef<Path>>(onnx_dir: P) -> Result<Config> {
    let cfg_path = onnx_dir.as_ref().join("tts.json");
    let file = File::open(&cfg_path)
        .with_context(|| format!("opening tts.json at {}", cfg_path.display()))?;
    let cfg: Config = serde_json::from_reader(BufReader::new(file))?;
    Ok(cfg)
}

// ---------- voice styles ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VoiceStyleData {
    style_ttl: StyleComponent,
    style_dp: StyleComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StyleComponent {
    data: Vec<Vec<Vec<f32>>>,
    dims: Vec<usize>,
    #[serde(rename = "type")]
    _dtype: String,
}

pub struct Style {
    pub ttl: Array3<f32>,
    pub dp: Array3<f32>,
}

pub fn load_voice_style<P: AsRef<Path>>(path: P) -> Result<Style> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("opening voice style {}", path.display()))?;
    let data: VoiceStyleData = serde_json::from_reader(BufReader::new(file))?;

    let ttl_dims = &data.style_ttl.dims;
    let dp_dims = &data.style_dp.dims;
    let ttl_dim1 = ttl_dims[1];
    let ttl_dim2 = ttl_dims[2];
    let dp_dim1 = dp_dims[1];
    let dp_dim2 = dp_dims[2];

    let mut ttl_flat = vec![0.0f32; ttl_dim1 * ttl_dim2];
    let mut dp_flat = vec![0.0f32; dp_dim1 * dp_dim2];

    let mut idx = 0;
    for batch in &data.style_ttl.data {
        for row in batch {
            for &val in row {
                ttl_flat[idx] = val;
                idx += 1;
            }
        }
    }
    idx = 0;
    for batch in &data.style_dp.data {
        for row in batch {
            for &val in row {
                dp_flat[idx] = val;
                idx += 1;
            }
        }
    }

    Ok(Style {
        ttl: Array3::from_shape_vec((1, ttl_dim1, ttl_dim2), ttl_flat)?,
        dp: Array3::from_shape_vec((1, dp_dim1, dp_dim2), dp_flat)?,
    })
}

// ---------- text preprocessing ----------

pub struct UnicodeProcessor {
    indexer: Vec<i64>,
}

impl UnicodeProcessor {
    pub fn new<P: AsRef<Path>>(unicode_indexer_json_path: P) -> Result<Self> {
        let file = File::open(unicode_indexer_json_path)?;
        let indexer: Vec<i64> = serde_json::from_reader(BufReader::new(file))?;
        Ok(Self { indexer })
    }

    pub fn call(&self, text: &str, lang: &str) -> Result<(Vec<Vec<i64>>, Array3<f32>)> {
        let processed = preprocess_text(text, lang)?;
        let length = processed.chars().count();
        let mut row = vec![0i64; length];
        for (j, c) in processed.chars().enumerate() {
            let val = c as usize;
            row[j] = if val < self.indexer.len() {
                self.indexer[val]
            } else {
                -1
            };
        }
        let mask = length_to_mask(&[length], Some(length));
        Ok((vec![row], mask))
    }
}

fn length_to_mask(lengths: &[usize], max_len: Option<usize>) -> Array3<f32> {
    let bsz = lengths.len();
    let max_len = max_len.unwrap_or_else(|| *lengths.iter().max().unwrap_or(&0));
    let mut mask = Array3::<f32>::zeros((bsz, 1, max_len));
    for (i, &len) in lengths.iter().enumerate() {
        for j in 0..len.min(max_len) {
            mask[[i, 0, j]] = 1.0;
        }
    }
    mask
}

pub fn preprocess_text(text: &str, lang: &str) -> Result<String> {
    // Unicode NFKD normalize
    let mut text: String = text.nfkd().collect();

    // Strip a wide emoji range (the model can't read emoji)
    let emoji = Regex::new(
        r"[\x{1F600}-\x{1F64F}\x{1F300}-\x{1F5FF}\x{1F680}-\x{1F6FF}\x{1F700}-\x{1F77F}\x{1F780}-\x{1F7FF}\x{1F800}-\x{1F8FF}\x{1F900}-\x{1F9FF}\x{1FA00}-\x{1FA6F}\x{1FA70}-\x{1FAFF}\x{2600}-\x{26FF}\x{2700}-\x{27BF}\x{1F1E6}-\x{1F1FF}]+",
    )
    .unwrap();
    text = emoji.replace_all(&text, "").to_string();

    // Normalize dashes, quotes and a few separators that confuse the model.
    let pairs = [
        ("–", "-"),
        ("‑", "-"),
        ("—", "-"),
        ("_", " "),
        ("\u{201C}", "\""),
        ("\u{201D}", "\""),
        ("\u{2018}", "'"),
        ("\u{2019}", "'"),
        ("´", "'"),
        ("`", "'"),
        ("[", " "),
        ("]", " "),
        ("|", " "),
        ("/", " "),
        ("#", " "),
        ("→", " "),
        ("←", " "),
    ];
    for (a, b) in &pairs {
        text = text.replace(a, b);
    }
    for s in &["♥", "☆", "♡", "©", "\\"] {
        text = text.replace(s, "");
    }
    text = text
        .replace("@", " at ")
        .replace("e.g.,", "for example,")
        .replace("i.e.,", "that is,");

    // Tidy spacing around punctuation.
    let space_punct = [r" ,", r" \.", r" !", r" \?", r" ;", r" :", r" '"];
    let punct_repl = [",", ".", "!", "?", ";", ":", "'"];
    for (re, rep) in space_punct.iter().zip(punct_repl.iter()) {
        text = Regex::new(re).unwrap().replace_all(&text, *rep).to_string();
    }
    while text.contains("\"\"") {
        text = text.replace("\"\"", "\"");
    }
    while text.contains("''") {
        text = text.replace("''", "'");
    }

    text = Regex::new(r"\s+")
        .unwrap()
        .replace_all(&text, " ")
        .to_string()
        .trim()
        .to_string();

    if !text.is_empty() {
        let tail = Regex::new(r#"[.!?;:,'"\u{201C}\u{201D}\u{2018}\u{2019})\]}…。」』】〉》›»]$"#).unwrap();
        if !tail.is_match(&text) {
            text.push('.');
        }
    }

    if !is_valid_lang(lang) {
        bail!("Invalid language: {}. Supported: {:?}", lang, AVAILABLE_LANGS);
    }
    Ok(format!("<{lang}>{text}</{lang}>"))
}

// ---------- inference ----------

pub struct TextToSpeech {
    cfg: Config,
    text_processor: UnicodeProcessor,
    dp: Session,
    text_enc: Session,
    vector_est: Session,
    vocoder: Session,
    pub sample_rate: i32,
}

impl TextToSpeech {
    pub fn load(onnx_dir: &Path) -> Result<Self> {
        let cfg = load_cfg(onnx_dir)?;
        let sample_rate = cfg.ae.sample_rate;

        // On Apple platforms (macOS + iOS) register the CoreML execution
        // provider so supported ops route to the Neural Engine / GPU. This is
        // a meaningful latency win on M-series Macs and effectively MANDATORY
        // for iOS, where the CPU-only fallback is too slow for real-time TTS.
        // Unsupported ops transparently fall back to CPU — registering the EP
        // is safe even if some Supertonic ops don't map to CoreML.
        let session = |name: &str| -> Result<Session> {
            let p = onnx_dir.join(name);
            let mut builder = Session::builder()?;
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                use ort::execution_providers::coreml::{
                    CoreMLComputeUnits, CoreMLExecutionProvider, CoreMLModelFormat,
                };
                let ep = CoreMLExecutionProvider::default()
                    // All = CPU + GPU + Neural Engine. The runtime picks the
                    // fastest path per op.
                    .with_compute_units(CoreMLComputeUnits::All)
                    // MLProgram (the modern .mlpackage format) supports more
                    // operators and is generally faster than the legacy
                    // NeuralNetwork format.
                    .with_model_format(CoreMLModelFormat::MLProgram)
                    .build();
                // append_execution_provider returns a Result we want to
                // SOFTEN — if CoreML init fails (e.g. on an older OS) we
                // still want the session to load on CPU rather than erroring.
                builder = builder.with_execution_providers([ep])?;
            }
            builder
                .commit_from_file(&p)
                .with_context(|| format!("loading ONNX session {}", p.display()))
        };

        let dp = session("duration_predictor.onnx")?;
        let text_enc = session("text_encoder.onnx")?;
        let vector_est = session("vector_estimator.onnx")?;
        let vocoder = session("vocoder.onnx")?;
        let text_processor = UnicodeProcessor::new(onnx_dir.join("unicode_indexer.json"))?;

        Ok(Self {
            cfg,
            text_processor,
            dp,
            text_enc,
            vector_est,
            vocoder,
            sample_rate,
        })
    }

    /// Synthesize one chunk of text. Returns trimmed PCM samples (44.1 kHz mono f32).
    pub fn synthesize_chunk(
        &mut self,
        text: &str,
        lang: &str,
        style: &Style,
        total_step: usize,
        speed: f32,
        rng_seed: Option<u64>,
    ) -> Result<Vec<f32>> {
        let (text_ids, text_mask) = self.text_processor.call(text, lang)?;
        let bsz = 1;
        let text_len = text_ids[0].len();
        if text_len == 0 {
            return Ok(Vec::new());
        }
        let flat: Vec<i64> = text_ids[0].clone();
        let text_ids_array = Array::from_shape_vec((bsz, text_len), flat)?;

        let text_ids_value = Value::from_array(text_ids_array)?;
        let text_mask_value = Value::from_array(text_mask.clone())?;
        let style_dp_value = Value::from_array(style.dp.clone())?;

        // 1. duration
        let dp_out = self.dp.run(ort::inputs! {
            "text_ids" => &text_ids_value,
            "style_dp" => &style_dp_value,
            "text_mask" => &text_mask_value,
        })?;
        let (_, duration_slice) = dp_out["duration"].try_extract_tensor::<f32>()?;
        let mut duration: Vec<f32> = duration_slice.to_vec();
        for d in duration.iter_mut() {
            *d /= speed.max(0.1);
        }

        // 2. text encoder
        let style_ttl_value = Value::from_array(style.ttl.clone())?;
        let enc_out = self.text_enc.run(ort::inputs! {
            "text_ids" => &text_ids_value,
            "style_ttl" => &style_ttl_value,
            "text_mask" => &text_mask_value,
        })?;
        let (te_shape, te_data) = enc_out["text_emb"].try_extract_tensor::<f32>()?;
        let text_emb = Array3::from_shape_vec(
            (te_shape[0] as usize, te_shape[1] as usize, te_shape[2] as usize),
            te_data.to_vec(),
        )?;

        // 3. denoising loop
        let (mut xt, latent_mask) = sample_noisy_latent(
            &duration,
            self.sample_rate,
            self.cfg.ae.base_chunk_size,
            self.cfg.ttl.chunk_compress_factor,
            self.cfg.ttl.latent_dim,
            rng_seed,
        );
        let total_step_arr = Array::from_elem(bsz, total_step as f32);
        for step in 0..total_step {
            let step_arr = Array::from_elem(bsz, step as f32);
            let xt_v = Value::from_array(xt.clone())?;
            let te_v = Value::from_array(text_emb.clone())?;
            let lm_v = Value::from_array(latent_mask.clone())?;
            let tm_v2 = Value::from_array(text_mask.clone())?;
            let cs_v = Value::from_array(step_arr)?;
            let ts_v = Value::from_array(total_step_arr.clone())?;
            let est = self.vector_est.run(ort::inputs! {
                "noisy_latent" => &xt_v,
                "text_emb" => &te_v,
                "style_ttl" => &style_ttl_value,
                "latent_mask" => &lm_v,
                "text_mask" => &tm_v2,
                "current_step" => &cs_v,
                "total_step" => &ts_v,
            })?;
            let (shape, data) = est["denoised_latent"].try_extract_tensor::<f32>()?;
            xt = Array3::from_shape_vec(
                (shape[0] as usize, shape[1] as usize, shape[2] as usize),
                data.to_vec(),
            )?;
        }

        // 4. vocoder
        let final_latent = Value::from_array(xt)?;
        let voc_out = self.vocoder.run(ort::inputs! { "latent" => &final_latent })?;
        let (_, wav_slice) = voc_out["wav_tts"].try_extract_tensor::<f32>()?;
        let wav: Vec<f32> = wav_slice.to_vec();

        // Trim to predicted duration.
        let n = (self.sample_rate as f32 * duration[0]) as usize;
        Ok(wav.into_iter().take(n).collect())
    }
}

/// Sample a noisy latent tensor, optionally with a deterministic RNG seed.
fn sample_noisy_latent(
    duration: &[f32],
    sample_rate: i32,
    base_chunk_size: i32,
    chunk_compress: i32,
    latent_dim: i32,
    seed: Option<u64>,
) -> (Array3<f32>, Array3<f32>) {
    let bsz = duration.len();
    let max_dur = duration.iter().fold(0.0f32, |a, &b| a.max(b));
    let wav_len_max = (max_dur * sample_rate as f32) as usize;
    let wav_lengths: Vec<usize> = duration
        .iter()
        .map(|&d| (d * sample_rate as f32) as usize)
        .collect();
    let chunk_size = (base_chunk_size * chunk_compress) as usize;
    let latent_len = (wav_len_max + chunk_size - 1) / chunk_size;
    let latent_dim_val = (latent_dim * chunk_compress) as usize;
    let mut noisy = Array3::<f32>::zeros((bsz, latent_dim_val, latent_len));
    let normal = Normal::new(0.0, 1.0).unwrap();
    if let Some(seed) = seed {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        for v in noisy.iter_mut() {
            *v = normal.sample(&mut rng);
        }
    } else {
        let mut rng = rand::thread_rng();
        for v in noisy.iter_mut() {
            *v = normal.sample(&mut rng);
        }
    }
    let latent_lengths: Vec<usize> = wav_lengths
        .iter()
        .map(|&len| (len + chunk_size - 1) / chunk_size)
        .collect();
    let latent_mask = length_to_mask(&latent_lengths, Some(latent_len));
    for b in 0..bsz {
        for d in 0..latent_dim_val {
            for t in 0..latent_len {
                noisy[[b, d, t]] *= latent_mask[[b, 0, t]];
            }
        }
    }
    (noisy, latent_mask)
}

/// Convenience: write a 16-bit PCM WAV.
pub fn write_wav<P: AsRef<Path>>(path: P, samples: &[f32], sample_rate: i32) -> Result<()> {
    use hound::{SampleFormat, WavSpec, WavWriter};
    let spec = WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec)?;
    for &s in samples {
        let v = s.clamp(-1.0, 1.0);
        w.write_sample((v * 32767.0) as i16)?;
    }
    w.finalize()?;
    Ok(())
}

/// Resolve the path to a voice-style JSON given the directory and the voice id ("M1", "F3", …).
pub fn voice_style_path(voice_styles_dir: &Path, voice_id: &str) -> PathBuf {
    voice_styles_dir.join(format!("{voice_id}.json"))
}
