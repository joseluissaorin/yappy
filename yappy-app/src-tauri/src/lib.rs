// Yappy — local TTS that reads anything on your screen.

// All modules compile on both desktop and mobile. Each module that wraps
// desktop-only APIs (tray, hotkey, bridge, browser-extension) provides
// no-op / Err-returning stubs on mobile so the public surface stays uniform
// and `commands::*` keeps calling the same symbols on every platform.
pub mod audiobook;
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

// Mobile-only helpers — Share-extension payload pickup, UIPasteboard wrapper.
#[cfg(mobile)]
mod mobile;

// Windows-native helpers — SMTC (system media transport controls) +
// taskbar progress + Jump List. Stubbed to no-ops on non-Windows targets
// so cross-platform call sites don't need cfg guards.
mod os_win;

use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ─── Platform-specific startup environment ─────────────────────────────

    // Linux: GTK4 + WebKit2GTK 2.42+ default to the DMABUF renderer, which
    // is broken on many configurations (Intel UHD, Nvidia proprietary with
    // GBM, Mesa < 23.x). Yappy's webview shows up as a black rectangle in
    // that case. The official upstream workaround is to disable DMABUF and
    // fall back to the GLES renderer, which is reliable across hardware.
    // We set this BEFORE any GTK/Webkit init.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Compositors on Wayland don't expose a synchronous keystroke-grab
        // API — tauri-plugin-global-shortcut silently no-ops. Set a flag the
        // settings UI can read to surface a helpful explanation.
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            std::env::set_var("YAPPY_HOTKEYS_UNSUPPORTED", "wayland");
        }
    }

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

    // Register hardware-acceleration execution providers at the ONNX Runtime
    // ENVIRONMENT level. Every session created later (Supertonic synthesis +
    // paddle-ocr-rs's internal sessions) inherits the EP list unless they
    // explicitly override it. Crucially this accelerates paddle-ocr-rs even
    // though that crate doesn't expose an EP knob of its own.
    //
    // ORT tries each EP in order and falls back to the next (and ultimately
    // CPU) if initialization fails — e.g. no GPU available, missing CUDA
    // runtime, older OS. So we hand it a long preference list per platform
    // and let runtime sort out what actually works on the user's machine.
    {
        #[allow(unused_mut)]
        let mut eps: Vec<ort::execution_providers::ExecutionProviderDispatch> = Vec::new();
        let mut requested: Vec<&'static str> = Vec::new();

        // ─ Apple platforms: CoreML (Neural Engine + Apple GPU + CPU).
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            use ort::execution_providers::coreml::{
                CoreMLComputeUnits, CoreMLExecutionProvider, CoreMLModelFormat,
            };
            eps.push(
                CoreMLExecutionProvider::default()
                    .with_compute_units(CoreMLComputeUnits::All)
                    .with_model_format(CoreMLModelFormat::MLProgram)
                    .build(),
            );
            requested.push("CoreML");
        }

        // ─ Windows: DirectML routes to any DX12 GPU (NVIDIA / AMD / Intel /
        //   Qualcomm). Free on Windows 10 1903+; no extra install needed.
        #[cfg(target_os = "windows")]
        {
            use ort::execution_providers::directml::DirectMLExecutionProvider;
            eps.push(DirectMLExecutionProvider::default().build());
            requested.push("DirectML");
        }

        // ─ NVIDIA CUDA (Linux + Windows, opt-in via the `cuda` Cargo
        //   feature). Requires CUDA Toolkit + cuDNN to be present at build
        //   AND runtime — falls back to CPU silently otherwise.
        #[cfg(all(feature = "cuda", any(target_os = "linux", target_os = "windows")))]
        {
            use ort::execution_providers::cuda::CUDAExecutionProvider;
            eps.push(CUDAExecutionProvider::default().build());
            requested.push("CUDA");
        }

        // ─ NVIDIA TensorRT — even faster than raw CUDA when supported.
        //   Opt-in via the `tensorrt` Cargo feature.
        #[cfg(all(feature = "tensorrt", any(target_os = "linux", target_os = "windows")))]
        {
            use ort::execution_providers::tensorrt::TensorRTExecutionProvider;
            eps.push(TensorRTExecutionProvider::default().build());
            requested.push("TensorRT");
        }

        // ─ AMD ROCm (Linux only, opt-in via `rocm` Cargo feature).
        #[cfg(all(feature = "rocm", target_os = "linux"))]
        {
            use ort::execution_providers::rocm::ROCmExecutionProvider;
            eps.push(ROCmExecutionProvider::default().build());
            requested.push("ROCm");
        }

        // ─ Intel OpenVINO (CPU + iGPU + dGPU on Intel hardware). Opt-in.
        #[cfg(all(feature = "openvino", any(target_os = "linux", target_os = "windows")))]
        {
            use ort::execution_providers::openvino::OpenVINOExecutionProvider;
            eps.push(OpenVINOExecutionProvider::default().build());
            requested.push("OpenVINO");
        }

        // ─ XNNPACK: optimized CPU kernels for ARM + x86. Cross-platform,
        //   small. Sits at the end of the list as a CPU acceleration above
        //   ORT's default CPU EP.
        {
            use ort::execution_providers::xnnpack::XNNPACKExecutionProvider;
            eps.push(XNNPACKExecutionProvider::default().build());
            requested.push("XNNPACK");
        }

        match ort::init().with_execution_providers(eps).commit() {
            Ok(_) => tracing::info!(
                "ort: registered execution providers (priority order): {}",
                requested.join(" → ")
            ),
            Err(e) => tracing::warn!(
                "ort: EP registration failed, falling back to CPU-only: {e}"
            ),
        }
    }

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

    let mut builder = tauri::Builder::default();
    // Windows: auto-update from GitHub Releases. The Tauri updater plugin
    // polls the configured release feed, downloads signed installers, and
    // (after user consent) installs in place. macOS users currently get
    // updates via DMG re-download; Windows users get this richer flow.
    #[cfg(target_os = "windows")]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    // Windows: single-instance plugin. When the user double-clicks a .epub
    // associated with Yappy (or runs the binary a second time for any
    // reason), the existing main window gets focused + the file path is
    // forwarded to it instead of spawning a duplicate process.
    #[cfg(target_os = "windows")]
    {
        builder = builder.plugin(
            tauri_plugin_single_instance::init(|app, argv, _cwd| {
                tracing::info!("single-instance: re-launch argv = {:?}", argv);
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.unminimize();
                }
                // Forward file argument (if any) to the frontend via event.
                if let Some(path) = argv.iter().skip(1).find(|a| {
                    let p = std::path::Path::new(a);
                    p.exists() && p.is_file()
                }) {
                    let _ = app.emit("file_open_request", path);
                }
            }),
        );
    }
    // Desktop-only plugins. iOS has no autostart concept and no global
    // hotkeys; loading these plugins on mobile would compile-error in some
    // cases and be inert in others.
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec![]),
            ))
            .plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }
    builder
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
        .manage(state.clone())
        .setup(move |app| {
            if let Err(e) = settings::SettingsStore::ensure(app.handle(), &state) {
                tracing::error!("settings init: {e:?}");
            }

            // Tray, bridge, hotkey: stubbed to no-ops on mobile (see each module).
            tray::setup_tray(app.handle())?;
            if state.settings.lock().unwrap().bridge_enabled {
                bridge::start(app.handle().clone(), state.clone());
            }
            if let Err(e) = hotkey::register_from_settings(app.handle(), &state) {
                tracing::error!("hotkey init: {e:?}");
            }

            // ─── Windows: SMTC (lock-screen-like media controls) ──────────
            // Hook the main window's HWND into the System Media Transport
            // Controls so media keys + Bluetooth headphone buttons + the
            // volume-flyout playback widget drive Yappy's playback.
            #[cfg(target_os = "windows")]
            if let Some(main) = app.get_webview_window("main") {
                if let Ok(hwnd) = main.hwnd() {
                    os_win::install_smtc_handlers(state.playback.clone(), hwnd.0 as isize);
                    // Mirror playback snapshots into SMTC so the volume
                    // flyout / media keys reflect current state.
                    state.playback.subscribe(move |snap| {
                        if snap.duration_secs < 0.1 {
                            os_win::smtc_clear();
                        } else {
                            os_win::smtc_set_playback_status(snap.playing);
                        }
                    });
                }
            }

            // iOS startup: pick up any Share-extension payload that landed in
            // the App Group container while the app was closed, AND install
            // MPRemoteCommandCenter handlers so lock-screen / AirPods / CarPlay
            // can drive playback.
            #[cfg(mobile)]
            {
                mobile::pickup_shared_payload(app.handle(), &state);
                mobile::install_now_playing_handlers(state.playback.clone());

                // Subscribe to playback snapshots — whenever play state /
                // position changes, refresh the Now Playing metadata so the
                // lock screen progress bar stays in sync.
                let app_handle = app.handle().clone();
                state.playback.subscribe(move |snap| {
                    // Skip refreshes when nothing's actually playing or queued.
                    if snap.duration_secs < 0.1 {
                        mobile::now_playing_set("", "", "", 0.0, 0.0, false);
                        return;
                    }
                    // Pull the document name from the active document, fall
                    // back to a generic title.
                    let title = app_handle
                        .try_state::<std::sync::Arc<crate::state::AppState>>()
                        .and_then(|s| {
                            s.documents
                                .lock()
                                .ok()
                                .and_then(|d| d.values().next().map(|doc| doc.filename.clone()))
                        })
                        .unwrap_or_else(|| "Yappy".to_string());
                    mobile::now_playing_set(
                        &title,
                        "Yappy",
                        "",
                        snap.duration_secs as f64,
                        snap.elapsed_secs as f64,
                        snap.playing,
                    );
                });
            }

            // Position the player: custom point > preset > default bottom-right.
            // (Window positioning is desktop-only; iOS uses a single full-screen
            // webview without a separate player window.)
            #[cfg(desktop)]
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
            commands::haptic_cmd,
            commands::share_file_cmd,
            commands::list_rendered_audiobooks_cmd,
            commands::library_play_cmd,
            commands::library_pause_cmd,
            commands::library_resume_cmd,
            commands::library_stop_cmd,
            commands::library_seek_cmd,
            commands::library_status_cmd,
            commands::library_delete_cmd,
            commands::library_chapters_cmd,
            commands::library_reindex_spotlight_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
