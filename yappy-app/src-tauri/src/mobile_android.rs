//! Android: la MISMA superficie pública que el mobile.rs de iOS, hablada por
//! JNI con el objeto Kotlin `com.yappy.app.Puente` (gen/android/…/Puente.kt)
//! en vez de por los `@_cdecl` de Swift. Lo que en iOS es:
//!   - App Group + Share Extension → aquí la MainActivity escribe líneas en
//!     `<files>/yappy-shared.txt` y `drain` las lee y borra;
//!   - AVAudioSession + MPNowPlayingInfoCenter → MediaSession + foco de audio
//!     + un servicio en primer plano (Sonido.kt);
//!   - el keepalive de silencio y la Live Activity de la imprenta → el mismo
//!     servicio con una notificación de progreso;
//!   - AVAudioPlayer (biblioteca y efectos) → dos MediaPlayer;
//!   - UNUserNotificationCenter → NotificationManager (POST_NOTIFICATIONS);
//!   - UIActivityViewController → ACTION_SEND con FileProvider;
//!   - RevenueCat en Swift → RevenueCat en Kotlin (Compras.kt), con el
//!     mismo contrato JSON de peticiones numeradas (compras.rs).
//!
//! Kotlin contesta por tres `external fun` que aquí se exportan con nombre
//! JNI (`Java_com_yappy_app_Puente_nativo*`).

use std::sync::{Arc, OnceLock};

use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};

use crate::state::AppState;

// ─── El pegamento JNI ───────────────────────────────────────────────────

/// El contexto NDK, sin pánico: Tauri lo inicializa DESPUÉS de `setup`,
/// así que las primeras llamadas del arranque pueden llegar antes de que
/// exista (el pánico de ndk-context tumbaba la app entera en el emulador).
fn contexto() -> Option<ndk_context::AndroidContext> {
    std::panic::catch_unwind(ndk_context::android_context).ok()
}

/// La Application, retenida para siempre (ndk-context guarda el puntero).
static APLICACION: OnceLock<GlobalRef> = OnceLock::new();

/// Kotlin → Rust, desde `Puente.arrancar`: la JavaVM y la Application. Si
/// Tauri no ha rellenado ndk-context (aquí nunca lo hace a tiempo), lo
/// rellenamos nosotros; con eso viven cpal, el puente y la tienda.
#[no_mangle]
pub extern "system" fn Java_com_yappy_app_Puente_nativoContexto(
    env: JNIEnv,
    _clase: jni::objects::JClass,
    app: JObject,
) {
    let Ok(g) = env.new_global_ref(app) else {
        return;
    };
    let _ = APLICACION.set(g);
    let Some(g) = APLICACION.get() else { return };
    if contexto().is_some() {
        return;
    }
    let Ok(vm) = env.get_java_vm() else { return };
    let vm_ptr = vm.get_java_vm_pointer() as *mut std::ffi::c_void;
    let ctx_ptr = g.as_obj().as_raw() as *mut std::ffi::c_void;
    let r = std::panic::catch_unwind(|| unsafe {
        ndk_context::initialize_android_context(vm_ptr, ctx_ptr);
    });
    tracing::info!(
        "mobile: contexto Android {}",
        if r.is_ok() {
            "rellenado desde Kotlin"
        } else {
            "ya estaba"
        }
    );
}

/// Espera (hasta `max_ms`) a que el contexto exista. Para lo que debe
/// hablar con Kotlin nada más arrancar (la tienda).
pub fn esperar_contexto(max_ms: u64) -> bool {
    let fin = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    while std::time::Instant::now() < fin {
        if contexto().is_some() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

fn vm() -> Option<JavaVM> {
    let ctx = contexto()?;
    let raw = ctx.vm() as *mut jni::sys::JavaVM;
    if raw.is_null() {
        return None;
    }
    unsafe { JavaVM::from_raw(raw).ok() }
}

static PUENTE: OnceLock<GlobalRef> = OnceLock::new();

/// La clase `Puente`, resuelta por el ClassLoader de la app (desde un hilo
/// nativo, `FindClass` solo ve las clases del sistema).
fn clase_puente(env: &mut JNIEnv) -> Option<GlobalRef> {
    if let Some(c) = PUENTE.get() {
        return Some(c.clone());
    }
    let ctx = contexto()?;
    let actividad = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };
    let loader = env
        .call_method(
            &actividad,
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;
    let nombre = env.new_string("com.yappy.app.Puente").ok()?;
    let clase = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&nombre)],
        )
        .ok()?
        .l()
        .ok()?;
    let global = env.new_global_ref(clase).ok()?;
    let _ = PUENTE.set(global.clone());
    Some(global)
}

/// Lo que devuelve un método del Puente, ya copiado fuera de la JVM.
enum Valor {
    Nada,
    Bool(bool),
    Int(i32),
    Double(f64),
    Texto(Option<String>),
}

/// Llama a un método estático del Puente. `textos` se convierten a
/// `String` de Java y se pasan DELANTE de `extra` (la firma debe estar en
/// ese orden), salvo que `extra_primero` sea verdadero. Cualquier fallo (VM
/// ausente, excepción Java) se traga con un log: el puente nunca tumba la
/// app.
fn llamar(
    nombre: &str,
    firma: &str,
    textos: &[&str],
    extra: &[JValue],
    extra_primero: bool,
) -> Valor {
    let Some(vm) = vm() else { return Valor::Nada };
    let Ok(mut env) = vm.attach_current_thread_permanently() else {
        return Valor::Nada;
    };
    let Some(clase) = clase_puente(&mut env) else {
        return Valor::Nada;
    };
    let mut objs: Vec<JObject> = Vec::with_capacity(textos.len());
    for t in textos {
        match env.new_string(t) {
            Ok(s) => objs.push(JObject::from(s)),
            Err(_) => return Valor::Nada,
        }
    }
    let mut args: Vec<JValue> = Vec::with_capacity(objs.len() + extra.len());
    if extra_primero {
        args.extend(extra.iter().cloned());
        args.extend(objs.iter().map(JValue::Object));
    } else {
        args.extend(objs.iter().map(JValue::Object));
        args.extend(extra.iter().cloned());
    }
    let r = env.call_static_method(
        <&jni::objects::JClass>::from(clase.as_obj()),
        nombre,
        firma,
        &args,
    );
    let devuelve = firma.rsplit(')').next().unwrap_or("V");
    match r {
        Ok(v) => match devuelve {
            "Z" => Valor::Bool(v.z().unwrap_or(false)),
            "I" => Valor::Int(v.i().unwrap_or(0)),
            "D" => Valor::Double(v.d().unwrap_or(0.0)),
            "Ljava/lang/String;" => {
                let obj = match v.l() {
                    Ok(o) => o,
                    Err(_) => return Valor::Texto(None),
                };
                if obj.is_null() {
                    Valor::Texto(None)
                } else {
                    let s: Option<String> =
                        env.get_string(&JString::from(obj)).ok().map(|j| j.into());
                    Valor::Texto(s)
                }
            }
            _ => Valor::Nada,
        },
        Err(e) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
            }
            tracing::warn!("puente: {nombre}: {e}");
            Valor::Nada
        }
    }
}

fn llamar_v(nombre: &str, firma: &str, textos: &[&str], extra: &[JValue]) {
    let _ = llamar(nombre, firma, textos, extra, false);
}
fn llamar_bool(nombre: &str, firma: &str, textos: &[&str], extra: &[JValue]) -> bool {
    match llamar(nombre, firma, textos, extra, false) {
        Valor::Bool(b) => b,
        _ => false,
    }
}
fn llamar_f64(nombre: &str, firma: &str, textos: &[&str], extra: &[JValue]) -> f64 {
    match llamar(nombre, firma, textos, extra, false) {
        Valor::Double(d) => d,
        _ => 0.0,
    }
}
fn llamar_i32(nombre: &str, firma: &str) -> i32 {
    match llamar(nombre, firma, &[], &[], false) {
        Valor::Int(i) => i,
        _ => 0,
    }
}
fn llamar_string(nombre: &str, firma: &str) -> Option<String> {
    match llamar(nombre, firma, &[], &[], false) {
        Valor::Texto(s) => s,
        _ => None,
    }
}

// ─── Lo compartido (intents → fichero) ──────────────────────────────────

fn ruta_compartidos() -> Option<std::path::PathBuf> {
    // El paquete de Android es el de Play (applicationId en build.gradle.kts).
    // (El de Play es com.joseluissaorin.yappy.android: el paquete original
    // quedó reservado para siempre en la cuenta personal.)
    for base in [
        "/data/data/com.joseluissaorin.yappy.android/files",
        "/data/user/0/com.joseluissaorin.yappy.android/files",
        "/data/data/com.joseluissaorin.yappy/files",
        "/data/user/0/com.joseluissaorin.yappy/files",
    ] {
        let b = std::path::PathBuf::from(base);
        if b.exists() {
            return Some(b.join("yappy-shared.txt"));
        }
    }
    None
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

/// El texto del portapapeles, para el gesto explícito de «pegar lo copiado».
pub fn pasteboard_text() -> Option<String> {
    llamar_string("portapapelesTexto", "()Ljava/lang/String;").filter(|s| !s.trim().is_empty())
}

// ─── ASR: el modelo ONNX común (asr_model.rs); aquí nada nativo ─────────
pub fn transcribe(_path: &str) -> Option<String> {
    None
}
pub fn asr_model_ready() -> bool {
    false
}
pub fn asr_download_model() {}

/// Decodifica cualquier audio que Android sepa leer (Ogg-Opus de WhatsApp
/// incluido) a un WAV PCM16. Devuelve la ruta del WAV si salió bien.
pub fn decodificar_audio(origen: &std::path::Path) -> Option<std::path::PathBuf> {
    let destino =
        std::env::temp_dir().join(format!("yappy-dec-{}.wav", uuid::Uuid::new_v4().simple()));
    let ok = llamar_bool(
        "decodificarAudio",
        "(Ljava/lang/String;Ljava/lang/String;)Z",
        &[&origen.to_string_lossy(), &destino.to_string_lossy()],
        &[],
    );
    if ok && destino.exists() {
        Some(destino)
    } else {
        let _ = std::fs::remove_file(&destino);
        None
    }
}

// ─── Now Playing / MediaSession ─────────────────────────────────────────

pub fn now_playing_set(
    title: &str,
    artist: &str,
    album: &str,
    duration: f64,
    position: f64,
    playing: bool,
) {
    // Título vacío = ya no suena nada (Kotlin lo trata como null).
    llamar_v(
        "nowPlaying",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DDZ)V",
        &[title, artist, album],
        &[
            JValue::Double(duration),
            JValue::Double(position),
            JValue::Bool(playing as u8),
        ],
    );
}

static PLAYBACK: OnceLock<Arc<crate::playback::PlaybackController>> = OnceLock::new();

pub fn install_now_playing_handlers(playback: Arc<crate::playback::PlaybackController>) {
    let _ = PLAYBACK.set(playback);
    tracing::info!("mobile: mandos de la MediaSession instalados");
}

/// Kotlin → Rust: los mandos de la pantalla de bloqueo, los auriculares y
/// el foco de audio. La MISMA semántica que en iOS (mobile.rs).
#[no_mangle]
pub extern "system" fn Java_com_yappy_app_Puente_nativoMando(
    mut env: JNIEnv,
    _clase: jni::objects::JClass,
    cual: JString,
    valor: jni::sys::jdouble,
) {
    let cual: String = env.get_string(&cual).map(Into::into).unwrap_or_default();
    let libro = crate::libro::activo();
    let Some(p) = PLAYBACK.get() else { return };
    match cual.as_str() {
        "play" => {
            if libro {
                audiofile_resume();
            } else {
                p.resume();
            }
        }
        "pause" => {
            if libro {
                audiofile_pause();
            } else {
                p.pause();
            }
        }
        "toggle" => {
            if libro {
                if audiofile_is_playing() {
                    audiofile_pause();
                } else {
                    audiofile_resume();
                }
            } else {
                let s = p.snapshot();
                if s.paused {
                    p.resume();
                } else if s.playing {
                    p.pause();
                }
            }
        }
        "next" => {
            if libro {
                audiofile_seek((audiofile_position() + 15.0).max(0.0));
            } else {
                p.saltar_chunk(1);
            }
        }
        "prev" => {
            if libro {
                audiofile_seek((audiofile_position() - 15.0).max(0.0));
            } else {
                p.saltar_chunk(-1);
            }
        }
        "seek" => {
            if libro {
                audiofile_seek(valor.max(0.0));
            } else {
                let elapsed = p.snapshot().elapsed_secs as f64;
                p.seek((valor - elapsed) as f32);
            }
        }
        "interrupcion" => {
            if valor >= 0.5 {
                let s = p.snapshot();
                if s.paused {
                    p.resume();
                }
            } else {
                if libro {
                    audiofile_pause();
                }
                p.pause();
            }
        }
        _ => {}
    }
}

// ─── Háptica, avisos, compartir ─────────────────────────────────────────

pub fn haptic(kind: &str) {
    llamar_v("haptic", "(Ljava/lang/String;)V", &[kind], &[]);
}

pub fn notify(identifier: &str, title: &str, body: &str) {
    llamar_v(
        "notificar",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[identifier, title, body],
        &[],
    );
}

pub fn notify_request() {
    llamar_v("avisosPedir", "()V", &[], &[]);
}

/// 0 sin decidir · 1 concedido · 2 denegado.
pub fn notify_status() -> i32 {
    llamar_i32("avisosEstado", "()I")
}

pub fn share_file(path: &str) {
    llamar_v("compartirFichero", "(Ljava/lang/String;)V", &[path], &[]);
}

// ─── El reproductor de ficheros (la biblioteca) ─────────────────────────

pub fn audiofile_play(path: &str, start_at_secs: f64) -> bool {
    llamar_bool(
        "ficheroPlay",
        "(Ljava/lang/String;D)Z",
        &[path],
        &[JValue::Double(start_at_secs)],
    )
}
pub fn audiofile_pause() {
    llamar_v("ficheroPause", "()V", &[], &[]);
}
pub fn audiofile_resume() {
    llamar_v("ficheroResume", "()V", &[], &[]);
}
pub fn audiofile_stop() {
    llamar_v("ficheroStop", "()V", &[], &[]);
}
pub fn audiofile_seek(secs: f64) {
    llamar_v("ficheroSeek", "(D)V", &[], &[JValue::Double(secs)]);
}
pub fn audiofile_position() -> f64 {
    llamar_f64("ficheroPosicion", "()D", &[], &[])
}
pub fn audiofile_duration() -> f64 {
    llamar_f64("ficheroDuracion", "()D", &[], &[])
}
pub fn audiofile_is_playing() -> bool {
    llamar_bool("ficheroSonando", "()Z", &[], &[])
}
pub fn audiofile_current_path() -> Option<String> {
    llamar_string("ficheroActual", "()Ljava/lang/String;")
}

// ─── El canal de efectos ────────────────────────────────────────────────

pub fn efecto_play(path: &str) -> f64 {
    llamar_f64("efectoPlay", "(Ljava/lang/String;)D", &[path], &[])
}
pub fn efecto_stop() {
    llamar_v("efectoStop", "()V", &[], &[]);
}

// ─── Spotlight: no existe en Android ────────────────────────────────────
pub fn spotlight_replace_all(_payload: &str) {}

// ─── La sesión de audio y el escudo de fondo ────────────────────────────

/// El foco de audio (las demás apps callan) antes de que cpal abra el stream.
pub fn audio_session_activate() {
    llamar_v("audioFocoPedir", "()V", &[], &[]);
}

/// Hay una lectura VIVA (sonando o en pausa reanudable): el servicio no se
/// apaga y el foco no se suelta.
pub fn audio_session_lectura(viva: bool) {
    llamar_v("lecturaViva", "(Z)V", &[], &[JValue::Bool(viva as u8)]);
}

pub fn activity_start(title: &str, total: i32) {
    llamar_v(
        "actividadEmpezar",
        "(Ljava/lang/String;I)V",
        &[title],
        &[JValue::Int(total)],
    );
}
pub fn activity_update(done: i32, total: i32, stage: &str, title: Option<&str>) {
    // Sin título nuevo se manda el vacío (Kotlin conserva el que tenía).
    let _ = llamar(
        "actividadActualizar",
        "(IILjava/lang/String;Ljava/lang/String;)V",
        &[stage, title.unwrap_or("")],
        &[JValue::Int(done), JValue::Int(total)],
        true,
    );
}
pub fn activity_end(title: &str) {
    llamar_v("actividadTerminar", "(Ljava/lang/String;)V", &[title], &[]);
}

/// El escudo: mientras vive, el proceso no muere con la pantalla apagada.
pub struct BackgroundAudioGuard;
impl BackgroundAudioGuard {
    pub fn begin() -> Self {
        llamar_v("fondoEmpezar", "()V", &[], &[]);
        tracing::info!("mobile: escudo de fondo levantado");
        BackgroundAudioGuard
    }
}
impl Drop for BackgroundAudioGuard {
    fn drop(&mut self) {
        llamar_v("fondoTerminar", "()V", &[], &[]);
        tracing::info!("mobile: escudo de fondo retirado");
    }
}

// ─── Los recursos: sin App Group ni Background Assets en Android ─────────
pub fn app_group_path() -> Option<String> {
    None
}
pub fn red_barata() -> bool {
    // Sin monitor de red todavía: se asume wifi (la descarga fp16 pesa la
    // mitad y la tarjeta de las voces sigue enseñando el tamaño).
    true
}
pub fn ba_reanudar(_cb: extern "C" fn(u64, u64, bool)) -> i32 {
    0
}

// ─── El paseo: portapapeles sin leerlo; sin PiP en Android ──────────────
pub fn portapapeles_tiene_enlace() -> bool {
    llamar_bool("portapapelesTieneEnlace", "()Z", &[], &[])
}
pub fn pip_iniciar(_path: &str, _x: f64, _y: f64, _w: f64, _h: f64) {}
pub fn pip_parar() {}

// ─── Las compras (RevenueCat en Kotlin) ─────────────────────────────────

pub fn compras_configurar(api_key: &str, entitlement: &str) {
    llamar_v(
        "comprasConfigurar",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[api_key, entitlement],
        &[],
    );
}
pub fn compras_es_pro() -> bool {
    llamar_bool("comprasEsPro", "()Z", &[], &[])
}
pub fn compras_ofertas(id: u64) {
    llamar_v("comprasOfertas", "(J)V", &[], &[JValue::Long(id as i64)]);
}
pub fn compras_comprar(id: u64, paquete: &str) {
    let _ = llamar(
        "comprasComprar",
        "(JLjava/lang/String;)V",
        &[paquete],
        &[JValue::Long(id as i64)],
        true,
    );
}
pub fn compras_restaurar(id: u64) {
    llamar_v("comprasRestaurar", "(J)V", &[], &[JValue::Long(id as i64)]);
}
pub fn compras_cliente(id: u64) {
    llamar_v("comprasCliente", "(J)V", &[], &[JValue::Long(id as i64)]);
}
pub fn compras_usuario() -> Option<String> {
    llamar_string("comprasUsuario", "()Ljava/lang/String;")
}
pub fn compras_gestionar() {
    llamar_v("comprasGestionar", "()V", &[], &[]);
}

/// Kotlin → Rust: la respuesta a una petición numerada de la tienda.
#[no_mangle]
pub extern "system" fn Java_com_yappy_app_Puente_nativoCompras(
    mut env: JNIEnv,
    _clase: jni::objects::JClass,
    id: jni::sys::jlong,
    json: JString,
) {
    let json: String = env.get_string(&json).map(Into::into).unwrap_or_default();
    crate::compras::responder(id as u64, json);
}

/// Kotlin → Rust: cada cambio de la cuerda.
#[no_mangle]
pub extern "system" fn Java_com_yappy_app_Puente_nativoPro(
    _env: JNIEnv,
    _clase: jni::objects::JClass,
    pro: jni::sys::jboolean,
) {
    crate::compras::pro_desde_la_tienda(pro != 0);
}
