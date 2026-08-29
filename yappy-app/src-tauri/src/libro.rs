//! EL LIBRO VIVO: un audiolibro (.yappy) sonando por el reproductor de
//! fichero de iOS, ESPEJADO como si fuera una sesión de lectura. El truco
//! que lo unifica todo: el snapshot del libro se publica por el MISMO
//! contador de revisiones del PlaybackController, así que la aguja, el
//! lector con su karaoke, el chip y la pantalla de bloqueo pintan el
//! audiolibro sin distinguir motores. Los mandos (pausa, saltos, seek,
//! parar) se enrutan aquí desde los comandos comunes.
//!
//! Solo iOS: en el escritorio la biblioteca tiene su propio `<audio>`.

use std::sync::Mutex;

#[cfg(target_os = "ios")]
use anyhow::{Context, Result};
use tauri::AppHandle;
use tauri::Runtime;

use crate::state::AppState;
use crate::yappy_pack::TiempoFrase;

pub struct LibroVivo {
    pub path: String,
    pub titulo: String,
    pub tiempos: Vec<TiempoFrase>,
    pub duracion: f32,
    pub total_parrafos: usize,
}

static LIBRO: Mutex<Option<LibroVivo>> = Mutex::new(None);
#[cfg(target_os = "ios")]
static TICKER_VIVO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn activo() -> bool {
    LIBRO.lock().unwrap().is_some()
}

/// La frase de `tiempos` que suena en `pos` (o la última empezada).
#[cfg(any(target_os = "ios", test))]
fn frase_en(tiempos: &[TiempoFrase], pos: f32) -> Option<usize> {
    if tiempos.is_empty() {
        return None;
    }
    let mut actual = 0usize;
    for (i, t) in tiempos.iter().enumerate() {
        if t.ini_s <= pos {
            actual = i;
        } else {
            break;
        }
    }
    Some(actual)
}

/// Publica el estado del libro como snapshot de reproducción (revisión del
/// contador común del controller).
#[cfg(target_os = "ios")]
fn publicar(state: &AppState, pos: f32, sonando: bool) {
    let guard = LIBRO.lock().unwrap();
    let Some(l) = guard.as_ref() else { return };
    let frase = frase_en(&l.tiempos, pos);
    let (texto, parrafo, oi, of, f_ini, f_fin) = match frase.and_then(|i| l.tiempos.get(i)) {
        Some(t) => (
            t.texto.clone(),
            t.parrafo,
            t.origen_ini,
            t.origen_fin,
            t.ini_s,
            t.fin_s,
        ),
        None => (String::new(), 0, 0, 0, 0.0, 0.0),
    };
    let idx = frase.unwrap_or(0);
    let total = l.tiempos.len();
    let titulo = l.titulo.clone();
    let path = l.path.clone();
    let duracion = l.duracion;
    let total_parrafos = l.total_parrafos;
    drop(guard);
    state.playback.publicar_libro(move |s| {
        s.estado = if sonando { "sonando" } else { "pausa" }.into();
        s.titulo = titulo;
        s.doc_path = path;
        s.playing = true;
        s.paused = !sonando;
        s.current_text = texto;
        s.current_index = idx;
        s.current_paragraph_index = parrafo;
        s.current_origen_ini = oi;
        s.current_origen_fin = of;
        s.base_paragraph_index = 0;
        s.chunks_cocinados = total;
        s.parrafo_max_cocinado = total_parrafos.saturating_sub(1);
        s.total = total;
        s.total_paragraphs = total_parrafos;
        s.elapsed_secs = pos;
        s.duration_secs = duracion;
        s.frase_ini_s = f_ini;
        s.frase_fin_s = f_fin;
    });
}

/// Enciende el ESPEJO para un .yappy que ya está sonando por el reproductor
/// de fichero (cualquier camino: biblioteca, deep link, pegatina). Si ese
/// mismo libro ya está espejado, no hace nada.
#[cfg(target_os = "ios")]
pub fn espejar<R: Runtime>(app: &AppHandle<R>, path: &str) {
    use tauri::Manager;
    {
        let guard = LIBRO.lock().unwrap();
        if guard.as_ref().is_some_and(|l| l.path == path) {
            return;
        }
    }
    let ruta = std::path::Path::new(path);
    let Ok(manifiesto) = crate::yappy_pack::leer_manifiesto(ruta) else {
        tracing::warn!("libro: espejar no pudo leer el manifiesto de {path}");
        return;
    };
    let Ok(pack) = crate::yappy_pack::leer(ruta) else {
        tracing::warn!("libro: espejar no pudo leer {path}");
        return;
    };
    let total_parrafos = pack
        .tiempos
        .iter()
        .map(|t| t.parrafo + 1)
        .max()
        .unwrap_or(0);
    {
        let mut guard = LIBRO.lock().unwrap();
        *guard = Some(LibroVivo {
            path: path.to_string(),
            titulo: if manifiesto.titulo.trim().is_empty() {
                ruta.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Yappy")
                    .to_string()
            } else {
                manifiesto.titulo.clone()
            },
            tiempos: pack.tiempos,
            duracion: manifiesto.duracion_secs,
            total_parrafos,
        });
    }
    let state = app.state::<std::sync::Arc<AppState>>();
    state.playback.modo_libro(true);
    let (n, d) = {
        let g = LIBRO.lock().unwrap();
        g.as_ref()
            .map(|l| (l.tiempos.len(), l.duracion))
            .unwrap_or((0, 0.0))
    };
    tracing::info!("libro: espejo fijado para {path} ({n} frases, {d:.0}s)");
    publicar(&state, crate::mobile::audiofile_position() as f32, true);
    arrancar_ticker(app.clone());
}

/// Abre un .yappy como LIBRO VIVO: para la lectura de síntesis, carga el
/// texto como documento (para el lector), arranca el reproductor de fichero
/// y enciende el espejo. Devuelve el documento listo para `reader.doc`.
#[cfg(target_os = "ios")]
pub fn abrir(
    app: &AppHandle,
    state: &AppState,
    path: &str,
    from_start: bool,
    desde_parrafo: Option<usize>,
) -> Result<crate::state::CurrentDocument> {
    let ruta = std::path::Path::new(path);
    let pack = crate::yappy_pack::leer(ruta).context("leer .yappy")?;
    if pack.texto.trim().is_empty() || pack.tiempos.is_empty() {
        anyhow::bail!("este audiolibro no trae texto sincronizado");
    }

    // Una sola voz en la casa: la lectura de síntesis se calla.
    state.playback.stop();

    let ricos = crate::capture::doc_loader::parse_markdown_rhythm(&pack.texto);
    let doc = crate::state::CurrentDocument {
        path: path.to_string(),
        filename: if pack.manifiesto.titulo.trim().is_empty() {
            ruta.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Yappy")
                .to_string()
        } else {
            pack.manifiesto.titulo.clone()
        },
        extension: "yappy".into(),
        paragraphs: ricos.iter().map(|r| r.text.clone()).collect(),
        char_count: pack.texto.chars().count(),
        loading: false,
        paragraph_pauses: ricos.iter().map(|r| r.pause_before).collect(),
        paragraph_kinds: ricos.iter().map(|r| r.kind.clone()).collect(),
        paragraph_speed_mult: ricos.iter().map(|r| r.speed_mult).collect(),
    };

    // El reproductor de fichero primero (library_play_cmd llama a espejar,
    // que fija el LIBRO con los tiempos y arranca el ticker).
    crate::commands::library_play_cmd(app.clone(), path.to_string(), Some(from_start))
        .map_err(|e| anyhow::anyhow!(e))?;
    // «Leer desde aquí»: directo a la primera frase de ese párrafo, en el
    // backend y sin carreras con el espejo.
    if let Some(p) = desde_parrafo {
        let destino = {
            let guard = LIBRO.lock().unwrap();
            guard
                .as_ref()
                .and_then(|l| l.tiempos.iter().find(|t| t.parrafo >= p))
                .map(|t| t.ini_s as f64)
        };
        if let Some(d) = destino {
            seek_a(state, d);
        }
    }
    Ok(doc)
}

/// El ticker del espejo: mientras el libro viva, publica posición y frase
/// (solo cuando algo cambia: la pausa quieta no inunda el frontend).
#[cfg(target_os = "ios")]
fn arrancar_ticker<R: Runtime>(app: AppHandle<R>) {
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    if TICKER_VIVO.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("yappy-libro".into())
        .spawn(move || {
            let mut anterior: (i64, bool) = (-1, false);
            let mut latidos: u32 = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if !activo() {
                    break;
                }
                latidos += 1;
                if latidos % 10 == 1 {
                    tracing::info!(
                        "libro: tick pos={:.1} sonando={}",
                        crate::mobile::audiofile_position(),
                        crate::mobile::audiofile_is_playing()
                    );
                }
                let state = app.state::<std::sync::Arc<AppState>>();
                let pos = crate::mobile::audiofile_position() as f32;
                let sonando = crate::mobile::audiofile_is_playing();
                let dur = LIBRO
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|l| l.duracion)
                    .unwrap_or(0.0);
                // Fin natural: el player calló al borde del final.
                if !sonando && dur > 0.0 && pos >= dur - 0.75 {
                    parar(&app, &state);
                    break;
                }
                let clave = ((pos * 4.0) as i64, sonando);
                if clave != anterior {
                    anterior = clave;
                    publicar(&state, pos, sonando);
                }
            }
            TICKER_VIVO.store(false, Ordering::SeqCst);
        })
        .ok();
}

/// Apaga SOLO el espejo (el player ya está parado o lo para otro): libro
/// fuera, modo libro fuera, snapshot a «inactivo».
pub fn apagar_espejo<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Manager;
    let habia = LIBRO.lock().unwrap().take().is_some();
    if !habia {
        return;
    }
    let state = app.state::<std::sync::Arc<AppState>>();
    state.playback.modo_libro(false);
    state.playback.publicar_libro(|s| {
        s.estado = "inactivo".into();
        s.titulo.clear();
        s.doc_path.clear();
        s.playing = false;
        s.paused = false;
        s.current_text.clear();
        s.elapsed_secs = 0.0;
        s.duration_secs = 0.0;
        s.total = 0;
        s.chunks_cocinados = 0;
    });
}

/// Cierra el libro vivo (si lo hay): guarda la posición, para el reproductor
/// y apaga el espejo. Llamado también cuando una lectura de síntesis nueva
/// pide la voz.
pub fn parar<R: Runtime>(app: &AppHandle<R>, _state: &AppState) {
    if !activo() {
        return;
    }
    #[cfg(target_os = "ios")]
    {
        crate::commands::persistir_resume_libro(app);
        crate::mobile::audiofile_stop();
        crate::mobile::now_playing_set("", "", "", 0.0, 0.0, false);
    }
    apagar_espejo(app);
}

// ── Los mandos, enrutados desde los comandos comunes ────────────────────

#[cfg(target_os = "ios")]
pub fn pausa(state: &AppState) {
    crate::mobile::audiofile_pause();
    publicar(state, crate::mobile::audiofile_position() as f32, false);
}

#[cfg(target_os = "ios")]
pub fn reanuda(state: &AppState) {
    crate::mobile::audiofile_resume();
    publicar(state, crate::mobile::audiofile_position() as f32, true);
}

#[cfg(target_os = "ios")]
pub fn toggle(state: &AppState) {
    if crate::mobile::audiofile_is_playing() {
        pausa(state);
    } else {
        reanuda(state);
    }
}

#[cfg(target_os = "ios")]
pub fn seek_a(state: &AppState, secs: f64) {
    crate::mobile::audiofile_seek(secs.max(0.0));
    publicar(
        state,
        crate::mobile::audiofile_position() as f32,
        crate::mobile::audiofile_is_playing(),
    );
}

/// Salto por FRASE del karaoke: a la frase anterior o siguiente según los
/// tiempos del pack (convención musical: atrás con la frase empezada más
/// de 1,2 s vuelve al principio de la actual).
#[cfg(target_os = "ios")]
pub fn saltar_frase(state: &AppState, delta: i32) {
    let destino = {
        let guard = LIBRO.lock().unwrap();
        let Some(l) = guard.as_ref() else { return };
        let pos = crate::mobile::audiofile_position() as f32;
        let Some(actual) = frase_en(&l.tiempos, pos) else {
            return;
        };
        let dentro = pos - l.tiempos[actual].ini_s;
        let destino = if delta < 0 && dentro > 1.2 {
            actual
        } else {
            actual.saturating_add_signed(delta as isize)
        }
        .min(l.tiempos.len().saturating_sub(1));
        l.tiempos[destino].ini_s as f64
    };
    seek_a(state, destino);
}

/// Salto por PÁRRAFO: a la primera frase del párrafo vecino.
#[cfg(target_os = "ios")]
pub fn saltar_parrafo(state: &AppState, delta: i32) {
    let destino = {
        let guard = LIBRO.lock().unwrap();
        let Some(l) = guard.as_ref() else { return };
        let pos = crate::mobile::audiofile_position() as f32;
        let Some(actual) = frase_en(&l.tiempos, pos) else {
            return;
        };
        let parrafo_actual = l.tiempos[actual].parrafo as i64;
        let max = l.tiempos.iter().map(|t| t.parrafo).max().unwrap_or(0) as i64;
        let objetivo = (parrafo_actual + delta as i64).clamp(0, max) as usize;
        l.tiempos
            .iter()
            .find(|t| t.parrafo == objetivo)
            .map(|t| t.ini_s as f64)
    };
    if let Some(d) = destino {
        seek_a(state, d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(ini: f32, parrafo: usize) -> TiempoFrase {
        TiempoFrase {
            ini_s: ini,
            fin_s: ini + 1.5,
            parrafo,
            origen_ini: 0,
            origen_fin: 10,
            texto: format!("frase {ini}"),
        }
    }

    #[test]
    fn la_frase_en_curso_se_encuentra() {
        let ts = vec![t(0.0, 0), t(2.0, 0), t(4.0, 1), t(7.5, 2)];
        assert_eq!(frase_en(&ts, 0.0), Some(0));
        assert_eq!(frase_en(&ts, 1.9), Some(0));
        assert_eq!(frase_en(&ts, 2.0), Some(1));
        assert_eq!(frase_en(&ts, 5.0), Some(2));
        assert_eq!(frase_en(&ts, 99.0), Some(3));
        assert_eq!(frase_en(&[], 1.0), None);
    }
}
