// Yappy — local TTS that reads anything on your screen.

// All modules compile on both desktop and mobile. Each module that wraps
// desktop-only APIs (tray, hotkey, bridge, browser-extension) provides
// no-op / Err-returning stubs on mobile so the public surface stays uniform
// and `commands::*` keeps calling the same symbols on every platform.
pub mod audiobook;
mod bridge;
mod capture;
mod cola;
mod commands;
mod credits;
mod enlaces;
mod history;
mod hotkey;
pub mod imprenta;
pub mod libro;

/// Sube la prioridad del hilo ACTUAL (la síntesis de una lectura en vivo):
/// en Apple, QoS user-initiated. En el resto de plataformas, no-op.
pub fn subir_prioridad_de_hilo() {
    #[cfg(target_vendor = "apple")]
    {
        extern "C" {
            fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
        }
        const QOS_CLASS_USER_INITIATED: u32 = 0x19;
        unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0) };
    }
}

/// Baja la prioridad del hilo ACTUAL (los trabajos de fondo: imprenta,
/// muestras): en Apple, QoS utility — que la lectura viva respire.
pub fn bajar_prioridad_de_hilo() {
    #[cfg(target_vendor = "apple")]
    {
        extern "C" {
            fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
        }
        const QOS_CLASS_UTILITY: u32 = 0x11;
        unsafe { pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0) };
    }
}
mod model;
pub mod yappy_pack;
// Speech-to-text (ASR): Parakeet TDT model manager + transcript history. Audio
// decoding is desktop-only (iOS decodes via AVFoundation in Swift).
mod asr_decode;
mod asr_model;
mod compras;
mod cuota;
mod playback;
mod puente;
mod settings;
mod state;
mod transcripts;
mod tray;
mod windows;

// Mobile-only helpers — Share-extension payload pickup, UIPasteboard wrapper.
// El módulo real llama a FFI de Swift: SOLO iOS. Android recibe stubs con
// la misma superficie (su share llega por intents → fichero en app data).
#[cfg(target_os = "ios")]
mod mobile;
#[cfg(target_os = "android")]
#[path = "mobile_android.rs"]
mod mobile;

// Windows-native helpers — SMTC (system media transport controls) +
// taskbar progress + Jump List. Stubbed to no-ops on non-Windows targets
// so cross-platform call sites don't need cfg guards.
mod os_win;

use std::sync::Arc;

#[cfg(mobile)]
use tauri::Emitter;
use tauri::Manager;

use crate::state::AppState;

/// El AppHandle para el progreso de las descargas de la instalación.
#[cfg(mobile)]
static APP_VOCES: once_cell::sync::OnceCell<tauri::AppHandle> = once_cell::sync::OnceCell::new();

/// Progreso de Background Assets (Swift → Rust): se traduce al mismo evento
/// que la descarga en la app, así la tarjeta del loro comiendo no distingue
/// de dónde vienen las voces.
#[cfg(mobile)]
extern "C" fn cb_progreso_instalacion(done: u64, total: u64, fin: bool) {
    let Some(app) = APP_VOCES.get() else { return };
    let _ = app.emit(
        "model_download",
        model::DownloadProgress {
            file: "instalación".into(),
            bytes_done: done,
            bytes_total: total,
            stage: if fin {
                "done".into()
            } else {
                "downloading".into()
            },
            overall_done: done,
            overall_total: total.max(1),
        },
    );
    if fin && model::is_model_ready(app).unwrap_or(false) {
        let _ = app.emit("model_ready", true);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ─── Platform-specific startup environment ─────────────────────────────

    // Windows: declare Per-Monitor V2 DPI awareness BEFORE any HWND is
    // created. Without this, GetWindowRect + BitBlt return DPI-virtualized
    // pixel coordinates on 4K + mixed-DPI multi-monitor setups, so the OCR
    // fallback captures the wrong region. Per-Monitor V2 also lets the
    // Tauri webview render crisp on Hi-DPI screens.
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

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

        // ─ macOS: SIN CoreML desde macOS 26. El compilador E5RT (ANE y
        //   también el camino GPU del MLProgram) rechaza el vector_estimator
        //   de Supertonic («unbounded dimension», error -7) y ORT no cae al
        //   siguiente EP cuando falla la COMPILACIÓN de la sesión: la carga
        //   moría. Verificado el 23-08-2026 con el smoke del bundle. XNNPACK
        //   (registrado abajo para todas las plataformas) sintetiza ~7× más
        //   rápido que el tiempo real en Apple Silicon: sobra.
        #[cfg(target_os = "macos")]
        {
            requested.push("CoreML-disabled(macOS 26: E5RT rechaza el modelo)");
        }

        // ─ iOS (device AND simulator): XNNPACK/CPU only — NO CoreML.
        //   On a real device, CoreML compiles Supertonic's ~256 MB MLProgram for
        //   the Neural Engine and duplicates the weights, which blows past iOS's
        //   hard per-app memory limit and the session creation hangs / the app is
        //   jetsam-killed (the symptom: "loading TextToSpeech…" never finishing,
        //   transcription stuck). The simulator separately can't compile large
        //   MLPrograms at all (error -7). XNNPACK loads the ONNX weights once,
        //   runs on the CPU, stays well under the memory cap, and is what we
        //   verified working. (XNNPACK is also pushed below for every platform;
        //   on iOS it ends up being the only EP, which is intentional.)
        #[cfg(target_os = "ios")]
        {
            requested.push("CoreML-disabled(iOS: memory limit)");
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
            Err(e) => tracing::warn!("ort: EP registration failed, falling back to CPU-only: {e}"),
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
            detectar_idioma: true,
            pausa_entre_parrafos_s: 0.0,
        };
        let mut first = true;
        let sid = pb.begin_session();
        engine
            .synthesize_streaming(
                "Yappy backend, playback and all, says hello.",
                &opts,
                |chunk| {
                    let ac = playback::AudioChunk {
                        index: chunk.index,
                        paragraph_index: chunk.paragraph_index,
                        total: chunk.total,
                        total_paragraphs: chunk.total_paragraphs,
                        text: chunk.text.clone(),
                        origen_ini: chunk.origen_ini,
                        origen_fin: chunk.origen_fin,
                        samples: chunk.samples.clone(),
                        source_sample_rate: chunk.sample_rate as u32,
                    };
                    if first {
                        pb.new_session(sid, vec![ac], false);
                        first = false;
                    } else {
                        pb.enqueue(sid, ac);
                    }
                    Ok(())
                },
            )
            .expect("synthesize");
        let start = std::time::Instant::now();
        loop {
            let s = pb.snapshot();
            if !s.playing && s.duration_secs > 0.0 {
                break;
            }
            if start.elapsed().as_secs() > 30 {
                break;
            }
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
            detectar_idioma: true,
            pausa_entre_parrafos_s: 0.0,
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
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
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
        }));
    }
    // Desktop-only plugins. iOS has no autostart concept and no global
    // hotkeys; loading these plugins on mobile would compile-error in some
    // cases and be inert in others.
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--autostart"]),
            ))
            .plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }
    let builder = builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init());
    // El plugin de notificaciones SOLO en escritorio: en iOS instala su
    // propio delegate de UNUserNotificationCenter y su willPresent hace
    // assertionFailure con notificaciones que no creó él (las nuestras van
    // por yappy_notify). Cinco crashes seguidos en el sim lo delataron.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_notification::init());
    builder
        .manage(state.clone())
        .setup(move |app| {
            if let Err(e) = settings::SettingsStore::ensure(app.handle(), &state) {
                tracing::error!("settings init: {e:?}");
            }

            // El puente escucha en el escritorio desde el arranque: barato
            // (un socket QUIC dormido) y necesario para que el QR exista.
            #[cfg(desktop)]
            puente::iniciar_servidor(app.handle().clone());

            // Los deep links yappy:// se procesan de verdad (Quick Actions,
            // widget, Spotlight, emparejamiento). Sin esto solo abrían la app.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let asa = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        enlaces::manejar(&asa, url.as_str());
                    }
                });
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

            // iOS startup: install MPRemoteCommandCenter handlers so lock-screen
            // / AirPods / CarPlay can drive playback.
            //
            // NOTE: we deliberately do NOT drain the Share-extension payload here.
            // Draining + emitting at setup races the webview, which hasn't
            // registered its listener yet on a cold launch, so the shared item is
            // lost. Instead the frontend pulls pending payloads via
            // `drain_shared_payloads_cmd` once it's ready (see shareIntake.ts).
            // Un arranque HUMANO (doble clic, Launchpad) enseña la ventana:
            // la casa es tray-first, pero «abrir la app y que no pase nada»
            // no es una bienvenida. El arranque de login (--autostart) sí se
            // queda calladito en la barra de menús.
            #[cfg(desktop)]
            if !std::env::args().any(|a| a == "--autostart") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }

            // EL ESPEJO HACIA LA INTERFAZ, registrado UNA VEZ y desde el
            // arranque: cada snapshot (síntesis O libro vivo) viaja como
            // evento. Antes vivía dentro de read_internal: solo existía
            // tras la primera lectura TTS (y se duplicaba con cada una),
            // así que un audiolibro abierto en frío sonaba con la interfaz
            // ciega (la portada quieta encima del audio).
            {
                let app_for_listener = app.handle().clone();
                state.playback.subscribe(move |snap| {
                    let _ = tauri::Emitter::emit(&app_for_listener, "playback_state", snap);
                });
            }

            // LA IMPRENTA despierta: encargos huérfanos a pausado, y el
            // runner en marcha (procesa la cola en orden).
            imprenta::arrancar(app.handle().clone());

            // EL PROGRESO DURADERO, en TODAS las plataformas: el motor
            // escribe por dónde vas (throttled) aunque la app muera sonando.
            {
                let handle = app.handle().clone();
                let ultimo = std::sync::Mutex::new((
                    std::time::Instant::now() - std::time::Duration::from_secs(10),
                    usize::MAX,
                    String::new(),
                ));
                state.playback.subscribe(move |snap| {
                    if snap.doc_path.is_empty() || snap.total_paragraphs == 0 {
                        return;
                    }
                    let parrafo = snap.base_paragraph_index + snap.current_paragraph_index;
                    let total = snap.base_paragraph_index + snap.total_paragraphs;
                    let flush = snap.estado == "pausa";
                    let mut u = ultimo.lock().unwrap();
                    let mismo = u.1 == parrafo && u.2 == snap.doc_path;
                    if mismo && !flush {
                        return;
                    }
                    if !flush && u.0.elapsed() < std::time::Duration::from_secs(2) {
                        return;
                    }
                    u.0 = std::time::Instant::now();
                    u.1 = parrafo;
                    u.2 = snap.doc_path.clone();
                    commands::guardar_progreso_disco(&handle, &snap.doc_path, parrafo, total);
                });
            }

            // LA TIENDA: RevenueCat arranca en iOS (en el resto
            // `compras::es_pro()` es verdadero y la percha no tiene fondo).
            compras::arrancar(app.handle().clone());

            // LOS CUENTOS EMPAQUETADOS: a la biblioteca en la primera apertura.
            commands::importar_cuentos_empaquetados(app.handle());

            // LAS VOCES SIN MURO (móvil): si faltan, primero se reanudan las
            // descargas que dejó la instalación (Background Assets, con su
            // progreso pintado como el loro comiendo); si no había, y la
            // red es barata, la descarga arranca sola desde el espejo.
            #[cfg(mobile)]
            {
                let app_voces = app.handle().clone();
                std::thread::Builder::new()
                    .name("yappy-voces".into())
                    .spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(600));
                        if model::is_model_ready(&app_voces).unwrap_or(false) {
                            return;
                        }
                        let _ = APP_VOCES.set(app_voces.clone());
                        let en_marcha = mobile::ba_reanudar(cb_progreso_instalacion);
                        if en_marcha > 0 {
                            tracing::info!("voces: {en_marcha} descargas de la instalación reanudadas");
                            return;
                        }
                        if !mobile::red_barata() {
                            tracing::info!("voces: red cara; la descarga espera al toque del usuario");
                            return;
                        }
                        tracing::info!("voces: red barata; descarga automática");
                        let app2 = app_voces.clone();
                        tauri::async_runtime::spawn(async move {
                            let app3 = app2.clone();
                            let r = model::download_model(&app2, move |p| {
                                let _ = app3.emit("model_download", p);
                            })
                            .await;
                            match r {
                                Ok(()) => {
                                    let _ = app2.emit("model_ready", true);
                                }
                                Err(e) => tracing::warn!("voces: descarga automática falló: {e:#}"),
                            }
                        });
                    })
                    .ok();
            }

            #[cfg(mobile)]
            {
                mobile::install_now_playing_handlers(state.playback.clone());

                // La PRECARGA del motor: cargar el modelo en frío tarda
                // segundos, y el primer compartir no debe pagarlos. A los
                // dos segundos del arranque, si el modelo está en disco, se
                // carga en silencio y la primera lectura sale al vuelo.
                {
                    let app2 = app.handle().clone();
                    let state2 = state.clone();
                    std::thread::Builder::new()
                        .name("yappy-precarga".into())
                        .spawn(move || {
                            bajar_prioridad_de_hilo();
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            if model::is_model_ready(&app2).unwrap_or(false) {
                                if let Ok(root) = model::model_root(&app2) {
                                    let t0 = std::time::Instant::now();
                                    if state2.engine_or_load(&root).is_ok() {
                                        tracing::info!(
                                            "precarga: motor listo en {:.1}s",
                                            t0.elapsed().as_secs_f32()
                                        );
                                    }
                                }
                            }
                        })
                        .ok();
                }

                // La cocina de muestras: las presentaciones de las voces se
                // sintetizan una vez en segundo plano y quedan en caché para
                // que tocar un cromo suene al instante.
                commands::precocinar_muestras(app.handle().clone(), state.clone());
                commands::precocinar_titulos(app.handle().clone(), state.clone());

                // Subscribe to playback snapshots — whenever play state /
                // position changes, refresh the Now Playing metadata so the
                // lock screen progress bar stays in sync.
                {
                    // El nivel de la voz (0..1, ~20 Hz): la señal nerviosa
                    // de la interfaz viva. Evento aparte y ligerísimo para
                    // no arrastrar el snapshot entero a esa cadencia.
                    let app_nivel = app.handle().clone();
                    state.playback.subscribe_nivel(move |v| {
                        let _ = app_nivel.emit("playback_nivel", v);
                    });
                }
                let app_handle = app.handle().clone();
                state.playback.subscribe(move |snap| {
                    // Skip refreshes when nothing's actually playing or queued.
                    if snap.duration_secs < 0.1 {
                        // Si lo que suena es un audiolibro por AVAudioPlayer
                        // (la Biblioteca), NO pisar su Now Playing.
                        if !mobile::audiofile_is_playing() {
                            mobile::now_playing_set("", "", "", 0.0, 0.0, false);
                        }
                        return;
                    }
                    // El título de la sesión de lectura actual, no «el primer
                    // documento del HashMap».
                    let title = app_handle
                        .try_state::<std::sync::Arc<crate::state::AppState>>()
                        .map(|s| s.titulo_actual.lock().unwrap().clone())
                        .filter(|t| !t.is_empty())
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
            cola::cola_listar_cmd,
            cola::cola_agregar_url_cmd,
            cola::cola_agregar_web_cmd,
            cola::cola_reintentar_archivo_cmd,
            imprenta::imprenta_encargar_cmd,
            imprenta::imprenta_listar_cmd,
            imprenta::imprenta_pausar_cmd,
            imprenta::imprenta_reanudar_cmd,
            imprenta::imprenta_cancelar_cmd,
            imprenta::imprenta_quitar_cmd,
            imprenta::imprenta_editar_cmd,
            imprenta::imprenta_reordenar_cmd,
            imprenta::imprenta_m4b_cmd,
            cola::cola_agregar_texto_cmd,
            cola::cola_agregar_portapapeles_cmd,
            cola::cola_agregar_archivo_cmd,
            cola::cola_agregar_audio_cmd,
            cola::cola_eliminar_cmd,
            cola::cola_favorito_cmd,
            cola::cola_renombrar_cmd,
            cola::cola_reordenar_cmd,
            cola::cola_reintentar_cmd,
            puente::puente_estado_cmd,
            puente::puente_emparejar_nuevo_cmd,
            puente::puente_revocar_cmd,
            puente::puente_vincular_cmd,
            puente::puente_movil_estado_cmd,
            puente::puente_desvincular_cmd,
            puente::puente_convertir_cmd,
            puente::puente_probar_cmd,
            commands::avisos_pedir_cmd,
            commands::avisos_estado_cmd,
            commands::biblioteca_documentos_cmd,
            commands::biblioteca_olvidar_cmd,
            commands::list_voices,
            commands::get_settings,
            commands::set_settings,
            commands::trigger_read_now_cmd,
            commands::stop_playback_cmd,
            commands::library_abrir_cmd,
            commands::toggle_pause_cmd,
            commands::saltar_frase_cmd,
            commands::saltar_parrafo_cmd,
            commands::pausar_cmd,
            commands::reanudar_cmd,
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
            commands::playback_snapshot_cmd,
            commands::open_main_window,
            commands::open_player_window,
            commands::open_transcribe_window,
            commands::request_macos_permissions,
            commands::sample_voice,
            commands::decir_cmd,
            commands::set_voz_al_azar_cmd,
            commands::progreso_todo_cmd,
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
            commands::read_document_cmd,
            commands::read_text_as_document_cmd,
            commands::get_current_document_cmd,
            commands::document_window_ready_cmd,
            commands::clear_current_document_cmd,
            commands::save_project_cmd,
            commands::load_project_cmd,
            commands::render_audiobook_cmd,
            commands::haptic_cmd,
            commands::share_file_cmd,
            commands::drain_shared_payloads_cmd,
            commands::audiobook_export_path_cmd,
            commands::list_rendered_audiobooks_cmd,
            commands::library_play_cmd,
            commands::library_pause_cmd,
            commands::library_resume_cmd,
            commands::library_stop_cmd,
            commands::library_seek_cmd,
            commands::library_status_cmd,
            commands::library_delete_cmd,
            commands::library_chapters_cmd,
            commands::library_audio_src_cmd,
            commands::library_tiempos_cmd,
            commands::library_import_yappy_cmd,
            commands::library_reindex_spotlight_cmd,
            commands::is_asr_model_ready,
            commands::download_asr_model_cmd,
            commands::transcribe_audio_cmd,
            commands::transcribe_sample_cmd,
            commands::sample_document_path_cmd,
            commands::save_transcript_cmd,
            commands::audio_selftest_cmd,
            commands::get_transcripts,
            commands::clear_transcripts_cmd,
            commands::delete_transcript_cmd,
            commands::set_paseo_cmd,
            commands::portapapeles_tiene_enlace_cmd,
            commands::pip_iniciar_cmd,
            commands::pip_parar_cmd,
            commands::cuentos_listar_cmd,
            cola::cola_agregar_cuento_cmd,
            compras::compras_usuario_cmd,
            compras::compras_estado_cmd,
            compras::compras_ofertas_cmd,
            compras::compras_comprar_cmd,
            compras::compras_restaurar_cmd,
            compras::compras_cliente_cmd,
            compras::compras_gestionar_cmd,
            compras::compras_simular_cmd,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            // macOS: doble clic en el .app (o clic en el Dock) con la app ya
            // corriendo emite Reopen: la ventana principal vuelve a la vista.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                if let Some(w) = _app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        });
}
