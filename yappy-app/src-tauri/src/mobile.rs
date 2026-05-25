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

// ─── NOW PLAYING (LOCK SCREEN / CONTROL CENTER / AIRPODS) ───────────────
//
// iOS surfaces Yappy's playback in MPNowPlayingInfoCenter when we set
// metadata + activate the AVAudioSession. The system then routes user
// interactions from the lock screen, Control Center, AirPods button
// presses, CarPlay, Apple Watch, etc. through MPRemoteCommandCenter. Each
// command (play / pause / skip ±15s / scrub) gets a callback we register
// once at startup.

extern "C" {
    fn yappy_now_playing_set(
        title: *const std::os::raw::c_char,
        artist: *const std::os::raw::c_char,
        album: *const std::os::raw::c_char,
        duration_secs: f64,
        position_secs: f64,
        is_playing: bool,
    );
    fn yappy_register_play_handler(cb: extern "C" fn());
    fn yappy_register_pause_handler(cb: extern "C" fn());
    fn yappy_register_toggle_play_pause_handler(cb: extern "C" fn());
    fn yappy_register_skip_forward_handler(cb: extern "C" fn());
    fn yappy_register_skip_backward_handler(cb: extern "C" fn());
    fn yappy_register_seek_handler(cb: extern "C" fn(f64));
}

/// Updates the system Now Playing metadata. `title=""` clears it.
pub fn now_playing_set(
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: f64,
    position_secs: f64,
    is_playing: bool,
) {
    use std::ffi::CString;
    if title.is_empty() {
        unsafe {
            yappy_now_playing_set(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0.0,
                0.0,
                false,
            )
        };
        return;
    }
    let t = CString::new(title).unwrap_or_default();
    let a = CString::new(artist).unwrap_or_default();
    let b = CString::new(album).unwrap_or_default();
    unsafe {
        yappy_now_playing_set(
            t.as_ptr(),
            a.as_ptr(),
            b.as_ptr(),
            duration_secs,
            position_secs,
            is_playing,
        )
    };
}

// ─── Remote-command callbacks. iOS calls these from the audio session
// thread; we hop onto a tokio task to avoid blocking. The actual playback
// state lives in AppState; we look it up via a OnceLock set at startup.

use std::sync::OnceLock;
static PLAYBACK: OnceLock<std::sync::Arc<crate::playback::PlaybackController>> = OnceLock::new();

extern "C" fn cb_play() {
    if let Some(p) = PLAYBACK.get() { p.resume(); }
}
extern "C" fn cb_pause() {
    if let Some(p) = PLAYBACK.get() { p.pause(); }
}
extern "C" fn cb_toggle() {
    if let Some(p) = PLAYBACK.get() {
        let s = p.snapshot();
        if s.playing { p.pause(); } else { p.resume(); }
    }
}
extern "C" fn cb_skip_forward() {
    if let Some(p) = PLAYBACK.get() { p.seek(15.0); }
}
extern "C" fn cb_skip_backward() {
    if let Some(p) = PLAYBACK.get() { p.seek(-15.0); }
}
extern "C" fn cb_seek(_absolute_secs: f64) {
    // playback.seek is delta-based, so we'd need to compute the diff from
    // the current position. For now, treat lock-screen scrubbing as a no-op
    // (rarely used during audiobook listening; v0.2 polish).
}

// ─── HAPTICS ────────────────────────────────────────────────────────────
// WKWebView doesn't expose UIImpactFeedbackGenerator via web APIs, so we
// bounce through a Tauri command. Frontend calls `haptic("light")` etc.;
// here we forward to the Swift bridge in Haptics.swift.

extern "C" {
    fn yappy_haptic(kind: *const std::os::raw::c_char);
}

pub fn haptic(kind: &str) {
    use std::ffi::CString;
    if let Ok(c) = CString::new(kind) {
        unsafe { yappy_haptic(c.as_ptr()) };
    }
}

// ─── LOCAL NOTIFICATIONS + SHARE SHEET ──────────────────────────────────
extern "C" {
    fn yappy_notify(
        identifier: *const std::os::raw::c_char,
        title: *const std::os::raw::c_char,
        body: *const std::os::raw::c_char,
    );
    fn yappy_share_file(path: *const std::os::raw::c_char);
}

pub fn notify(identifier: &str, title: &str, body: &str) {
    use std::ffi::CString;
    let id = CString::new(identifier).unwrap_or_default();
    let t = CString::new(title).unwrap_or_default();
    let b = CString::new(body).unwrap_or_default();
    unsafe { yappy_notify(id.as_ptr(), t.as_ptr(), b.as_ptr()) };
}

pub fn share_file(path: &str) {
    use std::ffi::CString;
    if let Ok(c) = CString::new(path) {
        unsafe { yappy_share_file(c.as_ptr()) };
    }
}

// ─── AVAudioPlayer file playback ────────────────────────────────────────
extern "C" {
    fn yappy_audiofile_play(path: *const std::os::raw::c_char, start_at: f64) -> bool;
    fn yappy_audiofile_pause();
    fn yappy_audiofile_resume();
    fn yappy_audiofile_stop();
    fn yappy_audiofile_seek(secs: f64);
    fn yappy_audiofile_position() -> f64;
    fn yappy_audiofile_duration() -> f64;
    fn yappy_audiofile_is_playing() -> bool;
    fn yappy_audiofile_current_path() -> *mut std::os::raw::c_char;
}

pub fn audiofile_play(path: &str, start_at_secs: f64) -> bool {
    use std::ffi::CString;
    let Ok(c) = CString::new(path) else { return false };
    unsafe { yappy_audiofile_play(c.as_ptr(), start_at_secs) }
}
pub fn audiofile_pause() { unsafe { yappy_audiofile_pause() } }
pub fn audiofile_resume() { unsafe { yappy_audiofile_resume() } }
pub fn audiofile_stop() { unsafe { yappy_audiofile_stop() } }
pub fn audiofile_seek(secs: f64) { unsafe { yappy_audiofile_seek(secs) } }
pub fn audiofile_position() -> f64 { unsafe { yappy_audiofile_position() } }
pub fn audiofile_duration() -> f64 { unsafe { yappy_audiofile_duration() } }
pub fn audiofile_is_playing() -> bool { unsafe { yappy_audiofile_is_playing() } }
pub fn audiofile_current_path() -> Option<String> {
    let raw = unsafe { yappy_audiofile_current_path() };
    if raw.is_null() { return None; }
    let s = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { yappy_free_string(raw) };
    Some(s)
}

/// Install lock-screen / Now Playing remote handlers. Call once at startup
/// after `AppState` is constructed.
pub fn install_now_playing_handlers(playback: std::sync::Arc<crate::playback::PlaybackController>) {
    let _ = PLAYBACK.set(playback);
    unsafe {
        yappy_register_play_handler(cb_play);
        yappy_register_pause_handler(cb_pause);
        yappy_register_toggle_play_pause_handler(cb_toggle);
        yappy_register_skip_forward_handler(cb_skip_forward);
        yappy_register_skip_backward_handler(cb_skip_backward);
        yappy_register_seek_handler(cb_seek);
    }
    tracing::info!("mobile: Now Playing remote handlers installed");
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
