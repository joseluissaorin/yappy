//! Tauri command handlers + central read flow.

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use yappy_core::engine::SynthesisOptions;
use yappy_core::voices::Voice;

use crate::asr_model;
use crate::bridge::{ConnectionInfo, BRIDGE_PORT};
use crate::capture;
use crate::credits;
use crate::history;
use crate::hotkey;
use crate::model;
use crate::playback::AudioChunk;
use crate::settings::{
    self, AppTheme, OcrEngine, PlayerPositionPreset, PlayerTheme, Quality, Settings,
};
use crate::state::AppState;
use crate::transcripts;
use crate::windows;

#[tauri::command]
pub fn list_voices() -> Vec<Voice> {
    yappy_core::VOICES.to_vec()
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
pub fn set_voz_al_azar_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    valor: bool,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.voz_al_azar = valor)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_voice_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    voice: String,
) -> Result<(), String> {
    settings::update(&app, state.inner(), |s| s.voice = voice.clone())
        .map_err(|e| e.to_string())?;
    // EL CAMBIO EN CALIENTE: si algo suena, la sesión se relanza desde el
    // párrafo actual con la voz nueva (conservando la pausa): nada de
    // cortar el reproductor y volver a darle.
    relanzar_con_voz(&app, state.inner(), voice);
    // Y los títulos DICHOS se recocinan con la voz nueva (la caché es por
    // voz: sin esto, tocar una pieza sonaba con la voz vieja).
    #[cfg(target_os = "ios")]
    precocinar_titulos(app.clone(), state.inner().clone());
    Ok(())
}

/// Relanza la sesión viva (si la hay) con otra voz, desde el párrafo en
/// curso y conservando el estado de pausa.
fn relanzar_con_voz(app: &AppHandle, state: &Arc<AppState>, voz: String) {
    let snap = state.playback.snapshot();
    if snap.estado == "inactivo" || snap.doc_path.is_empty() {
        return;
    }
    let Some(receta) = state.receta_sesion.lock().unwrap().clone() else {
        return;
    };
    if receta.doc_path != snap.doc_path {
        return;
    }
    let desde = snap.base_paragraph_index + snap.current_paragraph_index;
    let pausada = snap.estado == "pausa";
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = leer_parrafos(app, state, receta, desde, Some(voz), None, pausada).await {
            tracing::warn!("relanzar con voz nueva: {e}");
        }
    });
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
    settings::update(&app, state.inner(), |s| s.player_size = size).map_err(|e| e.to_string())?;
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
        "warn" => tracing::warn!(target = "frontend", "{source}: {message}"),
        "info" => tracing::info!(target = "frontend", "{source}: {message}"),
        _ => tracing::debug!(target = "frontend", "{source}: {message}"),
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
    let result = settings::update(&app, state.inner(), |x| *x = s).map_err(|e| e.to_string())?;
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

/// Decode a percent-encoded string (`%20` → space, etc.). Bytes that aren't a
/// valid `%XX` triplet pass through unchanged.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = (b[i + 1] as char).to_digit(16);
            let lo = (b[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Normalize a path that may arrive as a `file://` URL with percent-encoding —
/// which is exactly how iOS hands us a file shared or opened into the app
/// (e.g. `file:///private/var/.../Inbox/Cajita%20azul%20MD.md`). Strips the
/// `file:`/`file://` scheme and percent-decodes. Plain filesystem paths pass
/// through unchanged.
fn normalize_local_path(input: &str) -> std::path::PathBuf {
    let mut s = input.trim();
    if let Some(rest) = s.strip_prefix("file://") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("file:") {
        s = rest;
    }
    std::path::PathBuf::from(percent_decode(s))
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
        _ if caller_label.starts_with("document-") || caller_label == "document" => {
            caller_label.clone()
        }
        _ => windows::next_document_label(),
    };
    tracing::info!(
        "[doc:cmd] read_file_cmd: target window label = {label} (caller={caller_label})"
    );
    // iOS hands shared/opened files as percent-encoded `file://` URLs — normalize.
    let p = normalize_local_path(&path);
    let path = p.to_string_lossy().into_owned();
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
        tracing::warn!(
            "[doc:cmd] read_file_cmd: file does not exist: {}",
            p.display()
        );
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
        Err(e) => {
            tracing::error!("[doc:cmd] read_file_cmd: document_loaded (LOADING) emit FAILED: {e:?}")
        }
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

    let rich_result: Result<Vec<crate::capture::doc_loader::RichParagraph>, String> =
        match parse_result {
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
    state
        .documents
        .lock()
        .unwrap()
        .insert(label.clone(), payload.clone());
    tracing::info!("[doc:cmd] read_file_cmd: emit_to({label}, document_loaded FULL)");
    match app.emit_to(label.as_str(), "document_loaded", &payload) {
        Ok(_) => tracing::info!("[doc:cmd] read_file_cmd: document_loaded (FULL) emit OK"),
        Err(e) => {
            tracing::error!("[doc:cmd] read_file_cmd: document_loaded (FULL) emit FAILED: {e:?}")
        }
    }
    tracing::info!("[doc:cmd] read_file_cmd EXIT OK");
    Ok(())
}

/// Parse a document and RETURN its content directly — no window, no AppState
/// juggling. The iOS mobile reader (`/read`) renders the document in-page
/// instead of opening a desktop-style editor window.
#[tauri::command]
pub async fn read_document_cmd(path: String) -> Result<crate::state::CurrentDocument, String> {
    // iOS hands shared/opened files as percent-encoded `file://` URLs — normalize.
    let p = normalize_local_path(&path);
    let path = p.to_string_lossy().into_owned();
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
    if !p.exists() {
        return Err(format!("file not found: {path}"));
    }
    let path_for_thread = p.clone();
    let parse = tokio::time::timeout(
        std::time::Duration::from_secs(180),
        tokio::task::spawn_blocking(move || {
            capture::doc_loader::load_rich_from_file(&path_for_thread)
        }),
    )
    .await;
    let rich = match parse {
        Ok(Ok(Ok(v))) => v,
        Ok(Ok(Err(e))) => return Err(e.to_string()),
        Ok(Err(e)) => return Err(format!("parse task failed: {e}")),
        Err(_) => return Err(format!("timed out parsing {filename} after 180s")),
    };
    if rich.iter().all(|rp| rp.text.trim().is_empty()) {
        return Err("no readable text in this document".into());
    }
    let total_chars: usize = rich.iter().map(|rp| rp.text.chars().count()).sum();
    let (mut paragraphs, mut pauses, mut speeds, mut kinds) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for rp in rich {
        paragraphs.push(rp.text);
        pauses.push(rp.pause_before);
        speeds.push(rp.speed_mult);
        kinds.push(rp.kind);
    }
    Ok(crate::state::CurrentDocument {
        path,
        filename,
        extension: ext,
        paragraphs,
        char_count: total_chars,
        loading: false,
        paragraph_pauses: pauses,
        paragraph_speed_mult: speeds,
        paragraph_kinds: kinds,
    })
}

/// Load in-memory text (e.g. a web article extracted by defuddle from a shared
/// URL) as a document: write it to a temp `.md` and parse it through the normal
/// markdown reader so it gets sections/rhythm and opens in the reader instead of
/// being read "blind" via synthesize_text. Returns the same CurrentDocument that
/// read_document_cmd produces.
#[tauri::command]
pub async fn read_text_as_document_cmd(
    text: String,
    filename: String,
) -> Result<crate::state::CurrentDocument, String> {
    let stem: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let stem = if stem.is_empty() {
        "Shared article".to_string()
    } else {
        stem.chars().take(60).collect()
    };
    let path = std::env::temp_dir().join(format!("{stem}.md"));
    std::fs::write(&path, text.as_bytes()).map_err(|e| e.to_string())?;
    read_document_cmd(path.to_string_lossy().into_owned()).await
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
pub fn clear_current_document_cmd(state: State<'_, Arc<AppState>>, window: tauri::Window) {
    let label = window.label();
    tracing::info!("[doc:cmd] clear_current_document_cmd({label})");
    state.documents.lock().unwrap().remove(label);
}

/// Stable filesystem-safe key for project autosave. Short paths keep a
/// base64-url encoding of the absolute path (reversible, eyeball-debuggable).
/// Long paths — e.g. iOS app-container paths, whose base64 blows past the
/// 255-byte filename limit once we append ".json"/".json.tmp" and silently
/// fails the write — fall back to a fixed-length, deterministic hash prefixed
/// with the file stem so it's still recognisable.
fn project_key(path: &str) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(path.as_bytes());
    if b64.len() <= 180 {
        return b64;
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    let stem: String = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(48)
        .collect();
    let stem = if stem.is_empty() {
        "doc".to_string()
    } else {
        stem
    };
    format!("{stem}-{:016x}", h.finish())
}

fn project_path(app: &AppHandle, doc_path: &str) -> Result<std::path::PathBuf, String> {
    let mut p = app.path().app_config_dir().map_err(|e| e.to_string())?;
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
    let tmp = p.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, &project_json) {
        tracing::error!(
            "[doc:cmd] save_project_cmd write failed ({}): {e}",
            tmp.display()
        );
        return Err(e.to_string());
    }
    if let Err(e) = std::fs::rename(&tmp, &p) {
        tracing::error!(
            "[doc:cmd] save_project_cmd rename failed ({}): {e}",
            p.display()
        );
        return Err(e.to_string());
    }
    tracing::info!(
        "[doc:cmd] save_project_cmd → {} ({} bytes)",
        p.display(),
        project_json.len()
    );
    Ok(())
}

#[tauri::command]
pub fn load_project_cmd(app: AppHandle, doc_path: String) -> Result<Option<String>, String> {
    let p = project_path(&app, &doc_path)?;
    if !p.exists() {
        tracing::info!(
            "[doc:cmd] load_project_cmd: no project file at {}",
            p.display()
        );
        return Ok(None);
    }
    let json = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    tracing::info!(
        "[doc:cmd] load_project_cmd: loaded {} bytes from {}",
        json.len(),
        p.display()
    );
    Ok(Some(json))
}

/// Una entrada de la biblioteca de documentos del escritorio: cada proyecto
/// autosalvado (que hasta ahora era invisible) con lo justo para pintar la
/// ficha y reabrirlo.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentoBiblioteca {
    pub doc_path: String,
    pub filename: String,
    pub saved_at: Option<String>,
    pub parrafos: usize,
    /// false si el fichero original ya no está donde estaba.
    pub existe: bool,
}

#[tauri::command]
pub fn biblioteca_documentos_cmd(app: AppHandle) -> Result<Vec<DocumentoBiblioteca>, String> {
    let mut dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    dir.push("projects");
    let mut salida = Vec::new();
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Ok(salida);
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(json) = std::fs::read_to_string(&ruta) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
            continue;
        };
        let Some(doc_path) = v.get("doc_path").and_then(|p| p.as_str()) else {
            continue;
        };
        let filename = std::path::Path::new(doc_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| doc_path.to_string());
        salida.push(DocumentoBiblioteca {
            doc_path: doc_path.to_string(),
            filename,
            saved_at: v.get("saved_at").and_then(|s| s.as_str()).map(String::from),
            parrafos: v
                .get("paragraphs")
                .and_then(|p| p.as_array())
                .map(|a| a.len())
                .unwrap_or(0),
            existe: std::path::Path::new(doc_path).exists(),
        });
    }
    salida.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    Ok(salida)
}

#[tauri::command]
pub fn biblioteca_olvidar_cmd(app: AppHandle, doc_path: String) -> Result<(), String> {
    let p = project_path(&app, &doc_path)?;
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    if !crate::compras::es_pro() {
        return Err("parlanchin".into());
    }
    if paragraphs.is_empty() {
        return Err("nothing to render".into());
    }
    if !model::is_model_ready(&app).map_err(|e| e.to_string())? {
        return Err("voice model not installed".into());
    }
    let root = model::model_root(&app).map_err(|e| e.to_string())?;
    let engine = state.engine_or_load(&root).map_err(|e| e.to_string())?;
    #[cfg(desktop)]
    let avisar_al_terminar = state.settings.lock().unwrap().notify_on_done;

    let (default_voice, default_speed, default_lang, total_steps) = {
        let s = state.settings.lock().unwrap();
        (
            s.voice.clone(),
            s.speed,
            s.default_lang.clone(),
            s.quality.total_steps(),
        )
    };

    let state_for_render = state.inner().clone();
    let app_for_thread = app.clone();
    let total = paragraphs.len();
    let want_m4b = std::path::Path::new(&output_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("m4b") || e.eq_ignore_ascii_case("m4a"))
        .unwrap_or(false);
    // El FORMATO DE LA CASA: audio + karaoke + texto en un solo fichero.
    let want_yappy = std::path::Path::new(&output_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(crate::yappy_pack::EXTENSION))
        .unwrap_or(false);
    let texto_fuente: String = paragraphs
        .iter()
        .map(|p| p.text.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    #[allow(unused_variables)] // solo lo usa el brazo iOS del cierre
    let activity_title = std::path::Path::new(&output_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Audiobook".to_string());
    #[allow(unused_variables)] // solo lo usa el brazo iOS del cierre
    let activity_total = paragraphs.len() as i32;

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // Una sola síntesis a la vez en todo el proceso: el render sostiene
        // el candado del motor (la cocina de muestras usa try_lock y espera).
        let _motor = state_for_render.candado_motor.lock().unwrap();
        // iOS: engage silent-audio keepalive + Live Activity for the duration
        // of the render. The OS otherwise suspends us within ~30s of going to
        // background; audiobook renders can run for hours. The Live Activity
        // gives the user visible progress on Lock Screen / Dynamic Island.
        // Both are released automatically when this closure returns.
        #[cfg(target_os = "ios")]
        let _audio_keepalive = crate::mobile::BackgroundAudioGuard::begin();
        #[cfg(target_os = "ios")]
        crate::mobile::activity_start(&activity_title, activity_total);

        // Windows: paint taskbar progress indicator + announce the render in
        // the SMTC volume flyout so the user can see what's happening even
        // when the Yappy window is hidden.
        #[cfg(target_os = "windows")]
        {
            crate::os_win::taskbar_progress_set(0, activity_total.max(1) as u64);
            crate::os_win::smtc_set_metadata(&activity_title, "Yappy — rendering", false);
        }

        let mut combined: Vec<f32> = Vec::new();
        let mut sample_rate: u32 = 0;
        // Sample-offset → chapter title, collected as we go. Used only for m4b.
        let mut chapters: Vec<(usize, String)> = Vec::new();
        // EL KARAOKE DEL RENDER: los tiempos por frase que el playback en
        // vivo calcula y que aquí antes se tiraban a la basura.
        let mut tiempos: Vec<crate::yappy_pack::TiempoFrase> = Vec::new();

        for (i, p) in paragraphs.iter().enumerate() {
            #[cfg(target_os = "ios")]
            crate::mobile::activity_update(i as i32, activity_total, "synth", None);
            #[cfg(target_os = "windows")]
            crate::os_win::taskbar_progress_set(i as u64, activity_total.max(1) as u64);
            let _ = app_for_thread.emit(
                "audiobook_render_progress",
                serde_json::json!({ "index": i, "total": total, "stage": "synth" }),
            );

            // Insert a pause BEFORE this paragraph (skipped for the first one).
            if i > 0 {
                let pause_secs = p.pause_before.unwrap_or(0.35).max(0.0);
                if sample_rate > 0 && pause_secs > 0.0 {
                    let n = (pause_secs * sample_rate as f32) as usize;
                    combined.extend(std::iter::repeat_n(0.0_f32, n));
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
            detectar_idioma: true,
            pausa_entre_parrafos_s: 0.0,
            };

            type ChunkCapturado = (u32, Vec<f32>, String, usize, usize, bool);
            let captured: std::sync::Mutex<Vec<ChunkCapturado>> = std::sync::Mutex::new(Vec::new());
            engine
                .synthesize_streaming(&p.text, &opts, |chunk| {
                    captured.lock().unwrap().push((
                        chunk.sample_rate as u32,
                        chunk.samples.clone(),
                        chunk.text.clone(),
                        chunk.origen_ini,
                        chunk.origen_fin,
                        chunk.es_pausa,
                    ));
                    Ok(())
                })
                .map_err(|e| format!("synth failed on paragraph {}: {e:?}", i + 1))?;

            for (sr, samples, texto_frase, oi, of, es_pausa) in captured.lock().unwrap().iter() {
                if sample_rate == 0 {
                    sample_rate = *sr;
                }
                let ini = combined.len();
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
                if !es_pausa && !texto_frase.trim().is_empty() && sample_rate > 0 {
                    tiempos.push(crate::yappy_pack::TiempoFrase {
                        ini_s: ini as f32 / sample_rate as f32,
                        fin_s: combined.len() as f32 / sample_rate as f32,
                        parrafo: i,
                        origen_ini: *oi,
                        origen_fin: *of,
                        texto: texto_frase.clone(),
                    });
                }
            }
        }

        let _ = app_for_thread.emit(
            "audiobook_render_progress",
            serde_json::json!({ "index": total, "total": total, "stage": "writing" }),
        );
        #[cfg(target_os = "ios")]
        crate::mobile::activity_update(total as i32, activity_total, "writing", None);

        let final_sr = sample_rate.max(44100);

        if want_yappy || want_m4b {
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

            if want_yappy {
                // Codificar el m4b a un temporal ÚNICO y empaquetarlo con el
                // karaoke y el texto en el .yappy final.
                let tmp = std::env::temp_dir().join(format!(
                    "yappy-render-{}-{}.m4b",
                    std::process::id(),
                    combined.len()
                ));
                crate::audiobook::encode_m4b(&combined, final_sr, &chapter_objs, &meta, &tmp)
                    .map_err(|e| format!("m4b encode failed: {e:?}"))?;
                let manifiesto = crate::yappy_pack::ManifiestoPack {
                    version: crate::yappy_pack::VERSION,
                    titulo: if meta.title.is_empty() {
                        activity_title.clone()
                    } else {
                        meta.title.clone()
                    },
                    autor: meta.author.clone(),
                    voz: default_voice.clone(),
                    velocidad: default_speed,
                    idioma: default_lang.clone(),
                    duracion_secs: combined.len() as f32 / final_sr as f32,
                    capitulos: chapter_objs
                        .iter()
                        .map(|c| crate::yappy_pack::CapituloPack {
                            titulo: c.title.clone(),
                            inicio_s: c.start_secs as f32,
                        })
                        .collect(),
                    creado_unix: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    app_version: env!("CARGO_PKG_VERSION").to_string(),
                };
                // El título DICHO, si la caché lo tiene: suena al instante.
                let titulo_dicho = dicho_path(&app_for_thread, &default_voice, &manifiesto.titulo)
                    .ok()
                    .filter(|p| p.exists());
                crate::yappy_pack::escribir(
                    std::path::Path::new(&output_path),
                    &manifiesto,
                    &tmp,
                    &tiempos,
                    &texto_fuente,
                    titulo_dicho.as_deref(),
                )
                .map_err(|e| format!("no se pudo escribir el .yappy: {e:?}"))?;
                let _ = std::fs::remove_file(&tmp);
            } else {
                crate::audiobook::encode_m4b(
                    &combined,
                    final_sr,
                    &chapter_objs,
                    &meta,
                    std::path::Path::new(&output_path),
                )
                .map_err(|e| format!("m4b encode failed: {e:?}"))?;
            }
        } else {
            crate::playback::write_wav_file(&output_path, &combined, final_sr)
                .map_err(|e| format!("write wav failed: {e:?}"))?;
        }

        let _ = app_for_thread.emit(
            "audiobook_render_done",
            serde_json::json!({ "path": output_path, "samples": combined.len(), "sample_rate": final_sr }),
        );
        #[cfg(target_os = "ios")]
        {
            crate::mobile::activity_end(&activity_title);
            // Local notification — the user probably switched apps or
            // locked the phone during a multi-hour render. Bring them back.
            let mins = (combined.len() as f64 / final_sr as f64 / 60.0).round() as i64;
            crate::mobile::notify(
                "yappy.render.done",
                "Audiobook ready",
                &format!("{} — {} min of audio", activity_title, mins.max(1)),
            );
        }
        #[cfg(target_os = "windows")]
        {
            crate::os_win::taskbar_progress_clear();
            crate::os_win::smtc_clear();
        }
        // Escritorio: la notificación de «audiolibro listo» que el ajuste
        // notify_on_done prometía desde la 0.1.
        #[cfg(desktop)]
        {
            if avisar_al_terminar {
                use tauri_plugin_notification::NotificationExt;
                let mins = (combined.len() as f64 / final_sr as f64 / 60.0).round() as i64;
                let _ = app_for_thread
                    .notification()
                    .builder()
                    .title("Audiobook ready")
                    .body(format!("{} min of audio, in your library", mins.max(1)))
                    .show();
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// EL PERMISO DE AVISOS (el paseo lo pide explicando para qué). En
/// escritorio no hay diálogo: se da por concedido.
#[tauri::command]
pub fn avisos_pedir_cmd() {
    #[cfg(target_os = "ios")]
    crate::mobile::notify_request();
}

/// 0 sin decidir · 1 concedido · 2 denegado.
#[tauri::command]
pub fn avisos_estado_cmd() -> i32 {
    #[cfg(target_os = "ios")]
    {
        return crate::mobile::notify_status();
    }
    #[cfg(not(target_os = "ios"))]
    1
}

/// Begin reading a document. Called from the document window after the user clicks
/// "play all" or a specific paragraph. Synthesizes WITHOUT opening the mini-player.
///
/// `voice_override` is a one-shot voice that supersedes the persisted user voice
/// for THIS playback only — used by the "re-read this section with another voice" UI.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn read_document_paragraphs_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paragraphs: Vec<String>,
    from_index: usize,
    voice_override: Option<String>,
    // One-shot overrides for THIS playback. Used by the document window's
    // rhythm slider; multiplies the persisted global speed without mutating
    // settings.
    speed_override: Option<f32>,
    // El guion enriquecido del editor: clase de cada párrafo y anulaciones
    // por párrafo (pausa previa efectiva en segundos, multiplicador de
    // velocidad, voz). Arrays paralelos a `paragraphs`, opcionales para
    // compatibilidad con llamadores viejos.
    kinds: Option<Vec<String>>,
    pausas: Option<Vec<f32>>,
    velocidades: Option<Vec<f32>>,
    voces: Option<Vec<Option<String>>>,
    // Ruta del documento (para la aguja y la restauración), título humano de
    // la sesión, y si la sesión nace EN PAUSA (saltar desde el guion estando
    // en pausa no debe arrancar audio). Opcionales por compatibilidad.
    doc_path: Option<String>,
    titulo: Option<String>,
    start_paused: Option<bool>,
    doc_lang: Option<String>,
) -> Result<(), String> {
    // La voz de verdad manda: el dicho del título se calla al instante.
    #[cfg(target_os = "ios")]
    crate::mobile::efecto_stop();
    let receta = crate::state::RecetaLectura {
        paragraphs,
        kinds,
        pausas,
        velocidades,
        voces,
        doc_lang: doc_lang.filter(|l| !l.is_empty() && l != "auto"),
        doc_path: doc_path.unwrap_or_default(),
        titulo: titulo.unwrap_or_default(),
    };
    *state.receta_sesion.lock().unwrap() = Some(receta.clone());
    leer_parrafos(
        app,
        state.inner().clone(),
        receta,
        from_index,
        voice_override,
        speed_override,
        start_paused.unwrap_or(false),
    )
    .await
}

/// El cuerpo REAL de una sesión de lectura por párrafos. Vive fuera del
/// comando para poder RELANZARLA (cambio de voz en caliente) con la misma
/// receta desde cualquier punto.
pub async fn leer_parrafos(
    app: AppHandle,
    state: Arc<AppState>,
    receta: crate::state::RecetaLectura,
    from_index: usize,
    voice_override: Option<String>,
    speed_override: Option<f32>,
    start_paused: bool,
) -> Result<(), String> {
    let crate::state::RecetaLectura {
        paragraphs,
        kinds,
        pausas,
        velocidades,
        voces,
        doc_lang,
        doc_path,
        titulo,
    } = receta;
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
    let state_ref = &state;
    // Optional one-shot speed override. We temporarily mutate settings
    // in-memory to feed the engine; the disk-persisted value isn't touched.
    let original_speed = if let Some(s) = speed_override {
        let mut settings = state.settings.lock().unwrap();
        let prev = settings.speed;
        settings.speed = s.clamp(0.3, 3.0);
        Some(prev)
    } else {
        None
    };

    // Construye el Guion con lo que sabe el editor. Cada pieza pasa por el
    // guionizador (idioma + verbalización con spans) y luego recibe sus
    // anulaciones.
    // EL IDIOMA DEL TEXTO: el fijado en el taller manda; si no, se
    // DETECTA sobre el documento entero (whatlang, 31 idiomas).
    let idioma_base = match &doc_lang {
        Some(l) => l.clone(),
        None => {
            let pref = state.settings.lock().unwrap().default_lang.clone();
            yappy_core::lang_detect::detect_document_lang(&joined, &pref)
        }
    };
    let guion = {
        use yappy_core::guion::{construir_pieza, ClasePieza, Guion};
        let mut piezas = Vec::new();
        for (i, texto) in paragraphs.iter().enumerate().skip(from_index) {
            if texto.trim().is_empty() {
                continue;
            }
            let clase = kinds
                .as_ref()
                .and_then(|k| k.get(i))
                .map(|k| ClasePieza::desde_kind(k))
                .unwrap_or(ClasePieza::Parrafo);
            let mut pieza = construir_pieza(texto, clase, &idioma_base);
            if let Some(p) = pausas.as_ref().and_then(|v| v.get(i)) {
                pieza.pausa_antes_s = p.clamp(0.0, 10.0);
            }
            if let Some(m) = velocidades.as_ref().and_then(|v| v.get(i)) {
                pieza.mult_velocidad = m.clamp(0.25, 2.0);
            }
            if let Some(Some(v)) = voces.as_ref().and_then(|v| v.get(i)) {
                if !v.is_empty() {
                    pieza.voz = Some(v.clone());
                }
            }
            piezas.push(pieza);
        }
        Guion {
            idioma_base,
            piezas,
        }
    };

    let result = read_internal(
        &app,
        state_ref.clone(),
        joined,
        Some(guion),
        voice,
        doc_lang.clone(),
        "document".into(),
        ReadMode::Document {
            base_paragraph_index: from_index,
        },
        SessionMeta {
            doc_path,
            titulo,
            start_paused,
        },
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
pub fn stop_playback_cmd(app: AppHandle, state: State<'_, Arc<AppState>>) {
    if crate::libro::activo() {
        crate::libro::parar(&app, &state);
        return;
    }
    state.playback.stop();
}

/// Salto por FRASE dentro de lo ya sintetizado: instantáneo, sin resíntesis.
#[tauri::command]
pub fn saltar_frase_cmd(state: State<'_, Arc<AppState>>, delta: i32) {
    #[cfg(target_os = "ios")]
    if crate::libro::activo() {
        crate::libro::saltar_frase(&state, delta);
        return;
    }
    state.playback.saltar_chunk(delta);
}

/// Salto por PÁRRAFO dentro de lo ya sintetizado.
#[tauri::command]
pub fn saltar_parrafo_cmd(state: State<'_, Arc<AppState>>, delta: i32) {
    #[cfg(target_os = "ios")]
    if crate::libro::activo() {
        crate::libro::saltar_parrafo(&state, delta);
        return;
    }
    state.playback.saltar_parrafo(delta);
}

/// Pausa directa (no toggle): para la aguja y los mandos que saben lo que
/// quieren. Instantánea a nivel de mezclador.
#[tauri::command]
pub fn pausar_cmd(state: State<'_, Arc<AppState>>) {
    #[cfg(target_os = "ios")]
    if crate::libro::activo() {
        crate::libro::pausa(&state);
        return;
    }
    state.playback.pause();
}

/// Reanudación directa (no toggle).
#[tauri::command]
pub fn reanudar_cmd(state: State<'_, Arc<AppState>>) {
    #[cfg(target_os = "ios")]
    if crate::libro::activo() {
        crate::libro::reanuda(&state);
        return;
    }
    state.playback.resume();
}

#[tauri::command]
pub async fn toggle_pause_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    if crate::libro::activo() {
        crate::libro::toggle(&state);
        return Ok(());
    }
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

/// Trigger an iOS haptic. No-op on desktop. `kind` is one of:
/// light, medium, heavy, selection, success, warning, error.
#[tauri::command]
pub fn haptic_cmd(kind: String) {
    #[cfg(target_os = "ios")]
    crate::mobile::haptic(&kind);
    #[cfg(not(target_os = "ios"))]
    let _ = kind;
}

/// Pull any pending iOS Share-Sheet payloads from the App Group queue. The
/// frontend calls this when its listeners are ready (on mount + each
/// foreground) so a cold-launch share is never lost to a startup race.
/// Returns a newline-separated string (`url:`/`text:`/`audio:`/`transcript:`
/// lines) or null when the queue is empty. No-op (null) on desktop.
#[tauri::command]
pub fn drain_shared_payloads_cmd() -> Option<String> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        crate::mobile::drain_shared_payload_string()
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        None
    }
}

/// Present iOS's system share sheet for a file at `path`. The sheet shows
/// AirDrop, Apple Books, Messages, Mail, Files, etc. as destinations.
/// No-op on desktop (desktop already has its own "Save as" + Reveal in Finder).
#[tauri::command]
pub fn share_file_cmd(path: String) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    crate::mobile::share_file(&path);
    #[cfg(not(target_os = "ios"))]
    let _ = path;
    Ok(())
}

/// Build a destination path for an exported audiobook inside the app's
/// Documents directory. iOS has no save dialog and the webview can't touch the
/// filesystem, so the frontend asks the backend where to write; the resulting
/// `.m4b` then shows up in the in-app Library and can be shared out. `name` is a
/// human title (e.g. the document filename); we sanitise it into a safe
/// filename and always use the `.m4b` extension.
#[tauri::command]
pub fn audiobook_export_path_cmd(
    app: AppHandle,
    name: String,
    extension: Option<String>,
) -> Result<String, String> {
    use tauri::Manager;
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = {
        let cleaned: String = name
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
        if cleaned.is_empty() {
            "audiobook".to_string()
        } else {
            cleaned
        }
    };
    // El formato de la casa por defecto: .yappy (audio + karaoke + texto).
    let ext = extension.unwrap_or_else(|| crate::yappy_pack::EXTENSION.to_string());
    let mut path = dir.join(format!("{stem}.{ext}"));
    // Avoid clobbering an existing export: append " (2)", " (3)", …
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    Ok(path.to_string_lossy().into_owned())
}

/// List rendered audiobook files (`.m4b`, `.wav`, `.mp3`, `.m4a`) in the
/// app's documents directory along with extracted metadata (duration,
/// chapter count) and resume position. iOS users can't browse the
/// filesystem freely, so the in-app library is the only way to find
/// previously-rendered audio.
#[derive(serde::Serialize, Debug, Clone)]
pub struct LibraryItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub mtime_ms: i64,
    /// Duration in seconds. None for non-m4b files we don't parse.
    pub duration_secs: Option<f64>,
    /// Chapter count for m4b. 0 if file has no chapters or isn't an m4b.
    pub chapter_count: usize,
    /// First chapter title — handy for the library row subtitle when the
    /// filename is generic (e.g. "Chapter 1: Where it all began").
    pub first_chapter_title: Option<String>,
    /// Last-known playback position in seconds. 0 if never played or
    /// finished playing.
    pub resume_secs: f64,
}

fn library_resume_map_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("library_resume.json"))
}

/// Guarda la posición del audiolibro EN CURSO (si lo hay) en el mapa de
/// reanudación. Factorizado para que el libro vivo lo use al ceder la voz.
#[cfg(target_os = "ios")]
pub fn persistir_resume_libro<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(p) = crate::mobile::audiofile_current_path() {
        let pos = crate::mobile::audiofile_position();
        if pos > 5.0 {
            let mut map = read_resume_map_generico(app);
            map.insert(p, pos);
            let _ = write_resume_map_generico(app, &map);
        }
    }
}

#[cfg(target_os = "ios")]
fn ruta_resume_generico<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    // El MISMO fichero que usa la biblioteca (library_resume_map_path):
    // un solo mapa de posiciones, lo abra quien lo abra.
    use tauri::Manager;
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("library_resume.json"))
}

#[cfg(target_os = "ios")]
fn read_resume_map_generico<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> std::collections::HashMap<String, f64> {
    ruta_resume_generico(app)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

#[cfg(target_os = "ios")]
fn write_resume_map_generico<R: tauri::Runtime>(
    app: &AppHandle<R>,
    map: &std::collections::HashMap<String, f64>,
) -> Result<(), String> {
    let p = ruta_resume_generico(app).ok_or("sin app_data")?;
    std::fs::write(p, serde_json::to_vec(map).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn read_resume_map(app: &AppHandle) -> std::collections::HashMap<String, f64> {
    library_resume_map_path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_resume_map(
    app: &AppHandle,
    map: &std::collections::HashMap<String, f64>,
) -> Result<(), String> {
    let p = library_resume_map_path(app)?;
    let s = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(p, s).map_err(|e| e.to_string())
}

/// La ruta del audio REPRODUCIBLE de una pieza de biblioteca: para un
/// .yappy, su m4b interno extraído a la caché; para el resto, ella misma.
fn ruta_audio_de(app: &AppHandle, path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::Path::new(path);
    if crate::yappy_pack::es_yappy(p) {
        let cache = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("biblioteca-cache");
        crate::yappy_pack::extraer_audio(p, &cache).map_err(|e| e.to_string())
    } else {
        Ok(p.to_path_buf())
    }
}

/// El audio listo para el reproductor del ESCRITORIO (el webview lo toca
/// con un <audio> vía asset protocol). Devuelve la ruta absoluta.
#[tauri::command]
pub fn library_audio_src_cmd(app: AppHandle, path: String) -> Result<String, String> {
    ruta_audio_de(&app, &path).map(|p| p.to_string_lossy().to_string())
}

/// Los TIEMPOS de karaoke de un .yappy (vacío para m4b sueltos).
#[tauri::command]
pub fn library_tiempos_cmd(path: String) -> Vec<crate::yappy_pack::TiempoFrase> {
    let p = std::path::Path::new(&path);
    if !crate::yappy_pack::es_yappy(p) {
        return Vec::new();
    }
    crate::yappy_pack::leer(p)
        .map(|c| c.tiempos)
        .unwrap_or_default()
}

/// IMPORTAR un .yappy: a la biblioteca (document_dir) con el nombre de su
/// título, y su título DICHO a la caché de dichos (suena al instante).
#[tauri::command]
pub fn library_import_yappy_cmd(app: AppHandle, ruta: String) -> Result<String, String> {
    importar_yappy(&app, &ruta)
}

/// LOS CUENTOS EMPAQUETADOS: los `.yappy` de resources/cuentos entran en la
/// biblioteca en la primera apertura (una vez por título). Suenan sin
/// motor: son la primera escucha del onboarding mientras llegan las voces.
pub fn importar_cuentos_empaquetados(app: &AppHandle) {
    let Ok(base) = app.path().resource_dir() else {
        return;
    };
    let dirs = [
        base.join("_up_").join("resources").join("cuentos"),
        base.join("resources").join("cuentos"),
        base.join("cuentos"),
    ];
    let Some(dir) = dirs.iter().find(|d| d.is_dir()) else {
        return;
    };
    let marca = app
        .path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("cuentos-importados.json"));
    let mut hechos: Vec<String> = marca
        .as_ref()
        .and_then(|m| std::fs::read_to_string(m).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    let mut cambio = false;
    for e in entradas.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("yappy") {
            continue;
        }
        let nombre = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if hechos.contains(&nombre) {
            continue;
        }
        match importar_yappy(app, &p.to_string_lossy()) {
            Ok(destino) => {
                tracing::info!("cuento empaquetado en la biblioteca: {destino}");
                hechos.push(nombre);
                cambio = true;
            }
            Err(err) => tracing::warn!("cuento empaquetado {nombre}: {err}"),
        }
    }
    if cambio {
        if let (Some(m), Ok(json)) = (marca, serde_json::to_string(&hechos)) {
            let _ = std::fs::write(m, json);
        }
    }
}

/// Copia un `.yappy` a la biblioteca (Documentos) con su título como nombre.
pub fn importar_yappy(app: &AppHandle, ruta: &str) -> Result<String, String> {
    let origen = std::path::Path::new(ruta);
    if !crate::yappy_pack::es_yappy(origen) {
        return Err("no es un archivo .yappy".into());
    }
    let mani = crate::yappy_pack::leer_manifiesto(origen).map_err(|e| e.to_string())?;
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    let base: String = mani
        .titulo
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { ' ' } else { c })
        .collect::<String>()
        .trim()
        .chars()
        .take(80)
        .collect();
    let base = if base.is_empty() {
        "Audiolibro".to_string()
    } else {
        base
    };
    let mut destino = dir.join(format!("{base}.yappy"));
    let mut i = 2;
    while destino.exists() {
        destino = dir.join(format!("{base} ({i}).yappy"));
        i += 1;
    }
    std::fs::copy(origen, &destino).map_err(|e| e.to_string())?;
    // El título dicho, a la caché de dichos de SU voz.
    if let Ok(wav) = dicho_path(app, &mani.voz, &mani.titulo) {
        let _ = crate::yappy_pack::extraer_titulo_wav(&destino, &wav);
    }
    Ok(destino.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_rendered_audiobooks_cmd(app: AppHandle) -> Result<Vec<LibraryItem>, String> {
    use tauri::Manager;
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    let resume_map = read_resume_map(&app);
    let mut items: Vec<LibraryItem> = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());
        if !matches!(
            ext.as_deref(),
            Some("m4b") | Some("wav") | Some("mp3") | Some("m4a") | Some("yappy")
        ) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mut name = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        if ext.as_deref() == Some("yappy") {
            if let Ok(m) = crate::yappy_pack::leer_manifiesto(&path) {
                if !m.titulo.trim().is_empty() {
                    name = m.titulo;
                }
            }
        }
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // Extract m4b metadata for richer library rows.
        let (duration_secs, chapter_count, first_chapter_title) = if ext.as_deref() == Some("yappy")
        {
            match crate::yappy_pack::leer_manifiesto(&path) {
                Ok(m) => (
                    Some(m.duracion_secs as f64),
                    m.capitulos.len(),
                    m.capitulos.first().map(|c| c.titulo.clone()),
                ),
                Err(_) => (None, 0, None),
            }
        } else if matches!(ext.as_deref(), Some("m4b") | Some("m4a")) {
            if let Some(info) = crate::audiobook::read_m4b_info(&path) {
                (
                    Some(info.duration_secs),
                    info.chapter_count,
                    info.first_chapter_title,
                )
            } else {
                (None, 0, None)
            }
        } else {
            (None, 0, None)
        };
        let path_str = path.to_string_lossy().to_string();
        let resume_secs = resume_map.get(&path_str).copied().unwrap_or(0.0);
        items.push(LibraryItem {
            name,
            path: path_str,
            size: meta.len(),
            mtime_ms,
            duration_secs,
            chapter_count,
            first_chapter_title,
            resume_secs,
        });
    }
    items.sort_by_key(|it| std::cmp::Reverse(it.mtime_ms));
    Ok(items)
}

/// Start playback of a saved audiobook via AVAudioPlayer (iOS only).
/// Resumes from the last-known position unless `from_start` is true.
#[tauri::command]
pub fn library_play_cmd(
    _app: AppHandle,
    path: String,
    from_start: Option<bool>,
) -> Result<bool, String> {
    let start_at = if from_start.unwrap_or(false) {
        0.0
    } else {
        #[cfg(target_os = "ios")]
        {
            let map = read_resume_map(&_app);
            map.get(&path).copied().unwrap_or(0.0)
        }
        #[cfg(not(target_os = "ios"))]
        0.0
    };
    #[cfg(target_os = "ios")]
    {
        let reproducible = ruta_audio_de(&_app, &path)?;
        let ok = crate::mobile::audiofile_play(&reproducible.to_string_lossy(), start_at);
        // Push Now Playing metadata so the lock screen / Control Center
        // show what's playing. Pull title from filename.
        let title = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Yappy")
            .to_string();
        let duration = crate::mobile::audiofile_duration();
        crate::mobile::now_playing_set(&title, "Yappy", "", duration, start_at, true);
        // El ESPEJO: cualquier .yappy sonando se refleja como sesión de
        // reproducción (aguja, chip, karaoke del lector). Los .m4b sin
        // tiempos no tienen espejo (no hay frases que pintar).
        if ok && path.to_lowercase().ends_with(".yappy") {
            crate::libro::espejar(&_app, &path);
        }
        return Ok(ok);
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = path;
        let _ = start_at;
        Ok(false)
    }
}

#[tauri::command]
pub fn library_pause_cmd() {
    #[cfg(target_os = "ios")]
    {
        crate::mobile::audiofile_pause();
        // La pantalla de bloqueo debe reflejar la pausa (antes se quedaba
        // en «reproduciendo» para siempre).
        actualizar_now_playing_biblioteca(false);
    }
}

#[tauri::command]
pub fn library_resume_cmd() {
    #[cfg(target_os = "ios")]
    {
        crate::mobile::audiofile_resume();
        actualizar_now_playing_biblioteca(true);
    }
}

#[cfg(target_os = "ios")]
fn actualizar_now_playing_biblioteca(reproduciendo: bool) {
    if let Some(ruta) = crate::mobile::audiofile_current_path() {
        let titulo = std::path::Path::new(&ruta)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Yappy")
            .to_string();
        crate::mobile::now_playing_set(
            &titulo,
            "Yappy",
            "",
            crate::mobile::audiofile_duration(),
            crate::mobile::audiofile_position(),
            reproduciendo,
        );
    }
}

#[tauri::command]
pub fn library_stop_cmd(app: AppHandle) {
    #[cfg(target_os = "ios")]
    {
        // Persist the resume position before stopping so the next play
        // picks up where we left off.
        if let Some(p) = crate::mobile::audiofile_current_path() {
            let pos = crate::mobile::audiofile_position();
            let mut map = read_resume_map(&app);
            if pos > 5.0 {
                // Only save if more than 5s in (avoids "resume from beginning")
                map.insert(p, pos);
                let _ = write_resume_map(&app, &map);
            }
        }
        crate::mobile::audiofile_stop();
        // Clear the Now Playing — lock screen drops Yappy.
        crate::mobile::now_playing_set("", "", "", 0.0, 0.0, false);
        // Y el espejo del libro (si lo había) vuelve a «inactivo».
        crate::libro::apagar_espejo(&app);
    }
    #[cfg(not(target_os = "ios"))]
    let _ = app;
}

#[tauri::command]
pub fn library_seek_cmd(secs: f64) {
    #[cfg(target_os = "ios")]
    crate::mobile::audiofile_seek(secs);
    #[cfg(not(target_os = "ios"))]
    let _ = secs;
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct LibraryStatus {
    pub current_path: Option<String>,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub playing: bool,
}

#[tauri::command]
pub fn library_status_cmd() -> LibraryStatus {
    #[cfg(target_os = "ios")]
    {
        LibraryStatus {
            current_path: crate::mobile::audiofile_current_path(),
            position_secs: crate::mobile::audiofile_position(),
            duration_secs: crate::mobile::audiofile_duration(),
            playing: crate::mobile::audiofile_is_playing(),
        }
    }
    #[cfg(not(target_os = "ios"))]
    LibraryStatus {
        current_path: None,
        position_secs: 0.0,
        duration_secs: 0.0,
        playing: false,
    }
}

/// Read all chapters from an m4b file. Returns empty if the file isn't an
/// m4b or has no chpl atom.
#[derive(serde::Serialize, Debug, Clone)]
pub struct ChapterEntry {
    pub title: String,
    pub start_secs: f64,
}

#[tauri::command]
pub fn library_chapters_cmd(path: String) -> Vec<ChapterEntry> {
    let p = std::path::Path::new(&path);
    if crate::yappy_pack::es_yappy(p) {
        return crate::yappy_pack::leer_manifiesto(p)
            .map(|m| {
                m.capitulos
                    .into_iter()
                    .map(|c| ChapterEntry {
                        title: c.titulo,
                        start_secs: c.inicio_s as f64,
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    crate::audiobook::read_chpl_chapters(std::path::Path::new(&path))
        .unwrap_or_default()
        .into_iter()
        .map(|c| ChapterEntry {
            title: c.title,
            start_secs: c.start_secs,
        })
        .collect()
}

/// Push the current library state to iOS Spotlight so users can find their
/// rendered audiobooks via system search. No-op on desktop.
#[tauri::command]
pub async fn library_reindex_spotlight_cmd(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    {
        let items = list_rendered_audiobooks_cmd(app).await?;
        // Encode as tab/newline-delimited string. Swift splits on \n and \t.
        let payload: String = items
            .iter()
            .map(|i| {
                format!(
                    "{}\t{}\t{}\t{}",
                    i.path,
                    i.name,
                    i.duration_secs.unwrap_or(0.0),
                    i.chapter_count
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        crate::mobile::spotlight_replace_all(&payload);
    }
    #[cfg(not(target_os = "ios"))]
    let _ = app;
    Ok(())
}

#[tauri::command]
pub fn library_delete_cmd(app: AppHandle, path: String) -> Result<(), String> {
    // Stop playback if the file we're about to delete is currently playing.
    #[cfg(target_os = "ios")]
    if crate::mobile::audiofile_current_path().as_deref() == Some(path.as_str()) {
        crate::mobile::audiofile_stop();
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    // Remove its resume position too.
    let mut map = read_resume_map(&app);
    map.remove(&path);
    let _ = write_resume_map(&app, &map);
    Ok(())
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
    leer_portapapeles(app, state.inner().clone())
        .await
        .map_err(|e| e.to_string())
}

/// El mismo gesto, invocable desde el atajo global (sin State extractor).
pub async fn leer_portapapeles<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let text = capture::clipboard::read_text()
        .ok()
        .flatten()
        .unwrap_or_default();
    if text.trim().is_empty() {
        let _ = app.emit("capture_empty", true);
        return Ok(());
    }
    read_text(&app, state, text, "clipboard".into()).await
}

#[tauri::command]
pub fn is_model_ready(app: AppHandle) -> Result<bool, String> {
    model::is_model_ready(&app).map_err(|e| e.to_string())
}

/// La foto actual de la reproducción, bajo demanda. Las páginas que montan
/// DESPUÉS de que algo suene (la cinta, el cartel) la necesitan para pintar
/// la aguja o el estado de pausa sin esperar a la siguiente emisión.
/// Abre un .yappy como LIBRO VIVO: documento para el lector + audio del
/// reproductor de fichero + espejo de sesión (karaoke, aguja, bloqueo).
#[tauri::command]
pub async fn library_abrir_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
    from_start: Option<bool>,
    desde_parrafo: Option<usize>,
) -> Result<crate::state::CurrentDocument, String> {
    #[cfg(target_os = "ios")]
    {
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            crate::libro::abrir(
                &app,
                &state,
                &path,
                from_start.unwrap_or(false),
                desde_parrafo,
            )
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, state, path, from_start, desde_parrafo);
        Err("solo iOS".into())
    }
}

#[tauri::command]
pub fn playback_snapshot_cmd(
    state: tauri::State<'_, AppState>,
) -> crate::playback::PlaybackSnapshot {
    state.playback.snapshot()
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

/// Open (or focus) the dedicated transcription window on desktop. If `path` is
/// given, emit `transcribe_file` to that window so it transcribes the file. iOS
/// never calls this — it navigates to the `/transcribe` route in-window instead.
#[tauri::command]
pub fn open_transcribe_window(app: AppHandle, path: Option<String>) -> Result<(), String> {
    windows::show_transcribe(&app).map_err(|e| e.to_string())?;
    if let Some(p) = path {
        // The freshly-created window's JS may not have registered its listener
        // yet; a tiny delay lets it subscribe before we emit. Mirrors the
        // document-window hand-off race handled via AppState elsewhere.
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let _ = app2.emit_to("transcribe", "transcribe_file", p);
        });
    }
    Ok(())
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

// ----- MUESTRAS DE VOZ PRECOCINADAS -----
//
// La presentación de cada voz se sintetiza UNA vez (calidad rápida, velocidad
// 1.0) y se cachea como wav. Tocar un cromo = reproducir el fichero por el
// canal de efectos: instantáneo, y SIN tocar la sesión del documento (el que
// esté en pausa sigue en pausa). En el móvil, sintetizar en vivo tardaba
// decenas de segundos y encima mataba la sesión.

fn muestras_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("muestras");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn muestra_path(app: &AppHandle, voice: &str, lang: &str) -> Result<std::path::PathBuf, String> {
    let v: String = voice
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    Ok(muestras_dir(app)?.join(format!("{v}-{lang}-v1.wav")))
}

/// La presentación EMPAQUETADA de una voz (resources/muestras, AAC): suena
/// sin motor, desde el primer segundo de la primera apertura. Tauri copia
/// `../resources/*` bajo `_up_/resources/` en el bundle.
fn muestra_empaquetada(app: &AppHandle, voice: &str, lang: &str) -> Option<std::path::PathBuf> {
    let v: String = voice
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    let nombre = format!("{v}-{lang}.m4a");
    let base = app.path().resource_dir().ok()?;
    [
        base.join("_up_")
            .join("resources")
            .join("muestras")
            .join(&nombre),
        base.join("resources").join("muestras").join(&nombre),
        base.join("muestras").join(&nombre),
    ]
    .into_iter()
    .find(|c| c.exists())
}

/// Sintetiza la presentación de una voz y la escribe en la caché. Bloqueante:
/// llamar desde spawn_blocking o el hilo de cocina.
fn cocinar_muestra_blocking(
    app: &AppHandle,
    state: &Arc<AppState>,
    voice: &str,
    lang: &str,
) -> Result<std::path::PathBuf> {
    // NUNCA dos síntesis a la vez: si el motor está ocupado (lectura en
    // barrera, render de horas) la cocina NO espera: falla rápido y el
    // bucle de fondo reintenta más tarde.
    let _motor = state
        .candado_motor
        .try_lock()
        .map_err(|_| anyhow::anyhow!("motor ocupado"))?;
    let root = model::model_root(app)?;
    let engine = state.engine_or_load(&root)?;
    let texto = sample_for_voice(voice, lang);
    let opts = SynthesisOptions {
        voice: voice.to_string(),
        speed: 1.0,
        default_lang: lang.to_string(),
        total_steps: Quality::Fast.total_steps(),
        seed: Some(7),
        detectar_idioma: false,
        pausa_entre_parrafos_s: 0.0,
    };
    let chunks = engine.synthesize(&texto, &opts)?;
    let sr = chunks.first().map(|c| c.sample_rate).unwrap_or(44100) as u32;
    let samples: Vec<f32> = chunks
        .iter()
        .flat_map(|c| c.samples.iter().copied())
        .collect();
    let p = muestra_path(app, voice, lang).map_err(|e| anyhow::anyhow!(e))?;
    crate::playback::write_wav_file(&p, &samples, sr)?;
    tracing::info!(
        "muestra cocinada: {} ({lang}, {:.1}s)",
        voice,
        samples.len() as f32 / sr as f32
    );
    Ok(p)
}

/// El idioma de las presentaciones: el preferido del usuario, con «na»
/// (autodetección) cayendo a inglés.
fn lang_de_muestras(state: &Arc<AppState>) -> String {
    let l = state.settings.lock().unwrap().default_lang.clone();
    if l == "na" || l.is_empty() {
        "en".into()
    } else {
        l
    }
}

/// La cocina de fondo (móvil): cuando el modelo está listo y no suena nada,
/// va cocinando las presentaciones que falten. Corre en su propio hilo con
/// paciencia infinita; nunca pisa una lectura en marcha.
#[allow(dead_code)]
pub fn precocinar_muestras(app: AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("yappy-muestras".into())
        .spawn(move || {
            // Trabajo de fondo: por debajo de cualquier lectura viva.
            crate::bajar_prioridad_de_hilo();
            // Dejar que la app arranque tranquila antes de gastar CPU.
            std::thread::sleep(std::time::Duration::from_secs(12));
            loop {
                if !model::is_model_ready(&app).unwrap_or(false) {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    continue;
                }
                let lang = lang_de_muestras(&state);
                let faltan: Vec<String> = yappy_core::VOICES
                    .iter()
                    .filter(|v| {
                        muestra_empaquetada(&app, v.name, &lang).is_none()
                            && muestra_path(&app, v.name, &lang)
                                .map(|p| !p.exists())
                                .unwrap_or(false)
                    })
                    .map(|v| v.name.to_string())
                    .collect();
                if faltan.is_empty() {
                    tracing::info!("muestras: todas cocinadas ({lang})");
                    break;
                }
                for voz in faltan {
                    // Nunca competir con una lectura: esperar al silencio.
                    while state.playback.snapshot().estado != "inactivo" {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                    }
                    if let Err(e) = cocinar_muestra_blocking(&app, &state, &voz, &lang) {
                        tracing::warn!("muestras: {voz} falló: {e:?}");
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    }
                }
            }
        })
        .ok();
}

/// Devuelve la duración de la muestra en segundos (0.0 si hubo que cocinarla
/// y no se sabe aún, o en el camino vivo del escritorio).
#[tauri::command]
pub async fn sample_voice(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    voice: String,
    lang: Option<String>,
    sample_text: Option<String>,
) -> Result<f64, String> {
    let lang = match lang {
        Some(l) if !l.is_empty() && l != "na" => l,
        _ => lang_de_muestras(state.inner()),
    };
    #[cfg(target_os = "ios")]
    {
        // Camino instantáneo: la muestra ya está cocinada, o viene
        // EMPAQUETADA con la app (sin motor: la primera apertura ya habla).
        let p = muestra_path(&app, &voice, &lang)?;
        if p.exists() {
            return Ok(crate::mobile::efecto_play(&p.to_string_lossy()));
        }
        if let Some(e) = muestra_empaquetada(&app, &voice, &lang) {
            return Ok(crate::mobile::efecto_play(&e.to_string_lossy()));
        }
        // Aún no cocinada. Con una sesión de lectura viva NO se sintetiza
        // (dos síntesis a la vez tumbaron el simulador): la cocina de fondo
        // la hará al volver el silencio.
        if state.playback.snapshot().estado != "inactivo" {
            return Ok(0.0);
        }
        // Cocinarla al vuelo (corta, rápida) y sonarla. La sesión del
        // documento NO se toca en ningún caso.
        let app2 = app.clone();
        let st = state.inner().clone();
        let v = voice.clone();
        let l = lang.clone();
        let dur = tauri::async_runtime::spawn_blocking(move || -> Result<f64, String> {
            let p = cocinar_muestra_blocking(&app2, &st, &v, &l).map_err(|e| e.to_string())?;
            Ok(crate::mobile::efecto_play(&p.to_string_lossy()))
        })
        .await
        .map_err(|e| e.to_string())??;
        let _ = sample_text;
        return Ok(dur);
    }
    #[cfg(not(target_os = "ios"))]
    {
        let text = sample_text.unwrap_or_else(|| sample_for_voice(&voice, &lang));
        state.playback.stop();
        let h = app.clone();
        let s = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = read_with_voice_lang(&h, s, text, voice, lang, "sample".into()).await {
                tracing::error!("sample_voice: {e:?}");
            }
        });
        Ok(0.0)
    }
}

/// EL PROGRESO DURADERO: el motor escribe por dónde vas a disco
/// (progreso.json, clave por NOMBRE de fichero) mientras suena. Sobrevive
/// a cierres bruscos, al jetsam y a las migraciones del contenedor.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProgresoDoc {
    pub parrafo: usize,
    pub total: usize,
}

fn progreso_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("progreso.json"))
}

pub fn guardar_progreso_disco<R: Runtime>(
    app: &AppHandle<R>,
    doc_path: &str,
    parrafo: usize,
    total: usize,
) {
    let Some(p) = progreso_path(app) else { return };
    let nombre = doc_path.rsplit('/').next().unwrap_or(doc_path).to_string();
    if nombre.is_empty() {
        return;
    }
    let mut mapa: std::collections::HashMap<String, ProgresoDoc> = std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // El TOTAL es monótono: los primeros snapshots traen el total de la
    // cocina incremental (1, 2, 5…), no el del documento; encogerlo
    // inflaba el porcentaje y doraba piezas a medio leer.
    let total = mapa
        .get(&nombre)
        .map(|v| v.total.max(total))
        .unwrap_or(total);
    mapa.insert(nombre, ProgresoDoc { parrafo, total });
    if let Ok(json) = serde_json::to_string(&mapa) {
        let _ = std::fs::write(&p, json);
    }
}

/// Todo el progreso guardado (para sembrar la caché del frontend al abrir).
#[tauri::command]
pub fn progreso_todo_cmd(app: AppHandle) -> std::collections::HashMap<String, ProgresoDoc> {
    progreso_path(&app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// La voz con que se dice/lee una pieza: la del ajuste, o la del dado
/// (al azar estable por NOMBRE de documento) si está activo.
#[cfg_attr(desktop, allow(dead_code))]
fn voz_para(state: &Arc<AppState>, clave: &str) -> String {
    let s = state.settings.lock().unwrap();
    if !s.voz_al_azar || clave.is_empty() {
        return s.voice.clone();
    }
    let nombre = clave.rsplit('/').next().unwrap_or(clave);
    let mut h: u64 = 0;
    for b in nombre.bytes() {
        h = h.wrapping_mul(131).wrapping_add(b as u64);
    }
    yappy_core::VOICES[(h as usize) % yappy_core::VOICES.len()]
        .name
        .to_string()
}

/// La caché de títulos DICHOS: un wav por (voz, texto), instantáneo.
#[cfg_attr(desktop, allow(dead_code))]
pub(crate) fn dicho_path(
    app: &AppHandle,
    voice: &str,
    texto: &str,
) -> std::result::Result<std::path::PathBuf, String> {
    let mut h: u64 = 0;
    for b in voice.bytes().chain(texto.bytes()) {
        h = h.wrapping_mul(131).wrapping_add(b as u64);
    }
    let dir = muestras_dir(app)?.join("titulos");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{h:016x}.wav")))
}

#[cfg(target_os = "ios")]
fn cocinar_dicho_blocking(
    app: &AppHandle,
    state: &Arc<AppState>,
    texto: &str,
    voice: &str,
) -> std::result::Result<std::path::PathBuf, String> {
    let p = dicho_path(app, voice, texto)?;
    if p.exists() {
        return Ok(p);
    }
    let _motor = state
        .candado_motor
        .try_lock()
        .map_err(|_| "motor ocupado".to_string())?;
    let root = model::model_root(app).map_err(|e| e.to_string())?;
    let engine = state.engine_or_load(&root).map_err(|e| e.to_string())?;
    let lang = {
        let s = state.settings.lock().unwrap();
        let l = s.default_lang.clone();
        if l == "na" || l.is_empty() {
            "en".to_string()
        } else {
            l
        }
    };
    let opts = SynthesisOptions {
        voice: voice.to_string(),
        speed: 1.0,
        default_lang: lang,
        total_steps: Quality::Fast.total_steps(),
        seed: Some(7),
        detectar_idioma: true,
        pausa_entre_parrafos_s: 0.0,
    };
    let chunks = engine.synthesize(texto, &opts).map_err(|e| e.to_string())?;
    let sr = chunks.first().map(|c| c.sample_rate).unwrap_or(44100) as u32;
    let samples: Vec<f32> = chunks
        .iter()
        .flat_map(|c| c.samples.iter().copied())
        .collect();
    if samples.is_empty() {
        return Err("síntesis vacía".into());
    }
    crate::playback::write_wav_file(&p, &samples, sr).map_err(|e| e.to_string())?;
    Ok(p)
}

/// La cocina de TÍTULOS: presintetiza el decir de cada pieza de la cola
/// (con su voz efectiva) para que al elegirla hable AL INSTANTE. Corre en
/// su hilo, solo con el modelo listo y la casa en silencio.
#[allow(dead_code)]
pub fn precocinar_titulos(app: AppHandle, state: Arc<AppState>) {
    #[cfg(target_os = "ios")]
    std::thread::Builder::new()
        .name("yappy-titulos".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(20));
            loop {
                if !model::is_model_ready(&app).unwrap_or(false)
                    || state.playback.snapshot().estado != "inactivo"
                {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    continue;
                }
                let piezas = crate::cola::listar(&app).unwrap_or_default();
                let mut hechos = 0;
                for it in piezas.iter().take(40) {
                    if it.titulo.trim().is_empty() {
                        continue;
                    }
                    let clave = it.ruta.clone().unwrap_or_else(|| it.id.clone());
                    let voz = voz_para(&state, &clave);
                    let corto: String = it.titulo.chars().take(90).collect();
                    if dicho_path(&app, &voz, &corto)
                        .map(|p| p.exists())
                        .unwrap_or(true)
                    {
                        continue;
                    }
                    if state.playback.snapshot().estado != "inactivo" {
                        break;
                    }
                    match cocinar_dicho_blocking(&app, &state, &corto, &voz) {
                        Ok(_) => hechos += 1,
                        Err(_) => break,
                    }
                }
                if hechos > 0 {
                    tracing::info!("títulos precocinados: {hechos}");
                }
                std::thread::sleep(std::time::Duration::from_secs(45));
            }
        })
        .ok();
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, state);
    }
}

/// DECIR: la interfaz se lee a sí misma (docs/EL-JUGUETE.md §7). Sintetiza
/// un texto corto (el título de una pieza) con la voz por defecto y lo suena
/// por el canal de efectos, SIN tocar jamás la sesión de lectura. Si algo
/// suena o el motor está ocupado, silencio y a otra cosa: el juguete nunca
/// pisa una lectura. Devuelve la duración (0.0 si calló).
#[tauri::command]
pub async fn decir_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    texto: String,
    velocidad: Option<f32>,
    idioma: Option<String>,
) -> Result<f64, String> {
    #[cfg(target_os = "ios")]
    {
        if state.playback.snapshot().estado != "inactivo" {
            return Ok(0.0);
        }
        let corto: String = texto.chars().take(90).collect();
        if corto.trim().is_empty() {
            return Ok(0.0);
        }
        let app2 = app.clone();
        let st = state.inner().clone();
        let dur = tauri::async_runtime::spawn_blocking(move || -> Result<f64, String> {
            // Nunca dos síntesis a la vez: si el motor está ocupado, callar.
            let _motor = st
                .candado_motor
                .try_lock()
                .map_err(|_| "motor ocupado".to_string())?;
            let root = model::model_root(&app2).map_err(|e| e.to_string())?;
            let engine = st.engine_or_load(&root).map_err(|e| e.to_string())?;
            let (voice, lang) = {
                let s = st.settings.lock().unwrap();
                (s.voice.clone(), s.default_lang.clone())
            };
            // El paseo pide idioma y velocidad a mano (los sellos de idioma,
            // el deslizador con demo en vivo).
            let lang = match idioma {
                Some(l) if !l.is_empty() && l != "na" => l,
                _ => {
                    if lang == "na" || lang.is_empty() {
                        "en".into()
                    } else {
                        lang
                    }
                }
            };
            let opts = SynthesisOptions {
                voice,
                speed: velocidad.unwrap_or(1.0).clamp(0.5, 2.0),
                default_lang: lang,
                total_steps: Quality::Fast.total_steps(),
                seed: Some(7),
                detectar_idioma: true,
                pausa_entre_parrafos_s: 0.0,
            };
            let chunks = engine
                .synthesize(&corto, &opts)
                .map_err(|e| e.to_string())?;
            let sr = chunks.first().map(|c| c.sample_rate).unwrap_or(44100) as u32;
            let samples: Vec<f32> = chunks
                .iter()
                .flat_map(|c| c.samples.iter().copied())
                .collect();
            if samples.is_empty() {
                return Ok(0.0);
            }
            let p = muestras_dir(&app2)?.join("dicho.wav");
            crate::playback::write_wav_file(&p, &samples, sr).map_err(|e| e.to_string())?;
            Ok(crate::mobile::efecto_play(&p.to_string_lossy()))
        })
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(0.0);
        return Ok(dur);
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, state, texto, velocidad, idioma);
        Ok(0.0)
    }
}

/// LA LIBRETA DEL PASEO, a solas. Guardar los ajustes ENTEROS desde el
/// paseo pisaba lo que se hubiera cambiado por el camino (setVoice,
/// setSpeed, el tema): esto toca solo `paseo` (y, si toca, cierra el
/// primer arranque).
#[tauri::command]
pub async fn set_paseo_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paseo: crate::settings::Paseo,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock().unwrap();
        if paseo.hecho {
            s.first_launch_done = true;
        }
        s.paseo = paseo;
    }
    let snapshot = state.settings.lock().unwrap().clone();
    let _guard = state.save_lock.lock().unwrap();
    settings::SettingsStore::save(&app, &snapshot).map_err(|e| e.to_string())
}

// ─── LOS CUENTOS DE LA CASA (resources/cuentos/*.md) ─────────────────────

#[derive(serde::Serialize, Clone)]
pub struct Cuento {
    pub id: String,
    pub titulo: String,
    pub autor: String,
    pub idioma: String,
    /// La primera frase, para que el loro la diga de anticipo.
    pub primera_frase: String,
    pub palabras: usize,
    #[serde(skip)]
    pub texto: String,
}

fn dir_cuentos(app: &AppHandle) -> Option<std::path::PathBuf> {
    let base = app.path().resource_dir().ok()?;
    [
        base.join("_up_").join("resources").join("cuentos"),
        base.join("resources").join("cuentos"),
        base.join("cuentos"),
    ]
    .into_iter()
    .find(|d| d.is_dir())
}

/// Parsea un cuento con cabecera YAML sencilla (`clave: valor` entre `---`).
fn parsear_cuento(id: &str, crudo: &str) -> Option<Cuento> {
    let resto = crudo.strip_prefix("---")?;
    let (cabecera, cuerpo) = resto.split_once("\n---")?;
    let mut titulo = String::new();
    let mut autor = String::new();
    let mut idioma = String::from("es");
    for linea in cabecera.lines() {
        if let Some((k, v)) = linea.split_once(':') {
            let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
            match k.trim() {
                "titulo" => titulo = v,
                "autor" => autor = v,
                "idioma" => idioma = v,
                _ => {}
            }
        }
    }
    let texto = cuerpo.trim().to_string();
    if titulo.is_empty() || texto.is_empty() {
        return None;
    }
    let primera = texto
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let corte = primera
        .char_indices()
        .find(|(i, c)| *i > 20 && matches!(c, '.' | '!' | '?' | '…'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(primera.len().min(160));
    let primera_frase = primera[..corte].to_string();
    Some(Cuento {
        id: id.to_string(),
        titulo,
        autor,
        idioma,
        primera_frase,
        palabras: texto.split_whitespace().count(),
        texto,
    })
}

pub fn cuento_empaquetado(app: &AppHandle, id: &str) -> Option<Cuento> {
    let dir = dir_cuentos(app)?;
    let seguro: String = id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    let crudo = std::fs::read_to_string(dir.join(format!("{seguro}.md"))).ok()?;
    parsear_cuento(&seguro, &crudo)
}

/// Los cuentos empaquetados, en cualquier idioma (el paseo elige los del
/// idioma del usuario y, si no hay, los del inglés).
#[tauri::command]
pub fn cuentos_listar_cmd(app: AppHandle) -> Vec<Cuento> {
    let Some(dir) = dir_cuentos(&app) else {
        return Vec::new();
    };
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Cuento> = entradas
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("md") {
                return None;
            }
            let id = p.file_stem()?.to_string_lossy().to_string();
            let crudo = std::fs::read_to_string(&p).ok()?;
            parsear_cuento(&id, &crudo)
        })
        .collect();
    out.sort_by(|a, b| a.titulo.cmp(&b.titulo));
    out
}

// ─── EL PASEO: portapapeles, el loro en PiP y el usuario de la tienda ────

/// ¿Hay un enlace copiado? Sin leer el portapapeles (iOS no avisa).
#[tauri::command]
pub fn portapapeles_tiene_enlace_cmd() -> bool {
    #[cfg(target_os = "ios")]
    {
        crate::mobile::portapapeles_tiene_enlace()
    }
    #[cfg(not(target_os = "ios"))]
    {
        false
    }
}

/// El loro en PiP: un vídeo mudo empaquetado que sigue al usuario a Safari
/// mientras aprende a compartir. `x, y, ancho, alto` en puntos, donde la
/// página deja el hueco.
#[tauri::command]
pub fn pip_iniciar_cmd(
    app: AppHandle,
    x: f64,
    y: f64,
    ancho: f64,
    alto: f64,
) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    {
        let base = app.path().resource_dir().map_err(|e| e.to_string())?;
        let video = [
            base.join("_up_")
                .join("resources")
                .join("paseo")
                .join("loro-comparte.mp4"),
            base.join("resources")
                .join("paseo")
                .join("loro-comparte.mp4"),
            base.join("paseo").join("loro-comparte.mp4"),
        ]
        .into_iter()
        .find(|c| c.exists())
        .ok_or_else(|| "sin vídeo del loro".to_string())?;
        crate::mobile::pip_iniciar(&video.to_string_lossy(), x, y, ancho, alto);
        Ok(())
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, x, y, ancho, alto);
        Err("sin PiP en esta plataforma".into())
    }
}

#[tauri::command]
pub fn pip_parar_cmd() {
    #[cfg(target_os = "ios")]
    crate::mobile::pip_parar();
}

/// La presentación de cada voz, en el idioma preferido del usuario: los 31
/// idiomas que Yappy habla. Formas sin marca de género donde la lengua lo
/// pide (las voces son de ambos).
fn sample_for_voice(name: &str, lang: &str) -> String {
    let v = yappy_core::voices::by_name(name).unwrap_or_else(yappy_core::voices::default_voice);
    let n = &v.name;
    match lang {
        "es" => format!("Hola, soy {n}, y leeré todo lo que pongas delante de mí."),
        "fr" => format!("Bonjour, je suis {n}, et je lirai tout ce que vous me donnerez."),
        "de" => format!("Hallo, ich bin {n}, und ich lese gerne alles, was du mir gibst."),
        "it" => format!("Ciao, sono {n}, e leggerò ad alta voce qualsiasi cosa tu mi dia."),
        "pt" => format!("Olá, eu sou {n}, e vou ler tudo o que você me der."),
        "nl" => format!("Hallo, ik ben {n}, en ik lees alles voor wat je me geeft."),
        "pl" => format!("Cześć, jestem {n}. Przeczytam na głos wszystko, co mi dasz."),
        "ro" => format!("Bună, sunt {n}, și voi citi cu voce tare tot ce îmi dai."),
        "sv" => format!("Hej, jag är {n}, och jag läser gärna upp allt du ger mig."),
        "da" => format!("Hej, jeg er {n}, og jeg læser gerne alt højt for dig."),
        "fi" => format!("Hei, olen {n}. Luen ääneen kaiken, minkä annat minulle."),
        "et" => format!("Tere, mina olen {n}. Loen ette kõik, mille mulle annad."),
        "lt" => format!("Labas, aš esu {n}. Garsiai perskaitysiu viską, ką man duosi."),
        "lv" => format!("Sveiki, es esmu {n}. Es skaļi nolasīšu visu, ko man iedosi."),
        "hr" => format!("Bok, ja sam {n}. Naglas ću pročitati sve što mi daš."),
        "sl" => format!("Živjo, jaz sem {n}. Na glas preberem vse, kar mi daš."),
        "sk" => format!("Ahoj, som {n}. Nahlas prečítam všetko, čo mi dáš."),
        "cs" => format!("Ahoj, jsem {n}. Nahlas přečtu všechno, co mi dáš."),
        "hu" => format!("Szia, {n} vagyok. Felolvasok mindent, amit csak adsz."),
        "el" => format!("Γεια σου, με λένε {n}. Διαβάζω δυνατά ό,τι μου δώσεις."),
        "bg" => format!("Здравей, аз съм {n}. Ще прочета на глас всичко, което ми дадеш."),
        "uk" => format!("Привіт, я {n}. Прочитаю вголос усе, що ти мені даси."),
        "ru" => format!("Привет, я {n}. Прочитаю вслух всё, что ты мне дашь."),
        "tr" => format!("Merhaba, ben {n}. Bana verdiğin her şeyi sesli okurum."),
        "ar" => format!("مرحباً، أنا {n}. سأقرأ بصوت عالٍ كل ما تعطيني إياه."),
        "hi" => format!("नमस्ते, मैं {n} हूँ। आप जो भी देंगे, उसे ज़ोर से पढ़ने के लिए तैयार हूँ।"),
        "id" => format!("Halo, saya {n}. Saya akan membacakan apa pun yang kamu berikan."),
        "vi" => format!("Xin chào, tôi là {n}. Tôi sẽ đọc to mọi thứ bạn đưa cho tôi."),
        "ko" => format!("안녕하세요, 저는 {n}입니다. 무엇이든 소리내어 읽어드릴게요."),
        "ja" => format!("こんにちは、{n}です。 何でも声に出して読みます。"),
        _ => format!("Hi, I'm {n}. I'd love to read anything you put in front of me."),
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
pub fn save_current_audio_cmd(state: State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
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

pub async fn trigger_read_now<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) -> Result<()> {
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
                return read_text(
                    &app,
                    state,
                    capture.text,
                    capture.source.short_kind().to_string(),
                )
                .await;
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
    read_text(
        &app,
        state,
        capture.text,
        capture.source.short_kind().to_string(),
    )
    .await
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
#[allow(dead_code)] // camino de escritorio que hoy solo usa una plataforma
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
        ReadMode::Document {
            base_paragraph_index,
        },
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
    read_with_voice_lang_internal_with_mode(
        app,
        state,
        text,
        voice,
        forced_lang,
        source,
        ReadMode::MiniPlayer,
    )
    .await
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
    read_internal(
        app,
        state,
        text,
        None,
        voice,
        forced_lang,
        source,
        mode,
        SessionMeta::default(),
    )
    .await
}

/// Metadatos de la sesión que viajan hasta el snapshot: ruta y título del
/// documento (para la aguja y la pantalla de bloqueo) y si la sesión nace
/// en pausa (reposicionar sin sonar).
#[derive(Debug, Clone, Default)]
pub struct SessionMeta {
    pub doc_path: String,
    pub titulo: String,
    pub start_paused: bool,
}

/// ¿Ya hay colchón para ESTRENAR la sesión? Dos frases con texto (la que
/// suena y una entera por delante), o suficiente audio acumulado, o el final
/// del guion. Mientras no, los trozos se retienen y el estado sigue en
/// «preparando»: arrancar un pelín más tarde con margen suena mágico;
/// arrancar al instante y quedarse sin aire a mitad de frase, no.
pub fn colchon_listo(frases_con_texto: usize, segundos: f32, es_ultimo: bool) -> bool {
    es_ultimo || frases_con_texto >= 2 || segundos >= 4.0
}

/// El camino común de toda lectura. Si llega un Guion ya construido (el
/// editor de documentos, con sus clases de pieza y sus anulaciones por
/// párrafo), se sintetiza tal cual; si no, el texto plano pasa por el
/// guionizador dentro del motor.
#[allow(clippy::too_many_arguments)]
async fn read_internal<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    text: String,
    guion: Option<yappy_core::guion::Guion>,
    voice: String,
    forced_lang: Option<String>,
    source: String,
    mode: ReadMode,
    meta: SessionMeta,
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
    // Si un audiolibro estaba sonando, se cierra con su posición guardada:
    // una sola voz en la casa.
    crate::libro::parar(app, state.as_ref());
    state.playback.stop();
    // Claim a fresh id AFTER the stop bump so this synth task has a unique session.
    // Any older synth task that's still spinning will see `current_session()` change
    // and abort the next time it tries to emit a chunk.
    let session_id = state.playback.begin_session();

    let (opts, voice_overrides, save_history, history_max, mut successful_reads, vol) = {
        let s = state.settings.lock().unwrap();
        let chosen_voice = match forced_lang.as_deref() {
            Some(lang) => s
                .voice_overrides
                .get(lang)
                .cloned()
                .unwrap_or(voice.clone()),
            None => voice.clone(),
        };
        // La voz al azar por pieza: el NOMBRE del documento (estable entre
        // updates: los contenedores migran) elige el pájaro, siempre el
        // mismo para la misma pieza.
        let chosen_voice = if s.voz_al_azar {
            let clave: String = if meta.doc_path.is_empty() {
                text.chars().take(64).collect()
            } else {
                meta.doc_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&meta.doc_path)
                    .to_string()
            };
            let mut h: u64 = 0;
            for b in clave.bytes() {
                h = h.wrapping_mul(131).wrapping_add(b as u64);
            }
            yappy_core::VOICES[(h as usize) % yappy_core::VOICES.len()]
                .name
                .to_string()
        } else {
            chosen_voice
        };
        (
            SynthesisOptions {
                voice: chosen_voice,
                speed: s.speed,
                default_lang: forced_lang
                    .clone()
                    .unwrap_or_else(|| s.default_lang.clone()),
                total_steps: s.quality.total_steps(),
                seed: None,
                detectar_idioma: s.auto_lang_detect && forced_lang.is_none(),
                pausa_entre_parrafos_s: s.silence_secs.clamp(0.0, 5.0),
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
            // Desktop pops the floating player window. On iOS/Android there is no
            // separate window — a compact mini-player bar is rendered inside the
            // main window by the frontend (app layout) from `playback_state`.
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            let _ = windows::show_player(app);
        }
        ReadMode::Document { .. } => {
            // Document window is already visible; do NOT pop the mini-player.
        }
    }
    let base_paragraph_index = match &mode {
        ReadMode::Document {
            base_paragraph_index,
        } => *base_paragraph_index,
        ReadMode::MiniPlayer => 0,
    };
    // El título de la sesión: el del documento si llegó; si no, la primera
    // línea con chicha del texto. Lo enseñan la aguja, la pantalla de
    // bloqueo y el widget.
    let titulo_sesion: String = if !meta.titulo.trim().is_empty() {
        meta.titulo.trim().chars().take(70).collect()
    } else {
        text.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Yappy")
            .trim_start_matches('#')
            .trim()
            .chars()
            .take(70)
            .collect()
    };
    *state.titulo_actual.lock().unwrap() = titulo_sesion.clone();
    // La cocina se ve desde el primer milisegundo: el snapshot pasa a
    // «preparando» con su título y su ruta ANTES de que exista audio.
    state.playback.preparando(
        session_id,
        &titulo_sesion,
        &meta.doc_path,
        base_paragraph_index,
    );
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
    let guion_for_thread = guion;
    let source_for_history = source.clone();
    let preview_for_history: String = text.chars().take(180).collect();

    let arranque_pausado = meta.start_paused;
    tauri::async_runtime::spawn_blocking(move || {
        // La voz del que escucha MANDA: mientras este contador esté alto, la
        // imprenta vuelca su checkpoint y cede el motor, y las muestras
        // esperan. El guard garantiza el decremento pase lo que pase.
        struct Lector(Arc<AppState>);
        impl Drop for Lector {
            fn drop(&mut self) {
                self.0
                    .lectores
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        state_for_thread
            .lectores
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _lector = Lector(state_for_thread.clone());
        // La lectura corre con la CPU por delante de los trabajos de fondo.
        crate::subir_prioridad_de_hilo();

        use std::cell::{Cell, RefCell};
        let first_emitted = Cell::new(false);
        // EL COLCHÓN: no se empieza a sonar con una sola frase. Se retienen
        // los primeros trozos hasta tener dos frases con texto (o suficiente
        // audio, o el final del guion) y se estrena la sesión con TODO ese
        // material de golpe: aunque el móvil vaya lento y la síntesis sea
        // una a una, siempre hay margen mientras suena la primera frase.
        let pendientes: RefCell<Vec<AudioChunk>> = RefCell::new(Vec::new());
        let frases_colchon = Cell::new(0usize);
        let segundos_colchon = Cell::new(0f32);
        let detected_lang: RefCell<String> = RefCell::new(String::new());
        let opts_local = opts.clone();
        let my_session = session_id;

        // El candado se RETIENE durante toda la síntesis: nada de fondo
        // sintetiza a la vez robando la mitad de la CPU. (La imprenta lo
        // suelta en cuanto ve el contador de lectores; ver imprenta.rs.)
        let _motor = state_for_thread.candado_motor.lock().unwrap();

        // La síntesis va envuelta en catch_unwind: un PÁNICO en el motor (el
        // guionizador con un texto raro, un modelo corrupto) se saltaba el
        // Fallo y dejaba «preparando la voz» colgado para siempre. Y un fallo
        // ANTES del primer sonido se reintenta una vez: los tropiezos
        // transitorios no merecen un silencio.
        let mut res: Result<(), anyhow::Error> = Ok(());
        for intento in 0..2 {
            let al_chunk = |chunk: yappy_core::engine::AudioChunk| {
                // Cooperative cancel: if Stop fired (or a new read started), the
                // controller has bumped its session id. Bail out *before* spending
                // more time on this chunk's downstream work.
                if state_for_thread.playback.current_session() != my_session {
                    return Err(anyhow::anyhow!("session cancelled"));
                }
                *detected_lang.borrow_mut() = chunk.lang.clone();
                let sr = (chunk.sample_rate as u32).max(1);
                let ac = AudioChunk {
                    index: chunk.index,
                    paragraph_index: chunk.paragraph_index,
                    total: chunk.total,
                    total_paragraphs: chunk.total_paragraphs,
                    text: chunk.text.clone(),
                    origen_ini: chunk.origen_ini,
                    origen_fin: chunk.origen_fin,
                    samples: chunk.samples,
                    source_sample_rate: sr,
                };
                if !first_emitted.get() {
                    if !ac.text.trim().is_empty() {
                        frases_colchon.set(frases_colchon.get() + 1);
                    }
                    segundos_colchon
                        .set(segundos_colchon.get() + ac.samples.len() as f32 / sr as f32);
                    pendientes.borrow_mut().push(ac);
                    let es_ultimo = chunk.index + 1 >= chunk.total;
                    if colchon_listo(frases_colchon.get(), segundos_colchon.get(), es_ultimo) {
                        let listos = std::mem::take(&mut *pendientes.borrow_mut());
                        tracing::info!(
                            "colchón: estreno con {} trozos ({:.1}s de margen)",
                            listos.len(),
                            segundos_colchon.get()
                        );
                        state_for_thread
                            .playback
                            .new_session(my_session, listos, arranque_pausado);
                        first_emitted.set(true);
                    }
                } else {
                    state_for_thread.playback.enqueue(my_session, ac);
                }
                // Skip the UI event too if the session was just cancelled.
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
            };

            res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || match &guion_for_thread {
                    Some(g) => engine.synthesize_guion(g, &opts_local, al_chunk),
                    None => engine.synthesize_streaming(&text_for_thread, &opts_local, al_chunk),
                },
            ))
            .unwrap_or_else(|p| {
                let msg = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "pánico del motor de síntesis".into());
                Err(anyhow::anyhow!("pánico del motor: {msg}"))
            });

            let cancelado = matches!(&res, Err(e) if e.to_string().contains("session cancelled"));
            let sono_algo = first_emitted.get() || !pendientes.borrow().is_empty();
            if res.is_ok() || cancelado || sono_algo || intento == 1 {
                break;
            }
            tracing::warn!(
                "síntesis falló antes del primer sonido; reintentando una vez: {:?}",
                res.as_ref().err()
            );
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        let detected_lang = detected_lang.into_inner();

        // A cancellation is an expected outcome, not an error worth surfacing.
        let cancelled = matches!(&res, Err(e) if e.to_string().contains("session cancelled"));
        if let Err(e) = &res {
            if cancelled {
                tracing::info!("synth aborted by user (session {} stale)", my_session);
            } else {
                tracing::error!("synthesize_streaming: {e:?}");
                let _ = app_for_thread.emit("synth_error", e.to_string());
                // Si el colchón guardaba material, que SUENE lo que se pudo
                // cocinar antes del tropiezo; si no había nada, devolver el
                // snapshot de «preparando» a inactivo.
                let restos = std::mem::take(&mut *pendientes.borrow_mut());
                if !first_emitted.get() && !restos.is_empty() {
                    state_for_thread
                        .playback
                        .new_session(my_session, restos, arranque_pausado);
                    first_emitted.set(true);
                } else if !first_emitted.get() {
                    state_for_thread.playback.fallo(my_session);
                }
            }
        }
        // El guion entero cupo en el colchón sin disparar el estreno (no
        // debería pasar: el último trozo lo fuerza), o la síntesis terminó
        // limpia con material retenido: estrenar con lo que haya.
        if res.is_ok() && !first_emitted.get() {
            let restos = std::mem::take(&mut *pendientes.borrow_mut());
            if !restos.is_empty() {
                state_for_thread
                    .playback
                    .new_session(my_session, restos, arranque_pausado);
                first_emitted.set(true);
            }
        }

        // Record history + bump successful_reads.
        if res.is_ok() && save_history {
            let entry = history::HistoryEntry {
                id: format!(
                    "{}-{}",
                    history::now_unix(),
                    preview_for_history.chars().take(8).collect::<String>()
                ),
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

    Ok(())
}

// ═══════════════════════ SPEECH-TO-TEXT (ASR) ═══════════════════════════
//
// Transcribe arbitrary audio with Parakeet TDT. On desktop the ONNX engine in
// `yappy-core::asr` runs (CoreML/ANE on macOS, DirectML on Windows, XNNPACK/CUDA
// on Linux, all inherited from the global EP list in `lib.rs`). On iOS the work
// is done natively in Swift over the C-ABI in `mobile.rs`. The transcript is
// saved to `transcripts.rs` so it shows up in the in-app history.

use yappy_core::asr::{TranscribeOptions, TranscriptResult};

/// Cached, lazily-loaded desktop ASR engine. Loading the ~670 MB model takes a
/// moment, so we keep it resident across calls (mirrors the TTS engine cache in
/// `AppState`). Desktop-only; iOS holds its CoreML model in Swift.
mod asr_engine_cache {
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use yappy_core::asr::ParakeetAsr;

    static ENGINE: OnceLock<Mutex<Option<ParakeetAsr>>> = OnceLock::new();

    pub fn with_engine<R>(
        dir: &Path,
        f: impl FnOnce(&mut ParakeetAsr) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let slot = ENGINE.get_or_init(|| Mutex::new(None));
        let mut guard = slot.lock().unwrap();
        if guard.is_none() {
            *guard = Some(ParakeetAsr::from_dir(dir)?);
        }
        f(guard.as_mut().expect("engine just loaded"))
    }
}

#[tauri::command]
pub fn is_asr_model_ready(app: AppHandle) -> Result<bool, String> {
    // ONNX model, every platform — the main app's engine is `yappy-core::asr`
    // (ORT). The Share Extension's separate CoreML model is managed in Swift.
    asr_model::is_asr_model_ready(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_asr_model_cmd(app: AppHandle) -> Result<(), String> {
    let h = app.clone();
    asr_model::download_asr_model(&app, move |p| {
        let _ = h.emit("asr_model_download", &p);
    })
    .await
    .map_err(|e| e.to_string())
}

/// Transcribe an audio file at `path`. `source` labels where it came from
/// ("File" / "Shared"), `options` controls language hint + timestamp mode.
#[tauri::command]
pub async fn transcribe_audio_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
    options: Option<TranscribeOptions>,
    source: Option<String>,
) -> Result<TranscriptResult, String> {
    let opts = options.unwrap_or_default();
    // Shared/opened audio can arrive as a percent-encoded `file://` URL — normalize.
    let path = normalize_local_path(&path).to_string_lossy().into_owned();
    let (save, history_max) = {
        let s = state.settings.lock().unwrap();
        (s.save_history, s.history_max)
    };
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    let ready = is_asr_model_ready(app.clone()).unwrap_or(false);
    if !ready {
        let _ = app.emit("asr_model_missing", true);
        return Err("transcription model not downloaded".into());
    }

    let _ = app.emit("transcribe_progress", "transcribing");
    let result = run_transcription(&app, &path, &opts).await?;

    if save && !result.text.trim().is_empty() {
        let entry = transcripts::Transcript {
            id: format!(
                "{}-{}",
                transcripts::now_unix(),
                uuid::Uuid::new_v4()
                    .simple()
                    .to_string()
                    .chars()
                    .take(8)
                    .collect::<String>()
            ),
            created_at: transcripts::now_unix(),
            source: source.unwrap_or_else(|| "File".into()),
            filename,
            duration_secs: result.audio_secs,
            language: result.language.clone(),
            text: result.text.clone(),
        };
        let _ = transcripts::append(&app, entry, history_max.max(50));
    }
    let _ = app.emit("transcribe_progress", "done");
    Ok(result)
}

/// A short bundled speech clip ("The quick brown fox… Yappy transcription is
/// working.") embedded in the binary so the user (and our tests) can verify
/// transcription without supplying a file — works identically on every platform
/// incl. the iOS Simulator.
const SAMPLE_WAV: &[u8] = include_bytes!("../../resources/sample-speech.wav");

/// A bundled sample **Markdown** document ("The Lighthouse Keeper") so the
/// document reader can be tried without picking a file. Markdown (rather than the
/// flat PDF) is deliberate: it preserves real structure — headings at multiple
/// levels, a blockquote, a list, a horizontal rule — so the reader exercises
/// chapters, section rhythm, and per-section/per-paragraph controls end to end.
const SAMPLE_DOC: &[u8] = include_bytes!("../../resources/sample-doc.md");

#[tauri::command]
pub fn sample_document_path_cmd() -> Result<String, String> {
    let path = std::env::temp_dir().join("the-lighthouse-keeper.md");
    std::fs::write(&path, SAMPLE_DOC).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Transcribe the bundled sample clip. Reuses the normal transcribe flow.
#[tauri::command]
pub async fn transcribe_sample_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    options: Option<TranscribeOptions>,
) -> Result<TranscriptResult, String> {
    let path = std::env::temp_dir().join("yappy-sample-speech.wav");
    std::fs::write(&path, SAMPLE_WAV).map_err(|e| e.to_string())?;
    transcribe_audio_cmd(
        app,
        state,
        path.to_string_lossy().into_owned(),
        options,
        Some("Sample".into()),
    )
    .await
}

/// Transcribe via the ONNX Parakeet engine (`yappy-core::asr`) on EVERY platform.
/// ORT uses the global EP chain: CoreML on Apple devices, XNNPACK/CPU otherwise
/// (incl. the iOS Simulator) — this is the CoreML→ONNX/CPU fallback. The Share
/// Extension is the only place that uses native CoreML (FluidAudio) directly.
async fn run_transcription(
    app: &AppHandle,
    path: &str,
    opts: &TranscribeOptions,
) -> Result<TranscriptResult, String> {
    let root = asr_model::asr_model_root(app).map_err(|e| e.to_string())?;
    let path = path.to_string();
    let opts = opts.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<TranscriptResult> {
        let samples = crate::asr_decode::decode_to_mono16k(std::path::Path::new(&path))?;
        asr_engine_cache::with_engine(&root, |eng| {
            use yappy_core::asr::Transcriber;
            eng.transcribe_mono16k(&samples, &opts)
        })
    })
    .await
    .map_err(|e| format!("transcription task panicked: {e}"))?
    .map_err(|e| e.to_string())
}

/// Transcripción al servicio de la cola: mismo motor que la pestaña de
/// transcribir, devolviendo solo el texto. En iOS exige el modelo ya
/// descargado (la cola enseña el error con claridad si falta).
pub async fn transcribir_para_cola<R: Runtime>(
    app: &AppHandle<R>,
    ruta: &str,
) -> anyhow::Result<String> {
    if !asr_model::is_asr_model_ready(app)? {
        anyhow::bail!("falta el modelo de transcripción (descárgalo en ajustes)");
    }
    let root = asr_model::asr_model_root(app)?;
    let ruta = ruta.to_string();
    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let samples = crate::asr_decode::decode_to_mono16k(std::path::Path::new(&ruta))?;
        let opts = yappy_core::asr::TranscribeOptions::default();
        let out = asr_engine_cache::with_engine(&root, |eng| {
            use yappy_core::asr::Transcriber;
            eng.transcribe_mono16k(&samples, &opts)
        })?;
        Ok(out.text)
    })
    .await??;
    Ok(res)
}

/// Persist a transcript that was produced elsewhere (e.g. the iOS Share
/// Extension transcribed in-place and handed Yappy the finished text). Returns
/// the stored entry so the UI can surface it immediately.
#[tauri::command]
pub fn save_transcript_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    text: String,
    source: Option<String>,
    filename: Option<String>,
    language: Option<String>,
    duration_secs: Option<f32>,
) -> Result<transcripts::Transcript, String> {
    let history_max = { state.settings.lock().unwrap().history_max };
    let entry = transcripts::Transcript {
        id: format!(
            "{}-{}",
            transcripts::now_unix(),
            uuid::Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        ),
        created_at: transcripts::now_unix(),
        source: source.unwrap_or_else(|| "Shared".into()),
        filename,
        duration_secs: duration_secs.unwrap_or(0.0),
        language,
        text,
    };
    transcripts::append(&app, entry.clone(), history_max.max(50)).map_err(|e| e.to_string())?;
    Ok(entry)
}

/// Result of the audio round-trip self-test.
#[derive(Debug, Serialize)]
pub struct AudioSelfTest {
    pub ok: bool,
    pub rms: f32,
    pub peak: f32,
    pub synth_secs: f32,
    pub heard: String,
}

/// Creative audio verification that doesn't need speakers: synthesize a known
/// phrase with the TTS engine, measure the signal (RMS/peak — silence would be
/// ~0), then feed that exact audio back into the Parakeet STT engine. If it
/// transcribes back to the phrase, the synthesized audio is provably real,
/// intelligible speech (i.e. the audio pipeline genuinely produces sound).
#[tauri::command]
pub async fn audio_selftest_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<AudioSelfTest, String> {
    let st = state.inner().clone();
    let model_root = model::model_root(&app).map_err(|e| e.to_string())?;
    let asr_root = asr_model::asr_model_root(&app).map_err(|e| e.to_string())?;
    let voice = { st.settings.lock().unwrap().voice.clone() };
    tokio::task::spawn_blocking(move || -> anyhow::Result<AudioSelfTest> {
        const PHRASE: &str = "The quick brown fox jumps over the lazy dog.";
        // 1) TTS synth → raw samples.
        let engine = st.engine_or_load(&model_root)?;
        let opts = SynthesisOptions {
            voice,
            ..Default::default()
        };
        let chunks = engine.synthesize(PHRASE, &opts)?;
        let sr = engine.sample_rate();
        let mut samples: Vec<f32> = Vec::new();
        for c in &chunks {
            samples.extend_from_slice(&c.samples);
        }
        let n = samples.len().max(1) as f32;
        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / n).sqrt();
        let peak = samples.iter().fold(0f32, |a, &s| a.max(s.abs()));
        let synth_secs = samples.len() as f32 / sr as f32;
        // 2) Resample to 16 kHz and transcribe the synthesized audio back.
        let mono16 = crate::playback::resample_mono(&samples, sr as u32, 16_000)?;
        let heard = asr_engine_cache::with_engine(&asr_root, |eng| {
            use yappy_core::asr::Transcriber;
            Ok(eng
                .transcribe_mono16k(&mono16, &yappy_core::asr::TranscribeOptions::default())?
                .text)
        })
        .unwrap_or_default();
        Ok(AudioSelfTest {
            ok: rms > 0.003 && !heard.trim().is_empty(),
            rms,
            peak,
            synth_secs,
            heard,
        })
    })
    .await
    .map_err(|e| format!("audio self-test panicked: {e}"))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_transcripts(app: AppHandle) -> Result<transcripts::Transcripts, String> {
    transcripts::load(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_transcripts_cmd(app: AppHandle) -> Result<(), String> {
    transcripts::clear(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_transcript_cmd(app: AppHandle, id: String) -> Result<(), String> {
    transcripts::delete(&app, &id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests_colchon {
    use super::colchon_listo;

    #[test]
    fn el_colchon_espera_dos_frases_pero_no_para_siempre() {
        // Una sola frase corta: aún no.
        assert!(!colchon_listo(1, 2.1, false));
        // Dos frases con texto: a sonar.
        assert!(colchon_listo(2, 3.0, false));
        // Una frase larguísima ya da margen de sobra.
        assert!(colchon_listo(1, 4.5, false));
        // El final del guion estrena con lo que haya (texto de una frase).
        assert!(colchon_listo(1, 0.8, true));
        assert!(colchon_listo(0, 0.0, true));
        // Solo silencios de párrafo acumulados: seguir esperando.
        assert!(!colchon_listo(0, 1.5, false));
    }
}
