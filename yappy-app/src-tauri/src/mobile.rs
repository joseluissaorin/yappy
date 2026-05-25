//! iOS-only helpers — Share-extension payload pickup, UIPasteboard wrapper,
//! Vision OCR + WKWebView defuddle bridge. This module is gated behind
//! `cfg(mobile)` and never compiled into desktop builds.

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
