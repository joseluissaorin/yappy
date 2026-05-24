use anyhow::Result;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

pub fn setup_tray(handle: &tauri::AppHandle) -> Result<()> {
    let open_yappy = MenuItem::with_id(handle, "open_main", "Open Yappy…", true, None::<&str>)?;
    let read_now = MenuItem::with_id(handle, "read_now", "Read what I'm looking at", true, Some("Alt+Cmd+R"))?;
    let read_clipboard = MenuItem::with_id(handle, "read_clipboard", "Read clipboard", true, None::<&str>)?;
    let pause = MenuItem::with_id(handle, "pause", "Pause / Resume", true, Some("Alt+Cmd+Space"))?;
    let stop = MenuItem::with_id(handle, "stop", "Stop", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(handle)?;
    let voices = MenuItem::with_id(handle, "open_voices", "Voices…", true, None::<&str>)?;
    let prefs = MenuItem::with_id(handle, "open_prefs", "Preferences…", true, None::<&str>)?;
    let history = MenuItem::with_id(handle, "open_history", "History…", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(handle)?;
    let about = MenuItem::with_id(handle, "open_about", "About Yappy…", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "quit", "Quit Yappy", true, None::<&str>)?;
    let menu = Menu::with_items(
        handle,
        &[&open_yappy, &read_now, &read_clipboard, &pause, &stop, &sep1, &voices, &prefs, &history, &sep2, &about, &quit],
    )?;

    // Dedicated monochrome silhouette for the menu bar. Bundled at compile-time so it
    // survives moving the .app around. macOS treats it as a template (black-on-transparent
    // adapts to dark/light menu bar automatically).
    let tray_icon = Image::from_bytes(include_bytes!("../icons/yappy-tray-template@2x.png"))?;

    let _tray = TrayIconBuilder::with_id("yappy-tray")
        .tooltip("Yappy")
        .menu(&menu)
        .icon(tray_icon)
        .icon_as_template(true)
        .on_menu_event(|app, ev| match ev.id.as_ref() {
            "open_main" => show_main(app),
            "read_now" => {
                let h = app.clone();
                let state = app.state::<std::sync::Arc<crate::state::AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::commands::trigger_read_now(h, state).await {
                        tracing::error!("tray read_now: {e:?}");
                    }
                });
            }
            "pause" => {
                let h = app.clone();
                let state = app.state::<std::sync::Arc<crate::state::AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::commands::toggle_pause(h, state).await;
                });
            }
            "stop" => {
                let state = app.state::<std::sync::Arc<crate::state::AppState>>().inner().clone();
                state.playback.stop();
            }
            "read_clipboard" => {
                let h = app.clone();
                let state = app.state::<std::sync::Arc<crate::state::AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(Some(text)) = crate::capture::clipboard::read_text() {
                        if !text.trim().is_empty() {
                            let _ = crate::commands::read_text(&h, state, text, "clipboard".into()).await;
                        }
                    }
                });
            }
            "open_voices" => {
                show_main(app);
                let _ = app.emit("nav", "voices");
            }
            "open_prefs" => {
                show_main(app);
                let _ = app.emit("nav", "preferences");
            }
            "open_history" => {
                show_main(app);
                let _ = app.emit("nav", "history");
            }
            "open_about" => {
                show_main(app);
                let _ = app.emit("nav", "about");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                let app = tray.app_handle();
                show_main(app);
            }
        })
        .build(handle)?;
    Ok(())
}

fn show_main(handle: &tauri::AppHandle) {
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
