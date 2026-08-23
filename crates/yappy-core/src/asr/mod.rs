//! Speech-to-text (ASR) — the reverse of the TTS `engine`/`supertonic` stack.
//!
//! Yappy transcribes audio with NVIDIA **Parakeet TDT** (FastConformer encoder +
//! Token-Duration-Transducer decoder). On desktop (macOS / Windows / Linux) the
//! model runs through ONNX Runtime via the `parakeet` submodule, inheriting the
//! global EP list registered in `yappy-app`'s `lib.rs` (CoreML→ANE on macOS,
//! DirectML on Windows, XNNPACK/CUDA on Linux). On iOS the transcription is done
//! natively in Swift (CoreML, in-process even inside the Share Extension), so the
//! ONNX engine here is **not** compiled for iOS/Android — only the plain result
//! types below are, because the Tauri command signatures reference them on every
//! platform.
//!
//! Layered like the TTS side:
//!   - `audio`    — log-mel spectrogram front-end (NeMo FilterbankFeatures parity)
//!   - `vocab`    — SentencePiece `vocab.txt` (id → token)
//!   - `parakeet` — encoder + TDT greedy decoder over `ort::Session`s
//!
//! The ONNX inference + token-decoding logic is adapted from `parakeet-rs`
//! (MIT/Apache-2.0, © altunenes), itself derived from NVIDIA NeMo, and trimmed to
//! the TDT path Yappy ships. Kept on the workspace's pinned `ort = =2.0.0-rc.10`
//! — the `try_extract_tensor` `(shape, data)` API matches what `supertonic.rs`
//! already uses, so no ORT bump (and no TTS regression risk) is needed.

use serde::{Deserialize, Serialize};

// The ONNX Parakeet engine compiles on every platform (ort links the vendored
// iOS static lib too). On iOS it serves as the on-device engine AND the fallback
// when CoreML is unavailable (e.g. the Simulator): ORT's EP chain uses CoreML
// when present and XNNPACK/CPU otherwise. The Share Extension still uses native
// CoreML (FluidAudio) because of its strict memory budget.
pub mod audio;
pub mod parakeet;
pub mod vocab;

pub use parakeet::ParakeetAsr;

/// One decoded token (or word / segment, depending on `TimestampMode`) with its
/// start/end position in the source audio, in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedSegment {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

/// Granularity of the returned `segments` timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimestampMode {
    /// One segment per emitted sub-word token.
    Tokens,
    /// Segments merged at word boundaries (SentencePiece `▁`).
    Words,
}

impl Default for TimestampMode {
    fn default() -> Self {
        TimestampMode::Words
    }
}

/// Per-call transcription options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscribeOptions {
    /// BCP-47-ish language hint (e.g. "en", "es"). Parakeet TDT v3 is
    /// multilingual and auto-detects, so this is advisory / informational only.
    pub language: Option<String>,
    /// Granularity for `segments`. Defaults to `Words`.
    #[serde(default)]
    pub timestamps: TimestampMode,
}

/// The full result of transcribing one audio input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptResult {
    /// The full transcript text.
    pub text: String,
    /// Per-token / per-word timed segments (see `TimestampMode`).
    pub segments: Vec<TimedSegment>,
    /// Detected/assumed language code, when known.
    pub language: Option<String>,
    /// Length of the decoded audio in seconds.
    pub audio_secs: f32,
}

/// Common transcription surface. Desktop implements this over ONNX; the iOS
/// Swift path bypasses it entirely (it returns text directly across the FFI).
pub trait Transcriber {
    /// Transcribe mono 16 kHz `f32` samples. Callers must resample/downmix first
    /// (desktop does this in `yappy-app` with `rubato`; iOS in AVFoundation).
    fn transcribe_mono16k(
        &mut self,
        samples: &[f32],
        opts: &TranscribeOptions,
    ) -> anyhow::Result<TranscriptResult>;
}

/// Target sample rate the Parakeet front-end expects. Callers resample to this.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;
