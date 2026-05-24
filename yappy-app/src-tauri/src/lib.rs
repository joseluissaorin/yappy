// Yappy — local TTS that reads anything on your screen.

mod bridge;
mod capture;
mod commands;
mod credits;
mod history;
mod hotkey;
mod model;
mod playback;
mod settings;
mod state;
mod tray;
mod windows;

use std::sync::Arc;

use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // File logger.
    let log_path = dirs::data_dir()
        .map(|d| d.join("com.yappy.app").join("yappy.log"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/yappy.log"));
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();
    let writer = match log_file {
        Some(f) => tracing_subscriber::fmt::writer::BoxMakeWriter::new(std::sync::Mutex::new(f)),
        None => tracing_subscriber::fmt::writer::BoxMakeWriter::new(std::io::stderr),
    };
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,yappy=debug,yappy_app=debug,yappy_app_lib=debug,ort=warn,ort::logging=off")
            }),
        )
        .init();
    tracing::info!("yappy starting — log at {}", log_path.display());

    // smoke-play synthesis test
    if std::env::args().any(|a| a == "--smoke-play") {
        let model_root = dirs::data_dir()
            .map(|d| d.join("com.yappy.app/models/supertonic-3"))
            .unwrap();
        let cfg = yappy_core::engine::engine_config(&model_root);
        let engine = yappy_core::TtsEngine::new(cfg).expect("engine load");
        let pb = playback::PlaybackController::new();
        let opts = yappy_core::SynthesisOptions {
            voice: "Jessica".to_string(),
            speed: 1.05,
            default_lang: "en".to_string(),
            total_steps: 8,
            seed: Some(7),
        };
        let mut first = true;
        let sid = pb.begin_session();
        engine
            .synthesize_streaming("Yappy backend, playback and all, says hello.", &opts, |chunk| {
                let ac = playback::AudioChunk {
                    index: chunk.index,
                    paragraph_index: chunk.paragraph_index,
                    total: chunk.total,
                    total_paragraphs: chunk.total_paragraphs,
                    text: chunk.text.clone(),
                    samples: chunk.samples.clone(),
                    source_sample_rate: chunk.sample_rate as u32,
                };
                if first { pb.new_session(sid, vec![ac]); first = false; } else { pb.enqueue(sid, ac); }
                Ok(())
            })
            .expect("synthesize");
        let start = std::time::Instant::now();
        loop {
            let s = pb.snapshot();
            if !s.playing && s.duration_secs > 0.0 { break; }
            if start.elapsed().as_secs() > 30 { break; }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        std::process::exit(0);
    }
    if std::env::args().any(|a| a == "--smoke-test") {
        let model_root = dirs::data_dir()
            .map(|d| d.join("com.yappy.app/models/supertonic-3"))
            .unwrap();
        let cfg = yappy_core::engine::engine_config(&model_root);
        let engine = yappy_core::TtsEngine::new(cfg).expect("engine load");
        let opts = yappy_core::SynthesisOptions {
            voice: "Jessica".to_string(),
            speed: 1.05,
            default_lang: "en".to_string(),
            total_steps: 8,
            seed: Some(42),
        };
        let mut all = Vec::new();
        engine
            .synthesize_streaming("Yappy is now alive.", &opts, |chunk| {
                all.extend(chunk.samples);
                Ok(())
            })
            .expect("synth");
        let out = std::env::temp_dir().join("yappy_smoke.wav");
        yappy_core::supertonic::write_wav(&out, &all, engine.sample_rate()).expect("write wav");
        std::process::exit(0);
    }

    let state = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state.clone())
        .setup(move |app| {
            if let Err(e) = settings::SettingsStore::ensure(app.handle(), &state) {
                tracing::error!("settings init: {e:?}");
            }

            tray::setup_tray(app.handle())?;

            // Start the local WebSocket bridge for the browser extension.
            if state.settings.lock().unwrap().bridge_enabled {
                bridge::start(app.handle().clone(), state.clone());
            }

            // Initial hotkey registration from settings.
            if let Err(e) = hotkey::register_from_settings(app.handle(), &state) {
                tracing::error!("hotkey init: {e:?}");
            }

            // Position the player: custom point > preset > default bottom-right.
            if let Some(player) = app.get_webview_window("player") {
                let s = state.settings.lock().unwrap().clone();
                // Resize first so the preset math has the right outer size.
                let _ = windows::resize_player_for_size(&player, &s.player_size);
                if let Some((x, y)) = s.player_position {
                    let _ = player.set_position(tauri::PhysicalPosition::new(x, y));
                } else {
                    let _ = windows::position_player_with_preset(&player, s.player_position_preset);
                }
            }

            if settings::is_first_launch(app.handle()).unwrap_or(true) {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let label = window.label();
                    // Main: prevent close + hide (app stays alive in the tray).
                    // Document-* windows: allow close — multi-window editor; user
                    // explicitly wants this specific file editor gone. We clean
                    // up state.documents in the Destroyed handler below.
                    if label == "main" {
                        api.prevent_close();
                        let _ = window.hide();
                        tracing::info!("[lifecycle] intercepted close on '{label}' → hide()");
                    } else if label.starts_with("document") {
                        tracing::info!("[lifecycle] document window '{label}' closing → allow destroy");
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    let label = window.label().to_string();
                    if label.starts_with("document") {
                        // Clean up the per-window document state so we don't leak.
                        if let Some(state) = window.try_state::<std::sync::Arc<crate::state::AppState>>() {
                            let removed = state.documents.lock().unwrap().remove(&label).is_some();
                            tracing::info!(
                                "[lifecycle] document window '{label}' destroyed — state cleanup (removed={removed})"
                            );
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_voices,
            commands::get_settings,
            commands::set_settings,
            commands::trigger_read_now_cmd,
            commands::stop_playback_cmd,
            commands::toggle_pause_cmd,
            commands::set_speed_cmd,
            commands::set_voice_cmd,
            commands::set_voice_override_cmd,
            commands::set_default_lang_cmd,
            commands::set_quality_cmd,
            commands::set_volume_cmd,
            commands::set_silence_cmd,
            commands::set_hotkey_cmd,
            commands::set_player_position_cmd,
            commands::skip_cmd,
            commands::synthesize_text,
            commands::read_clipboard_cmd,
            commands::read_file_cmd,
            commands::download_model_cmd,
            commands::is_model_ready,
            commands::open_main_window,
            commands::open_player_window,
            commands::request_macos_permissions,
            commands::sample_voice,
            commands::capture_diagnostics,
            commands::get_history,
            commands::clear_history_cmd,
            commands::replay_history_cmd,
            commands::save_current_audio_cmd,
            commands::list_credits,
            commands::list_licenses,
            commands::set_player_preset_cmd,
            commands::set_player_theme_cmd,
            commands::set_app_theme_cmd,
            commands::set_player_size_cmd,
            commands::set_ocr_engine_cmd,
            commands::reset_settings_cmd,
            commands::export_settings_cmd,
            commands::import_settings_cmd,
            commands::bridge_status,
            commands::bridge_regenerate_token_cmd,
            commands::bridge_clear_pairing_cmd,
            commands::set_bridge_enabled_cmd,
            commands::open_browser_extensions_cmd,
            commands::get_extension_path_cmd,
            commands::reveal_extension_folder_cmd,
            commands::reveal_log_file_cmd,
            commands::get_log_path_cmd,
            commands::tail_log_cmd,
            commands::log_frontend_cmd,
            commands::set_launch_at_login_cmd,
            commands::read_document_paragraphs_cmd,
            commands::get_current_document_cmd,
            commands::document_window_ready_cmd,
            commands::clear_current_document_cmd,
            commands::save_project_cmd,
            commands::load_project_cmd,
            commands::render_audiobook_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
