//! Android: la MISMA superficie pública que el mobile.rs de iOS, sin FFI de
//! Swift. Lo que en iOS es App Group + Swift, aquí es:
//!   - payloads compartidos: la MainActivity (Kotlin) escribe líneas en
//!     `<files>/yappy-shared.txt`; drain los lee y borra.
//!   - reproducción de audiolibros: pendiente (v1: el reproductor de
//!     síntesis funciona; los .m4b se abren con la app del sistema).
//!   - Now Playing / Live Activities / Spotlight / hápticos: no-ops de
//!     momento (MediaSession llega en la pasada Android de verdad).

use std::sync::Arc;

use crate::state::AppState;

fn ruta_compartidos() -> Option<std::path::PathBuf> {
    // El paquete es fijo (tauri.conf identifier → com.yappy.app en Android).
    let base = std::path::PathBuf::from("/data/data/com.yappy.app/files");
    base.exists().then(|| base.join("yappy-shared.txt"))
}

pub fn pickup_shared_payload<R: tauri::Runtime>(
    _handle: &tauri::AppHandle<R>,
    _state: &Arc<AppState>,
) {
}

pub fn drain_shared_payload_string() -> Option<String> {
    let ruta = ruta_compartidos()?;
    let contenido = std::fs::read_to_string(&ruta).ok()?;
    let _ = std::fs::remove_file(&ruta);
    (!contenido.trim().is_empty()).then_some(contenido)
}

pub fn transcribe(_path: &str) -> Option<String> {
    None
}
pub fn asr_model_ready() -> bool {
    false
}
pub fn asr_download_model() {}

pub fn now_playing_set(
    _title: &str,
    _artist: &str,
    _album: &str,
    _duration: f64,
    _position: f64,
    _playing: bool,
) {
}

pub fn haptic(_kind: &str) {}
pub fn notify(_identifier: &str, _title: &str, _body: &str) {}
pub fn share_file(_path: &str) {}

pub fn audiofile_play(_path: &str, _start_at_secs: f64) -> bool {
    false
}
pub fn audiofile_pause() {}
pub fn audiofile_resume() {}
pub fn audiofile_stop() {}
pub fn audiofile_seek(_secs: f64) {}
pub fn audiofile_position() -> f64 {
    0.0
}
pub fn audiofile_duration() -> f64 {
    0.0
}
pub fn audiofile_is_playing() -> bool {
    false
}
pub fn audiofile_current_path() -> Option<String> {
    None
}

pub fn spotlight_replace_all(_payload: &str) {}

pub fn install_now_playing_handlers(_playback: Arc<crate::playback::PlaybackController>) {}

pub fn audio_session_activate() {}

pub fn activity_start(_title: &str, _total: i32) {}
pub fn activity_update(_done: i32, _total: i32, _stage: &str, _title: Option<&str>) {}
pub fn activity_end(_title: &str) {}

pub struct BackgroundAudioGuard;
impl BackgroundAudioGuard {
    pub fn begin() -> Self {
        BackgroundAudioGuard
    }
}
impl Drop for BackgroundAudioGuard {
    fn drop(&mut self) {}
}
