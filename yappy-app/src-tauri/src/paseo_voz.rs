//! LA VOZ PREGRABADA DEL PASEO. Todo lo que el loro dice en el paseo viene
//! renderizado de fábrica (`resources/paseo-voz/<lang>.<clave>.<hash>.m4a`,
//! con la voz del loro; lo genera `tooling/paseo/voz.mjs`) y suena AL
//! INSTANTE por el canal de efectos. El hash es el del texto que se va a
//! decir: si el diccionario cambió y nadie regeneró, no casa y se sintetiza
//! en vivo (el camino viejo, ahora solo de reserva).
//!
//! Dentro del paquete solo van español e inglés; los otros 29 idiomas se
//! bajan del espejo R2 de la casa (`ESPEJO_VOZ/<fichero>.m4a`) al arrancar
//! en ese idioma o al elegirlo, a `app_data/paseo-voz/` (por nombre: la ruta
//! se resuelve viva, ver ios-rutas-contenedor). La app calcula los nombres
//! con el hash del texto, así que no hay manifiesto remoto.
//!
//! Si el usuario elige otro pájaro, la cocina del paseo prepara en segundo
//! plano las frases que quedan con SU voz (`muestras/paseo/`); mientras no
//! estén, suena la grabada del loro, que también es instantánea.
//!
//! Callar es en seco: `paseo_callar_cmd` para el canal de efectos y sube el
//! TURNO, así que una síntesis en vivo que termine tarde ya no suena encima
//! del paso siguiente.

// En escritorio no hay paseo: los comandos existen pero callan.
#![cfg_attr(not(mobile), allow(dead_code))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::state::AppState;

/// La voz con la que está grabado el paseo: la de fábrica
/// (`Settings::default().voice`). Debe casar con `RECETA.voz` del script.
pub const VOZ_LORO: &str = "Alex";

/// Los idiomas que van dentro del paquete (`EMPAQUETADOS` del script).
pub const EMPAQUETADOS: [&str; 2] = ["es", "en"];

/// El espejo de la voz del paseo. `YAPPY_PASEO_VOZ_URL` lo cambia (pruebas
/// contra un servidor local).
const ESPEJO_VOZ: &str = "https://modelos.yappy.joseluissaorin.com/paseo-voz";

/// Sube con cada decir y con cada callar: lo que se sintetizó para un turno
/// viejo no suena.
static TURNO: AtomicU64 = AtomicU64::new(0);
/// Generación de la cocina del paseo: una cocina nueva despide a la vieja.
static COCINA: AtomicU64 = AtomicU64::new(0);

/// FNV-1a de 64 bits sobre «lang\ntexto», 10 cifras hex: el mismo cálculo
/// que `huella()` en tooling/paseo/voz.mjs.
pub fn huella(lang: &str, texto: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in lang
        .bytes()
        .chain(std::iter::once(b'\n'))
        .chain(texto.bytes())
    {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")[..10].to_string()
}

/// `<lang>.<clave>.<hash>` (sin extensión). La clave pierde el prefijo
/// `paseo.` y todo lo que no sea letra, cifra, punto, guion o subrayado.
pub fn nombre_clip(lang: &str, clave: &str, texto: &str) -> String {
    let clave = clave.strip_prefix("paseo.").unwrap_or(clave);
    let seguro: String = clave
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    let lang: String = lang.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!("{lang}.{seguro}.{}", huella(&lang, texto))
}

fn lang_efectivo(state: &AppState, idioma: Option<String>) -> String {
    match idioma {
        Some(l) if !l.is_empty() && l != "na" => l,
        _ => {
            let l = state.settings.lock().unwrap().default_lang.clone();
            if l == "na" || l.is_empty() {
                "en".into()
            } else {
                l
            }
        }
    }
}

#[cfg(mobile)]
fn dir_cocina(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let d = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("muestras")
        .join("paseo");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

#[cfg(mobile)]
fn ruta_cocinada(app: &AppHandle, voz: &str, nombre: &str) -> Option<std::path::PathBuf> {
    let v: String = voz
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    dir_cocina(app)
        .ok()
        .map(|d| d.join(format!("{v}.{nombre}.wav")))
}

#[cfg(mobile)]
fn dir_descargas(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let d = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("paseo-voz");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Suena el clip empaquetado si existe. Devuelve la duración sin escalar
/// (0.0 si no está).
#[cfg(target_os = "ios")]
fn sonar_empaquetado(app: &AppHandle, nombre: &str, rate: f32) -> f64 {
    use tauri::Manager;
    let Ok(base) = app.path().resource_dir() else {
        return 0.0;
    };
    let f = format!("{nombre}.m4a");
    let ruta = [
        base.join("_up_")
            .join("resources")
            .join("paseo-voz")
            .join(&f),
        base.join("resources").join("paseo-voz").join(&f),
        base.join("paseo-voz").join(&f),
    ]
    .into_iter()
    .find(|c| c.exists());
    match ruta {
        Some(r) => crate::mobile::efecto_play_rate(&r.to_string_lossy(), rate),
        None => 0.0,
    }
}

#[cfg(target_os = "android")]
fn sonar_empaquetado(_app: &AppHandle, nombre: &str, rate: f32) -> f64 {
    // Tauri deja resources/ en los assets del APK con el mismo «_up_».
    crate::mobile::efecto_play_recurso(&format!("_up_/resources/paseo-voz/{nombre}.m4a"), rate)
}

/// Sintetiza `texto` con la voz y el idioma dados (bloqueante). No espera al
/// motor: si está ocupado, falla.
#[cfg(mobile)]
fn sintetizar(
    app: &AppHandle,
    st: &Arc<AppState>,
    texto: &str,
    voz: &str,
    lang: &str,
    velocidad: f32,
) -> Result<(Vec<f32>, u32), String> {
    let _motor = st
        .candado_motor
        .try_lock()
        .map_err(|_| "motor ocupado".to_string())?;
    let root = crate::model::model_root(app).map_err(|e| e.to_string())?;
    let engine = st.engine_or_load(&root).map_err(|e| e.to_string())?;
    let opts = yappy_core::engine::SynthesisOptions {
        voice: voz.to_string(),
        speed: velocidad.clamp(0.5, 2.0),
        default_lang: lang.to_string(),
        total_steps: crate::settings::Quality::Fast.total_steps(),
        seed: Some(7),
        detectar_idioma: false,
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
    Ok((samples, sr))
}

/// EL LORO DICE UN PASO. `clave` es la del diccionario (identifica el clip),
/// `texto` lo que dice de verdad (su hash elige el fichero), `idioma` el del
/// texto y `velocidad` la de la demo del dial (se aplica al reproducir).
/// Calla si algo de verdad suena. Devuelve los segundos que va a sonar.
#[tauri::command]
pub async fn paseo_decir_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    clave: String,
    texto: String,
    idioma: Option<String>,
    velocidad: Option<f32>,
) -> Result<f64, String> {
    #[cfg(mobile)]
    {
        if state.playback.snapshot().estado != "inactivo" || texto.trim().is_empty() {
            return Ok(0.0);
        }
        let turno = TURNO.fetch_add(1, Ordering::SeqCst) + 1;
        let lang = lang_efectivo(&state, idioma);
        let rate = velocidad.unwrap_or(1.0).clamp(0.5, 2.0);
        let voz = state.settings.lock().unwrap().voice.clone();
        let nombre = nombre_clip(&lang, &clave, &texto);

        // 1) Con la voz elegida, si la cocina (o una síntesis anterior) ya
        //    la dejó hecha.
        if let Some(p) = ruta_cocinada(&app, &voz, &nombre).filter(|p| p.exists()) {
            let d = crate::mobile::efecto_play_rate(&p.to_string_lossy(), rate);
            if d > 0.0 {
                tracing::info!("paseo: {nombre} cocinado con {voz} ({d:.1} s)");
                return Ok(d / rate as f64);
            }
        }
        // 2) La grabación de fábrica: al instante.
        let t0 = std::time::Instant::now();
        let d = sonar_empaquetado(&app, &nombre, rate);
        if d > 0.0 {
            tracing::info!(
                "paseo: {nombre} pregrabado ({d:.1} s, arranca en {} ms)",
                t0.elapsed().as_millis()
            );
            return Ok(d / rate as f64);
        }
        // 2b) La bajada del espejo (los idiomas que no van en el paquete).
        if let Some(p) = dir_descargas(&app)
            .ok()
            .map(|d| d.join(format!("{nombre}.m4a")))
            .filter(|p| p.exists())
        {
            let d = crate::mobile::efecto_play_rate(&p.to_string_lossy(), rate);
            if d > 0.0 {
                tracing::info!("paseo: {nombre} bajado ({d:.1} s)");
                return Ok(d / rate as f64);
            }
        }
        // 3) Reserva: síntesis en vivo (textos con partes dinámicas o un
        //    diccionario que cambió sin regenerar los audios).
        tracing::info!("paseo: sin audio pregrabado para {nombre}; en vivo");
        let st = state.inner().clone();
        let dur = tauri::async_runtime::spawn_blocking(move || -> Result<f64, String> {
            let (samples, sr) = sintetizar(&app, &st, &texto, &voz, &lang, rate)?;
            let p = if (rate - 1.0).abs() < 0.01 {
                ruta_cocinada(&app, &voz, &nombre).ok_or("sin carpeta")?
            } else {
                dir_cocina(&app)?.join("dicho-paseo.wav")
            };
            crate::playback::write_wav_file(&p, &samples, sr).map_err(|e| e.to_string())?;
            // Si mientras tanto se pasó de paso (o se dijo otra cosa), callar.
            if TURNO.load(Ordering::SeqCst) != turno {
                return Ok(0.0);
            }
            Ok(crate::mobile::efecto_play(&p.to_string_lossy()))
        })
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(0.0);
        return Ok(dur);
    }
    #[cfg(not(mobile))]
    {
        let _ = (app, state, clave, texto, idioma, velocidad, &TURNO);
        Ok(0.0)
    }
}

/// Corta en seco lo que el loro esté diciendo (al cambiar de paso, al
/// saltar, al salir del paseo).
#[tauri::command]
pub fn paseo_callar_cmd() {
    TURNO.fetch_add(1, Ordering::SeqCst);
    tracing::info!("paseo: callar");
    #[cfg(mobile)]
    crate::mobile::efecto_stop();
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrasePaseo {
    pub clave: String,
    pub texto: String,
    pub idioma: Option<String>,
}

/// LA COCINA DEL PASEO: con otro pájaro elegido, prepara en segundo plano
/// las frases que quedan con su voz (en el orden en que se dirán). Nunca
/// pisa una lectura; una cocina nueva despide a la anterior.
#[tauri::command]
pub fn paseo_precocinar_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    voz: String,
    frases: Vec<FrasePaseo>,
) {
    let generacion = COCINA.fetch_add(1, Ordering::SeqCst) + 1;
    #[cfg(mobile)]
    {
        if voz.eq_ignore_ascii_case(VOZ_LORO) || frases.is_empty() {
            return;
        }
        let st = state.inner().clone();
        let _ = std::thread::Builder::new()
            .name("yappy-paseo-voz".into())
            .spawn(move || {
                crate::bajar_prioridad_de_hilo();
                let viva = || COCINA.load(Ordering::SeqCst) == generacion;
                let mut hechas = 0;
                for f in frases {
                    let lang = lang_efectivo(&st, f.idioma.clone());
                    let nombre = nombre_clip(&lang, &f.clave, &f.texto);
                    let Some(p) = ruta_cocinada(&app, &voz, &nombre) else {
                        return;
                    };
                    if p.exists() {
                        continue;
                    }
                    // Esperar al silencio y al motor libre, sin rendirse
                    // mientras esta cocina siga siendo la vigente.
                    let mut intentos = 0;
                    loop {
                        if !viva() {
                            return;
                        }
                        let listo = crate::model::is_model_ready(&app).unwrap_or(false)
                            && st.playback.snapshot().estado == "inactivo";
                        if listo {
                            match sintetizar(&app, &st, &f.texto, &voz, &lang, 1.0) {
                                Ok((s, sr)) => {
                                    if crate::playback::write_wav_file(&p, &s, sr).is_ok() {
                                        hechas += 1;
                                    }
                                    break;
                                }
                                Err(e) if e == "motor ocupado" => {}
                                Err(e) => {
                                    tracing::warn!("paseo: cocina de {nombre}: {e}");
                                    break;
                                }
                            }
                        }
                        intentos += 1;
                        if intentos > 600 {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
                tracing::info!("paseo: {hechas} frases cocinadas con {voz}");
            });
    }
    #[cfg(not(mobile))]
    {
        let _ = (app, state, voz, frases, generacion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El mismo hash que tooling/paseo/voz.mjs (valores sacados de allí).
    #[test]
    fn huella_casa_con_el_script() {
        assert_eq!(huella("es", "Hola, soy tu loro."), "00fb5a6846");
        assert_eq!(
            nombre_clip("es", "paseo.hola_corto", "Hola, soy tu loro."),
            "es.hola_corto.00fb5a6846"
        );
    }
}

/// LA BAJADA: trae del espejo los audios del paseo de un idioma que no va
/// en el paquete (los que falten). En segundo plano, cuatro a la vez; un 404
/// (texto cambiado sin resubir) se salta y ese paso irá en vivo.
#[tauri::command]
pub fn paseo_voz_descargar_cmd(app: AppHandle, frases: Vec<FrasePaseo>) {
    #[cfg(mobile)]
    {
        use std::collections::HashSet;
        use std::sync::Mutex;
        static EN_VUELO: Mutex<Option<HashSet<String>>> = Mutex::new(None);

        let Ok(dir) = dir_descargas(&app) else { return };
        let mut pendientes: Vec<String> = Vec::new();
        {
            let mut vuelo = EN_VUELO.lock().unwrap();
            let vuelo = vuelo.get_or_insert_with(HashSet::new);
            for f in frases {
                let lang = f.idioma.clone().unwrap_or_default();
                // es y en van empaquetados en iOS y en Android (allí en los
                // assets del APK: build.gradle.kts quita `<dir>_*` del
                // ignoreAssetsPattern de aapt para que entre `_up_/resources`).
                let empaquetado = EMPAQUETADOS.contains(&lang.as_str());
                if lang.is_empty() || empaquetado {
                    continue;
                }
                let nombre = nombre_clip(&lang, &f.clave, &f.texto);
                if dir.join(format!("{nombre}.m4a")).exists() || !vuelo.insert(nombre.clone()) {
                    continue;
                }
                pendientes.push(nombre);
            }
        }
        if pendientes.is_empty() {
            return;
        }
        let base = std::env::var("YAPPY_PASEO_VOZ_URL").unwrap_or_else(|_| ESPEJO_VOZ.to_string());
        tauri::async_runtime::spawn(async move {
            let Ok(client) = reqwest::Client::builder()
                .user_agent("Yappy (https://yappy.joseluissaorin.com)")
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(60))
                .build()
            else {
                return;
            };
            let total = pendientes.len();
            let bien = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let cola = std::sync::Arc::new(Mutex::new(pendientes));
            let mut obreros = Vec::new();
            for _ in 0..4 {
                let (client, cola, dir, base, bien) = (
                    client.clone(),
                    cola.clone(),
                    dir.clone(),
                    base.clone(),
                    bien.clone(),
                );
                obreros.push(tauri::async_runtime::spawn(async move {
                    loop {
                        let Some(nombre) = cola.lock().unwrap().pop() else {
                            break;
                        };
                        let url = format!("{base}/{nombre}.m4a");
                        let ok = async {
                            let r = client.get(&url).send().await.ok()?;
                            if !r.status().is_success() {
                                tracing::info!("paseo: {url} → {}", r.status());
                                return None;
                            }
                            let cuerpo = r.bytes().await.ok()?;
                            if cuerpo.len() < 512 {
                                return None;
                            }
                            let parte = dir.join(format!("{nombre}.m4a.part"));
                            std::fs::write(&parte, &cuerpo).ok()?;
                            std::fs::rename(&parte, dir.join(format!("{nombre}.m4a"))).ok()
                        }
                        .await;
                        if ok.is_some() {
                            bien.fetch_add(1, Ordering::SeqCst);
                        }
                        if let Some(v) = EN_VUELO.lock().unwrap().as_mut() {
                            v.remove(&nombre);
                        }
                    }
                }));
            }
            for o in obreros {
                let _ = o.await;
            }
            let bien = bien.load(Ordering::SeqCst);
            if total > 1 || bien < total {
                tracing::info!("paseo: bajados {bien}/{total} audios del espejo");
            }
        });
    }
    #[cfg(not(mobile))]
    {
        let _ = (app, frases, ESPEJO_VOZ, EMPAQUETADOS);
    }
}
