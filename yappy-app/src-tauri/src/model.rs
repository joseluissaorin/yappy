//! Las voces (Supertonic 3): dónde viven, cómo se descargan y cómo se sabe
//! que están enteras.
//!
//! Cuatro palancas contra el muro de la descarga (septiembre de 2026):
//!   1. En iOS las voces LLEGAN CON LA INSTALACIÓN (Background Assets): la
//!      extensión YappyRecursos las coloca en el App Group, y por eso ahí
//!      está la raíz del modelo en el teléfono.
//!   2. La VARIANTE: pesos en fp16 (la mitad) en los teléfonos; fp32 en el
//!      escritorio, donde la descarga duele menos y la CPU x86 no tiene
//!      aritmética de media precisión.
//!   3. El ESPEJO propio (R2, tras Cloudflare) con las cuatro descargas EN
//!      PARALELO y reanudables por rangos; Hugging Face queda de reserva
//!      (solo tiene fp32).
//!   4. Arrancar sola en la primera apertura si la red es barata (lib.rs).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Los ficheros ONNX del modelo (en ambas variantes se llaman igual).
pub const REQUIRED_ONNX: &[&str] = &[
    "duration_predictor.onnx",
    "text_encoder.onnx",
    "vector_estimator.onnx",
    "vocoder.onnx",
];
pub const REQUIRED_AUX: &[&str] = &["tts.json", "unicode_indexer.json"];
pub const VOICE_STYLE_FILES: &[&str] = &[
    "M1.json", "M2.json", "M3.json", "M4.json", "M5.json", "F1.json", "F2.json", "F3.json",
    "F4.json", "F5.json",
];

/// El espejo de la casa (R2 con dominio propio) y la reserva.
pub const ESPEJO: &str = "https://modelos.yappy.joseluissaorin.com/supertonic-3";
const HF_BASE: &str = "https://huggingface.co/Supertone/supertonic-3/resolve/main";

/// La variante de pesos que descarga esta plataforma.
pub fn variante() -> &'static str {
    if let Ok(v) = std::env::var("YAPPY_VARIANTE_MODELO") {
        if v == "fp16" || v == "fp32" {
            return if v == "fp16" { "fp16" } else { "fp32" };
        }
    }
    if cfg!(any(target_os = "ios", target_os = "android")) {
        "fp16"
    } else {
        "fp32"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub file: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub stage: String,
    pub overall_done: u64,
    pub overall_total: u64,
}

/// El contenedor compartido del App Group (iOS): donde deja las voces la
/// extensión de la instalación. None fuera de iOS o si no hay grupo.
#[cfg(target_os = "ios")]
fn app_group_dir() -> Option<PathBuf> {
    crate::mobile::app_group_path().map(PathBuf::from)
}
#[cfg(not(target_os = "ios"))]
fn app_group_dir() -> Option<PathBuf> {
    None
}

pub fn model_root(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<PathBuf> {
    // iOS: la raíz vive en el App Group (compartida con YappyRecursos). Si
    // una instalación vieja tenía las voces en el contenedor privado, se
    // mudan de una vez (mismo volumen: es un rename).
    if let Some(grupo) = app_group_dir() {
        let nueva = grupo.join("models").join("supertonic-3");
        let vieja = handle
            .path()
            .app_data_dir()
            .ok()
            .map(|d| d.join("models").join("supertonic-3"));
        if let Some(vieja) = vieja {
            if vieja.join("onnx").exists() && !nueva.join("onnx").exists() {
                let _ = std::fs::create_dir_all(nueva.parent().unwrap_or(&grupo));
                match std::fs::rename(&vieja, &nueva) {
                    Ok(()) => tracing::info!("voces mudadas al App Group"),
                    Err(e) => tracing::warn!("no pude mudar las voces al App Group: {e}"),
                }
            }
        }
        std::fs::create_dir_all(&nueva)?;
        return Ok(nueva);
    }
    let mut p = handle.path().app_data_dir()?;
    std::fs::create_dir_all(&p)?;
    p.push("models");
    p.push("supertonic-3");
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn is_model_ready(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<bool> {
    let root = model_root(handle)?;
    for name in REQUIRED_ONNX {
        let p = root.join("onnx").join(name);
        if !p.exists() || std::fs::metadata(&p)?.len() < 100_000 {
            return Ok(false);
        }
    }
    for aux in REQUIRED_AUX {
        if !root.join("onnx").join(aux).exists() {
            return Ok(false);
        }
    }
    for vs in VOICE_STYLE_FILES {
        if !root.join("voice_styles").join(vs).exists() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Una descarga pendiente: de dónde (espejo primero, reserva después) y a
/// dónde. La reserva de Hugging Face solo existe para fp32.
struct Pendiente {
    remoto: String,
    urls: Vec<String>,
    local: PathBuf,
}

fn pendientes(root: &std::path::Path) -> Vec<Pendiente> {
    let v = variante();
    let mut out = Vec::new();
    let hf = |rel: &str| format!("{HF_BASE}/{rel}");
    for name in REQUIRED_ONNX {
        let local = root.join("onnx").join(name);
        let mut urls = vec![format!("{ESPEJO}/{v}/onnx/{name}")];
        if v == "fp32" {
            urls.push(hf(&format!("onnx/{name}")));
        } else {
            // Sin espejo, mejor fp32 de la reserva que nada.
            urls.push(format!("{ESPEJO}/fp32/onnx/{name}"));
            urls.push(hf(&format!("onnx/{name}")));
        }
        out.push(Pendiente {
            remoto: format!("onnx/{name}"),
            urls,
            local,
        });
    }
    for aux in REQUIRED_AUX {
        out.push(Pendiente {
            remoto: format!("onnx/{aux}"),
            urls: vec![format!("{ESPEJO}/onnx/{aux}"), hf(&format!("onnx/{aux}"))],
            local: root.join("onnx").join(aux),
        });
    }
    for vs in VOICE_STYLE_FILES {
        out.push(Pendiente {
            remoto: format!("voice_styles/{vs}"),
            urls: vec![
                format!("{ESPEJO}/voice_styles/{vs}"),
                hf(&format!("voice_styles/{vs}")),
            ],
            local: root.join("voice_styles").join(vs),
        });
    }
    out
}

fn ya_esta(p: &std::path::Path) -> bool {
    p.exists() && std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) > 100
}

/// Tamaño remoto (HEAD) probando las URL en orden. None si nadie contesta.
async fn tamano_remoto(client: &reqwest::Client, urls: &[String]) -> Option<(String, u64)> {
    for u in urls {
        if let Ok(r) = client.head(u).send().await {
            if r.status().is_success() {
                if let Some(n) = r.content_length() {
                    return Some((u.clone(), n));
                }
            }
        }
    }
    None
}

/// Descarga UN fichero: reanuda el `.part` por rangos si lo hay, y si la URL
/// preferida falla prueba la siguiente (empezando de cero, por si el otro
/// origen sirve un fichero distinto).
async fn bajar_uno(
    client: &reqwest::Client,
    p: &Pendiente,
    hecho_total: Arc<AtomicU64>,
    emit: Arc<dyn Fn(DownloadProgress) + Send + Sync>,
    overall_total: u64,
) -> Result<u64> {
    use futures_util::StreamExt;
    let tmp = p.local.with_extension("part");
    let mut ultimo_error: Option<anyhow::Error> = None;
    for (i, url) in p.urls.iter().enumerate() {
        // Solo el primer origen reanuda; los de reserva empiezan limpios.
        let ya: u64 = if i == 0 {
            std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0)
        } else {
            let _ = std::fs::remove_file(&tmp);
            0
        };
        let mut req = client.get(url);
        if ya > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={ya}-"));
        }
        let resp = match req.send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("descarga {url}: {e}");
                ultimo_error = Some(anyhow!(e));
                continue;
            }
        };
        let reanuda = ya > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let mut file = if reanuda {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&tmp)
                .await?
        } else {
            tokio::fs::File::create(&tmp).await?
        };
        let base = if reanuda { ya } else { 0 };
        let bytes_total = resp.content_length().unwrap_or(0) + base;
        let mut bytes_done = base;
        let mut contado = base;
        if !reanuda && ya > 0 {
            // El servidor no reanudó: lo ya contado en el total global sobra.
            hecho_total.fetch_sub(ya.min(hecho_total.load(Ordering::SeqCst)), Ordering::SeqCst);
        }
        let mut stream = resp.bytes_stream();
        let mut fallo = None;
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    fallo = Some(anyhow!(e));
                    break;
                }
            };
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            bytes_done += chunk.len() as u64;
            let overall =
                hecho_total.fetch_add(chunk.len() as u64, Ordering::SeqCst) + chunk.len() as u64;
            contado = bytes_done;
            if bytes_done % (256 * 1024) < chunk.len() as u64 {
                emit(DownloadProgress {
                    file: p.remoto.clone(),
                    bytes_done,
                    bytes_total,
                    stage: "downloading".into(),
                    overall_done: overall,
                    overall_total,
                });
            }
        }
        drop(file);
        if let Some(e) = fallo {
            tracing::warn!("descarga {url} cortada en {contado} bytes: {e}");
            ultimo_error = Some(e);
            continue;
        }
        tokio::fs::rename(&tmp, &p.local).await?;
        emit(DownloadProgress {
            file: p.remoto.clone(),
            bytes_done,
            bytes_total,
            stage: "done".into(),
            overall_done: hecho_total.load(Ordering::SeqCst),
            overall_total,
        });
        return Ok(bytes_done);
    }
    Err(ultimo_error.unwrap_or_else(|| anyhow!("sin origen para {}", p.remoto)))
}

/// Descarga todo lo que falte del modelo, en paralelo, con progreso global.
pub async fn download_model(
    handle: &tauri::AppHandle<impl tauri::Runtime>,
    emit: impl Fn(DownloadProgress) + Send + Sync + 'static,
) -> Result<()> {
    let root = model_root(handle)?;
    std::fs::create_dir_all(root.join("onnx"))?;
    std::fs::create_dir_all(root.join("voice_styles"))?;
    let emit: Arc<dyn Fn(DownloadProgress) + Send + Sync> = Arc::new(emit);

    let client = reqwest::Client::builder()
        .user_agent("Yappy/0.3 (https://yappy.joseluissaorin.com)")
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()?;

    let faltan: Vec<Pendiente> = pendientes(&root)
        .into_iter()
        .filter(|p| !ya_esta(&p.local))
        .collect();
    if faltan.is_empty() {
        return Ok(());
    }
    tracing::info!(
        "voces: faltan {} ficheros (variante {}), origen {}",
        faltan.len(),
        variante(),
        ESPEJO
    );

    // El total, preguntando tamaños en paralelo (lo ya bajado a medias
    // cuenta como hecho).
    let tamanos =
        futures::future::join_all(faltan.iter().map(|p| tamano_remoto(&client, &p.urls))).await;
    let overall_total: u64 = tamanos
        .iter()
        .map(|t| t.as_ref().map(|(_, n)| *n).unwrap_or(0))
        .sum::<u64>()
        .max(1);
    let ya_parciales: u64 = faltan
        .iter()
        .map(|p| {
            std::fs::metadata(p.local.with_extension("part"))
                .map(|m| m.len())
                .unwrap_or(0)
        })
        .sum();
    let hecho_total = Arc::new(AtomicU64::new(ya_parciales));
    emit(DownloadProgress {
        file: String::new(),
        bytes_done: 0,
        bytes_total: 0,
        stage: "start".into(),
        overall_done: ya_parciales,
        overall_total,
    });

    // Cuatro a la vez: los ONNX grandes en paralelo saturan la línea; los
    // json pequeños pasan entre medias.
    let sem = Arc::new(tokio::sync::Semaphore::new(4));
    let tareas = faltan.iter().map(|p| {
        let sem = sem.clone();
        let client = client.clone();
        let hecho = hecho_total.clone();
        let emit = emit.clone();
        async move {
            let _permiso = sem.acquire().await.expect("semáforo");
            bajar_uno(&client, p, hecho, emit, overall_total)
                .await
                .with_context(|| format!("descargando {}", p.remoto))
        }
    });
    let resultados = futures::future::join_all(tareas).await;
    let mut errores = Vec::new();
    for r in resultados {
        if let Err(e) = r {
            errores.push(format!("{e:#}"));
        }
    }
    if !errores.is_empty() {
        return Err(anyhow!("faltaron ficheros: {}", errores.join(" · ")));
    }
    if !is_model_ready(handle)? {
        return Err(anyhow!("model files missing after download"));
    }
    tracing::info!("voces completas ({})", variante());
    Ok(())
}
