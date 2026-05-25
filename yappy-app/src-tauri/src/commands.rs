//! Tauri command handlers + central read flow.

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use yappy_core::engine::SynthesisOptions;
use yappy_core::voices::Voice;

use crate::bridge::{ConnectionInfo, BRIDGE_PORT};
use crate::capture;
use crate::credits;
use crate::history;
use crate::hotkey;
use crate::model;
use crate::playback::AudioChunk;
use crate::settings::{
    self, AppTheme, OcrEngine, PlayerPositionPreset, PlayerTheme, Quality, Settings, SettingsStore,
};
use crate::state::AppState;
use crate::windows;

#[tauri::command]
pub fn list_voices() -> Vec<Voice> {
    yappy_core::VOICES.iter().cloned().collect()
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| *s = settings)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_speed_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    speed: f32,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.speed = speed.clamp(0.5, 3.0))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_voice_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    voice: String,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.voice = voice)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_voice_override_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    lang: String,
    voice: Option<String>,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| match voice {
        Some(v) => {
            s.voice_overrides.insert(lang, v);
        }
        None => {
            s.voice_overrides.remove(&lang);
        }
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_lang_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    lang: String,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.default_lang = lang)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_quality_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    quality: Quality,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.quality = quality)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_volume_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    volume: f32,
) -> Result<(), String> {
    let clamped = volume.clamp(0.0, 2.0);
    state.playback.set_volume(clamped);
    settings::update(&app, state.inner(), |s| s.volume = clamped)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_silence_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    silence_secs: f32,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| {
        s.silence_secs = silence_secs.clamp(0.0, 2.0)
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn skip_cmd(state: State<'_, Arc<AppState>>, delta_secs: f32) {
    state.playback.seek(delta_secs);
}

// ----- CREDITS -----
#[tauri::command]
pub fn list_credits() -> Vec<&'static credits::Credit> {
    credits::credits().iter().collect()
}
#[tauri::command]
pub fn list_licenses() -> Vec<&'static credits::LicenseDoc> {
    credits::license_docs().iter().collect()
}

// ----- PLAYER + APP APPEARANCE -----
#[tauri::command]
pub fn set_player_preset_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    preset: PlayerPositionPreset,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| {
        s.player_position_preset = preset;
        s.player_position = None;
    })
    .map_err(|e| e.to_string())?;
    if let Some(player) = app.get_webview_window("player") {
        let _ = windows::position_player_with_preset(&player, preset);
    }
    Ok(())
}

#[tauri::command]
pub fn set_player_theme_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    theme: PlayerTheme,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.player_theme = theme)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_theme_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    theme: AppTheme,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.app_theme = theme)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_player_size_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    size: String,
) -> Result<(), String> {
    let size_for_window = size.clone();
    settings::update(&app, state.inner(), |s| s.player_size = size)
        .map_err(|e| e.to_string())?;
    if let Some(player) = app.get_webview_window("player") {
        let _ = windows::resize_player_for_size(&player, &size_for_window);
    }
    Ok(())
}

#[tauri::command]
pub fn set_ocr_engine_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    engine: OcrEngine,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.ocr_engine = engine)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_settings_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Settings, String> {
    let result = settings::update(&app, state.inner(), |s| *s = Settings::default())
        .map_err(|e| e.to_string())?;
    let _ = hotkey::register_from_settings(&app, state.inner());
    Ok(result)
}

#[tauri::command]
pub fn export_settings_cmd(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let s = state.settings.lock().unwrap().clone();
    serde_json::to_string_pretty(&s).map_err(|e| e.to_string())
}

// ----- BRIDGE / EXTENSION -----
#[derive(Debug, serde::Serialize)]
pub struct BridgeStatus {
    pub enabled: bool,
    pub token: String,
    pub port: u16,
    pub connections: Vec<ConnectionInfo>,
}

#[tauri::command]
pub async fn bridge_status(state: State<'_, Arc<AppState>>) -> Result<BridgeStatus, String> {
    let (enabled, token) = {
        let s = state.settings.lock().unwrap();
        (s.bridge_enabled, s.bridge_token.clone())
    };
    let conns = state.bridge.connections.lock().await;
    Ok(BridgeStatus {
        enabled,
        token,
        port: BRIDGE_PORT,
        connections: conns.values().map(|h| h.info.clone()).collect(),
    })
}

/// Returns the absolute filesystem path of the bundled chromium extension.
/// The frontend uses this to (a) display it, (b) Show-in-Finder, (c) prompt the
/// user to "Load unpacked" via chrome://extensions.
#[tauri::command]
pub fn get_extension_path_cmd(app: AppHandle) -> Result<String, String> {
    let resource = app
        .path()
        .resolve("resources/extension", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    Ok(resource.to_string_lossy().into_owned())
}

/// Forward a frontend `console.log` line into the backend tracing pipeline.
/// Used so the `yappy.log` file contains BOTH backend and frontend events,
/// which we need to debug "stuck on X" issues where the user can't open DevTools.
#[tauri::command]
pub fn log_frontend_cmd(level: String, source: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target = "frontend", "{source}: {message}"),
        "warn"  => tracing::warn!(target = "frontend", "{source}: {message}"),
        "info"  => tracing::info!(target = "frontend", "{source}: {message}"),
        _       => tracing::debug!(target = "frontend", "{source}: {message}"),
    }
}

/// Reveal Yappy's log file in Finder. The log lives at
/// `~/Library/Application Support/com.yappy.app/yappy.log` and accumulates
/// everything tracing emits at info+ level — handy for "it's stuck" reports.
#[tauri::command]
pub fn reveal_log_file_cmd() -> Result<String, String> {
    let log_path = dirs::data_dir()
        .map(|d| d.join("com.yappy.app").join("yappy.log"))
        .ok_or_else(|| "no data dir".to_string())?;
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .args(["-R"]) // -R reveals the file IN its parent folder
            .arg(&log_path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(log_path.to_string_lossy().to_string())
}

/// Return the path to the log file (so the frontend can show it).
#[tauri::command]
pub fn get_log_path_cmd() -> Result<String, String> {
    let log_path = dirs::data_dir()
        .map(|d| d.join("com.yappy.app").join("yappy.log"))
        .ok_or_else(|| "no data dir".to_string())?;
    Ok(log_path.to_string_lossy().to_string())
}

/// Read the last N kilobytes of the log file. Returned as a string for the UI.
#[tauri::command]
pub fn tail_log_cmd(max_kb: Option<u64>) -> Result<String, String> {
    let log_path = dirs::data_dir()
        .map(|d| d.join("com.yappy.app").join("yappy.log"))
        .ok_or_else(|| "no data dir".to_string())?;
    let max_bytes = max_kb.unwrap_or(64) * 1024;
    let metadata = std::fs::metadata(&log_path).map_err(|e| e.to_string())?;
    let len = metadata.len();
    let start = len.saturating_sub(max_bytes);
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(&log_path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Reveal the bundled extension folder in Finder so the user can drag it onto
/// chrome://extensions (Load unpacked).
#[tauri::command]
pub fn reveal_extension_folder_cmd(app: AppHandle) -> Result<(), String> {
    let path = get_extension_path_cmd(app)?;
    #[cfg(target_os = "macos")]
    {
        // `open -R` reveals the target IN its parent folder. For a directory itself
        // we just `open` the directory.
        let status = std::process::Command::new("open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("open exited with status {status}"));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path; // unused on other OSes
        return Err("reveal not implemented on this platform".into());
    }
    Ok(())
}

#[tauri::command]
pub fn open_browser_extensions_cmd(browser: String) -> Result<(), String> {
    // The browsers' internal URLs (chrome://, vivaldi://, brave://, …) cannot be opened by
    // the OS directly — only the browser itself knows them. Use AppleScript's `open location`
    // which is supported by every Chromium browser + Safari.
    #[cfg(target_os = "macos")]
    {
        let internal_url = match browser.as_str() {
            "Safari" => "safari://extensions",
            "Vivaldi" => "vivaldi://extensions",
            "Brave Browser" => "brave://extensions",
            "Microsoft Edge" => "edge://extensions",
            "Arc" => "arc://extensions",
            _ => "chrome://extensions",
        };
        let script = format!(
            r#"tell application "{}" to activate
delay 0.2
tell application "{}" to open location "{}""#,
            browser, browser, internal_url
        );
        let out = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn set_launch_at_login_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.launch_at_login = enabled)
        .map_err(|e| e.to_string())?;
    // Autostart is desktop-only — iOS apps launch on user tap, not on boot.
    // We still persist the setting on iOS so the UI can reflect it, but skip
    // the plugin call (the plugin isn't even loaded on mobile).
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        let mgr = app.autolaunch();
        if enabled {
            mgr.enable().map_err(|e| e.to_string())?;
        } else {
            mgr.disable().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn bridge_regenerate_token_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let new_token = uuid::Uuid::new_v4().to_string();
    let new_token_for_save = new_token.clone();
    settings::update(&app, state.inner(), |s| {
        s.bridge_token = new_token_for_save;
    })
    .map_err(|e| e.to_string())?;
    // Force-disconnect every paired extension so they reconnect against the new token.
    // Without this the live sockets keep working with the old token and the UI looks like
    // the regenerate did nothing.
    {
        let mut conns = state.bridge.connections.lock().await;
        for (browser, _h) in conns.drain() {
            let _ = app.emit("bridge_disconnected", &browser);
        }
        // Dropping the senders here ends the writer-tasks; the sockets close shortly after.
    }
    Ok(new_token)
}

/// Clear the bridge token so the next chromium-extension connection re-claims it
/// (auto-pair). Use this when the dev/prod token drift breaks an extension's pairing.
#[tauri::command]
pub async fn bridge_clear_pairing_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.bridge_token = String::new())
        .map_err(|e| e.to_string())?;
    // Drop every paired connection so its next reconnect (with whatever token it stored)
    // wins the auto-pair race against the now-empty desktop token.
    let mut conns = state.bridge.connections.lock().await;
    for (browser, _h) in conns.drain() {
        let _ = app.emit("bridge_disconnected", &browser);
    }
    Ok(())
}

#[tauri::command]
pub fn set_bridge_enabled_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.bridge_enabled = enabled)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_settings_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    json: String,
) -> Result<Settings, String> {
    let s: Settings = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let result = settings::update(&app, state.inner(), |x| *x = s)
        .map_err(|e| e.to_string())?;
    let _ = hotkey::register_from_settings(&app, state.inner());
    Ok(result)
}

#[tauri::command]
pub fn set_hotkey_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    action: String,
    combo: String,
) -> Result<(), String> {
    let action = match action.as_str() {
        "read_now" => hotkey::Action::ReadNow,
        "pause_resume" => hotkey::Action::PauseResume,
        "read_clipboard" => hotkey::Action::ReadClipboard,
        _ => return Err(format!("unknown action: {action}")),
    };
    hotkey::set_hotkey(&app, state.inner(), action, combo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_player_position_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    x: Option<i32>,
    y: Option<i32>,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| {
        s.player_position = match (x, y) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        };
        // If user explicitly drags, mark as Custom so we don't overwrite next launch.
        if x.is_some() && y.is_some() {
            s.player_position_preset = PlayerPositionPreset::Custom;
        }
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_file_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
    path: String,
    // target_window: if provided, load INTO that window (swap content of an existing
    // doc window). If absent, allocate a fresh document window so multiple files can
    // be open side-by-side.
    target_window: Option<String>,
) -> Result<(), String> {
    // Decide which window to target.
    // - Caller provided `target_window` (e.g. doc window's swap button): reuse it.
    // - Caller came from a "document-*" window: reuse it.
    // - Otherwise (from main/player): allocate a brand-new doc window.
    let caller_label = window.label().to_string();
    let label = match target_window {
        Some(l) if !l.is_empty() => l,
        _ if caller_label.starts_with("document-") || caller_label == "document" => caller_label.clone(),
        _ => windows::next_document_label(),
    };
    tracing::info!("[doc:cmd] read_file_cmd: target window label = {label} (caller={caller_label})");
    let p = std::path::PathBuf::from(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let filename = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_string();

    tracing::info!(
        "[doc:cmd] read_file_cmd ENTER path={} ext={} filename={}",
        p.display(),
        ext,
        filename
    );
    if !p.exists() {
        tracing::warn!("[doc:cmd] read_file_cmd: file does not exist: {}", p.display());
        return Err(format!("file not found: {path}"));
    }

    // Stage 1: immediately show the document window with a LOADING placeholder.
    // This is critical for PDFs — pdf-extract can take 30-120s on big docs,
    // and without this the user sees a frozen home screen and assumes the app
    // hung. Loading state replaces the empty "drop a file" view with a clear
    // "parsing your file…" spinner.
    let loading_payload = crate::state::CurrentDocument {
        path: path.clone(),
        filename: filename.clone(),
        extension: ext.clone(),
        paragraphs: Vec::new(),
        char_count: 0,
        loading: true,
        paragraph_pauses: Vec::new(),
        paragraph_speed_mult: Vec::new(),
        paragraph_kinds: Vec::new(),
    };
    tracing::info!("[doc:cmd] read_file_cmd: storing LOADING placeholder for window={label}");
    state
        .documents
        .lock()
        .unwrap()
        .insert(label.clone(), loading_payload.clone());
    tracing::info!("[doc:cmd] read_file_cmd: calling windows::show_document({label})");
    match windows::show_document(&app, &label) {
        Ok(_) => tracing::info!("[doc:cmd] read_file_cmd: show_document OK"),
        Err(e) => tracing::error!("[doc:cmd] read_file_cmd: show_document FAILED: {e:?}"),
    }
    state.playback.stop();
    tracing::info!("[doc:cmd] read_file_cmd: emit_to({label}, document_loaded LOADING)");
    match app.emit_to(label.as_str(), "document_loaded", &loading_payload) {
        Ok(_) => tracing::info!("[doc:cmd] read_file_cmd: document_loaded (LOADING) emit OK"),
        Err(e) => tracing::error!("[doc:cmd] read_file_cmd: document_loaded (LOADING) emit FAILED: {e:?}"),
    }

    // Stage 2: parse via the rich loader so markdown structure becomes reading
    // rhythm (heading → pause + slower speed, list → small pause, hr → big pause).
    let path_for_thread = p.clone();
    let parse_result = tokio::time::timeout(
        std::time::Duration::from_secs(180),
        tokio::task::spawn_blocking(move || {
            capture::doc_loader::load_rich_from_file(&path_for_thread)
        }),
    )
    .await;

    let rich_result: Result<Vec<crate::capture::doc_loader::RichParagraph>, String> = match parse_result {
        Ok(Ok(Ok(v))) => Ok(v),
        Ok(Ok(Err(e))) => Err(e.to_string()),
        Ok(Err(e)) => Err(format!("parse task failed: {e}")),
        Err(_) => Err(format!(
            "timed out parsing {filename} after 180s — file may be too large or malformed"
        )),
    };

    let rich = match rich_result {
        Ok(v) => {
            let total_chars: usize = v.iter().map(|p| p.text.chars().count()).sum();
            tracing::info!(
                "[doc:cmd] read_file_cmd: parse OK, {} paragraphs / {} chars from {}",
                v.len(),
                total_chars,
                filename
            );
            v
        }
        Err(e) => {
            state.documents.lock().unwrap().remove(&label);
            let _ = app.emit_to(
                label.as_str(),
                "document_error",
                serde_json::json!({ "filename": filename, "error": e.clone() }),
            );
            return Err(e);
        }
    };
    if rich.iter().all(|p| p.text.trim().is_empty()) {
        let err = "no readable text in this document".to_string();
        state.documents.lock().unwrap().remove(&label);
        let _ = app.emit_to(
            label.as_str(),
            "document_error",
            serde_json::json!({ "filename": filename, "error": err.clone() }),
        );
        return Err(err);
    }

    // Stage 3: success — split rich paragraphs into parallel arrays for the frontend.
    let total_chars: usize = rich.iter().map(|p| p.text.chars().count()).sum();
    let mut paragraphs: Vec<String> = Vec::with_capacity(rich.len());
    let mut paragraph_pauses: Vec<f32> = Vec::with_capacity(rich.len());
    let mut paragraph_speed_mult: Vec<f32> = Vec::with_capacity(rich.len());
    let mut paragraph_kinds: Vec<String> = Vec::with_capacity(rich.len());
    for rp in rich {
        paragraphs.push(rp.text);
        paragraph_pauses.push(rp.pause_before);
        paragraph_speed_mult.push(rp.speed_mult);
        paragraph_kinds.push(rp.kind);
    }
    let payload = crate::state::CurrentDocument {
        path,
        filename,
        extension: ext,
        paragraphs,
        char_count: total_chars,
        loading: false,
        paragraph_pauses,
        paragraph_speed_mult,
        paragraph_kinds,
    };
    tracing::info!(
        "[doc:cmd] read_file_cmd: storing FULL document for {label} (paragraphs={}, chars={})",
        payload.paragraphs.len(),
        payload.char_count
    );
    state.documents.lock().unwrap().insert(label.clone(), payload.clone());
    tracing::info!("[doc:cmd] read_file_cmd: emit_to({label}, document_loaded FULL)");
    match app.emit_to(label.as_str(), "document_loaded", &payload) {
        Ok(_) => tracing::info!("[doc:cmd] read_file_cmd: document_loaded (FULL) emit OK"),
        Err(e) => tracing::error!("[doc:cmd] read_file_cmd: document_loaded (FULL) emit FAILED: {e:?}"),
    }
    tracing::info!("[doc:cmd] read_file_cmd EXIT OK");
    Ok(())
}

/// Pulled by the document window on mount to recover from the open-race
/// (backend emits the load event before the window's JS subscribes).
#[tauri::command]
pub fn get_current_document_cmd(
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
) -> Option<crate::state::CurrentDocument> {
    let label = window.label();
    let snap = state.documents.lock().unwrap().get(label).cloned();
    match &snap {
        Some(d) => tracing::info!(
            "[doc:cmd] get_current_document_cmd({label}) → Some(filename={}, paragraphs={}, loading={})",
            d.filename, d.paragraphs.len(), d.loading
        ),
        None => tracing::info!("[doc:cmd] get_current_document_cmd({label}) → None"),
    }
    snap
}

/// Called by the document window once it has mounted + registered its
/// `document_loaded` listener. The backend immediately re-emits the current
/// document so the window can't miss it.
#[tauri::command]
pub fn document_window_ready_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
) -> Result<(), String> {
    let label = window.label().to_string();
    tracing::info!("[doc:cmd] document_window_ready_cmd ENTER (label={label})");
    let snap = state.documents.lock().unwrap().get(&label).cloned();
    match &snap {
        Some(doc) => {
            tracing::info!(
                "[doc:cmd] document_window_ready({label}): re-emitting filename={} paragraphs={} loading={}",
                doc.filename,
                doc.paragraphs.len(),
                doc.loading
            );
            match app.emit_to(label.as_str(), "document_loaded", doc) {
                Ok(_) => tracing::info!("[doc:cmd] document_loaded emit OK"),
                Err(e) => tracing::error!("[doc:cmd] document_loaded emit FAILED: {e:?}"),
            }
        }
        None => {
            tracing::warn!(
                "[doc:cmd] document_window_ready({label}): no document in state — nothing to re-emit"
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub fn clear_current_document_cmd(
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
) {
    let label = window.label();
    tracing::info!("[doc:cmd] clear_current_document_cmd({label})");
    state.documents.lock().unwrap().remove(label);
}

/// Stable filesystem-safe key for project autosave. Encodes the absolute document
/// path as base64-url so different files don't collide and we can debug by eye.
fn project_key(path: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(path.as_bytes())
}

fn project_path(app: &AppHandle, doc_path: &str) -> Result<std::path::PathBuf, String> {
    let mut p = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    p.push("projects");
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    p.push(format!("{}.json", project_key(doc_path)));
    Ok(p)
}

#[tauri::command]
pub fn save_project_cmd(
    app: AppHandle,
    doc_path: String,
    project_json: String,
) -> Result<(), String> {
    let p = project_path(&app, &doc_path)?;
    tracing::info!("[doc:cmd] save_project_cmd → {}", p.display());
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, &project_json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn load_project_cmd(app: AppHandle, doc_path: String) -> Result<Option<String>, String> {
    let p = project_path(&app, &doc_path)?;
    if !p.exists() {
        tracing::info!("[doc:cmd] load_project_cmd: no project file at {}", p.display());
        return Ok(None);
    }
    let json = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    tracing::info!("[doc:cmd] load_project_cmd: loaded {} bytes from {}", json.len(), p.display());
    Ok(Some(json))
}

/// One paragraph as the audiobook renderer sees it: text + optional voice/speed
/// override + optional pause (silence in seconds) before it.
#[derive(Debug, Clone, Deserialize)]
pub struct ParagraphSpec {
    pub text: String,
    pub voice: Option<String>,
    pub speed: Option<f32>,
    /// Silence (in seconds) inserted BEFORE this paragraph's audio. 0 = no gap.
    /// Useful for chapter breaks. The first paragraph's pause is ignored.
    pub pause_before: Option<f32>,
    /// If set, this paragraph starts a new chapter in the rendered m4b. The
    /// chapter's timestamp will be the sample-accurate position of this
    /// paragraph in the combined audio. Ignored for .wav output.
    pub chapter_title: Option<String>,
}

/// Optional top-level metadata for m4b output (ignored for .wav).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudiobookMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub album: Option<String>,
}

/// Render the whole document end-to-end into a single .wav file, applying
/// per-paragraph voice/speed overrides and inserting silence between paragraphs.
/// Runs synchronously off the audio thread — does NOT touch playback state.
/// Emits `audiobook_render_progress` events so the UI can show a progress bar.
#[tauri::command]
pub async fn render_audiobook_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paragraphs: Vec<ParagraphSpec>,
    output_path: String,
    metadata: Option<AudiobookMeta>,
) -> Result<(), String> {
    if paragraphs.is_empty() {
        return Err("nothing to render".into());
    }
    if !model::is_model_ready(&app).map_err(|e| e.to_string())? {
        return Err("voice model not installed".into());
    }
    let root = model::model_root(&app).map_err(|e| e.to_string())?;
    let engine = state.engine_or_load(&root).map_err(|e| e.to_string())?;

    let (default_voice, default_speed, default_lang, total_steps) = {
        let s = state.settings.lock().unwrap();
        (s.voice.clone(), s.speed, s.default_lang.clone(), s.quality.total_steps())
    };

    let app_for_thread = app.clone();
    let total = paragraphs.len();
    let want_m4b = std::path::Path::new(&output_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("m4b") || e.eq_ignore_ascii_case("m4a"))
        .unwrap_or(false);

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // iOS: engage silent-audio keepalive for the duration of the render.
        // The OS otherwise suspends us within ~30s of going to background;
        // audiobook renders can run for hours. The guard releases the audio
        // session automatically when this closure returns (Drop).
        #[cfg(target_os = "ios")]
        let _audio_keepalive = crate::mobile::BackgroundAudioGuard::begin();

        let mut combined: Vec<f32> = Vec::new();
        let mut sample_rate: u32 = 0;
        // Sample-offset → chapter title, collected as we go. Used only for m4b.
        let mut chapters: Vec<(usize, String)> = Vec::new();

        for (i, p) in paragraphs.iter().enumerate() {
            let _ = app_for_thread.emit(
                "audiobook_render_progress",
                serde_json::json!({ "index": i, "total": total, "stage": "synth" }),
            );

            // Insert a pause BEFORE this paragraph (skipped for the first one).
            if i > 0 {
                let pause_secs = p.pause_before.unwrap_or(0.35).max(0.0);
                if sample_rate > 0 && pause_secs > 0.0 {
                    let n = (pause_secs * sample_rate as f32) as usize;
                    combined.extend(std::iter::repeat(0.0_f32).take(n));
                }
            }

            // Record chapter mark BEFORE we append this paragraph's audio so the
            // chapter timestamp points at the start of speech, not at any
            // leading silence we just inserted.
            if let Some(title) = p.chapter_title.as_ref() {
                let trimmed = title.trim();
                if !trimmed.is_empty() {
                    chapters.push((combined.len(), trimmed.to_string()));
                }
            }

            let opts = yappy_core::engine::SynthesisOptions {
                voice: p.voice.clone().unwrap_or_else(|| default_voice.clone()),
                speed: p.speed.unwrap_or(default_speed),
                default_lang: default_lang.clone(),
                total_steps,
                seed: None,
            };

            let captured: std::sync::Mutex<Vec<(u32, Vec<f32>)>> = std::sync::Mutex::new(Vec::new());
            engine
                .synthesize_streaming(&p.text, &opts, |chunk| {
                    captured
                        .lock()
                        .unwrap()
                        .push((chunk.sample_rate as u32, chunk.samples.clone()));
                    Ok(())
                })
                .map_err(|e| format!("synth failed on paragraph {}: {e:?}", i + 1))?;

            for (sr, samples) in captured.lock().unwrap().iter() {
                if sample_rate == 0 {
                    sample_rate = *sr;
                }
                // For audiobook export we resample mismatched paragraphs to the FIRST
                // paragraph's sample rate. In practice supertonic emits a fixed SR,
                // so this is a defensive path.
                if *sr != sample_rate {
                    let resampled = crate::playback::resample_mono(samples, *sr, sample_rate)
                        .map_err(|e| format!("resample failed: {e:?}"))?;
                    combined.extend(resampled);
                } else {
                    combined.extend(samples.iter().copied());
                }
            }
        }

        let _ = app_for_thread.emit(
            "audiobook_render_progress",
            serde_json::json!({ "index": total, "total": total, "stage": "writing" }),
        );

        let final_sr = sample_rate.max(44100);

        if want_m4b {
            // Convert sample offsets → seconds.
            let chapter_objs: Vec<crate::audiobook::Chapter> = chapters
                .into_iter()
                .map(|(offset, title)| crate::audiobook::Chapter {
                    title,
                    start_secs: offset as f64 / final_sr as f64,
                })
                .collect();
            // If no explicit chapters were marked, give the file one at 0.0 so
            // players still show a navigable entry instead of "no chapters".
            let chapter_objs = if chapter_objs.is_empty() {
                vec![crate::audiobook::Chapter {
                    title: metadata
                        .as_ref()
                        .and_then(|m| m.title.clone())
                        .unwrap_or_else(|| "Chapter 1".to_string()),
                    start_secs: 0.0,
                }]
            } else {
                chapter_objs
            };

            let meta = crate::audiobook::M4bMetadata {
                title: metadata.as_ref().and_then(|m| m.title.clone()).unwrap_or_default(),
                author: metadata.as_ref().and_then(|m| m.author.clone()).unwrap_or_default(),
                album: metadata.as_ref().and_then(|m| m.album.clone()).unwrap_or_default(),
            };

            crate::audiobook::encode_m4b(
                &combined,
                final_sr,
                &chapter_objs,
                &meta,
                std::path::Path::new(&output_path),
            )
            .map_err(|e| format!("m4b encode failed: {e:?}"))?;
        } else {
            crate::playback::write_wav_file(&output_path, &combined, final_sr)
                .map_err(|e| format!("write wav failed: {e:?}"))?;
        }

        let _ = app_for_thread.emit(
            "audiobook_render_done",
            serde_json::json!({ "path": output_path, "samples": combined.len(), "sample_rate": final_sr }),
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Begin reading a document. Called from the document window after the user clicks
/// "play all" or a specific paragraph. Synthesizes WITHOUT opening the mini-player.
///
/// `voice_override` is a one-shot voice that supersedes the persisted user voice
/// for THIS playback only — used by the "re-read this section with another voice" UI.
#[tauri::command]
pub async fn read_document_paragraphs_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paragraphs: Vec<String>,
    from_index: usize,
    voice_override: Option<String>,
    // One-shot overrides for THIS playback. Used by the document window's rhythm
    // slider — multiplies the persisted global speed without mutating settings.
    speed_override: Option<f32>,
) -> Result<(), String> {
    let joined = paragraphs
        .iter()
        .skip(from_index)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n");
    if joined.trim().is_empty() {
        return Err("nothing to read".into());
    }
    let voice = match voice_override {
        Some(v) if !v.is_empty() => v,
        _ => state.settings.lock().unwrap().voice.clone(),
    };
    // Optional one-shot speed override. We temporarily mutate settings in-memory
    // to feed the engine — the disk-persisted value isn't touched.
    let original_speed = if let Some(s) = speed_override {
        let mut settings = state.settings.lock().unwrap();
        let prev = settings.speed;
        settings.speed = s.clamp(0.3, 3.0);
        Some(prev)
    } else {
        None
    };

    let result = read_with_voice_lang_internal_with_mode(
        &app,
        state.inner().clone(),
        joined,
        voice,
        None,
        "document".into(),
        ReadMode::Document { base_paragraph_index: from_index },
    )
    .await
    .map_err(|e| e.to_string());

    // Restore the in-memory settings.speed after enqueuing (the engine has
    // already snapshotted it inside read_with_voice_lang_internal_with_mode).
    if let Some(prev) = original_speed {
        state.settings.lock().unwrap().speed = prev;
    }

    result
}

#[tauri::command]
pub fn stop_playback_cmd(state: State<'_, Arc<AppState>>) {
    state.playback.stop();
}

#[tauri::command]
pub async fn toggle_pause_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    toggle_pause(app, state.inner().clone())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trigger_read_now_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    trigger_read_now(app, state.inner().clone())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn synthesize_text(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    text: String,
) -> Result<(), String> {
    tracing::info!("synthesize_text invoked, text len={}", text.chars().count());
    read_text(&app, state.inner().clone(), text, "manual".into())
        .await
        .map_err(|e| {
            tracing::error!("synthesize_text error: {e:?}");
            e.to_string()
        })
}

#[tauri::command]
pub async fn read_clipboard_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let text = capture::clipboard::read_text()
        .ok()
        .flatten()
        .unwrap_or_default();
    if text.trim().is_empty() {
        let _ = app.emit("capture_empty", true);
        return Ok(());
    }
    read_text(&app, state.inner().clone(), text, "clipboard".into())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_model_ready(app: AppHandle) -> Result<bool, String> {
    model::is_model_ready(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_model_cmd(app: AppHandle) -> Result<(), String> {
    let h = app.clone();
    model::download_model(&app, move |p| {
        let _ = h.emit("model_download", &p);
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
    windows::show_main(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_player_window(app: AppHandle) -> Result<(), String> {
    windows::show_player(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn request_macos_permissions() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"])
            .status();
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CaptureDiagnostics {
    pub front_app: Option<String>,
    pub selection_preview: String,
    pub active_doc_preview: String,
    pub clipboard_preview: String,
}

#[tauri::command]
pub fn capture_diagnostics() -> Result<CaptureDiagnostics, String> {
    let front_app = capture::front_app_name();
    let selection_preview = capture::selection::capture_selection()
        .ok()
        .flatten()
        .map(|s| truncate(&s, 200))
        .unwrap_or_default();
    let active_doc_preview = front_app
        .as_ref()
        .and_then(|n| capture::active_doc::active_document_text(n).ok().flatten())
        .map(|s| truncate(&s, 200))
        .unwrap_or_default();
    let clipboard_preview = capture::clipboard::read_text()
        .ok()
        .flatten()
        .map(|s| truncate(&s, 200))
        .unwrap_or_default();
    Ok(CaptureDiagnostics {
        front_app,
        selection_preview,
        active_doc_preview,
        clipboard_preview,
    })
}

fn truncate(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push_str(" …");
        out
    }
}

#[tauri::command]
pub async fn sample_voice(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    voice: String,
    lang: Option<String>,
    sample_text: Option<String>,
) -> Result<(), String> {
    let lang = lang.unwrap_or_else(|| state.settings.lock().unwrap().default_lang.clone());
    let text = sample_text.unwrap_or_else(|| sample_for_voice(&voice, &lang));
    state.playback.stop();
    let h = app.clone();
    let s = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = read_with_voice_lang(&h, s, text, voice, lang, "sample".into()).await {
            tracing::error!("sample_voice: {e:?}");
        }
    });
    Ok(())
}

fn sample_for_voice(name: &str, lang: &str) -> String {
    let v = yappy_core::voices::by_name(name).unwrap_or_else(yappy_core::voices::default_voice);
    match lang {
        "es" => format!("Hola, soy {}, y leeré todo lo que pongas delante de mí.", v.name),
        "fr" => format!("Bonjour, je suis {}, et je lirai tout ce que vous me donnerez.", v.name),
        "de" => format!("Hallo, ich bin {}, und ich lese gerne alles, was du mir gibst.", v.name),
        "it" => format!("Ciao, sono {}, e leggerò ad alta voce qualsiasi cosa tu mi dia.", v.name),
        "pt" => format!("Olá, eu sou {}, e vou ler tudo o que você me der.", v.name),
        "ja" => format!("こんにちは、{}です。 何でも声に出して読みます。", v.name),
        "ko" => format!("안녕하세요, 저는 {}입니다. 무엇이든 소리내어 읽어드릴게요.", v.name),
        _ => format!(
            "Hi, I'm {}. I'd love to read anything you put in front of me.",
            v.name
        ),
    }
}

// ----- HISTORY -----

#[tauri::command]
pub fn get_history(app: AppHandle) -> Result<history::History, String> {
    history::load(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_history_cmd(app: AppHandle) -> Result<(), String> {
    history::clear(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn replay_history_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    let h = history::load(&app).map_err(|e| e.to_string())?;
    let entry = h
        .entries
        .iter()
        .find(|e| e.id == id)
        .cloned()
        .ok_or("history entry not found")?;
    read_text(&app, state.inner().clone(), entry.text, "history".into())
        .await
        .map_err(|e| e.to_string())
}

// ----- SAVE AS AUDIO -----

#[tauri::command]
pub fn save_current_audio_cmd(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<(), String> {
    let (samples, sr) = state.playback.session_audio();
    if samples.is_empty() {
        return Err("no audio in current session".into());
    }
    crate::playback::write_wav_file(&path, &samples, sr).map_err(|e| e.to_string())
}

// ----- CENTRAL FLOWS -----

pub async fn toggle_pause<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) -> Result<()> {
    let snap = state.playback.snapshot();
    if !snap.playing && !snap.paused {
        return trigger_read_now(app, state).await;
    }
    if snap.paused {
        state.playback.resume();
    } else {
        state.playback.pause();
    }
    Ok(())
}

pub async fn trigger_read_now<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<AppState>,
) -> Result<()> {
    tracing::info!("trigger_read_now: entering");
    let _ = app.emit("capture_progress", "thinking");

    // 1. Fast path: selection (always wins) OR paired-browser delegation.
    if let Some(fc) = capture::fast_capture(&state).await {
        match fc {
            capture::FastCapture::Done(capture) => {
                tracing::info!(
                    "trigger_read_now: fast captured {} chars source={}",
                    capture.text.chars().count(),
                    capture.source.short_kind()
                );
                if capture.text.trim().is_empty() {
                    let _ = app.emit("capture_empty", true);
                    return Ok(());
                }
                let _ = app.emit(
                    "capture_info",
                    serde_json::json!({
                        "source": capture.source,
                        "len": capture.text.chars().count(),
                        "preview": capture.text.chars().take(120).collect::<String>(),
                    }),
                );
                return read_text(&app, state, capture.text, capture.source.short_kind().to_string()).await;
            }
            capture::FastCapture::Delegated(source) => {
                let _ = app.emit(
                    "capture_info",
                    serde_json::json!({ "source": source, "len": 0, "preview": "" }),
                );
                return Ok(());
            }
        }
    }

    // 2. Slow path: browser-AS / active-doc / OCR (no selection found, no paired browser).
    let capture = tokio::task::spawn_blocking(capture::smart_capture_blocking).await??;
    tracing::info!(
        "trigger_read_now: captured {} chars source={}",
        capture.text.chars().count(),
        capture.source.short_kind(),
    );
    if capture.text.trim().is_empty() {
        tracing::info!("trigger_read_now: empty capture");
        let _ = app.emit("capture_empty", true);
        return Ok(());
    }
    let _ = app.emit(
        "capture_info",
        serde_json::json!({
            "source": capture.source,
            "len": capture.text.chars().count(),
            "preview": capture.text.chars().take(120).collect::<String>(),
        }),
    );
    read_text(&app, state, capture.text, capture.source.short_kind().to_string()).await
}

pub async fn read_text<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    source: String,
) -> Result<()> {
    let voice = state.settings.lock().unwrap().voice.clone();
    read_with_voice(app, state, text, voice, source).await
}

/// Like `read_text`, but suppresses the mini-player (document window owns the UI).
pub async fn read_text_in_document<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    source: String,
    base_paragraph_index: usize,
) -> Result<()> {
    let voice = state.settings.lock().unwrap().voice.clone();
    read_with_voice_lang_internal_with_mode(
        app,
        state,
        text,
        voice,
        None,
        source,
        ReadMode::Document { base_paragraph_index },
    )
    .await
}

#[derive(Debug, Clone)]
pub enum ReadMode {
    /// Default: opens / focuses the mini-player.
    MiniPlayer,
    /// Document mode: synth happens but mini-player is NOT shown; the document
    /// window listens to playback_state and chunk_synthesized to drive its UI.
    Document { base_paragraph_index: usize },
}

pub async fn read_with_voice<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    voice: String,
    source: String,
) -> Result<()> {
    // We may also override per-language inside the engine loop below.
    read_with_voice_lang_internal(app, state, text, voice, None, source).await
}

pub async fn read_with_voice_lang<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    voice: String,
    lang: String,
    source: String,
) -> Result<()> {
    read_with_voice_lang_internal(app, state, text, voice, Some(lang), source).await
}

async fn read_with_voice_lang_internal<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    voice: String,
    forced_lang: Option<String>,
    source: String,
) -> Result<()> {
    read_with_voice_lang_internal_with_mode(app, state, text, voice, forced_lang, source, ReadMode::MiniPlayer).await
}

async fn read_with_voice_lang_internal_with_mode<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    voice: String,
    forced_lang: Option<String>,
    source: String,
    mode: ReadMode,
) -> Result<()> {
    tracing::info!(
        "read_with_voice: voice={} source={} chars={} forced_lang={:?}",
        voice,
        source,
        text.chars().count(),
        forced_lang
    );
    if !model::is_model_ready(app).map_err(|e| anyhow::anyhow!(e))? {
        let _ = app.emit("model_missing", true);
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
        return Ok(());
    }
    let root = model::model_root(app)?;
    let engine = state.engine_or_load(&root)?;
    state.playback.stop();
    // Claim a fresh id AFTER the stop bump so this synth task has a unique session.
    // Any older synth task that's still spinning will see `current_session()` change
    // and abort the next time it tries to emit a chunk.
    let session_id = state.playback.begin_session();

    let (opts, voice_overrides, save_history, history_max, mut successful_reads, vol) = {
        let s = state.settings.lock().unwrap();
        let chosen_voice = match forced_lang.as_deref() {
            Some(lang) => s.voice_overrides.get(lang).cloned().unwrap_or(voice.clone()),
            None => voice.clone(),
        };
        (
            SynthesisOptions {
                voice: chosen_voice,
                speed: s.speed,
                default_lang: forced_lang.clone().unwrap_or_else(|| s.default_lang.clone()),
                total_steps: s.quality.total_steps(),
                seed: None,
            },
            s.voice_overrides.clone(),
            s.save_history,
            s.history_max,
            s.successful_reads,
            s.volume,
        )
    };

    // Set volume on the playback controller.
    state.playback.set_volume(vol);

    match &mode {
        ReadMode::MiniPlayer => {
            let _ = windows::show_player(app);
        }
        ReadMode::Document { .. } => {
            // Document window is already visible; do NOT pop the mini-player.
        }
    }
    let base_paragraph_index = match &mode {
        ReadMode::Document { base_paragraph_index } => *base_paragraph_index,
        ReadMode::MiniPlayer => 0,
    };
    let _ = app.emit(
        "playback_starting",
        serde_json::json!({
            "text_preview": text.chars().take(140).collect::<String>(),
            "source": source,
            "base_paragraph_index": base_paragraph_index,
        }),
    );

    let app_for_thread = app.clone();
    let state_for_thread = state.clone();
    let text_for_thread = text.clone();
    let app_for_history = app.clone();
    let source_for_history = source.clone();
    let preview_for_history: String = text.chars().take(180).collect();

    tauri::async_runtime::spawn_blocking(move || {
        let mut first_emitted = false;
        let mut detected_lang = String::new();
        let opts_local = opts.clone();
        let my_session = session_id;
        let res = engine.synthesize_streaming(&text_for_thread, &opts_local, |chunk| {
            // Cooperative cancel: if Stop fired (or a new read started), the controller
            // has bumped its session id. Bail out *before* spending more time on this chunk's
            // downstream work — the engine respects an Err return by halting the loop.
            if state_for_thread.playback.current_session() != my_session {
                return Err(anyhow::anyhow!("session cancelled"));
            }
            detected_lang = chunk.lang.clone();
            let ac = AudioChunk {
                index: chunk.index,
                paragraph_index: chunk.paragraph_index,
                total: chunk.total,
                total_paragraphs: chunk.total_paragraphs,
                text: chunk.text.clone(),
                samples: chunk.samples.clone(),
                source_sample_rate: chunk.sample_rate as u32,
            };
            if !first_emitted {
                state_for_thread.playback.new_session(my_session, vec![ac]);
                first_emitted = true;
            } else {
                state_for_thread.playback.enqueue(my_session, ac);
            }
            // Skip the UI event too if the session was just cancelled (very tight race window).
            if state_for_thread.playback.current_session() != my_session {
                return Err(anyhow::anyhow!("session cancelled"));
            }
            let _ = app_for_thread.emit(
                "chunk_synthesized",
                serde_json::json!({
                    "index": chunk.index,
                    "total": chunk.total,
                    "text": chunk.text,
                    "lang": chunk.lang,
                }),
            );
            tracing::debug!(
                "chunk {}/{} lang={} chars={}",
                chunk.index + 1,
                chunk.total,
                chunk.lang,
                chunk.text.chars().count()
            );
            let _ = &voice_overrides;
            Ok(())
        });

        // A cancellation is an expected outcome, not an error worth surfacing.
        let cancelled = matches!(&res, Err(e) if e.to_string().contains("session cancelled"));
        if let Err(e) = &res {
            if cancelled {
                tracing::info!("synth aborted by user (session {} stale)", my_session);
            } else {
                tracing::error!("synthesize_streaming: {e:?}");
                let _ = app_for_thread.emit("synth_error", e.to_string());
            }
        }

        // Record history + bump successful_reads.
        if res.is_ok() && save_history {
            let entry = history::HistoryEntry {
                id: format!("{}-{}", history::now_unix(), preview_for_history.chars().take(8).collect::<String>()),
                started_at: history::now_unix(),
                source: source_for_history,
                app_name: None,
                voice: opts_local.voice.clone(),
                lang: detected_lang,
                text: text_for_thread,
                duration_secs: state_for_thread.playback.snapshot().duration_secs,
            };
            let _ = history::append(&app_for_thread, entry, history_max);
            successful_reads = successful_reads.saturating_add(1);
            // Bump the counter in-memory only; this thread holds no stale knowledge of
            // voice/position so it must NOT overwrite the persisted struct.
            // The counter rides along on the next user-driven save.
            let new_val = successful_reads;
            settings::bump_in_memory(&state_for_thread, |s| s.successful_reads = new_val);
            // Best-effort persist of just the counter, race-free via update().
            let _ = settings::update(&app_for_thread, &state_for_thread, |s| {
                s.successful_reads = new_val;
            });
            if successful_reads == 1 {
                let _ = app_for_thread.emit("first_read", true);
            }
        }
    });

    let app_for_listener = app.clone();
    state.playback.subscribe(move |snap| {
        let _ = app_for_listener.emit("playback_state", snap);
    });

    Ok(())
}
