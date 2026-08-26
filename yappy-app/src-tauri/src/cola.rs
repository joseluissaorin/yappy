//! La cola: la puerta de entrada de Yappy en el móvil (y el escritorio).
//!
//! Todo lo que entra (una URL compartida, un vídeo de YouTube, un PDF, un
//! texto, una nota de voz) se convierte en un item de la cola con estado
//! visible: pendiente → preparando → listo (o error, con su porqué y botón
//! de reintento). «Preparar» significa extraer un documento legible: el
//! artículo limpio, la transcripción del vídeo, el archivo copiado a casa.
//! Escucharlo es abrir ese documento por el camino normal de lectura.
//!
//! Persistencia: {app_data}/cola/cola.json + un fichero por item extraído
//! ({app_data}/cola/{id}.md). Cada cambio emite `cola_actualizada`.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TipoItem {
    Url,
    Youtube,
    Archivo,
    Texto,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EstadoItem {
    Pendiente,
    Preparando,
    Listo,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCola {
    pub id: String,
    pub tipo: TipoItem,
    pub titulo: String,
    /// La URL o el nombre de archivo original: lo que enseña la ficha.
    pub origen: String,
    /// Ruta local del documento legible (cuando está listo).
    pub ruta: Option<String>,
    pub estado: EstadoItem,
    pub error: Option<String>,
    pub agregado_unix: u64,
    /// Tamaño del texto extraído, para estimar duración en la ficha.
    pub chars: Option<usize>,
}

fn ahora_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn nuevo_id() -> String {
    // Sin dependencia de uuid: tiempo + contador atómico basta y es legible.
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!("{}-{:04}", ahora_unix(), N.fetch_add(1, Ordering::Relaxed) % 10000)
}

fn dir_cola<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .context("app_data_dir")?
        .join("cola");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn ruta_indice<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    Ok(dir_cola(app)?.join("cola.json"))
}

static CANDADO: Mutex<()> = Mutex::new(());

pub fn listar<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<ItemCola>> {
    let _g = CANDADO.lock().unwrap();
    listar_sin_candado(app)
}

fn listar_sin_candado<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<ItemCola>> {
    let ruta = ruta_indice(app)?;
    if !ruta.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(&ruta)?;
    Ok(serde_json::from_str(&json).unwrap_or_default())
}

fn guardar<R: Runtime>(app: &AppHandle<R>, items: &[ItemCola]) -> Result<()> {
    let ruta = ruta_indice(app)?;
    let tmp = ruta.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(items)?)?;
    fs::rename(&tmp, &ruta)?;
    Ok(())
}

fn emitir<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("cola_actualizada", ());
}

fn mutar<R: Runtime>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut Vec<ItemCola>),
) -> Result<Vec<ItemCola>> {
    let _g = CANDADO.lock().unwrap();
    let mut items = listar_sin_candado(app)?;
    f(&mut items);
    guardar(app, &items)?;
    drop(_g);
    emitir(app);
    Ok(items)
}

fn actualizar_item<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    f: impl FnOnce(&mut ItemCola),
) -> Result<()> {
    mutar(app, |items| {
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            f(item);
        }
    })?;
    Ok(())
}

// ── Alta de items ────────────────────────────────────────────────────────

fn es_youtube(url: &str) -> bool {
    url.contains("youtube.com/watch")
        || url.contains("youtube.com/shorts")
        || url.contains("youtu.be/")
        || url.contains("m.youtube.com/watch")
}

fn id_video_youtube(url: &str) -> Option<String> {
    if let Some(resto) = url.split("youtu.be/").nth(1) {
        return Some(resto.split(['?', '&', '/']).next()?.to_string());
    }
    if let Some(resto) = url.split("shorts/").nth(1) {
        return Some(resto.split(['?', '&', '/']).next()?.to_string());
    }
    url.split("v=")
        .nth(1)
        .map(|s| s.split('&').next().unwrap_or(s).to_string())
}

pub fn agregar_url<R: Runtime>(app: &AppHandle<R>, url: String) -> Result<ItemCola> {
    // La misma URL compartida dos veces (el doble toque, el reintento del
    // usuario impaciente) no duplica el segmento: se devuelve el que ya
    // está en la cinta, salvo que aquel muriera en error.
    if let Ok(items) = listar(app) {
        if let Some(existente) = items
            .iter()
            .find(|i| i.origen == url && i.estado != EstadoItem::Error)
        {
            return Ok(existente.clone());
        }
    }
    let tipo = if es_youtube(&url) { TipoItem::Youtube } else { TipoItem::Url };
    let item = ItemCola {
        id: nuevo_id(),
        tipo,
        titulo: url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .chars()
            .take(80)
            .collect(),
        origen: url,
        ruta: None,
        estado: EstadoItem::Pendiente,
        error: None,
        agregado_unix: ahora_unix(),
        chars: None,
    };
    let clon = item.clone();
    mutar(app, |items| items.insert(0, item))?;
    procesar_en_segundo_plano(app.clone(), clon.id.clone());
    Ok(clon)
}

pub fn agregar_texto<R: Runtime>(
    app: &AppHandle<R>,
    texto: String,
    titulo: Option<String>,
) -> Result<ItemCola> {
    let id = nuevo_id();
    let ruta = dir_cola(app)?.join(format!("{id}.md"));
    fs::write(&ruta, &texto)?;
    let titulo = titulo.unwrap_or_else(|| {
        texto
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("texto")
            .trim_start_matches('#')
            .trim()
            .chars()
            .take(60)
            .collect()
    });
    let item = ItemCola {
        id,
        tipo: TipoItem::Texto,
        titulo,
        origen: "texto compartido".into(),
        ruta: Some(ruta.to_string_lossy().to_string()),
        estado: EstadoItem::Listo,
        error: None,
        agregado_unix: ahora_unix(),
        chars: Some(texto.chars().count()),
    };
    let clon = item.clone();
    mutar(app, |items| items.insert(0, item))?;
    Ok(clon)
}

/// Un archivo (PDF, EPUB, DOCX, MD…) ya accesible en disco: se copia a la
/// cola para que sobreviva aunque el original desaparezca (los ficheros
/// del App Group del share sheet se limpian).
pub fn agregar_archivo<R: Runtime>(app: &AppHandle<R>, ruta_original: String) -> Result<ItemCola> {
    let origen = PathBuf::from(&ruta_original);
    let nombre = origen
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".into());
    let id = nuevo_id();
    let ext = origen
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "bin".into());
    let destino = dir_cola(app)?.join(format!("{id}.{ext}"));
    fs::copy(&origen, &destino).with_context(|| format!("copiando {ruta_original}"))?;
    let item = ItemCola {
        id,
        tipo: TipoItem::Archivo,
        titulo: nombre.clone(),
        origen: nombre,
        ruta: Some(destino.to_string_lossy().to_string()),
        estado: EstadoItem::Listo,
        error: None,
        agregado_unix: ahora_unix(),
        chars: None,
    };
    let clon = item.clone();
    mutar(app, |items| items.insert(0, item))?;
    Ok(clon)
}

pub fn agregar_audio<R: Runtime>(app: &AppHandle<R>, ruta_original: String) -> Result<ItemCola> {
    let origen = PathBuf::from(&ruta_original);
    let nombre = origen
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".into());
    let id = nuevo_id();
    let ext = origen
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "wav".into());
    let destino = dir_cola(app)?.join(format!("{id}.{ext}"));
    fs::copy(&origen, &destino).with_context(|| format!("copiando {ruta_original}"))?;
    let item = ItemCola {
        id,
        tipo: TipoItem::Audio,
        titulo: nombre.clone(),
        origen: nombre,
        ruta: Some(destino.to_string_lossy().to_string()),
        estado: EstadoItem::Pendiente,
        error: None,
        agregado_unix: ahora_unix(),
        chars: None,
    };
    let clon = item.clone();
    mutar(app, |items| items.insert(0, item))?;
    Ok(clon)
}

pub fn eliminar<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<()> {
    let dir = dir_cola(app)?;
    mutar(app, |items| {
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            let item = items.remove(pos);
            if let Some(r) = item.ruta {
                let p = PathBuf::from(r);
                if p.starts_with(&dir) {
                    let _ = fs::remove_file(p);
                }
            }
        }
    })?;
    Ok(())
}

// ── Preparación (extracción) ─────────────────────────────────────────────

pub fn procesar_en_segundo_plano<R: Runtime>(app: AppHandle<R>, id: String) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = procesar(&app, &id).await {
            tracing::warn!("cola: {id} falló: {e:#}");
            let _ = actualizar_item(&app, &id, |it| {
                it.estado = EstadoItem::Error;
                it.error = Some(formatear_error(&e));
            });
        }
    });
}

/// Errores para personas, no trazas: la ficha de la cola los enseña tal cual.
fn formatear_error(e: &anyhow::Error) -> String {
    let s = e.to_string();
    if s.contains("dns") || s.contains("connect") || s.contains("timed out") {
        "sin conexión, o el sitio no responde".to_string()
    } else {
        s.chars().take(200).collect()
    }
}

async fn procesar<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<()> {
    let item = listar(app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| anyhow!("item desaparecido"))?;
    if item.estado == EstadoItem::Listo {
        return Ok(());
    }
    actualizar_item(app, id, |it| {
        it.estado = EstadoItem::Preparando;
        it.error = None;
    })?;

    match item.tipo {
        TipoItem::Url => preparar_articulo(app, &item).await,
        TipoItem::Youtube => preparar_youtube(app, &item).await,
        TipoItem::Audio => preparar_transcripcion(app, &item).await,
        TipoItem::Archivo | TipoItem::Texto => Ok(()),
    }
}

async fn preparar_articulo<R: Runtime>(app: &AppHandle<R>, item: &ItemCola) -> Result<()> {
    let cliente = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(25))
        .build()?;
    let html = cliente
        .get(&item.origen)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let (titulo, markdown) = extraer_articulo(&html, &item.origen)?;
    terminar_con_markdown(app, &item.id, titulo, markdown)
}

/// Readability puro (dom_smoothie) con salida markdown: conserva títulos,
/// listas y citas para que el guionizador les dé su ritmo.
pub fn extraer_articulo(html: &str, url: &str) -> Result<(Option<String>, String)> {
    use dom_smoothie::{Config, Readability, TextMode};
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };
    let mut r = Readability::new(html, Some(url), Some(cfg))
        .map_err(|e| anyhow!("readability: {e:?}"))?;
    let articulo = r.parse().map_err(|e| anyhow!("no parece un artículo legible ({e:?})"))?;
    let titulo = {
        let t = articulo.title.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    // dom_smoothie escapa puntuación al estilo markdown («N\.º», «\(2\)»);
    // para LEER EN VOZ ALTA queremos el texto llano.
    let cuerpo = desescapar_markdown(articulo.text_content.trim());
    if cuerpo.chars().count() < 80 {
        return Err(anyhow!("el artículo quedó vacío tras limpiarlo"));
    }
    let markdown = match &titulo {
        Some(t) if !cuerpo.starts_with(&format!("# {t}")) => format!("# {t}\n\n{cuerpo}"),
        _ => cuerpo,
    };
    Ok((titulo, markdown))
}

/// Quita los escapes de markdown que no aportan nada hablado.
fn desescapar_markdown(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut cs = s.chars().peekable();
    while let Some(c) = cs.next() {
        if c == '\\' {
            if let Some(sig) = cs.peek() {
                if "\\`*_{}[]()#+-.!|<>~".contains(*sig) {
                    out.push(*sig);
                    cs.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

#[cfg(target_os = "android")]
async fn preparar_youtube<R: Runtime>(_app: &AppHandle<R>, _item: &ItemCola) -> Result<()> {
    Err(anyhow!("los subtítulos de YouTube llegan pronto a Android; de momento comparte artículos, documentos o audio"))
}

#[cfg(not(target_os = "android"))]
async fn preparar_youtube<R: Runtime>(app: &AppHandle<R>, item: &ItemCola) -> Result<()> {
    let video = id_video_youtube(&item.origen)
        .ok_or_else(|| anyhow!("no reconozco el identificador del vídeo"))?;
    let api = yt_transcript_rs::api::YouTubeTranscriptApi::new(None, None, None)
        .map_err(|e| anyhow!("youtube: {e}"))?;
    // Preferencia: el idioma del usuario, luego inglés, luego lo que haya.
    let pref = {
        let estado = app.state::<std::sync::Arc<crate::state::AppState>>();
        let s = estado.settings.lock().unwrap();
        s.default_lang.clone()
    };
    let idiomas: Vec<&str> = vec![pref.as_str(), "en", "es"];
    let transcripcion = api
        .fetch_transcript(&video, &idiomas, false)
        .await
        .map_err(|e| anyhow!("este vídeo no tiene transcripción disponible ({e})"))?;

    // Los subtítulos vienen en frases cortas; se agrupan en párrafos de
    // ~4 frases para que la lectura respire como prosa.
    let mut parrafos: Vec<String> = Vec::new();
    let mut actual = String::new();
    let mut frases = 0usize;
    for parte in transcripcion.parts() {
        let t = parte.text.trim();
        if t.is_empty() {
            continue;
        }
        if !actual.is_empty() {
            actual.push(' ');
        }
        actual.push_str(t);
        frases += 1;
        let corta_por_frases =
            frases >= 4 && (t.ends_with('.') || t.ends_with('?') || t.ends_with('!'));
        if corta_por_frases || actual.chars().count() > 700 {
            parrafos.push(std::mem::take(&mut actual));
            frases = 0;
        }
    }
    if !actual.trim().is_empty() {
        parrafos.push(actual);
    }
    if parrafos.is_empty() {
        return Err(anyhow!("la transcripción llegó vacía"));
    }
    let markdown = parrafos.join("\n\n");
    terminar_con_markdown(app, &item.id, None, markdown)
}

async fn preparar_transcripcion<R: Runtime>(app: &AppHandle<R>, item: &ItemCola) -> Result<()> {
    let ruta = item
        .ruta
        .clone()
        .ok_or_else(|| anyhow!("el audio no llegó a copiarse"))?;
    let texto = crate::commands::transcribir_para_cola(app, &ruta).await?;
    if texto.trim().is_empty() {
        return Err(anyhow!("no se oyó nada transcribible"));
    }
    terminar_con_markdown(app, &item.id, None, texto)
}

fn terminar_con_markdown<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    titulo: Option<String>,
    markdown: String,
) -> Result<()> {
    let ruta = dir_cola(app)?.join(format!("{id}.md"));
    fs::write(&ruta, &markdown)?;
    let chars = markdown.chars().count();
    actualizar_item(app, id, |it| {
        it.estado = EstadoItem::Listo;
        it.ruta = Some(ruta.to_string_lossy().to_string());
        it.chars = Some(chars);
        if let Some(t) = titulo {
            it.titulo = t;
        } else if let Some(primera) = markdown.lines().find(|l| !l.trim().is_empty()) {
            let limpio: String = primera.trim_start_matches('#').trim().chars().take(60).collect();
            if !limpio.is_empty() {
                it.titulo = limpio;
            }
        }
    })
}

// ── Comandos Tauri ───────────────────────────────────────────────────────

#[tauri::command]
pub fn cola_listar_cmd(app: AppHandle) -> Result<Vec<ItemCola>, String> {
    listar(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cola_agregar_url_cmd(app: AppHandle, url: String) -> Result<ItemCola, String> {
    agregar_url(&app, url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cola_agregar_texto_cmd(
    app: AppHandle,
    texto: String,
    titulo: Option<String>,
) -> Result<ItemCola, String> {
    agregar_texto(&app, texto, titulo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cola_agregar_archivo_cmd(app: AppHandle, ruta: String) -> Result<ItemCola, String> {
    agregar_archivo(&app, ruta).map_err(|e| e.to_string())
}

/// «Pegar lo copiado»: lee el portapapeles EL SOLO (nada de pelear con el
/// menú de pegar del sistema) y lo añade a la cinta. Si lo copiado es una
/// URL, entra por el camino de artículo.
#[tauri::command]
pub fn cola_agregar_portapapeles_cmd(app: AppHandle) -> Result<ItemCola, String> {
    let texto = crate::capture::clipboard::read_text()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let texto = texto.trim().to_string();
    if texto.is_empty() {
        return Err("portapapeles vacío".into());
    }
    let es_url = texto.starts_with("http://") || texto.starts_with("https://");
    if es_url && !texto.contains(char::is_whitespace) {
        agregar_url(&app, texto).map_err(|e| e.to_string())
    } else {
        agregar_texto(&app, texto, None).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn cola_agregar_audio_cmd(app: AppHandle, ruta: String) -> Result<ItemCola, String> {
    let item = agregar_audio(&app, ruta).map_err(|e| e.to_string())?;
    procesar_en_segundo_plano(app, item.id.clone());
    Ok(item)
}

#[tauri::command]
pub fn cola_eliminar_cmd(app: AppHandle, id: String) -> Result<(), String> {
    eliminar(&app, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cola_reintentar_cmd(app: AppHandle, id: String) -> Result<(), String> {
    let _ = actualizar_item(&app, &id, |it| {
        it.estado = EstadoItem::Pendiente;
        it.error = None;
    });
    procesar_en_segundo_plano(app, id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraer_articulo_conserva_estructura() {
        let html = r#"<!doctype html><html><head><title>El faro</title></head><body>
          <nav>menú menú menú</nav>
          <article>
            <h1>El faro del fin del mundo</h1>
            <p>Durante el siglo XIX, los fareros vivían aislados durante meses.
            Las tormentas del Atlántico Sur golpeaban la torre con una violencia
            que hoy cuesta imaginar, y sin embargo la luz no se apagó ni una sola
            noche en cuarenta años de servicio continuado.</p>
            <h2>La rutina</h2>
            <p>Cada tarde subían los ciento veinte escalones con el aceite al
            hombro, limpiaban la óptica, anotaban el estado del mar en el diario
            y encendían la llama justo antes de que el sol tocara el agua.</p>
          </article>
          <footer>pie de página con enlaces</footer>
        </body></html>"#;
        let (titulo, md) = extraer_articulo(html, "https://example.com/faro").unwrap();
        assert!(titulo.is_some());
        assert!(md.contains("faro"), "cuerpo: {md}");
        assert!(md.contains("ciento veinte escalones"));
        assert!(!md.contains("pie de página"), "el footer debe caer: {md}");
    }

    #[test]
    fn youtube_ids() {
        assert_eq!(id_video_youtube("https://youtu.be/dQw4w9WgXcQ?t=1").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(id_video_youtube("https://www.youtube.com/watch?v=abc123&x=1").as_deref(), Some("abc123"));
        assert!(es_youtube("https://m.youtube.com/watch?v=x"));
        assert!(!es_youtube("https://example.com/watch"));
    }
}
