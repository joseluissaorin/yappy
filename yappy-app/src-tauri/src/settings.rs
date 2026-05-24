//! Persisted user settings — wide enough to fit serious customization.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::state::AppState;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlayerPositionPreset {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Custom,
}
impl Default for PlayerPositionPreset {
    fn default() -> Self { Self::BottomRight }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlayerTheme { Cream, Dark, Translucent }
impl Default for PlayerTheme { fn default() -> Self { Self::Cream } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme { Cream, Dark, System }
impl Default for AppTheme { fn default() -> Self { Self::Cream } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OcrEngine { Auto, AppleVision, Paddle }
impl Default for OcrEngine { fn default() -> Self { Self::Auto } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Quality { Fast, Balanced, Best }
impl Default for Quality { fn default() -> Self { Self::Balanced } }
impl Quality {
    pub fn total_steps(self) -> usize {
        match self { Quality::Fast => 5, Quality::Balanced => 8, Quality::Best => 12 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // --- voice & synth ---
    pub voice: String,
    pub voice_overrides: HashMap<String, String>,
    pub speed: f32,
    pub volume: f32,
    pub silence_secs: f32,
    pub default_lang: String,
    pub quality: Quality,

    // --- hotkeys ---
    pub hotkey_read_now: String,
    pub hotkey_pause_resume: String,
    pub hotkey_read_clipboard: String,

    // --- behavior ---
    pub auto_lang_detect: bool,
    pub save_history: bool,
    pub history_max: usize,
    pub successful_reads: u32,
    /// Auto-launch on login (macOS LaunchAgent / Windows registry).
    pub launch_at_login: bool,
    /// Show a system notification when reading completes.
    pub notify_on_done: bool,
    /// Play a tiny sound effect on done / error.
    pub sound_effects: bool,
    /// What to read from a file: full document vs first N kb.
    pub doc_chunk_kb_limit: u32,

    // --- ocr ---
    pub ocr_engine: OcrEngine,
    pub ocr_languages: Vec<String>,

    // --- model ---
    pub model_ready: bool,
    pub first_launch_done: bool,

    // --- main app appearance ---
    pub app_theme: AppTheme,
    pub karaoke_in_player: bool,

    // --- player window appearance & placement ---
    pub player_position_preset: PlayerPositionPreset,
    pub player_position: Option<(i32, i32)>,
    pub player_pinned: bool,
    pub player_compact: bool,
    pub player_theme: PlayerTheme,
    pub player_opacity: f32,
    /// 0 = never auto-hide. Otherwise hide N seconds after the last action when not playing.
    pub player_autohide_secs: u32,
    pub player_show_source: bool,
    pub player_show_waves: bool,
    /// Slim, regular, large. Resizes the floating window accordingly.
    pub player_size: String, // "slim" | "regular" | "large"

    // --- browser extension bridge ---
    /// Random UUID used by the chromium extension to authenticate. Generated on first launch.
    pub bridge_token: String,
    pub bridge_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut voice_overrides = HashMap::new();
        voice_overrides.insert("en".into(), "Alex".into());
        voice_overrides.insert("es".into(), "Daniel".into());
        voice_overrides.insert("fr".into(), "Olivia".into());
        voice_overrides.insert("de".into(), "James".into());
        voice_overrides.insert("it".into(), "Sarah".into());
        voice_overrides.insert("pt".into(), "Sam".into());
        voice_overrides.insert("ja".into(), "Lily".into());
        voice_overrides.insert("ko".into(), "Emily".into());
        Self {
            voice: "Alex".into(),
            voice_overrides,
            speed: 1.05,
            volume: 1.0,
            silence_secs: 0.35,
            default_lang: "en".into(),
            quality: Quality::Balanced,
            hotkey_read_now: "alt+cmd+r".into(),
            hotkey_pause_resume: "alt+cmd+space".into(),
            hotkey_read_clipboard: "alt+cmd+v".into(),
            auto_lang_detect: true,
            save_history: true,
            history_max: 50,
            successful_reads: 0,
            launch_at_login: false,
            notify_on_done: false,
            sound_effects: false,
            doc_chunk_kb_limit: 256,
            ocr_engine: OcrEngine::Auto,
            ocr_languages: vec!["en".into()],
            model_ready: false,
            first_launch_done: false,
            app_theme: AppTheme::Cream,
            karaoke_in_player: true,
            player_position_preset: PlayerPositionPreset::BottomRight,
            player_position: None,
            player_pinned: false,
            player_compact: true,
            player_theme: PlayerTheme::Cream,
            player_opacity: 1.0,
            player_autohide_secs: 0,
            player_show_source: true,
            player_show_waves: true,
            player_size: "regular".into(),
            // Empty by default; first extension hello claims it (auto-pair).
            bridge_token: String::new(),
            bridge_enabled: true,
        }
    }
}

pub struct SettingsStore;

impl SettingsStore {
    pub fn path(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<PathBuf> {
        let mut p = handle.path().app_config_dir()?;
        std::fs::create_dir_all(&p)?;
        p.push("settings.json");
        Ok(p)
    }

    /// Load with backup fallback: if `settings.json` is missing/corrupt,
    /// try `settings.json.bak`. If both fail, defaults.
    pub fn load(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<Settings> {
        let p = Self::path(handle)?;
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(s) = serde_json::from_slice::<Settings>(&bytes) {
                return Ok(s);
            }
            tracing::warn!("settings.json corrupt — trying .bak");
        }
        let bak = p.with_extension("json.bak");
        if let Ok(bytes) = std::fs::read(&bak) {
            if let Ok(s) = serde_json::from_slice::<Settings>(&bytes) {
                tracing::info!("settings restored from .bak");
                return Ok(s);
            }
        }
        Ok(Settings::default())
    }

    /// Atomic write: write to `.tmp` then `rename()`; also keep a one-deep `.bak`.
    /// Verifies the written file deserializes back to identical content.
    pub fn save(handle: &tauri::AppHandle<impl tauri::Runtime>, settings: &Settings) -> Result<()> {
        let p = Self::path(handle)?;
        let tmp = p.with_extension("json.tmp");
        let bak = p.with_extension("json.bak");
        let json = serde_json::to_vec_pretty(settings)?;
        // 1) write to tmp.
        std::fs::write(&tmp, &json)?;
        // 2) verify tmp parses to the same struct (catches disk-full / partial-write).
        let verified: Settings = serde_json::from_slice(&std::fs::read(&tmp)?)?;
        if serde_json::to_vec(&verified)? != serde_json::to_vec(settings)? {
            return Err(anyhow::anyhow!("settings round-trip verification failed"));
        }
        // 3) move current main to .bak (best effort).
        if p.exists() {
            let _ = std::fs::rename(&p, &bak);
        }
        // 4) rename tmp to main.
        std::fs::rename(&tmp, &p)?;
        Ok(())
    }

    pub fn ensure(handle: &tauri::AppHandle<impl tauri::Runtime>, state: &Arc<AppState>) -> Result<()> {
        let path = Self::path(handle)?;
        let first = !path.exists();
        let s = Self::load(handle)?;
        *state.settings.lock().unwrap() = s.clone();
        if first {
            let _ = Self::save(handle, &s);
        }
        Ok(())
    }
}

/// Single mutation point for settings.
///
/// Every Tauri command that changes settings calls this, never reads-modify-writes
/// the file directly. This prevents two concurrent setters from each loading the
/// current state, mutating their own field, and overwriting each other.
pub fn update<R, F>(
    handle: &tauri::AppHandle<impl tauri::Runtime>,
    state: &Arc<AppState>,
    mutate: F,
) -> Result<Settings>
where
    F: FnOnce(&mut Settings) -> R,
{
    // Mutate in-memory under settings.lock.
    {
        let mut s = state.settings.lock().unwrap();
        mutate(&mut s);
    }
    // Serialize disk writes so two concurrent updates can't write stale snapshots
    // in the wrong order. Re-snap AFTER acquiring save_lock so the bytes we write
    // contain every mutation that landed before this save.
    let _guard = state.save_lock.lock().unwrap();
    let snap = state.settings.lock().unwrap().clone();
    SettingsStore::save(handle, &snap)?;
    Ok(snap)
}

/// Save the current in-memory settings as-is (no mutation step).
/// Used for places that already mutated the struct via the older inline pattern,
/// or for explicit "save now" callsites. Same race-free guarantees as `update()`.
pub fn save_current(
    handle: &tauri::AppHandle<impl tauri::Runtime>,
    state: &Arc<AppState>,
) -> Result<Settings> {
    let _guard = state.save_lock.lock().unwrap();
    let snap = state.settings.lock().unwrap().clone();
    SettingsStore::save(handle, &snap)?;
    Ok(snap)
}

/// Bump an in-memory counter WITHOUT persisting to disk.
/// Used for `successful_reads` so the synth task doesn't race with user-facing setters
/// that fire while audio plays. We persist this counter only on natural save points.
pub fn bump_in_memory<R, F>(state: &Arc<AppState>, mutate: F) -> R
where
    F: FnOnce(&mut Settings) -> R,
{
    let mut s = state.settings.lock().unwrap();
    mutate(&mut s)
}

pub fn is_first_launch(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<bool> {
    let s = SettingsStore::load(handle)?;
    Ok(!s.first_launch_done)
}
