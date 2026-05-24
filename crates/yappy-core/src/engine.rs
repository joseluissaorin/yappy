//! High-level engine: input text → normalized chunks → audio.
//!
//! Streaming model: call `synthesize_iter` to get an iterator of `AudioChunk`s that
//! yield as soon as each paragraph is rendered. The Tauri app uses this to start
//! audio playback after the first paragraph and stream more as they arrive.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};

use crate::chunker::{chunk_for_language, Chunk};
use crate::lang_detect::detect_lang;
use crate::normalize::normalize;
use crate::supertonic::{load_voice_style, voice_style_path, Style, TextToSpeech};
use crate::voices::{by_name, default_voice, Voice};

/// Per-call synthesis options.
#[derive(Debug, Clone)]
pub struct SynthesisOptions {
    pub voice: String,
    pub speed: f32,
    /// Default language hint, used when per-paragraph detection is unsure.
    pub default_lang: String,
    /// Number of denoising steps (Supertonic). 5–12, default 8.
    pub total_steps: usize,
    /// Optional deterministic seed (for tests / reproducibility).
    pub seed: Option<u64>,
}

impl Default for SynthesisOptions {
    fn default() -> Self {
        Self {
            voice: default_voice().name.to_string(),
            speed: 1.05,
            default_lang: "en".to_string(),
            total_steps: 8,
            seed: None,
        }
    }
}

/// One synth-chunk's audio plus metadata. The engine splits the input by
/// double-newline first (paragraphs), then within each paragraph by sentence
/// for long ones. `index` is a flat counter across all chunks; `paragraph_index`
/// identifies the parent paragraph so consumers can map audio → paragraph for
/// karaoke highlight.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub index: usize,
    pub paragraph_index: usize,
    pub total: usize,
    pub total_paragraphs: usize,
    pub text: String,
    pub lang: String,
    pub samples: Vec<f32>,
    pub sample_rate: i32,
    pub start_char: usize,
    pub end_char: usize,
}

/// Engine wide configuration.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Directory containing the four ONNX files + tts.json + unicode_indexer.json.
    pub onnx_dir: PathBuf,
    /// Directory containing voice_styles/M1.json …
    pub voice_styles_dir: PathBuf,
}

pub struct TtsEngine {
    cfg: EngineConfig,
    tts: Mutex<TextToSpeech>,
    style_cache: Mutex<HashMap<String, std::sync::Arc<Style>>>,
}

impl TtsEngine {
    pub fn new(cfg: EngineConfig) -> Result<Self> {
        let tts = TextToSpeech::load(&cfg.onnx_dir)
            .with_context(|| format!("loading TextToSpeech from {}", cfg.onnx_dir.display()))?;
        Ok(Self {
            cfg,
            tts: Mutex::new(tts),
            style_cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn sample_rate(&self) -> i32 {
        self.tts.lock().unwrap().sample_rate
    }

    pub fn voice_for(&self, name: &str) -> &'static Voice {
        by_name(name).unwrap_or_else(default_voice)
    }

    fn style_for(&self, voice: &Voice) -> Result<std::sync::Arc<Style>> {
        let mut cache = self.style_cache.lock().unwrap();
        if let Some(s) = cache.get(voice.id) {
            return Ok(s.clone());
        }
        let path = voice_style_path(&self.cfg.voice_styles_dir, voice.id);
        let style = load_voice_style(&path)
            .with_context(|| format!("loading voice style {}", path.display()))?;
        let arc = std::sync::Arc::new(style);
        cache.insert(voice.id.to_string(), arc.clone());
        Ok(arc)
    }

    /// Synthesize the full text and return all paragraphs as a vector. For UI
    /// callers prefer `synthesize_streaming` so audio starts faster.
    pub fn synthesize(&self, text: &str, opts: &SynthesisOptions) -> Result<Vec<AudioChunk>> {
        let mut out = Vec::new();
        self.synthesize_streaming(text, opts, |chunk| {
            out.push(chunk);
            Ok(())
        })?;
        Ok(out)
    }

    /// Stream chunks to a callback as they're rendered. The callback may return
    /// `Err` to abort early (e.g. when the user pressed pause / stop).
    pub fn synthesize_streaming<F>(&self, text: &str, opts: &SynthesisOptions, mut on_chunk: F) -> Result<()>
    where
        F: FnMut(AudioChunk) -> Result<()>,
    {
        let voice = self.voice_for(&opts.voice);
        let style = self.style_for(voice)?;

        // Per-paragraph detect language, normalize, chunk.
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let total: usize = paragraphs
            .iter()
            .map(|p| {
                let lang = detect_lang(p, &opts.default_lang);
                let normed = normalize(p.trim(), &lang);
                chunk_for_language(&normed, &lang).len()
            })
            .sum();

        let total_paragraphs = paragraphs.iter().filter(|p| !p.trim().is_empty()).count();
        let mut emit_idx: usize = 0;
        let mut para_idx: usize = 0;
        for para in paragraphs {
            if para.trim().is_empty() {
                continue;
            }
            let lang = detect_lang(para, &opts.default_lang);
            tracing::info!("synth: paragraph {} lang={} chars={}", para_idx, lang, para.chars().count());
            let normed = normalize(para.trim(), &lang);
            let chunks: Vec<Chunk> = chunk_for_language(&normed, &lang);

            for c in chunks {
                let mut tts = self.tts.lock().unwrap();
                let samples = tts.synthesize_chunk(
                    &c.text,
                    &lang,
                    &style,
                    opts.total_steps,
                    opts.speed,
                    opts.seed,
                )?;
                let sample_rate = tts.sample_rate;
                drop(tts);
                let chunk = AudioChunk {
                    index: emit_idx,
                    paragraph_index: para_idx,
                    total,
                    total_paragraphs,
                    text: c.text.clone(),
                    lang: lang.clone(),
                    samples,
                    sample_rate,
                    start_char: c.start,
                    end_char: c.end,
                };
                emit_idx += 1;
                on_chunk(chunk)?;
            }
            para_idx += 1;
        }
        Ok(())
    }
}

/// Resolve the model directory layout from a single root.
///
/// Yappy ships the model assets into the app data directory:
///     <root>/onnx/        — duration_predictor.onnx, text_encoder.onnx, …, tts.json, unicode_indexer.json
///     <root>/voice_styles/— M1.json, …, F5.json
pub fn engine_config(root: &Path) -> EngineConfig {
    EngineConfig {
        onnx_dir: root.join("onnx"),
        voice_styles_dir: root.join("voice_styles"),
    }
}
