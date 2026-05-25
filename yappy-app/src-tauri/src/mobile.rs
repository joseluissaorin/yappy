//! iOS-only helpers — Share-extension payload pickup, UIPasteboard wrapper,
//! background-audio keepalive, Vision OCR + WKWebView defuddle bridge. This
//! module is gated behind `cfg(mobile)` and never compiled into desktop
//! builds.

use std::sync::Arc;

use crate::state::AppState;

/// Called on app startup AND whenever the app is foregrounded. Drains the
/// App Group queue that the Share Extension fills when the user picks
/// "Share → Yappy" in any other iOS app, and emits an `ios_shared_payload`
/// Tauri event to the frontend with the contents. The frontend listens for
/// this and routes URLs through the defuddle path / text directly into TTS.
///
/// The Swift side returns a newline-separated string of items, each prefixed
/// "url:<...>" or "text:<...>". An empty queue produces a NULL pointer.
pub fn pickup_shared_payload<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    _state: &Arc<AppState>,
) {
    let raw = unsafe { yappy_drain_shared_payload() };
    if raw.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { yappy_free_string(raw) };
    if s.trim().is_empty() {
        return;
    }
    tracing::info!(
        "mobile: drained {} shared payload(s) from App Group",
        s.lines().count()
    );
    let _ = tauri::Emitter::emit(handle, "ios_shared_payload", s);
}

extern "C" {
    fn yappy_drain_shared_payload() -> *mut std::os::raw::c_char;
    fn yappy_free_string(ptr: *mut std::os::raw::c_char);
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
    // ─── Live Activity (ActivityKit) — see LiveActivityBridge.swift ───
    fn yappy_activity_start(title: *const std::os::raw::c_char, total: i32);
    fn yappy_activity_update(
        done: i32,
        total: i32,
        stage: *const std::os::raw::c_char,
        title: *const std::os::raw::c_char,
    );
    fn yappy_activity_end(title: *const std::os::raw::c_char);
}

/// Start a Live Activity for an audiobook render. The widget shows progress
/// on the Lock Screen + Dynamic Island. `title` is the document name; `total`
/// is the number of paragraphs we expect to synth (0 = indeterminate).
pub fn activity_start(title: &str, total: i32) {
    let c = std::ffi::CString::new(title).unwrap_or_default();
    unsafe { yappy_activity_start(c.as_ptr(), total) };
}

/// Update progress. `stage` is "synth" or "writing"; `title` is optional
/// (passes through unchanged when None).
pub fn activity_update(done: i32, total: i32, stage: &str, title: Option<&str>) {
    let c_stage = std::ffi::CString::new(stage).unwrap_or_default();
    let c_title = title.and_then(|t| std::ffi::CString::new(t).ok());
    let title_ptr = c_title.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    unsafe { yappy_activity_update(done, total, c_stage.as_ptr(), title_ptr) };
}

pub fn activity_end(title: &str) {
    let c = std::ffi::CString::new(title).unwrap_or_default();
    unsafe { yappy_activity_end(c.as_ptr()) };
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
