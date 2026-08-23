//! Router de deep links `yappy://`.
//!
//! Hasta ahora el esquema estaba declarado pero NADIE lo procesaba: las
//! Quick Actions, el widget y Spotlight solo abrían la app y ahí moría el
//! gesto. Este módulo convierte cada URL en una acción de verdad.
//!
//! Formas entendidas:
//!   yappy://shared                  → llegó algo del Share Sheet (drenar)
//!   yappy://action/read-clipboard   → leer el portapapeles ya
//!   yappy://action/resume           → reanudar/alternar reproducción
//!   yappy://action/open             → abrir el selector de documento
//!   yappy://library?path=…          → reproducir ese audiolibro (Spotlight)
//!   yappy://pair?nodo=…&token=…     → emparejar con un ordenador (el puente)

use tauri::{AppHandle, Emitter, Manager, Runtime};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Accion {
    pub tipo: String,
    pub path: Option<String>,
    pub datos: Option<String>,
}

pub fn manejar<R: Runtime>(app: &AppHandle<R>, url: &str) {
    tracing::info!("deep link: {url}");
    let sin_esquema = url
        .trim_start_matches("yappy://")
        .trim_start_matches("yappy:");
    let (ruta, query) = match sin_esquema.split_once('?') {
        Some((r, q)) => (r.trim_end_matches('/'), Some(q)),
        None => (sin_esquema.trim_end_matches('/'), None),
    };
    let parametro = |clave: &str| -> Option<String> {
        query.and_then(|q| {
            q.split('&').find_map(|par| {
                let (k, v) = par.split_once('=')?;
                (k == clave).then(|| urlencoding_decode(v))
            })
        })
    };

    let accion = match ruta {
        "shared" => Accion { tipo: "shared".into(), path: None, datos: None },
        "action/read-clipboard" => Accion { tipo: "read-clipboard".into(), path: None, datos: None },
        "action/resume" => Accion { tipo: "resume".into(), path: None, datos: None },
        "action/open" => Accion { tipo: "open".into(), path: None, datos: None },
        "library" => Accion { tipo: "library".into(), path: parametro("path"), datos: None },
        "pair" => Accion {
            tipo: "pair".into(),
            path: None,
            datos: query.map(|q| q.to_string()),
        },
        otro => Accion { tipo: otro.to_string(), path: None, datos: None },
    };

    // Reanudar es tan común (widget, pantalla de bloqueo) que se resuelve
    // aquí mismo, sin pasar por el webview.
    if accion.tipo == "resume" {
        if let Some(estado) = app.try_state::<std::sync::Arc<crate::state::AppState>>() {
            let snap = estado.playback.snapshot();
            if snap.paused {
                estado.playback.resume();
                return;
            }
        }
    }

    let _ = app.emit("yappy_accion", accion);
}

fn urlencoding_decode(s: &str) -> String {
    // Decodificación percent mínima (sin dependencia): suficiente para
    // rutas de fichero y tokens.
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 && i + 2 <= bytes.len() - 1 + 1 {
            if let (Some(a), Some(b)) = (
                (bytes.get(i + 1).copied()).and_then(|c| (c as char).to_digit(16)),
                (bytes.get(i + 2).copied()).and_then(|c| (c as char).to_digit(16)),
            ) {
                out.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
