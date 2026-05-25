use anyhow::Result;
use tauri::{LogicalPosition, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

use crate::settings::PlayerPositionPreset;

pub fn position_player_bottom_right<R: Runtime>(player: &WebviewWindow<R>) -> Result<()> {
    position_player_with_preset(player, PlayerPositionPreset::BottomRight)
}

pub fn position_player_with_preset<R: Runtime>(
    player: &WebviewWindow<R>,
    preset: PlayerPositionPreset,
) -> Result<()> {
    let Some(monitor) = player.current_monitor()? else { return Ok(()); };
    let size = monitor.size();
    let pos = monitor.position();
    let scale = monitor.scale_factor();
    let ps = player.outer_size().unwrap_or(PhysicalSize { width: 380, height: 94 });
    let margin = (16.0 * scale) as i32;
    let top_margin = (40.0 * scale) as i32;
    let x = match preset {
        PlayerPositionPreset::TopLeft
        | PlayerPositionPreset::BottomLeft => pos.x + margin,
        PlayerPositionPreset::TopCenter
        | PlayerPositionPreset::BottomCenter => pos.x + (size.width as i32 - ps.width as i32) / 2,
        PlayerPositionPreset::TopRight
        | PlayerPositionPreset::BottomRight => pos.x + (size.width as i32 - ps.width as i32) - margin,
        PlayerPositionPreset::Custom => return Ok(()),
    };
    let y = match preset {
        PlayerPositionPreset::TopLeft
        | PlayerPositionPreset::TopCenter
        | PlayerPositionPreset::TopRight => pos.y + top_margin,
        PlayerPositionPreset::BottomLeft
        | PlayerPositionPreset::BottomCenter
        | PlayerPositionPreset::BottomRight => pos.y + (size.height as i32 - ps.height as i32) - margin - top_margin,
        PlayerPositionPreset::Custom => return Ok(()),
    };
    player.set_position(PhysicalPosition::new(x, y))?;
    Ok(())
}

pub fn resize_player_for_size<R: Runtime>(player: &WebviewWindow<R>, size: &str) -> Result<()> {
    let scale = player.current_monitor()?.map(|m| m.scale_factor()).unwrap_or(1.0);
    let (w, h) = match size {
        "slim"    => (320.0, 78.0),
        "large"   => (520.0, 130.0),
        _         => (380.0, 94.0),
    };
    let _ = player.set_size(PhysicalSize::new((w * scale) as u32, (h * scale) as u32));
    Ok(())
}

pub fn show_player<R: Runtime>(handle: &tauri::AppHandle<R>) -> Result<()> {
    if let Some(w) = handle.get_webview_window("player") {
        // Do NOT reposition here — the user's saved position / preset was applied at startup,
        // and they may have moved the player by hand since. show_player just makes it visible.
        w.show()?;
    }
    Ok(())
}

pub fn hide_player<R: Runtime>(handle: &tauri::AppHandle<R>) -> Result<()> {
    if let Some(w) = handle.get_webview_window("player") {
        w.hide()?;
    }
    Ok(())
}

pub fn show_main<R: Runtime>(handle: &tauri::AppHandle<R>) -> Result<()> {
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

pub fn show_document<R: Runtime>(handle: &tauri::AppHandle<R>, label: &str) -> Result<()> {
    match handle.get_webview_window(label) {
        Some(w) => {
            tracing::info!("[doc:win] show_document({label}): window exists, calling show()+set_focus()");
            let _ = w.show();
            // `unminimize` only exists on desktop builds of Tauri (UIKit windows
            // can't be minimized — they're either visible or backgrounded by
            // the OS, never iconified).
            #[cfg(desktop)]
            let _ = w.unminimize();
            let _ = w.set_focus();
            Ok(())
        }
        None => {
            tracing::info!("[doc:win] show_document({label}): creating fresh window");
            create_document_window(handle, label, None)?;
            Ok(())
        }
    }
}

/// Create a new document window with a unique label. If `title` is provided,
/// it's used as the OS title bar text; otherwise "Yappy — Document".
pub fn create_document_window<R: Runtime>(
    handle: &tauri::AppHandle<R>,
    label: &str,
    title: Option<&str>,
) -> Result<()> {
    use tauri::WebviewUrl;
    let url = WebviewUrl::App("/document".into());
    let title_str = title.unwrap_or("Yappy — Document");
    let builder = tauri::WebviewWindowBuilder::new(handle, label, url)
        .title(title_str)
        .inner_size(900.0, 720.0)
        .min_inner_size(720.0, 540.0)
        .resizable(true)
        .visible(true)
        .background_color(tauri::webview::Color(0xff, 0xf8, 0xd7, 0xff));
    // Window-chrome builders (decorations/center/hidden_title/title_bar_style
    // /shadow) only exist on desktop platforms — iOS webview windows are
    // full-screen and have no chrome to configure.
    #[cfg(desktop)]
    let builder = builder.decorations(true).center().shadow(true);
    #[cfg(target_os = "macos")]
    let builder = builder
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay);
    builder
        .build()
        .map_err(|e| anyhow::anyhow!("create document window '{label}': {e:?}"))?;
    tracing::info!("[doc:win] create_document_window({label}) OK");
    Ok(())
}

/// Generate a fresh window label, monotonically increasing.
pub fn next_document_label() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("document-{ts}-{id}")
}

#[allow(dead_code)]
pub fn _unused() -> LogicalPosition<f64> {
    LogicalPosition::new(0.0, 0.0)
}
