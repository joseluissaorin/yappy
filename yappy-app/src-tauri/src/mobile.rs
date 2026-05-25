//! iOS-only helpers — Share-extension payload pickup, UIPasteboard wrapper,
//! background-audio keepalive, Vision OCR + WKWebView defuddle bridge. This
//! module is gated behind `cfg(mobile)` and never compiled into desktop
//! builds.

use std::sync::Arc;

use crate::state::AppState;

/// Called once on app startup. Checks the App Group container shared with the
/// Share Extension for any payload (text or URL) that was queued while the app
/// was closed, then asks the engine to read it aloud.
///
/// On the first launch (or in the simulator without a paired Share Extension)
/// the container will simply be empty — this is a fast no-op in that case.
pub fn pickup_shared_payload<R: tauri::Runtime>(_handle: &tauri::AppHandle<R>, _state: &Arc<AppState>) {
    // Phase 4 wires the actual Share Extension. For Phase 1 (compile pass) we
    // only need the symbol to exist so `lib.rs::setup` can call it
    // unconditionally on iOS without a `#[cfg]` block.
    tracing::debug!("mobile: pickup_shared_payload (stub) — Share Extension not wired yet");
}

// ─── BACKGROUND-AUDIO KEEPALIVE ──────────────────────────────────────────
//
// iOS suspends UIKit apps within ~30s of going to background unless they're
// "actively producing audio". Audiobook renders can take HOURS, so we play
// a silent audio loop the whole time. The implementation lives in
// `gen/apple/Sources/yappy-app/AudioSession.swift`; here we just declare
// the C ABI and provide RAII wrappers.

extern "C" {
    fn yappy_background_audio_begin();
    fn yappy_background_audio_end();
}

/// RAII guard for background-audio keepalive. Drop = release.
/// Returning this from `start_background_audio()` lets a caller hold it
/// across an async render scope without remembering to call `end()`.
pub struct BackgroundAudioGuard;

impl BackgroundAudioGuard {
    pub fn begin() -> Self {
        unsafe { yappy_background_audio_begin() };
        tracing::info!("mobile: background-audio keepalive engaged");
        Self
    }
}

impl Drop for BackgroundAudioGuard {
    fn drop(&mut self) {
        unsafe { yappy_background_audio_end() };
        tracing::info!("mobile: background-audio keepalive released");
    }
}
