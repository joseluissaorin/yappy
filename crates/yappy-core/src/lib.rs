//! yappy-core — the Rust core powering Yappy, a local text-to-speech app.
//!
//! Layered:
//!   - `supertonic` — vendored Supertonic 3 ONNX inference (MIT, Supertone Inc.)
//!   - `voices`     — named voice registry (Alex, James, … Emily)
//!   - `normalize`  — text normalization (numbers, dates, currencies, abbreviations, URLs)
//!   - `lang_detect`— per-paragraph language detection (whatlang)
//!   - `chunker`    — paragraph / sentence chunking for streaming synthesis
//!   - `engine`     — high-level Engine tying the above together

pub mod chunker;
pub mod engine;
pub mod lang_detect;
pub mod normalize;
pub mod supertonic;
pub mod voices;

pub use engine::{AudioChunk, EngineConfig, SynthesisOptions, TtsEngine};
pub use voices::{Voice, VOICES};
