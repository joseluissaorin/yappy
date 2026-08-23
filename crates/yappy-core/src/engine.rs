//! High-level engine: input text → normalized chunks → audio.
//!
//! Streaming model: call `synthesize_iter` to get an iterator of `AudioChunk`s that
//! yield as soon as each paragraph is rendered. The Tauri app uses this to start
//! audio playback after the first paragraph and stream more as they arrive.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};

use crate::guion::{construir_desde_texto, trocear, Guion};
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
    /// Rango (en CARACTERES) del texto ORIGINAL de la pieza al que
    /// corresponde este trozo. Es lo que permite el karaoke exacto aunque
    /// la verbalización haya cambiado el texto hablado.
    pub origen_ini: usize,
    pub origen_fin: usize,
    /// true si es un silencio de ritmo (pausa antes de una pieza), no voz.
    pub es_pausa: bool,
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
    ///
    /// El texto plano pasa primero por el guionizador: piezas con ritmo,
    /// idioma por pieza y verbalización con spans.
    pub fn synthesize_streaming<F>(&self, text: &str, opts: &SynthesisOptions, on_chunk: F) -> Result<()>
    where
        F: FnMut(AudioChunk) -> Result<()>,
    {
        let guion = construir_desde_texto(text, &opts.default_lang);
        self.synthesize_guion(&guion, opts, on_chunk)
    }

    /// Sintetiza un Guion completo, con el ritmo EN VIVO: las pausas de
    /// título/cita/separador se emiten como trozos de silencio y la
    /// velocidad por pieza multiplica la global.
    pub fn synthesize_guion<F>(&self, guion: &Guion, opts: &SynthesisOptions, mut on_chunk: F) -> Result<()>
    where
        F: FnMut(AudioChunk) -> Result<()>,
    {
        let estilo_global = self.style_for(self.voice_for(&opts.voice))?;

        // Primera pasada barata: contar trozos (voz + silencios de ritmo)
        // para que el progreso y los índices cuadren con lo emitido.
        let total: usize = guion
            .piezas
            .iter()
            .map(|p| trocear(p).len() + usize::from(p.pausa_antes_s > 0.005))
            .sum();
        let total_paragraphs = guion.piezas.len();
        let sample_rate = self.sample_rate();

        let mut emit_idx = 0usize;
        for (para_idx, pieza) in guion.piezas.iter().enumerate() {
            let trozos = trocear(pieza);

            // La pausa de ritmo, como silencio real, ANTES de la pieza.
            if pieza.pausa_antes_s > 0.005 {
                let n = (pieza.pausa_antes_s * sample_rate as f32) as usize;
                on_chunk(AudioChunk {
                    index: emit_idx,
                    paragraph_index: para_idx,
                    total,
                    total_paragraphs,
                    text: String::new(),
                    lang: pieza.idioma.clone(),
                    samples: vec![0.0; n],
                    sample_rate,
                    origen_ini: 0,
                    origen_fin: 0,
                    es_pausa: true,
                })?;
                emit_idx += 1;
            }
            if trozos.is_empty() {
                continue;
            }

            let estilo = match &pieza.voz {
                Some(v) => self.style_for(self.voice_for(v))?,
                None => estilo_global.clone(),
            };
            let velocidad = (opts.speed * pieza.mult_velocidad).clamp(0.3, 3.0);
            tracing::info!(
                "synth: pieza {} lang={} trozos={} vel={:.2}",
                para_idx,
                pieza.idioma,
                trozos.len(),
                velocidad
            );

            for t in trozos {
                let mut tts = self.tts.lock().unwrap();
                let samples = tts.synthesize_chunk(
                    &t.texto,
                    &pieza.idioma,
                    &estilo,
                    opts.total_steps,
                    velocidad,
                    opts.seed,
                )?;
                let sample_rate = tts.sample_rate;
                drop(tts);
                on_chunk(AudioChunk {
                    index: emit_idx,
                    paragraph_index: para_idx,
                    total,
                    total_paragraphs,
                    text: t.texto,
                    lang: pieza.idioma.clone(),
                    samples,
                    sample_rate,
                    origen_ini: t.origen.start,
                    origen_fin: t.origen.end,
                    es_pausa: false,
                })?;
                emit_idx += 1;
            }
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
