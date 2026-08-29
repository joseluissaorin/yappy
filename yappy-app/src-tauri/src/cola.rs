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
    /// Favorito: fijado arriba de la cinta, con su estrella.
    #[serde(default)]
    pub favorito: bool,
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
    format!(
        "{}-{:04}",
        ahora_unix(),
        N.fetch_add(1, Ordering::Relaxed) % 10000
    )
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
    let mut items: Vec<ItemCola> = serde_json::from_str(&json).unwrap_or_default();
    // Las rutas se guardan RELATIVAS (solo el nombre de fichero) y se
    // resuelven aquí contra el contenedor VIVO. iOS migra los datos a un
    // contenedor con UUID nuevo en cada instalación o actualización: las
    // rutas absolutas de ayer mienten hoy, y cada update de TestFlight
    // dejaba TODA la cinta con piezas zombis que fallaban en silencio.
    let dir = dir_cola(app)?;
    for it in &mut items {
        if let Some(r) = &it.ruta {
            if let Some(nombre) = PathBuf::from(r).file_name() {
                let viva = dir.join(nombre);
                it.ruta = Some(viva.to_string_lossy().to_string());
            }
        }
    }
    Ok(items)
}

fn guardar<R: Runtime>(app: &AppHandle<R>, items: &[ItemCola]) -> Result<()> {
    // Simetría con listar: al disco van solo los NOMBRES de fichero (el
    // contenedor cambia de UUID con cada instalación).
    let mut relativos = items.to_vec();
    for it in &mut relativos {
        if let Some(r) = &it.ruta {
            if let Some(nombre) = PathBuf::from(r).file_name() {
                it.ruta = Some(nombre.to_string_lossy().to_string());
            }
        }
    }
    let ruta = ruta_indice(app)?;
    let tmp = ruta.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&relativos)?)?;
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
    let tipo = if es_youtube(&url) {
        TipoItem::Youtube
    } else {
        TipoItem::Url
    };
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
        favorito: false,
    };
    let clon = item.clone();
    mutar(app, |items| items.insert(0, item))?;
    procesar_en_segundo_plano(app.clone(), clon.id.clone());
    Ok(clon)
}

/// Una página web VIVA compartida desde Safari: el fichero trae la URL y
/// el título en comentarios de cabecera y después el DOM tal cual lo veía
/// el usuario (con su sesión: sin muro). Si la misma URL ya estaba en la
/// cinta (incluso muerta en error de muro), se le da este HTML y se
/// reprocesa: compartir de nuevo ARREGLA la pieza.
pub fn agregar_web<R: Runtime>(app: &AppHandle<R>, ruta_html: String) -> Result<ItemCola> {
    let contenido = fs::read_to_string(&ruta_html)
        .with_context(|| format!("leyendo la página compartida {ruta_html}"))?;
    let cabecera = |clave: &str| -> Option<String> {
        contenido.lines().take(3).find_map(|l| {
            l.trim()
                .strip_prefix(&format!("<!-- yappy-{clave}: "))
                .and_then(|r| r.strip_suffix(" -->"))
                .map(|s| s.trim().to_string())
        })
    };
    let url = cabecera("url").ok_or_else(|| anyhow!("página compartida sin URL"))?;
    let titulo = cabecera("titulo")
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            url.trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("www.")
                .chars()
                .take(80)
                .collect()
        });

    let existente = listar(app)?.into_iter().find(|i| i.origen == url);
    let item = match existente {
        Some(mut it) => {
            it.estado = EstadoItem::Pendiente;
            it.error = None;
            let clon = it.clone();
            mutar(app, |items| {
                if let Some(v) = items.iter_mut().find(|i| i.id == clon.id) {
                    v.estado = EstadoItem::Pendiente;
                    v.error = None;
                }
            })?;
            clon
        }
        None => {
            let nuevo = ItemCola {
                id: nuevo_id(),
                tipo: TipoItem::Url,
                titulo,
                origen: url,
                ruta: None,
                estado: EstadoItem::Pendiente,
                error: None,
                agregado_unix: ahora_unix(),
                chars: None,
                favorito: false,
            };
            let clon = nuevo.clone();
            mutar(app, |items| items.insert(0, nuevo))?;
            clon
        }
    };
    // El HTML vivo queda donde preparar_articulo lo busca primero.
    fs::write(dir_cola(app)?.join(format!("{}.html", item.id)), &contenido)?;
    let _ = fs::remove_file(&ruta_html);
    procesar_en_segundo_plano(app.clone(), item.id.clone());
    Ok(item)
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
        favorito: false,
    };
    let clon = item.clone();
    mutar(app, |items| items.insert(0, item))?;
    Ok(clon)
}

/// Un archivo (PDF, EPUB, DOCX, MD…) ya accesible en disco: se copia a la
/// cola para que sobreviva aunque el original desaparezca (los ficheros
/// del App Group del share sheet se limpian).
/// El nombre de fichero, vuelto humano: sin extensión, sin el sufijo
/// aleatorio «-a1b2c3» que añade la extensión de compartir, y con guiones
/// y guiones bajos como espacios.
fn titulo_de_nombre(nombre: &str) -> String {
    let sin_ext = nombre.rsplit_once('.').map(|(a, _)| a).unwrap_or(nombre);
    let sin_sufijo = match sin_ext.rsplit_once('-') {
        Some((base, sufijo))
            if sufijo.len() == 6 && sufijo.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            base
        }
        _ => sin_ext,
    };
    let limpio = sin_sufijo.replace(['-', '_'], " ").trim().to_string();
    if limpio.is_empty() {
        "documento".into()
    } else {
        limpio
    }
}

/// El selector de iOS a veces entrega la ruta como URL («file:///…», con
/// espacios en %20): se normaliza a ruta de fichero real antes de tocarla.
fn ruta_de_selector(ruta: &str) -> String {
    let p = PathBuf::from(ruta);
    if p.exists() {
        return ruta.to_string();
    }
    let sin_esquema = ruta.strip_prefix("file://").unwrap_or(ruta);
    let decodificada = percent_decode(sin_esquema);
    if PathBuf::from(&decodificada).exists() {
        decodificada
    } else {
        ruta.to_string()
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn agregar_archivo<R: Runtime>(app: &AppHandle<R>, ruta_original: String) -> Result<ItemCola> {
    let ruta_original = ruta_de_selector(&ruta_original);
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
        titulo: titulo_de_nombre(&nombre),
        origen: nombre,
        ruta: Some(destino.to_string_lossy().to_string()),
        estado: EstadoItem::Listo,
        error: None,
        agregado_unix: ahora_unix(),
        chars: None,
        favorito: false,
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
        favorito: false,
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
        // El UA de Safari en iPhone: somos exactamente eso (un WebKit en un
        // iPhone) y varios periódicos sirven el muro al UA de escritorio.
        .user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1")
        .timeout(std::time::Duration::from_secs(25))
        .build()?;

    // Los sitios con extractor PROPIO (no hay artículo que raspar: hay una
    // API mejor): Hacker News, X/Twitter, Reddit, Bluesky, Mastodon,
    // Archive.org y los Google Docs públicos.
    let host = host_de(&item.origen);
    if host == "news.ycombinator.com" && item.origen.contains("item?id=") {
        let (titulo, markdown) = extraer_hn(&cliente, &item.origen).await?;
        return terminar_con_markdown(app, &item.id, titulo, markdown);
    }
    if (host == "x.com" || host == "twitter.com" || host == "mobile.twitter.com")
        && item.origen.contains("/status/")
    {
        let (titulo, markdown) = extraer_tweet(&cliente, &item.origen).await?;
        return terminar_con_markdown(app, &item.id, titulo, markdown);
    }
    if host.ends_with("reddit.com") && item.origen.contains("/comments/") {
        let (titulo, markdown) = extraer_reddit(&cliente, &item.origen).await?;
        return terminar_con_markdown(app, &item.id, titulo, markdown);
    }
    if host == "bsky.app" && item.origen.contains("/post/") {
        let (titulo, markdown) = extraer_bluesky(&cliente, &item.origen).await?;
        return terminar_con_markdown(app, &item.id, titulo, markdown);
    }
    if es_url_mastodon(&item.origen) {
        if let Ok((titulo, markdown)) = extraer_mastodon(&cliente, &item.origen).await {
            return terminar_con_markdown(app, &item.id, titulo, markdown);
        }
        // Si la instancia no habla la API, se sigue por el camino normal.
    }
    if host == "archive.org" && item.origen.contains("/details/") {
        let (titulo, markdown) = extraer_archive(&cliente, &item.origen).await?;
        return terminar_con_markdown(app, &item.id, titulo, markdown);
    }
    if host == "docs.google.com" && item.origen.contains("/document/d/") {
        let (titulo, markdown) = extraer_gdoc(&cliente, &item.origen).await?;
        return terminar_con_markdown(app, &item.id, titulo, markdown);
    }

    // LA VÍA CON SESIÓN: si el compartir trajo el HTML que el usuario VEÍA
    // (Safari + preprocesado JS: con su suscripción, sin muro), está en
    // cola/{id}.html y no hay nada que descargar.
    let ruta_html = dir_cola(app)?.join(format!("{}.html", item.id));
    // EL RESCATE MANUAL: si la ficha pidió «recuperar del archivo», el
    // marcador manda la descarga por la Wayback Machine.
    let ruta_wayback = dir_cola(app)?.join(format!("{}.wayback", item.id));
    let html = if ruta_html.exists() {
        fs::read_to_string(&ruta_html)?
    } else {
        let url_efectiva = if ruta_wayback.exists() {
            format!("https://web.archive.org/web/2/{}", item.origen)
        } else {
            item.origen.clone()
        };
        let (bytes, content_type) = descargar_crudo(&cliente, &url_efectiva).await?;
        // Un enlace DIRECTO a un documento (PDF, EPUB, Word…): al
        // contenedor y por el camino de archivos, no hay HTML que raspar.
        if let Some(ext) = extension_de_documento(&bytes, &content_type, &item.origen) {
            return terminar_con_archivo(app, item, &bytes, ext);
        }
        decodificar_html(&bytes, &content_type)
    };

    let html_podado = podar_html(&html, &item.origen);
    let mut resultado = extraer_articulo(&html_podado, &item.origen);
    // EL RESCATE AMP: si la página declara su versión AMP y lo extraído
    // quedó corto (o murió), la variante suele venir limpia y sin muro.
    let corto = match &resultado {
        Ok((_, md)) => md.chars().count() < 900,
        Err(_) => true,
    };
    if corto && !ruta_html.exists() {
        if let Some(url_amp) = enlace_amp(&html, &item.origen) {
            if let Ok((bytes, ct)) = descargar_crudo(&cliente, &url_amp).await {
                let html_amp = decodificar_html(&bytes, &ct);
                let podado_amp = podar_html(&html_amp, &item.origen);
                if let Ok((t, md)) = extraer_articulo(&podado_amp, &item.origen) {
                    let mejor = match &resultado {
                        Ok((_, md_previo)) => md.chars().count() > md_previo.chars().count(),
                        Err(_) => true,
                    };
                    if mejor {
                        resultado = Ok((t, md));
                    }
                }
            }
        }
    }
    let (titulo, markdown) = resultado?;
    // El HTML compartido y el marcador ya sirvieron.
    let _ = fs::remove_file(&ruta_html);
    let _ = fs::remove_file(&ruta_wayback);
    terminar_con_markdown(app, &item.id, titulo, markdown)
}

/// Descarga cruda: bytes + content-type (con un reintento suave, que las
/// redes de móvil parpadean).
async fn descargar_crudo(cliente: &reqwest::Client, url: &str) -> Result<(Vec<u8>, String)> {
    let mut ultimo_error = None;
    for intento in 0..2 {
        if intento > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        }
        match cliente.get(url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(resp) => {
                    let ct = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_lowercase();
                    let bytes = resp.bytes().await?.to_vec();
                    return Ok((bytes, ct));
                }
                Err(e) => return Err(e.into()),
            },
            Err(e) => ultimo_error = Some(e),
        }
    }
    Err(ultimo_error
        .map(Into::into)
        .unwrap_or_else(|| anyhow!("sin conexión")))
}

/// ¿La URL apunta a un DOCUMENTO y no a una página? Se decide por firma
/// de bytes, content-type o extensión de la URL. Devuelve la extensión.
fn extension_de_documento(bytes: &[u8], content_type: &str, url: &str) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF") {
        return Some("pdf");
    }
    let ct = content_type.split(';').next().unwrap_or("").trim();
    match ct {
        "application/pdf" => return Some("pdf"),
        "application/epub+zip" => return Some("epub"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            return Some("docx")
        }
        "application/msword" => return Some("doc"),
        "application/rtf" | "text/rtf" => return Some("rtf"),
        "application/vnd.oasis.opendocument.text" => return Some("odt"),
        _ => {}
    }
    // Un ZIP (PK) con la extensión delatora en la URL.
    let ruta = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    if bytes.starts_with(b"PK") {
        if ruta.ends_with(".epub") {
            return Some("epub");
        }
        if ruta.ends_with(".docx") {
            return Some("docx");
        }
        if ruta.ends_with(".odt") {
            return Some("odt");
        }
    }
    if ct == "text/plain" && (ruta.ends_with(".txt") || ruta.ends_with(".text")) {
        return Some("txt");
    }
    None
}

/// El documento remoto queda en el contenedor y la pieza pasa a ser un
/// ARCHIVO listo (el lector ya sabe abrir pdf/epub/docx/…).
fn terminar_con_archivo<R: Runtime>(
    app: &AppHandle<R>,
    item: &ItemCola,
    bytes: &[u8],
    ext: &'static str,
) -> Result<()> {
    let destino = dir_cola(app)?.join(format!("{}.{ext}", item.id));
    fs::write(&destino, bytes)?;
    // El título, del nombre del fichero en la URL (si dice algo).
    let nombre_url = item
        .origen
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();
    let nombre = percent_decode(&nombre_url);
    let sin_ext = nombre.rsplit_once('.').map(|(n, _)| n).unwrap_or(&nombre);
    let legible = sin_ext.replace(['-', '_'], " ").trim().to_string();
    let titulo = if legible.chars().filter(|c| c.is_alphabetic()).count() >= 4 {
        titulo_de_nombre(&legible)
    } else {
        format!("Documento {}", ext.to_uppercase())
    };
    actualizar_item(app, &item.id, |it| {
        it.tipo = TipoItem::Archivo;
        it.estado = EstadoItem::Listo;
        it.ruta = Some(destino.to_string_lossy().to_string());
        it.titulo = titulo;
    })?;
    Ok(())
}

/// Decodifica el HTML honrando el charset: primero el del content-type,
/// después el del <meta>, y por defecto UTF-8. Las webs viejas en
/// ISO-8859-1 llegaban con las tildes rotas.
fn decodificar_html(bytes: &[u8], content_type: &str) -> String {
    let charset_de = |s: &str| -> Option<String> {
        let i = s.find("charset=")?;
        let resto = &s[i + 8..];
        let fin = resto
            .find([';', '"', '\'', ' ', '>', '/'])
            .unwrap_or(resto.len());
        Some(resto[..fin].trim_matches(['"', '\'']).to_string())
    };
    let mut etiqueta = charset_de(content_type);
    if etiqueta.is_none() {
        // Olfatear el <meta charset> en la cabecera del documento.
        let cabeza = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_lowercase();
        etiqueta = charset_de(&cabeza);
    }
    if let Some(e) = etiqueta {
        if let Some(enc) = encoding_rs::Encoding::for_label(e.as_bytes()) {
            let (texto, _, _) = enc.decode(bytes);
            return texto.into_owned();
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// El enlace AMP que la propia página declara en su <head>.
fn enlace_amp(html: &str, url_base: &str) -> Option<String> {
    let doc = dom_query::Document::from(html);
    let href = doc
        .select(r#"link[rel="amphtml"]"#)
        .attr("href")
        .map(|h| h.to_string())?;
    if href.starts_with("http") {
        Some(href)
    } else if let Some(resto) = href.strip_prefix('/') {
        let raiz = url_base.split('/').take(3).collect::<Vec<_>>().join("/");
        Some(format!("{raiz}/{resto}"))
    } else {
        None
    }
}

/// El host de una URL, sin «www.»: para decidir extractores por sitio.
fn host_de(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_lowercase()
}

/// Los conectores de la conversación hablada, en el idioma del hilo.
struct Conectores {
    dice: &'static str,
    responde: &'static str,
    anade: &'static str,
    escribe: &'static str,
    en_red: &'static str,
    cita_de: &'static str,
}

fn conectores(idioma: &str) -> Conectores {
    match idioma {
        "es" => Conectores {
            dice: "dice",
            responde: "responde",
            anade: "añade",
            escribe: "escribe",
            en_red: "en",
            cita_de: "Cita de",
        },
        "fr" => Conectores {
            dice: "dit",
            responde: "répond",
            anade: "ajoute",
            escribe: "écrit",
            en_red: "sur",
            cita_de: "Citation de",
        },
        "de" => Conectores {
            dice: "sagt",
            responde: "antwortet",
            anade: "ergänzt",
            escribe: "schreibt",
            en_red: "auf",
            cita_de: "Zitat von",
        },
        "it" => Conectores {
            dice: "dice",
            responde: "risponde",
            anade: "aggiunge",
            escribe: "scrive",
            en_red: "su",
            cita_de: "Citazione di",
        },
        "pt" => Conectores {
            dice: "diz",
            responde: "responde",
            anade: "acrescenta",
            escribe: "escreve",
            en_red: "no",
            cita_de: "Citação de",
        },
        _ => Conectores {
            dice: "says",
            responde: "replies",
            anade: "adds",
            escribe: "writes",
            en_red: "on",
            cita_de: "Quoting",
        },
    }
}

/// HACKER NEWS como conversación: la API pública de Algolia da el hilo
/// entero en un JSON; se leen el título, el texto del post y los
/// comentarios con su autor («Fulano dice: …»), nada de raspar la tabla.
async fn extraer_hn(cliente: &reqwest::Client, url: &str) -> Result<(Option<String>, String)> {
    let id: u64 = url
        .split("item?id=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("enlace de Hacker News sin id"))?;
    let v: serde_json::Value = cliente
        .get(format!("https://hn.algolia.com/api/v1/items/{id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let titulo = v["title"]
        .as_str()
        .unwrap_or("Hilo de Hacker News")
        .to_string();
    // El idioma del hilo decide los conectores («dice»/«says»).
    let muestra = format!(
        "{titulo} {} {}",
        v["text"].as_str().unwrap_or(""),
        v["children"][0]["text"].as_str().unwrap_or("")
    );
    let con = conectores(&yappy_core::lang_detect::detect_document_lang(
        &texto_de_fragmento_html(&muestra),
        "en",
    ));
    let mut md = format!("# {titulo}\n\n");
    if let Some(texto) = v["text"].as_str() {
        md.push_str(&texto_de_fragmento_html(texto));
        md.push_str("\n\n");
    }
    // Los comentarios, en orden y con jerarquía hablada: primer nivel
    // «dice», respuestas «responde». Tope generoso para no leer mil.
    let mut cuantos = 0usize;
    fn caminar(
        nodo: &serde_json::Value,
        nivel: usize,
        md: &mut String,
        cuantos: &mut usize,
        con: &Conectores,
    ) {
        if *cuantos >= 60 || nivel > 2 {
            return;
        }
        if let Some(hijos) = nodo["children"].as_array() {
            for h in hijos {
                let (Some(autor), Some(texto)) = (h["author"].as_str(), h["text"].as_str()) else {
                    continue;
                };
                let verbo = if nivel == 0 { con.dice } else { con.responde };
                md.push_str(&format!(
                    "{autor} {verbo}: {}\n\n",
                    texto_de_fragmento_html(texto)
                ));
                *cuantos += 1;
                caminar(h, nivel + 1, md, cuantos, con);
                if *cuantos >= 60 {
                    return;
                }
            }
        }
    }
    caminar(&v, 0, &mut md, &mut cuantos, &con);
    let markdown = limpiar_markdown_hablado(&md);
    if markdown.chars().count() < 80 {
        return Err(anyhow!("el hilo está vacío"));
    }
    Ok((Some(titulo), markdown))
}

/// Un fragmento de HTML (comentarios de HN, texto con <p> sin cerrar) a
/// texto plano con párrafos.
fn texto_de_fragmento_html(html: &str) -> String {
    let con_saltos = html.replace("<p>", "\n\n").replace("<br>", "\n");
    let doc = dom_query::Document::from(con_saltos.as_str());
    let texto = doc.text().to_string();
    let mut t = texto.trim().to_string();
    while t.contains("\n\n\n") {
        t = t.replace("\n\n\n", "\n\n");
    }
    t
}

/// X/TWITTER por la API de sindicación (la de los embeds): el texto del
/// tuit, su autor y la cita si la hay, sin raspar la maraña de la web.
async fn extraer_tweet(cliente: &reqwest::Client, url: &str) -> Result<(Option<String>, String)> {
    let id: u64 = url
        .split("/status/")
        .nth(1)
        .and_then(|s| s.split(['?', '/', '#']).next())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("enlace de X sin id de tuit"))?;
    let v: serde_json::Value = cliente
        .get(format!(
            "https://cdn.syndication.twimg.com/tweet-result?id={id}&token={}&lang=es",
            token_sindicacion(id)
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let autor = v["user"]["name"].as_str().unwrap_or("Alguien").to_string();
    // Los ARTÍCULOS de X (título + cuerpo largo) no viajan por la
    // sindicación (solo la vista previa): el cuerpo entero se pide aparte.
    if v.get("article").is_some() {
        return extraer_articulo_de_x(cliente, id, &autor, &v).await;
    }
    // Los tuits largos («notas») llegan enteros en note_tweet.
    let texto = v["note_tweet"]["text"]
        .as_str()
        .or_else(|| v["text"].as_str())
        .ok_or_else(|| anyhow!("el tuit no se puede leer (¿borrado o privado?)"))?;
    let con = conectores(&yappy_core::lang_detect::detect_document_lang(texto, "en"));
    let mut md = format!("# {autor} {} X\n\n{texto}\n", con.en_red);
    if let Some(cita) = v["quoted_tweet"].as_object() {
        if let (Some(qn), Some(qt)) = (
            cita.get("user").and_then(|u| u["name"].as_str()),
            cita.get("text").and_then(|t| t.as_str()),
        ) {
            md.push_str(&format!("\n{} {qn}: {qt}\n", con.cita_de));
        }
    }
    let markdown = limpiar_markdown_hablado(&md);
    Ok((Some(format!("{autor} {} X", con.en_red)), markdown))
}

/// UN ARTÍCULO DE X entero: el cuerpo vive en bloques draft-js que la
/// API de fxtwitter sí sirve; si ese espejo falla, al menos el título y
/// la vista previa de la sindicación, avisando de que es un adelanto.
async fn extraer_articulo_de_x(
    cliente: &reqwest::Client,
    id: u64,
    autor: &str,
    sindicacion: &serde_json::Value,
) -> Result<(Option<String>, String)> {
    let titulo_previo = sindicacion["article"]["title"]
        .as_str()
        .unwrap_or("Artículo en X")
        .trim()
        .to_string();
    // El espejo con el cuerpo completo.
    if let Ok(resp) = cliente
        .get(format!("https://api.fxtwitter.com/status/{id}"))
        .send()
        .await
    {
        if let Ok(resp) = resp.error_for_status() {
            if let Ok(fx) = resp.json::<serde_json::Value>().await {
                let art = &fx["tweet"]["article"];
                if let Some(bloques) = art["content"]["blocks"].as_array() {
                    let titulo = art["title"]
                        .as_str()
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .unwrap_or(titulo_previo.clone());
                    let quien = fx["tweet"]["author"]["name"].as_str().unwrap_or(autor);
                    let texto_muestra: String = bloques
                        .iter()
                        .filter_map(|b| b["text"].as_str())
                        .take(6)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let con = conectores(&yappy_core::lang_detect::detect_document_lang(
                        &texto_muestra,
                        "en",
                    ));
                    let firma = match con.en_red {
                        "en" => format!("Por {quien}, en X."),
                        "sur" => format!("Par {quien}, sur X."),
                        "auf" => format!("Von {quien}, auf X."),
                        "su" => format!("Di {quien}, su X."),
                        "no" => format!("Por {quien}, no X."),
                        _ => format!("By {quien}, on X."),
                    };
                    let mut md = format!("# {titulo}\n\n{firma}\n\n");
                    for b in bloques {
                        let Some(texto) = b["text"].as_str() else {
                            continue;
                        };
                        let texto = texto.trim();
                        if texto.is_empty() {
                            continue;
                        }
                        match b["type"].as_str().unwrap_or("unstyled") {
                            "header-one" => md.push_str(&format!("# {texto}\n\n")),
                            "header-two" => md.push_str(&format!("## {texto}\n\n")),
                            "header-three" | "header-four" => {
                                md.push_str(&format!("### {texto}\n\n"))
                            }
                            "unordered-list-item" => md.push_str(&format!("- {texto}\n\n")),
                            "ordered-list-item" => md.push_str(&format!("1. {texto}\n\n")),
                            "blockquote" => md.push_str(&format!("{texto}\n\n")),
                            "atomic" | "code-block" => {}
                            _ => md.push_str(&format!("{texto}\n\n")),
                        }
                    }
                    let markdown = limpiar_markdown_hablado(&md);
                    if markdown.chars().count() > 300 {
                        return Ok((Some(titulo), markdown));
                    }
                }
            }
        }
    }
    // Sin espejo: la vista previa honesta.
    let preview = sindicacion["article"]["preview_text"]
        .as_str()
        .unwrap_or("");
    if preview.trim().is_empty() {
        return Err(anyhow!(
            "X no deja leer este artículo sin cuenta (prueba a compartirlo desde Safari con tu sesión abierta)"
        ));
    }
    let md = format!(
        "# {titulo_previo}\n\nPor {autor}, en X.\n\n{preview}\n\n(Esto es solo el comienzo: X no deja leer el artículo completo sin cuenta.)"
    );
    Ok((Some(titulo_previo), limpiar_markdown_hablado(&md)))
}

/// El token de la API de sindicación: ((id/1e15)·π) en base 36, sin ceros
/// ni punto (el mismo cálculo que hace el widget oficial de embeds).
fn token_sindicacion(id: u64) -> String {
    let x = (id as f64 / 1e15) * std::f64::consts::PI;
    let digitos = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut entero = x.trunc() as u64;
    let mut frac = x.fract();
    let mut cabeza = Vec::new();
    if entero == 0 {
        cabeza.push(b'0');
    }
    while entero > 0 {
        cabeza.push(digitos[(entero % 36) as usize]);
        entero /= 36;
    }
    cabeza.reverse();
    let mut s: String = String::from_utf8(cabeza).unwrap_or_default();
    s.push('.');
    for _ in 0..11 {
        frac *= 36.0;
        let d = frac.trunc() as usize;
        s.push(digitos[d.min(35)] as char);
        frac -= frac.trunc();
    }
    s.replace(['0', '.'], "")
}

/// El libro de Gutenberg entre sus marcadores «*** START/END OF THE
/// PROJECT GUTENBERG EBOOK ***», re-envuelto con su título sin la marca.
fn recortar_gutenberg(html: &str) -> Option<String> {
    let a = html.find("*** START OF")?;
    let fin_marcador = html[a + 12..].find("***").map(|p| a + 12 + p + 3)?;
    let b = html.rfind("*** END OF")?;
    if b <= fin_marcador || b - fin_marcador < 5000 {
        return None;
    }
    let titulo = html
        .find("<title>")
        .and_then(|t| {
            let desde = t + 7;
            html[desde..]
                .find("</title>")
                .map(|f| html[desde..desde + f].trim())
        })
        .map(|t| {
            t.trim_start_matches("The Project Gutenberg eBook of ")
                .trim_start_matches("The Project Gutenberg EBook of ")
                .to_string()
        })
        .unwrap_or_else(|| "Libro".to_string());
    Some(format!(
        "<html><head><title>{titulo}</title></head><body>{}</body></html>",
        &html[fin_marcador..b]
    ))
}

/// REDDIT como conversación: el hilo entero vive en la versión .json de
/// la misma URL (post + comentarios anidados), sin raspar nada.
async fn extraer_reddit(cliente: &reqwest::Client, url: &str) -> Result<(Option<String>, String)> {
    let limpia = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');
    // Reddit exige un User-Agent de APP (el de navegador se bloquea) y
    // aun así capa algunas redes: el error se cuenta claro.
    let resp = cliente
        .get(format!("{limpia}.json?raw_json=1&limit=80"))
        .header(
            "User-Agent",
            "ios:com.joseluissaorin.yappy:v0.2 (lector en voz alta)",
        )
        .header("Accept", "application/json")
        .send()
        .await?;
    if resp.status().as_u16() == 403 || resp.status().as_u16() == 429 {
        return Err(anyhow!(
            "Reddit bloqueó la petición desde esta red (prueba a compartirlo desde Safari)"
        ));
    }
    let v: serde_json::Value = resp.error_for_status()?.json().await?;
    let post = &v[0]["data"]["children"][0]["data"];
    let titulo = post["title"]
        .as_str()
        .unwrap_or("Hilo de Reddit")
        .to_string();
    let mut md = format!("# {titulo}\n\n");
    let con = conectores(&yappy_core::lang_detect::detect_document_lang(
        &format!("{titulo} {}", post["selftext"].as_str().unwrap_or("")),
        "en",
    ));
    if let Some(cuerpo) = post["selftext"].as_str() {
        if !cuerpo.trim().is_empty() {
            let autor = post["author"].as_str().unwrap_or("alguien");
            md.push_str(&format!("{autor} {}: {}\n\n", con.escribe, cuerpo.trim()));
        }
    }
    fn caminar(
        nodos: &serde_json::Value,
        nivel: usize,
        md: &mut String,
        cuantos: &mut usize,
        con: &Conectores,
    ) {
        if *cuantos >= 60 || nivel > 2 {
            return;
        }
        let Some(hijos) = nodos["data"]["children"].as_array() else {
            return;
        };
        for h in hijos {
            if h["kind"].as_str() != Some("t1") {
                continue;
            }
            let d = &h["data"];
            let (Some(autor), Some(texto)) = (d["author"].as_str(), d["body"].as_str()) else {
                continue;
            };
            let verbo = if nivel == 0 { con.dice } else { con.responde };
            md.push_str(&format!("{autor} {verbo}: {}\n\n", texto.trim()));
            *cuantos += 1;
            if d["replies"].is_object() {
                caminar(&d["replies"], nivel + 1, md, cuantos, con);
            }
            if *cuantos >= 60 {
                return;
            }
        }
    }
    let mut cuantos = 0usize;
    caminar(&v[1], 0, &mut md, &mut cuantos, &con);
    let markdown = limpiar_markdown_hablado(&md);
    if markdown.chars().count() < 80 {
        return Err(anyhow!("el hilo está vacío"));
    }
    Ok((Some(titulo), markdown))
}

/// BLUESKY: el hilo por la API pública (sin cuenta ninguna).
async fn extraer_bluesky(cliente: &reqwest::Client, url: &str) -> Result<(Option<String>, String)> {
    let tras = url
        .split("/profile/")
        .nth(1)
        .ok_or_else(|| anyhow!("enlace de Bluesky sin perfil"))?;
    let mut partes = tras.split('/');
    let actor = partes.next().unwrap_or("");
    let rkey = tras
        .split("/post/")
        .nth(1)
        .and_then(|s| s.split(['?', '#', '/']).next())
        .ok_or_else(|| anyhow!("enlace de Bluesky sin post"))?;
    let did = if actor.starts_with("did:") {
        actor.to_string()
    } else {
        let v: serde_json::Value = cliente
            .get(format!(
                "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={actor}"
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        v["did"]
            .as_str()
            .ok_or_else(|| anyhow!("no se pudo resolver el usuario de Bluesky"))?
            .to_string()
    };
    let v: serde_json::Value = cliente
        .get(format!(
            "https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread?uri=at://{did}/app.bsky.feed.post/{rkey}&depth=6"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let hilo = &v["thread"];
    let nombre_de = |post: &serde_json::Value| -> String {
        post["author"]["displayName"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| post["author"]["handle"].as_str())
            .unwrap_or("Alguien")
            .to_string()
    };
    let autor = nombre_de(&hilo["post"]);
    let texto = hilo["post"]["record"]["text"]
        .as_str()
        .ok_or_else(|| anyhow!("el post de Bluesky no se puede leer"))?;
    let con = conectores(&yappy_core::lang_detect::detect_document_lang(texto, "en"));
    let mut md = format!("# {autor} {} Bluesky\n\n{texto}\n\n", con.en_red);
    fn caminar(
        nodo: &serde_json::Value,
        nivel: usize,
        md: &mut String,
        cuantos: &mut usize,
        nombre_de: &dyn Fn(&serde_json::Value) -> String,
        con: &Conectores,
    ) {
        if *cuantos >= 40 || nivel > 3 {
            return;
        }
        let Some(respuestas) = nodo["replies"].as_array() else {
            return;
        };
        for r in respuestas {
            let Some(texto) = r["post"]["record"]["text"].as_str() else {
                continue;
            };
            let quien = nombre_de(&r["post"]);
            let verbo = if nivel == 0 { con.responde } else { con.anade };
            md.push_str(&format!("{quien} {verbo}: {}\n\n", texto.trim()));
            *cuantos += 1;
            caminar(r, nivel + 1, md, cuantos, nombre_de, con);
            if *cuantos >= 40 {
                return;
            }
        }
    }
    let mut cuantos = 0usize;
    caminar(hilo, 0, &mut md, &mut cuantos, &nombre_de, &con);
    Ok((
        Some(format!("{autor} {} Bluesky", con.en_red)),
        limpiar_markdown_hablado(&md),
    ))
}

/// ¿Huele a URL de Mastodon? Cualquier instancia con /@usuario/ID-largo.
fn es_url_mastodon(url: &str) -> bool {
    let ruta = url.split("://").nth(1).unwrap_or("");
    let mut seg = ruta.split('/').skip(1);
    let (Some(usuario), Some(id)) = (seg.next(), seg.next()) else {
        return false;
    };
    usuario.starts_with('@')
        && id.len() >= 10
        && id.chars().all(|c| c.is_ascii_digit())
        && seg.next().is_none()
}

/// MASTODON: el estado y sus respuestas por la API pública de la instancia.
async fn extraer_mastodon(
    cliente: &reqwest::Client,
    url: &str,
) -> Result<(Option<String>, String)> {
    let host = host_de(url);
    let id = url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();
    let v: serde_json::Value = cliente
        .get(format!("https://{host}/api/v1/statuses/{id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let nombre_de = |s: &serde_json::Value| -> String {
        s["account"]["display_name"]
            .as_str()
            .filter(|n| !n.trim().is_empty())
            .or_else(|| s["account"]["acct"].as_str())
            .unwrap_or("Alguien")
            .to_string()
    };
    let autor = nombre_de(&v);
    let texto = texto_de_fragmento_html(v["content"].as_str().unwrap_or(""));
    if texto.trim().is_empty() {
        return Err(anyhow!("la publicación está vacía"));
    }
    let con = conectores(&yappy_core::lang_detect::detect_document_lang(&texto, "en"));
    let mut md = format!("# {autor} {} Mastodon\n\n{texto}\n\n", con.en_red);
    if let Ok(ctx) = cliente
        .get(format!("https://{host}/api/v1/statuses/{id}/context"))
        .send()
        .await
    {
        if let Ok(ctx) = ctx.error_for_status() {
            if let Ok(ctx) = ctx.json::<serde_json::Value>().await {
                if let Some(desc) = ctx["descendants"].as_array() {
                    for (i, d) in desc.iter().take(40).enumerate() {
                        let cuerpo = texto_de_fragmento_html(d["content"].as_str().unwrap_or(""));
                        if cuerpo.trim().is_empty() {
                            continue;
                        }
                        let verbo = if i == 0 { con.responde } else { con.anade };
                        md.push_str(&format!("{} {verbo}: {}\n\n", nombre_de(d), cuerpo.trim()));
                    }
                }
            }
        }
    }
    Ok((
        Some(format!("{autor} {} Mastodon", con.en_red)),
        limpiar_markdown_hablado(&md),
    ))
}

/// ARCHIVE.ORG de verdad: el texto completo del libro (el _djvu.txt que
/// acompaña a cada escaneo), no la ficha.
async fn extraer_archive(cliente: &reqwest::Client, url: &str) -> Result<(Option<String>, String)> {
    let id = url
        .split("/details/")
        .nth(1)
        .and_then(|s| s.split(['/', '?', '#']).next())
        .ok_or_else(|| anyhow!("enlace de archive.org sin identificador"))?;
    let meta: serde_json::Value = cliente
        .get(format!("https://archive.org/metadata/{id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let titulo = meta["metadata"]["title"].as_str().map(|t| t.to_string());
    let fichero = meta["files"]
        .as_array()
        .and_then(|fs| {
            fs.iter()
                .find_map(|f| f["name"].as_str().filter(|n| n.ends_with("_djvu.txt")))
        })
        .ok_or_else(|| anyhow!("este objeto de archive.org no tiene texto completo"))?;
    let texto = cliente
        .get(format!("https://archive.org/download/{id}/{fichero}"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    // Los escaneos pueden ser descomunales: se corta con cabeza.
    let recortado: String = texto.chars().take(2_000_000).collect();
    let cuerpo = limpiar_markdown_hablado(&recortado);
    if cuerpo.chars().count() < 200 {
        return Err(anyhow!("el texto del escaneo está vacío"));
    }
    let markdown = match &titulo {
        Some(t) => format!("# {t}\n\n{cuerpo}"),
        None => cuerpo,
    };
    Ok((titulo, markdown))
}

/// GOOGLE DOCS públicos: el propio documento exporta a texto plano.
async fn extraer_gdoc(cliente: &reqwest::Client, url: &str) -> Result<(Option<String>, String)> {
    let id = url
        .split("/document/d/")
        .nth(1)
        .and_then(|s| s.split(['/', '?', '#']).next())
        .ok_or_else(|| anyhow!("enlace de Google Docs sin identificador"))?;
    let resp = cliente
        .get(format!(
            "https://docs.google.com/document/d/{id}/export?format=txt"
        ))
        .send()
        .await?;
    let resp = resp
        .error_for_status()
        .map_err(|_| anyhow!("el documento no es público (pide iniciar sesión)"))?;
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    if !ct.contains("text/plain") {
        return Err(anyhow!("el documento no es público (pide iniciar sesión)"));
    }
    let texto = resp
        .text()
        .await?
        .replace("\r\n", "\n")
        .replace('\u{feff}', "");
    let cuerpo = limpiar_markdown_hablado(&texto);
    if cuerpo.chars().count() < 40 {
        return Err(anyhow!("el documento está vacío"));
    }
    let titulo = cuerpo
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().chars().take(70).collect::<String>());
    Ok((titulo, cuerpo))
}

/// LA PODA: antes de Readability, se cae del DOM todo lo que jamás debe
/// LEERSE EN VOZ ALTA: navegación, pies de foto, y en MediaWiki
/// (Wikipedia y familia) el índice, las infoboxes, las referencias, los
/// «[editar]» y las secciones de cola (Referencias, Véase también…).
pub fn podar_html(html: &str, url: &str) -> String {
    use dom_query::Document;
    // PROYECTO GUTENBERG (y espejos): el libro vive entre los marcadores
    // oficiales; el preámbulo legal y la licencia del final jamás se leen.
    if let Some(recortado) = recortar_gutenberg(html) {
        let doc = Document::from(recortado.as_str());
        doc.select("table, .pg-boilerplate, #pg-header, #pg-footer")
            .remove();
        return doc.html().to_string();
    }
    let doc = Document::from(html);

    // Genérico, para CUALQUIER web: lo que es decorado o pie, fuera.
    for sel in [
        "figcaption",
        "figure",
        "nav",
        "[role=\"navigation\"]",
        "[role=\"doc-toc\"]",
    ] {
        doc.select(sel).remove();
    }

    // LA PODA DE PRENSA, internacional: los venenos universales de los
    // medios (promos, relacionados, compartir, cookies, comentarios,
    // newsletters). Selectores medidos: por clase EXACTA o prefijo claro,
    // nunca substrings golosos que se lleven contenido por delante.
    for sel in [
        "aside",
        "[role=\"complementary\"]",
        "[class^=\"related\"]",
        "[class*=\"related-content\"]",
        "[class*=\"related-articles\"]",
        "[class*=\"related-stories\"]",
        "[class*=\"recirculation\"]",
        // Solo las FORMAS de widget: en Substack el post entero se llama
        // «newsletter-post» y un substring goloso se comía el artículo.
        "[class*=\"newsletter-signup\"]",
        "[class*=\"newsletter-widget\"]",
        "[class*=\"newsletter-form\"]",
        "[class*=\"newsletter-subscribe\"]",
        "[class*=\"newsletter-promo\"]",
        "[class*=\"newsletter-banner\"]",
        "[class*=\"subscribe-\"]",
        "[class*=\"subscription-\"]",
        "[class*=\"paywall\"]",
        "[class*=\"cookie-banner\"]",
        "[class*=\"cookie-consent\"]",
        "[class*=\"consent-banner\"]",
        "[id*=\"cookie-banner\"]",
        "[class*=\"share-bar\"]",
        "[class*=\"share-buttons\"]",
        "[class*=\"social-share\"]",
        "[class*=\"social-links\"]",
        "[class*=\"comments-section\"]",
        "[id=\"comments\"]",
        "[class*=\"comment-count\"]",
        "[class*=\"advertisement\"]",
        "[class^=\"ad-\"]",
        "[class*=\"-advert\"]",
        "[data-testid*=\"ad-\"]",
        "[class*=\"breadcrumb\"]",
        "[class*=\"most-read\"]",
        "[class*=\"most-viewed\"]",
        "[class*=\"trending\"]",
        "[class*=\"read-more\"]",
        "[class*=\"tags-list\"]",
        "[class*=\"article-tags\"]",
        "[class*=\"author-bio\"]",
        "[class*=\"support-us\"]",
        "[class*=\"donation\"]",
        "[class*=\"promo-box\"]",
        "[class*=\"outbrain\"]",
        "[class*=\"taboola\"]",
    ] {
        doc.select(sel).remove();
    }

    // Y los GRANDES, por nombre: cada casa esconde su morralla en clases
    // propias. Se poda solo lo verificado como no-contenido.
    let dominio = url;
    let por_sitio: &[&str] = if dominio.contains("elpais.com") {
        &[
            "[class*=\"a_md_\"]",
            "[class*=\"sumario\"]",
            ".a_e_txt",
            ".cs_t",
            "[class*=\"a_t_i\"]",
        ]
    } else if dominio.contains("elmundo.es") {
        &[
            "[class*=\"ue-c-article__premium\"]",
            ".ue-c-article__byline",
            "[class*=\"ue-c-ad\"]",
        ]
    } else if dominio.contains("eldiario.es") {
        // OJO: el CUERPO vive en «partner-wrapper article-page__body-row»
        // (eldiario llama «partner» a su maquetación con huecos de anuncio):
        // nada de [class*="partner"] goloso, solo los bloques verificados.
        &[
            ".cmp-focos-elDiario-partner",
            ".cmp-last-minute-bar-partner",
            "[class*=\"news-sponsored\"]",
            "[class*=\"branded-content\"]",
            ".news-header-info",
            "[class*=\"footer-info\"]",
        ]
    } else if dominio.contains("lavanguardia.com") {
        &["[class*=\"epigraph\"]", "[class*=\"author-data\"]"]
    } else if dominio.contains("nytimes.com") {
        &[
            "[data-testid*=\"share\"]",
            "[class*=\"bottom-of-article\"]",
            "#bottom-wrapper",
        ]
    } else if dominio.contains("theguardian.com") {
        &[
            "[data-component=\"meta-byline\"]",
            "gu-island",
            "[data-component=\"nav\"]",
            "[name=\"FooterLinks\"]",
        ]
    } else if dominio.contains("bbc.co") || dominio.contains("bbc.com") {
        &[
            "[data-component=\"tag-list\"]",
            "[data-component=\"links-block\"]",
            "[data-component=\"topic-list\"]",
            "[data-component=\"byline-block\"]",
            "[data-e2e=\"most-read\"]",
            "[data-testid=\"most-read\"]",
            "[data-e2e*=\"recommendations\"]",
        ]
    } else if dominio.contains("washingtonpost.com") {
        &["[data-qa*=\"subscribe\"]", "[data-qa=\"article-byline\"]"]
    } else if dominio.contains("lemonde.fr") {
        &["[class*=\"meta__\"]", ".article__reactions", ".services"]
    } else if dominio.contains("spiegel.de")
        || dominio.contains("zeit.de")
        || dominio.contains("faz.net")
    {
        &[
            "[class*=\"paywall\"]",
            "[class*=\"metadata\"]",
            "[class*=\"sharing\"]",
        ]
    } else if dominio.contains("corriere.it") || dominio.contains("repubblica.it") {
        &[
            "[class*=\"paywall\"]",
            "[class*=\"social\"]",
            "[class*=\"firma\"]",
        ]
    } else if dominio.contains("folha.uol") || dominio.contains("globo.com") {
        &["[class*=\"share\"]", "[class*=\"paywall\"]"]
    } else if dominio.contains("xataka.com") || dominio.contains("genbeta.com") {
        &[
            "[class*=\"social-buttons\"]",
            ".article-taxonomy",
            "[class*=\"recommendations\"]",
        ]
    } else if dominio.contains("medium.com") || dominio.contains("substack.com") {
        &[
            "[class*=\"metabar\"]",
            "[class*=\"postMeterBar\"]",
            "[class*=\"subscription-widget\"]",
            "[class*=\"subscribe-widget\"]",
            "[class*=\"captioned-image\"] figcaption",
        ]
    } else if dominio.contains("stackoverflow.com") {
        &[
            ".js-voting-container",
            ".user-info",
            ".comments",
            ".bottom-notice",
            "[id*=\"sidebar\"]",
        ]
    } else if dominio.contains("reddit.com") {
        &[".side", ".footer-parent", ".infobar", "#header"]
    } else if dominio.contains("archive.org") {
        &[
            "[class*=\"donation\"]",
            "[id*=\"donato\"]",
            "[class*=\"banner\"]",
        ]
    } else {
        &[]
    };
    for sel in por_sitio {
        doc.select(sel).remove();
    }

    let es_wiki = html.contains("mw-parser-output")
        || url.contains("wikipedia.org")
        || url.contains("wikiwand.com")
        || url.contains("fandom.com");
    if es_wiki {
        for sel in [
            "#toc",
            ".toc",
            ".mw-editsection",
            "sup.reference",
            ".reference",
            ".reflist",
            "ol.references",
            ".mw-references-wrap",
            ".infobox",
            ".navbox",
            ".vertical-navbox",
            ".sidebar",
            ".hatnote",
            ".metadata",
            ".ambox",
            ".thumb",
            ".thumbcaption",
            ".gallery",
            ".catlinks",
            ".printfooter",
            ".shortdescription",
            ".mw-jump-link",
            ".noprint",
            ".dablink",
            ".rellink",
            ".magnify",
            ".mw-cite-backlink",
            ".side-box",
            ".portalbox",
            "table",
        ] {
            doc.select(sel).remove();
        }
        // Las SECCIONES DE COLA: del encabezado de referencias en adelante,
        // nada se lee. Multiidioma (es/en/fr/de/it/pt y variantes).
        let colas = [
            "referencias",
            "véase también",
            "vease tambien",
            "enlaces externos",
            "bibliografía",
            "bibliografia",
            "notas",
            "notas y referencias",
            "references",
            "see also",
            "external links",
            "bibliography",
            "notes",
            "further reading",
            "cited sources",
            "works cited",
            "citations",
            "footnotes",
            "sources",
            "références",
            "voir aussi",
            "liens externes",
            "einzelnachweise",
            "weblinks",
            "literatur",
            "siehe auch",
            "note",
            "voci correlate",
            "collegamenti esterni",
            "referências",
            "ver também",
            "ligações externas",
            "примечания",
            "ссылки",
            "литература",
            "см. также",
            "источники",
            "библиография",
            "脚注",
            "出典",
            "参考文献",
            "関連項目",
            "外部リンク",
            "注釈",
            "参考资料",
            "外部链接",
            "参见",
            "注释",
            "مراجع",
            "انظر أيضا",
            "انظر أيضًا",
            "وصلات خارجية",
            "ملاحظات",
            "مصادر",
        ];
        // «Redirigido desde…» y demás avisos de redirección.
        doc.select(".mw-redirectedfrom, #contentSub, .mw-indicators")
            .remove();
        let mut cortar = false;
        for nodo in doc.select("h2, h3").nodes() {
            let texto = nodo.text().to_lowercase();
            let texto = texto.trim();
            if !cortar
                && colas
                    .iter()
                    .any(|c| texto == *c || texto.starts_with(&format!("{c}[")))
            {
                cortar = true;
            }
            if cortar {
                // La Wikipedia moderna ENVUELVE cada h2 en un div
                // .mw-heading (y el HTML de Parsoid, cada sección entera
                // en un <section>): el corte debe operar sobre el
                // envoltorio MÁS ALTO, o solo decapita el título y deja
                // el cuerpo de la cola (o la sección hermana siguiente).
                let objetivo = match nodo.parent() {
                    Some(p) if p.has_class("mw-heading") => match p.parent() {
                        Some(s)
                            if s.node_name()
                                .map(|n| n.eq_ignore_ascii_case("section"))
                                .unwrap_or(false) =>
                        {
                            s
                        }
                        _ => p,
                    },
                    _ => *nodo,
                };
                let mut siguiente = objetivo.next_element_sibling();
                objetivo.remove_from_parent();
                while let Some(n) = siguiente {
                    siguiente = n.next_element_sibling();
                    n.remove_from_parent();
                }
                // Sin break: en estructuras anidadas pueden quedar colas
                // vivas fuera del corte (secciones hermanas de otro nivel);
                // quitar un nodo ya desprendido es inofensivo.
            }
        }
    }
    preservar_versos(&doc);
    doc.html().to_string()
}

/// LOS VERSOS SOBREVIVEN: Readability colapsa los saltos de línea
/// LITERALES (white-space: pre-wrap) y convertía cualquier poema en una
/// masa sin puntos. Aquí, todo bloque hoja cuyo texto tenga pinta de
/// verso (tres o más líneas cortas) cambia sus \n por <br>, que el
/// extractor SÍ respeta (comprobado empíricamente).
fn preservar_versos(doc: &dom_query::Document) {
    for sel in doc.select("p, div, blockquote, li, td, pre").iter() {
        // Solo HOJAS de texto: si dentro hay más bloques, ya se visitarán.
        if sel
            .select("p, div, ul, ol, table, blockquote, h1, h2, h3, pre")
            .exists()
        {
            continue;
        }
        let interior = sel.html().to_string();
        if !interior.contains('\n') || interior.contains("<br") {
            continue;
        }
        let texto = sel.text().to_string();
        let lineas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).collect();
        if lineas.len() < 3 {
            continue;
        }
        let media = lineas.iter().map(|l| l.chars().count()).sum::<usize>() / lineas.len();
        if media > 60 {
            continue;
        }
        sel.set_html(interior.replace('\n', "<br>"));
    }
}

/// La limpieza HABLADA del markdown final: marcas de cita [1] [nota 2],
/// restos de [editar], líneas de tabla y flechas de referencia. Vale para
/// TODO origen (también documentos y pegados).
pub fn limpiar_markdown_hablado(md: &str) -> String {
    // EL IDIOMA DEL TEXTO manda en las adaptaciones habladas: el marcador
    // de código, las abreviaturas y las monedas se dicen en su lengua.
    let idioma = yappy_core::lang_detect::detect_document_lang(md, "es");
    limpiar_markdown_hablado_en(md, &idioma)
}

/// El marcador hablado de un bloque de código, en el idioma del texto.
fn marcador_codigo(idioma: &str) -> &'static str {
    match idioma {
        "es" => "(Hay un ejemplo de código.)",
        "fr" => "(Il y a un exemple de code.)",
        "de" => "(Hier steht ein Codebeispiel.)",
        "it" => "(C'è un esempio di codice.)",
        "pt" => "(Há um exemplo de código.)",
        _ => "(There is a code example.)",
    }
}

pub fn limpiar_markdown_hablado_en(md: &str, idioma: &str) -> String {
    // Las líneas-botón que ninguna voz debe pronunciar.
    let botones = [
        "anterior",
        "siguiente",
        "previous",
        "next",
        "compartir",
        "share",
        "escribir comentario",
        "leer más",
        "read more",
        "fin de más leídas",
        "final de más leídas",
        "copiar enlace",
        "suscríbete",
        "subscribe",
        "_",
    ];
    let mut out: Vec<String> = Vec::new();
    let mut contador_lista: usize = 0;
    let mut en_codigo = false;
    for linea in md.lines() {
        let l = linea.trim_end();
        let compacta = l.trim();
        // LOS BLOQUES DE CÓDIGO no se leen en voz alta: se anuncian una
        // vez («hay un ejemplo de código») y se saltan enteros.
        if compacta.starts_with("```") || compacta.starts_with("~~~") {
            if !en_codigo {
                out.push(marcador_codigo(idioma).to_string());
                out.push(String::new());
            }
            en_codigo = !en_codigo;
            continue;
        }
        if en_codigo {
            continue;
        }
        // Líneas que jamás se leen: tablas markdown y flechas de cita.
        if compacta.starts_with('|') && compacta.matches('|').count() >= 2 {
            continue;
        }
        if compacta == "↑"
            || (!compacta.is_empty()
                && compacta.len() > 2
                && compacta.chars().all(|c| "↑↓·|-–—:_* ".contains(c)))
        {
            continue;
        }
        let mut limpia = quitar_marcas_de_cita(l);
        limpia = aplanar_enlaces(&limpia);
        limpia = silenciar_enfasis(&limpia);
        limpia = despegar_palabras(&limpia);
        limpia = quitar_emojis(&limpia);
        limpia = normalizar_hablado_en(&limpia, idioma);
        let plana = limpia.trim().trim_start_matches("- ").trim().to_lowercase();
        if botones.contains(&plana.as_str()) {
            continue;
        }
        // Las coletillas sociales de cierre («Puedes seguir a MATERIA en
        // Facebook…», «Síguenos en…») tampoco son prosa.
        let sin_almohadillas = plana.trim_start_matches('#').trim_start();
        if [
            "puedes seguir a ",
            "síguenos en ",
            "siguenos en ",
            "follow us on ",
            "follow me on ",
        ]
        .iter()
        .any(|p| sin_almohadillas.starts_with(p))
        {
            continue;
        }
        // Las listas «1. 1. 1.» (el numerado perdido) se renumeran.
        let cuerpo_lista = limpia.trim_start();
        if let Some(resto) = cuerpo_lista.strip_prefix("1. ") {
            contador_lista += 1;
            if contador_lista > 1 {
                let sangria = &limpia[..limpia.len() - cuerpo_lista.len()];
                limpia = format!("{sangria}{contador_lista}. {resto}");
            }
        } else if !cuerpo_lista.is_empty() {
            contador_lista = 0;
        }
        out.push(limpia);
    }
    // El código SANGRADO (cuatro espacios, estilo markdown clásico):
    // tres líneas seguidas con pinta de código se anuncian y se callan.
    colapsar_codigo_sangrado(&mut out, idioma);
    // LAS ESTROFAS: tres o más «párrafos» seguidos de una línea corta sin
    // punto final son VERSOS que la extracción separó de más; se re-unen
    // con salto simple (una estrofa = una pieza, pausa de verso, no de
    // párrafo).
    reunir_estrofas(&mut out);
    // EL MURO DE CIERRE: si en el último tramo aparece la invitación a
    // suscribirse («Suscríbete para seguir leyendo», «Lee sin límites»),
    // de ahí al final ya no hay artículo: se corta.
    let muros_de_cola = [
        "suscríbete para seguir leyendo",
        "suscribete para seguir leyendo",
        "subscribe to continue reading",
        "subscribe to read more",
        "lee sin límites",
        "lee sin limites",
        "hazte suscriptor",
        "already a subscriber",
    ];
    let desde = out.len().saturating_sub((out.len() / 3).max(12));
    if let Some(corte) = (desde..out.len()).find(|&i| {
        let pelada = out[i].trim().trim_start_matches('#').trim().to_lowercase();
        muros_de_cola.contains(&pelada.as_str())
    }) {
        out.truncate(corte);
    }
    // Colapsar el exceso de líneas vacías que deja la poda.
    let mut texto = out.join("\n");
    while texto.contains("\n\n\n") {
        texto = texto.replace("\n\n\n", "\n\n");
    }
    texto.trim().to_string()
}

/// ¿Esta línea es un VERSO suelto? Corta, con letras, sin cierre de
/// frase (los títulos # no cuentan).
fn parece_verso_suelto(l: &str) -> bool {
    let t = l.trim();
    if t.is_empty() || t.starts_with('#') {
        return false;
    }
    let n = t.chars().count();
    (2..=60).contains(&n)
        && t.chars().any(|c| c.is_alphabetic())
        && !t.ends_with(['.', '!', '?', ':', ';'])
}

/// Re-une en estrofas los versos que quedaron como párrafos sueltos:
/// runs de ≥3 versos separados por UNA línea en blanco cada uno.
fn reunir_estrofas(out: &mut Vec<String>) {
    let mut i = 0;
    while i < out.len() {
        if !parece_verso_suelto(&out[i]) {
            i += 1;
            continue;
        }
        // Medir el run: verso, blanco, verso, blanco, verso…
        let mut j = i;
        let mut versos = 1usize;
        loop {
            let mut k = j + 1;
            let mut blancos = 0;
            while k < out.len() && out[k].trim().is_empty() {
                blancos += 1;
                k += 1;
            }
            if blancos == 1 && k < out.len() && parece_verso_suelto(&out[k]) {
                versos += 1;
                j = k;
            } else {
                break;
            }
        }
        if versos >= 3 {
            // Sustituir el tramo [i..=j] por la estrofa con saltos simples.
            let estrofa: Vec<String> = out[i..=j]
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect();
            out.splice(i..=j, [estrofa.join("\n")]);
        }
        i += 1;
    }
}

/// ¿Esta línea parece CÓDIGO? Sangría de 4+ espacios y densidad alta
/// de símbolos de programa.
fn parece_codigo(l: &str) -> bool {
    if !l.starts_with("    ") || l.trim().is_empty() {
        return false;
    }
    let t = l.trim();
    let simbolos = t
        .chars()
        .filter(|c| "{}()[];=<>|&$#\\/_→+".contains(*c))
        .count();
    if simbolos * 3 >= t.chars().count() {
        return true;
    }
    [
        "//",
        "def ",
        "fn ",
        "var ",
        "let ",
        "const ",
        "import ",
        "from ",
        "function ",
        "return ",
        "for ",
        "while ",
        "if ",
        "else",
        "elif ",
        "class ",
        "print",
        "console.",
        "#include",
        "public ",
        "private ",
        "void ",
        "int ",
        "match ",
        "use ",
    ]
    .iter()
    .any(|p| t.starts_with(p))
}

/// Runs de ≥3 líneas de código sangrado: se anuncian una vez y se callan.
fn colapsar_codigo_sangrado(out: &mut Vec<String>, idioma: &str) {
    let mut i = 0;
    while i < out.len() {
        if parece_codigo(&out[i]) {
            let mut j = i;
            // El run admite líneas vacías INTERIORES entre código.
            let mut ultimas_codigo = i;
            while j < out.len() && (parece_codigo(&out[j]) || out[j].trim().is_empty()) {
                if parece_codigo(&out[j]) {
                    ultimas_codigo = j;
                }
                j += 1;
            }
            let lineas_codigo = out[i..=ultimas_codigo]
                .iter()
                .filter(|l| parece_codigo(l))
                .count();
            if lineas_codigo >= 3 {
                out.splice(i..=ultimas_codigo, [marcador_codigo(idioma).to_string()]);
                i += 1;
                continue;
            }
            i = j;
        } else {
            i += 1;
        }
    }
}

/// Los emojis no se pronuncian: fuera del texto hablado.
fn quitar_emojis(linea: &str) -> String {
    linea
        .chars()
        .filter(|c| {
            let u = *c as u32;
            !((0x1F000..=0x1FAFF).contains(&u)
                || (0x2600..=0x27BF).contains(&u)
                || (0x2B00..=0x2BFF).contains(&u)
                || (0xFE00..=0xFE0F).contains(&u)
                || u == 0x200D
                || (0x1F1E6..=0x1F1FF).contains(&u))
        })
        .collect()
}

/// NORMALIZACIÓN HABLADA conservadora, en el idioma del texto: en
/// español y en inglés se habla su lengua; en el resto solo lo
/// inequívoco y universal (nada de meter palabras de otro idioma).
fn normalizar_hablado_en(linea: &str, idioma: &str) -> String {
    match idioma {
        "es" => normalizar_hablado_es(linea),
        "en" => normalizar_hablado_ingles(linea),
        _ => linea.to_string(),
    }
}

/// La versión inglesa: percent, monedas, rangos con «to».
fn normalizar_hablado_ingles(linea: &str) -> String {
    let mut t = linea.to_string();
    t = t.replace(" & ", " and ");
    t = reemplazar_sufijo_numerico(&t, "%", "percent", "percent");
    t = reemplazar_sufijo_numerico(&t, "€", "euro", "euros");
    t = reemplazar_sufijo_numerico(&t, "£", "pound", "pounds");
    t = reemplazar_moneda_prefija(&t, '$', "dollar", "dollars");
    t = reemplazar_rango_de_anos_con(&t, " to ");
    t
}

/// NORMALIZACIÓN HABLADA conservadora en ESPAÑOL: solo transformaciones
/// inequívocas (lo dudoso se deja tal cual, que la voz ya se defiende).
fn normalizar_hablado_es(linea: &str) -> String {
    let mut t = linea.to_string();
    // El ampersand entre palabras.
    t = t.replace(" & ", " y ");
    // Abreviaturas españolas de siempre (la tabla exige límite de palabra
    // por delante y el punto por detrás; «D.» se queda: es inicial).
    // Las que pueden cerrar la frase (el punto es suyo Y del final)
    // recuperan el punto si lo siguiente es mayúscula o el fin.
    for (abrev, entero) in [
        ("EE. UU.", "Estados Unidos"),
        ("EE.UU.", "Estados Unidos"),
        ("a. C.", "antes de Cristo"),
        ("a.C.", "antes de Cristo"),
        ("d. C.", "después de Cristo"),
        ("d.C.", "después de Cristo"),
    ] {
        t = reemplazar_cierre_posible(&t, abrev, entero);
    }
    let tabla: &[(&str, &str)] = &[
        ("Sr.", "señor"),
        ("Sra.", "señora"),
        ("Sres.", "señores"),
        ("Dr.", "doctor"),
        ("Dra.", "doctora"),
        ("Dña.", "doña"),
        ("Ud.", "usted"),
        ("Vd.", "usted"),
        ("Uds.", "ustedes"),
        ("núm.", "número"),
        ("pág.", "página"),
        ("págs.", "páginas"),
        ("art.", "artículo"),
        ("cap.", "capítulo"),
        ("vol.", "volumen"),
    ];
    for (abrev, entero) in tabla {
        t = reemplazar_abreviatura(&t, abrev, entero);
    }
    // «etc.» conserva el punto si cerraba la frase.
    t = reemplazar_etc(&t);
    // Porcentajes: «3,5 %» y «3.5%» se dicen «por ciento».
    t = reemplazar_sufijo_numerico(&t, "%", "por ciento", "por ciento");
    // Monedas tras el número (el uso español).
    t = reemplazar_sufijo_numerico(&t, "€", "euro", "euros");
    t = reemplazar_sufijo_numerico(&t, "£", "libra", "libras");
    // El dólar delante del número (el uso inglés): $5 → 5 dólares.
    t = reemplazar_moneda_prefija(&t, '$', "dólar", "dólares");
    // Rangos de años: «1936-1939» → «1936 a 1939».
    t = reemplazar_rango_de_anos_con(&t, " a ");
    t
}

/// Reemplaza una abreviatura con límite de palabra por delante.
fn reemplazar_abreviatura(texto: &str, abrev: &str, entero: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(pos) = resto.find(abrev) {
        let antes_ok = pos == 0
            || resto[..pos]
                .chars()
                .last()
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(true);
        let tras = &resto[pos + abrev.len()..];
        let despues_ok = tras.is_empty() || tras.starts_with([' ', '\u{a0}']);
        out.push_str(&resto[..pos]);
        if antes_ok && despues_ok {
            out.push_str(entero);
        } else {
            out.push_str(abrev);
        }
        resto = tras;
    }
    out.push_str(resto);
    out
}

/// Abreviatura cuyo punto puede SER el punto final de la frase (EE. UU.,
/// a. C.): si lo siguiente es mayúscula o el fin, se repone el punto.
fn reemplazar_cierre_posible(texto: &str, abrev: &str, entero: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(pos) = resto.find(abrev) {
        let antes_ok = pos == 0
            || resto[..pos]
                .chars()
                .last()
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(true);
        let tras = &resto[pos + abrev.len()..];
        out.push_str(&resto[..pos]);
        if antes_ok {
            let cierra = tras
                .trim_start()
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(true);
            out.push_str(entero);
            if cierra {
                out.push('.');
            }
        } else {
            out.push_str(abrev);
        }
        resto = tras;
    }
    out.push_str(resto);
    out
}

/// «etc.» → «etcétera», conservando el punto cuando cerraba la frase
/// (lo delata la mayúscula siguiente o el fin de línea).
fn reemplazar_etc(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(pos) = resto.find("etc.") {
        let antes_ok = pos == 0
            || resto[..pos]
                .chars()
                .last()
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(true);
        let tras = &resto[pos + 4..];
        out.push_str(&resto[..pos]);
        if antes_ok {
            let cierra_frase = tras
                .trim_start()
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(true);
            out.push_str(if cierra_frase {
                "etcétera."
            } else {
                "etcétera"
            });
        } else {
            out.push_str("etc.");
        }
        resto = tras;
    }
    out.push_str(resto);
    out
}

/// «12,50 €» → «12,50 euros» (y «1 €» → «1 euro»): el símbolo que sigue a
/// un número, con o sin espacio.
fn reemplazar_sufijo_numerico(texto: &str, simbolo: &str, singular: &str, plural: &str) -> String {
    let cs: Vec<char> = texto.chars().collect();
    let sim: Vec<char> = simbolo.chars().collect();
    let mut out = String::with_capacity(texto.len() + 8);
    let mut i = 0;
    while i < cs.len() {
        if cs[i..].starts_with(&sim[..]) && i > 0 {
            // Hacia atrás: espacio opcional y el número.
            let mut j = i;
            if j > 0 && cs[j - 1] == ' ' {
                j -= 1;
            }
            let fin_num = j;
            while j > 0 && (cs[j - 1].is_ascii_digit() || cs[j - 1] == ',' || cs[j - 1] == '.') {
                j -= 1;
            }
            let numero: String = cs[j..fin_num].iter().collect();
            if !numero.is_empty() && numero.chars().any(|c| c.is_ascii_digit()) {
                let palabra = if numero == "1" { singular } else { plural };
                // Lo ya emitido incluye el número: garantizar el espacio.
                if !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(palabra);
                i += sim.len();
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// «$3,2 millones» → «3,2 dólares millones» NO: solo «$N» pelado → «N dólares».
fn reemplazar_moneda_prefija(texto: &str, simbolo: char, singular: &str, plural: &str) -> String {
    let cs: Vec<char> = texto.chars().collect();
    let mut out = String::with_capacity(texto.len() + 8);
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == simbolo && i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == ',' || cs[j] == '.') {
                j += 1;
            }
            // Si después viene una PALABRA de cantidad (millones…), no se
            // toca: «$3,2 millones» leído «3,2 dólares millones» es peor.
            let cola: String = cs[j..].iter().take(12).collect();
            let cantidad = ["millon", "millón", "billion", "million", "mil "]
                .iter()
                .any(|m| cola.trim_start().to_lowercase().starts_with(m));
            if !cantidad {
                let numero: String = cs[i + 1..j].iter().collect();
                let palabra = if numero == "1" { singular } else { plural };
                out.push_str(&numero);
                out.push(' ');
                out.push_str(palabra);
                i = j;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// «1936-1939» → «1936 a 1939» (solo año-año: cuatro y cuatro cifras).
fn reemplazar_rango_de_anos_con(texto: &str, conector: &str) -> String {
    let cs: Vec<char> = texto.chars().collect();
    let mut out = String::with_capacity(texto.len() + 4);
    let mut i = 0;
    while i < cs.len() {
        if (cs[i] == '-' || cs[i] == '–' || cs[i] == '—')
            && i >= 4
            && i + 4 < cs.len()
            && cs[i - 4..i].iter().all(|c| c.is_ascii_digit())
            && cs[i + 1..i + 5].iter().all(|c| c.is_ascii_digit())
            && (i < 5 || !cs[i - 5].is_ascii_digit())
            && (i + 5 >= cs.len() || !cs[i + 5].is_ascii_digit())
        {
            out.push_str(conector);
            i += 1;
            continue;
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// APLANAR ENLACES: «[texto](url "título")» queda en «texto» (el ancla es
/// prosa; la URL no se pronuncia jamás); «![alt](url)» desaparece entera;
/// una URL desnuda entre paréntesis o suelta también calla.
fn aplanar_enlaces(linea: &str) -> String {
    let cs: Vec<char> = linea.chars().collect();
    let mut out = String::with_capacity(linea.len());
    let mut i = 0;
    while i < cs.len() {
        // Imagen: ![alt](…): fuera entera.
        let es_imagen = cs[i] == '!' && cs.get(i + 1) == Some(&'[');
        if es_imagen || cs[i] == '[' {
            let abre = if es_imagen { i + 1 } else { i };
            if let Some(cierra_rel) = corchete_cierre(&cs, abre) {
                let despues = cierra_rel + 1;
                if cs.get(despues) == Some(&'(') {
                    if let Some(par_rel) = parentesis_cierre(&cs, despues) {
                        if !es_imagen {
                            let texto: String = cs[abre + 1..cierra_rel].iter().collect();
                            // Un ancla cuyo texto ES una URL también calla.
                            if !parece_url(texto.trim()) {
                                out.push_str(&texto);
                            }
                        }
                        i = par_rel + 1;
                        continue;
                    }
                }
            }
        }
        // URL desnuda entre paréntesis: «(https://…)» entera fuera.
        if cs[i] == '(' {
            if let Some(par_rel) = parentesis_cierre(&cs, i) {
                let interior: String = cs[i + 1..par_rel].iter().collect();
                if parece_url(interior.trim()) {
                    i = par_rel + 1;
                    continue;
                }
            }
        }
        // URL desnuda en medio del texto: se calla hasta el espacio.
        if cs[i] == 'h' {
            let resto: String = cs[i..].iter().take(8).collect();
            if resto.starts_with("http://") || resto.starts_with("https://") {
                while i < cs.len() && !cs[i].is_whitespace() {
                    i += 1;
                }
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

fn corchete_cierre(cs: &[char], abre: usize) -> Option<usize> {
    let mut nivel = 0;
    for (k, &c) in cs.iter().enumerate().skip(abre) {
        if c == '[' {
            nivel += 1;
        } else if c == ']' {
            nivel -= 1;
            if nivel == 0 {
                return Some(k);
            }
        }
    }
    None
}

fn parentesis_cierre(cs: &[char], abre: usize) -> Option<usize> {
    let mut nivel = 0;
    for (k, &c) in cs.iter().enumerate().skip(abre) {
        if c == '(' {
            nivel += 1;
        } else if c == ')' {
            nivel -= 1;
            if nivel == 0 {
                return Some(k);
            }
        }
    }
    None
}

fn parece_url(t: &str) -> bool {
    t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("www.")
        || t.starts_with('#')
        || (t.contains('.')
            && !t.contains(' ')
            && t.len() > 4
            && t.chars().filter(|c| *c == '.').count() >= 1
            && t.rsplit('.')
                .next()
                .map(|ext| ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or(false)
            && !t.ends_with('.'))
}

/// El énfasis no se pronuncia: los asteriscos (enteros o huérfanos, el
/// azote de La Vanguardia: «revista* Science *.») desaparecen del habla.
fn silenciar_enfasis(linea: &str) -> String {
    let compacta = linea.trim_start();
    // Los encabezados markdown («# », «## ») se respetan: son del guion.
    let (prefijo, cuerpo) = if compacta.starts_with('#') {
        let n = linea.len() - compacta.len();
        let corte = n + compacta
            .chars()
            .take_while(|c| *c == '#' || *c == ' ')
            .count();
        (&linea[..corte], &linea[corte..])
    } else {
        (&linea[..0], linea)
    };
    let sin: String = cuerpo.chars().filter(|c| *c != '*').collect();
    format!("{prefijo}{sin}")
}

/// El título sin la marca del sitio: «Cuento - Autor - Ciudad Seva» se
/// queda en «Cuento - Autor» (el ÚLTIMO segmento cae si parece marca:
/// corto y sin puntuación de frase). «GitHub - tauri-apps/tauri: …»
/// también suelta su prefijo-marca cuando va delante.
fn recortar_marca_de_sitio(titulo: &str) -> String {
    let mut t = titulo.trim().to_string();
    for sep in [" - ", " | ", " — ", " – "] {
        if let Some(pos) = t.rfind(sep) {
            let cola = t[pos + sep.len()..].trim();
            let palabras = cola.split_whitespace().count();
            if (1..=4).contains(&palabras)
                && !cola.contains('.')
                && t[..pos].split_whitespace().count() >= 2
            {
                t = t[..pos].trim().to_string();
            }
        }
    }
    t
}

/// Despegar palabras que la poda dejó unidas: «Cádiz.Los» y «,Cuando».
fn despegar_palabras(linea: &str) -> String {
    let cs: Vec<char> = linea.chars().collect();
    let mut out = String::with_capacity(linea.len() + 8);
    for k in 0..cs.len() {
        out.push(cs[k]);
        if k + 1 < cs.len() {
            let a = cs[k];
            let b = cs[k + 1];
            if (a == '.' || a == ',' || a == ':' || a == ';')
                && b.is_alphabetic()
                && b.is_uppercase()
            {
                // «x.Los» pide aire; «EE.UU» y «S.A» no (sigla corta).
                let previa_mayus = k >= 1 && cs[k - 1].is_uppercase();
                if !previa_mayus {
                    out.push(' ');
                }
            }
        }
    }
    out
}

/// Quita [1], [23], [nota 4], [cita requerida], [editar] y hermanas, en
/// cualquier posición de la línea, sin tocar corchetes con contenido real.
fn quitar_marcas_de_cita(linea: &str) -> String {
    let mut out = String::with_capacity(linea.len());
    let cs: Vec<char> = linea.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '[' {
            if let Some(cierre) = cs[i..].iter().position(|&c| c == ']') {
                let interior: String = cs[i + 1..i + cierre].iter().collect();
                let int = interior.trim().to_lowercase();
                let es_numero = !int.is_empty() && int.chars().all(|c| c.is_ascii_digit());
                let es_nota = int
                    .strip_prefix("nota ")
                    .map(|r| r.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or(false)
                    || int
                        .strip_prefix("note ")
                        .map(|r| r.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or(false)
                    || int
                        .strip_prefix("n ")
                        .map(|r| r.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or(false);
                let es_marca = matches!(
                    int.as_str(),
                    "editar"
                        | "edit"
                        | "cita requerida"
                        | "citation needed"
                        | "aclaración requerida"
                        | "cuál"
                        | "cuándo"
                        | "quién"
                        | "sic"
                );
                if es_numero || es_nota || es_marca {
                    i += cierre + 1;
                    continue;
                }
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// La extracción del artículo, por DOS vías que compiten:
///
/// 1. el JSON-LD del propio sitio (articleBody): el texto del editor,
///    limpio de nacimiento (la prensa seria lo publica entero, muro
///    visual aparte);
/// 2. Readability (dom_smoothie) sobre el DOM podado.
///
/// Gana la más larga (el JSON-LD a veces viene truncado por el muro; a
/// Readability a veces se le cuela morralla): en el empate razonable,
/// el JSON-LD, que es más limpio.
pub fn extraer_articulo(html: &str, url: &str) -> Result<(Option<String>, String)> {
    let ld = articulo_desde_ldjson(html);
    let readability = extraer_con_readability(html, url);
    match (ld, readability) {
        (Some((t_ld, md_ld)), Ok((t_rd, md_rd))) => {
            if md_ld.chars().count() * 10 >= md_rd.chars().count() * 9 {
                Ok((t_ld.or(t_rd), md_ld))
            } else {
                Ok((t_rd, md_rd))
            }
        }
        (Some((t_ld, md_ld)), Err(_)) => Ok((t_ld, md_ld)),
        (None, r) => r,
    }
}

/// El artículo servido en bandeja por el sitio: JSON-LD (schema.org) con
/// articleBody. Devuelve título y markdown ya limpios, o None.
fn articulo_desde_ldjson(html: &str) -> Option<(Option<String>, String)> {
    let doc = dom_query::Document::from(html);
    for guion in doc.select(r#"script[type="application/ld+json"]"#).iter() {
        let crudo = guion.text();
        let Ok(v) = serde_json::from_str::<serde_json::Value>(crudo.trim()) else {
            continue;
        };
        // El objeto puede venir suelto, en lista, o dentro de @graph.
        let mut candidatos: Vec<&serde_json::Value> = Vec::new();
        match &v {
            serde_json::Value::Array(a) => candidatos.extend(a.iter()),
            obj => {
                candidatos.push(obj);
                if let Some(g) = obj.get("@graph").and_then(|g| g.as_array()) {
                    candidatos.extend(g.iter());
                }
            }
        }
        for c in candidatos {
            let Some(cuerpo) = c.get("articleBody").and_then(|b| b.as_str()) else {
                continue;
            };
            if cuerpo.chars().count() < 900 {
                continue;
            }
            let titulo = c
                .get("headline")
                .and_then(|h| h.as_str())
                .map(|h| recortar_marca_de_sitio(desescapar_entidades(h).trim()))
                .filter(|h| !h.is_empty());
            let plano = desescapar_entidades(cuerpo);
            let con_parrafos = partir_en_parrafos(&plano);
            let cuerpo_md = limpiar_markdown_hablado(&con_parrafos);
            if cuerpo_md.chars().count() < 600 {
                continue;
            }
            let markdown = match &titulo {
                Some(t) => format!("# {t}\n\n{cuerpo_md}"),
                None => cuerpo_md,
            };
            return Some((titulo, markdown));
        }
    }
    None
}

/// Las cinco entidades HTML que sobreviven dentro del JSON-LD.
fn desescapar_entidades(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// El articleBody suele llegar de un tirón, sin saltos: para navegar por
/// párrafos (y respirar), se parte por los \n que traiga o, si no trae,
/// por frases agrupadas en párrafos de unos 500 caracteres.
fn partir_en_parrafos(texto: &str) -> String {
    if texto.contains('\n') {
        return texto
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
    }
    let mut parrafos: Vec<String> = Vec::new();
    let mut actual = String::new();
    let mut chars = texto.chars().peekable();
    let mut frase = String::new();
    while let Some(c) = chars.next() {
        frase.push(c);
        let cierra = matches!(c, '.' | '!' | '?' | '…')
            && chars.peek().map(|n| n.is_whitespace()).unwrap_or(true);
        if cierra {
            // Las abreviaturas de una letra («J. Luis») no cierran frase.
            let previa = frase.trim_end_matches(c).trim_end();
            let sigla = previa
                .rsplit(char::is_whitespace)
                .next()
                .map(|p| p.chars().count() == 1)
                .unwrap_or(false);
            if !sigla {
                actual.push_str(frase.trim_start());
                actual.push(' ');
                frase.clear();
                if actual.chars().count() > 500 {
                    parrafos.push(actual.trim().to_string());
                    actual = String::new();
                }
            }
        }
    }
    actual.push_str(&frase);
    if !actual.trim().is_empty() {
        parrafos.push(actual.trim().to_string());
    }
    parrafos.join("\n\n")
}

/// Readability puro (dom_smoothie) con salida markdown: conserva títulos,
/// listas y citas para que el guionizador les dé su ritmo.
fn extraer_con_readability(html: &str, url: &str) -> Result<(Option<String>, String)> {
    use dom_smoothie::{Config, Readability, TextMode};
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };
    let mut r =
        Readability::new(html, Some(url), Some(cfg)).map_err(|e| anyhow!("readability: {e:?}"))?;
    let articulo = r
        .parse()
        .map_err(|e| anyhow!("no parece un artículo legible ({e:?})"))?;
    let titulo = {
        let t = recortar_marca_de_sitio(articulo.title.trim());
        (!t.is_empty()).then(|| t.to_string())
    };
    // dom_smoothie escapa puntuación al estilo markdown («N\.º», «\(2\)»);
    // para LEER EN VOZ ALTA queremos el texto llano.
    let cuerpo = limpiar_markdown_hablado(&desescapar_markdown(articulo.text_content.trim()));
    if cuerpo.chars().count() < 80 {
        return Err(anyhow!("el artículo quedó vacío tras limpiarlo"));
    }
    // El detector de MUROS: si lo extraído es corto y huele a página de
    // verificación/robots, error honesto en vez de leer el muro en alto.
    if cuerpo.chars().count() < 600 {
        let bajo = cuerpo.to_lowercase();
        let muros = [
            "enable javascript",
            "ad blocker",
            "are you a robot",
            "not a robot",
            "just a moment",
            "seguir comprando",
            "verify you are human",
            "captcha",
            "access denied",
        ];
        if muros.iter().any(|m| bajo.contains(m)) {
            return Err(anyhow!(
                "la página pidió verificación humana (muro anti-robots)"
            ));
        }
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
                if "\\`*_{}[]()#+-.!|<>~\"'".contains(*sig) {
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
            let limpio: String = primera
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(60)
                .collect();
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
pub fn cola_agregar_web_cmd(app: AppHandle, ruta: String) -> Result<ItemCola, String> {
    agregar_web(&app, ruta).map_err(|e| e.to_string())
}

/// EL RESCATE DEL ARCHIVO: reintenta una pieza muerta descargándola de la
/// Wayback Machine (deja el marcador y reprocesa).
#[tauri::command]
pub fn cola_reintentar_archivo_cmd(app: AppHandle, id: String) -> Result<(), String> {
    let marcador = dir_cola(&app)
        .map_err(|e| e.to_string())?
        .join(format!("{id}.wayback"));
    fs::write(marcador, b"1").map_err(|e| e.to_string())?;
    actualizar_item(&app, &id, |it| {
        it.estado = EstadoItem::Pendiente;
        it.error = None;
    })
    .map_err(|e| e.to_string())?;
    procesar_en_segundo_plano(app.clone(), id);
    Ok(())
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

/// Favorito: marca/desmarca y, al marcar, sube la pieza arriba del todo.
#[tauri::command]
pub fn cola_favorito_cmd(app: AppHandle, id: String, favorito: bool) -> Result<(), String> {
    mutar(&app, |items| {
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            items[pos].favorito = favorito;
            if favorito {
                let it = items.remove(pos);
                items.insert(0, it);
            }
        }
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Renombrar una pieza (el título humano de la ficha y del lector).
#[tauri::command]
pub fn cola_renombrar_cmd(app: AppHandle, id: String, titulo: String) -> Result<(), String> {
    let titulo = titulo.trim().chars().take(120).collect::<String>();
    if titulo.is_empty() {
        return Err("título vacío".into());
    }
    actualizar_item(&app, &id, |it| it.titulo = titulo).map_err(|e| e.to_string())
}

/// Reordenar: mueve la pieza al índice destino (el arrastre de la cinta).
#[tauri::command]
pub fn cola_reordenar_cmd(app: AppHandle, id: String, indice: usize) -> Result<(), String> {
    mutar(&app, |items| {
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            let it = items.remove(pos);
            let destino = indice.min(items.len());
            items.insert(destino, it);
        }
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
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
    fn la_poda_de_wikipedia_calla_lo_que_no_se_lee() {
        let html = r##"<!doctype html><html><head><title>Loro</title></head><body>
        <div class="mw-parser-output">
          <div class="hatnote">Para otros usos, véase Loro (desambiguación).</div>
          <table class="infobox"><tr><th>Reino</th><td>Animalia</td></tr></table>
          <div id="toc" class="toc"><ul><li>1 Descripción</li><li>2 Referencias</li></ul></div>
          <p>Los loros son aves tropicales que pueden imitar la voz humana.<sup class="reference">[1]</sup></p>
          <h2>Descripción<span class="mw-editsection">[editar]</span></h2>
          <p>Tienen un pico curvado y una inteligencia notable que sorprende a cualquiera.</p>
          <div class="thumb"><img src="x.jpg"><div class="thumbcaption">Un loro posado en su rama</div></div>
          <figure><img src="y.jpg"><figcaption>Pie de foto que jamás debe leerse</figcaption></figure>
          <h2>Referencias<span class="mw-editsection">[editar]</span></h2>
          <ol class="references"><li>Enciclopedia de aves, 1998.</li></ol>
          <h2>Enlaces externos</h2>
          <ul><li><a href="https://aves.example">Aves del mundo</a></li></ul>
        </div></body></html>"##;
        let podado = podar_html(html, "https://es.wikipedia.org/wiki/Loro");
        assert!(!podado.contains("desambiguación"), "el hatnote debe caer");
        assert!(!podado.contains("Animalia"), "la infobox debe caer");
        assert!(!podado.contains("1 Descripción"), "el índice debe caer");
        assert!(
            !podado.contains("posado en su rama"),
            "el pie de thumb debe caer"
        );
        assert!(
            !podado.contains("jamás debe leerse"),
            "el figcaption debe caer"
        );
        assert!(
            !podado.contains("Enciclopedia de aves"),
            "las referencias deben caer"
        );
        assert!(
            !podado.contains("Aves del mundo"),
            "los enlaces externos deben caer"
        );
        assert!(
            podado.contains("imitar la voz humana"),
            "el cuerpo se queda"
        );
        assert!(
            podado.contains("inteligencia notable"),
            "las secciones reales se quedan"
        );
    }

    #[test]
    fn las_marcas_de_cita_no_se_pronuncian() {
        let md = "Los loros[1] hablan[23] mucho[nota 4].\nEsto sigue[cita requerida] así[editar].\n| a | b |\n↑\nY el [Quijote] se queda.";
        let limpio = limpiar_markdown_hablado(md);
        assert!(!limpio.contains("[1]"));
        assert!(!limpio.contains("[23]"));
        assert!(!limpio.contains("[nota 4]"));
        assert!(!limpio.contains("[cita requerida]"));
        assert!(!limpio.contains("[editar]"));
        assert!(!limpio.contains("| a |"), "las tablas markdown no se leen");
        assert!(!limpio.contains("↑"));
        assert!(
            limpio.contains("[Quijote]"),
            "los corchetes con contenido real se respetan"
        );
        assert!(limpio.contains("Los loros hablan mucho."));
    }

    /// El mismo cliente que usa la app (la API de sindicación de X
    /// devuelve 400 sin User-Agent de navegador).
    fn cliente_de_prueba() -> reqwest::Client {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1")
            .build()
            .unwrap()
    }

    #[test]
    fn el_codigo_no_se_lee_en_voz_alta() {
        let md =
            "Antes.\n\n```rust\nfn main() { println!(\"hola\"); }\nlet x = 1;\n```\n\nDespués.";
        let out = limpiar_markdown_hablado(md);
        assert!(out.contains("(Hay un ejemplo de código.)"));
        assert!(!out.contains("println"));
        assert!(out.contains("Después."));
        // Y el sangrado clásico de cuatro espacios.
        let md2 = "Prosa.\n\n    let a = 1;\n    let b = 2;\n    return a + b;\n\nMás prosa.";
        let out2 = limpiar_markdown_hablado(md2);
        assert!(out2.contains("(Hay un ejemplo de código.)"), "{out2}");
        assert!(!out2.contains("return a"));
    }

    #[test]
    fn los_emojis_no_se_pronuncian() {
        let out = limpiar_markdown_hablado("Qué gran día 🎉🚀 para leer ☀️.");
        assert_eq!(out, "Qué gran día  para leer .");
    }

    #[test]
    fn la_normalizacion_hablada_es_conservadora() {
        assert_eq!(
            normalizar_hablado_es("La guerra de 1936-1939 marcó al Sr. García."),
            "La guerra de 1936 a 1939 marcó al señor García."
        );
        assert_eq!(
            normalizar_hablado_es("Subió un 3,5 % este año."),
            "Subió un 3,5 por ciento este año."
        );
        assert_eq!(
            normalizar_hablado_es("Cuesta 12,50 € en EE. UU."),
            "Cuesta 12,50 euros en Estados Unidos."
        );
        assert_eq!(
            normalizar_hablado_es("Pagó $5 por el libro."),
            "Pagó 5 dólares por el libro."
        );
        // Lo dudoso NO se toca.
        assert_eq!(
            normalizar_hablado_es("El ISBN 84-376-0494-7 sigue igual."),
            "El ISBN 84-376-0494-7 sigue igual."
        );
        assert_eq!(
            normalizar_hablado_es("Recaudó $3,2 millones."),
            "Recaudó $3,2 millones."
        );
        assert_eq!(
            normalizar_hablado_es("D. Quijote no cambia."),
            "D. Quijote no cambia."
        );
        assert_eq!(
            normalizar_hablado_es("Fruta, pan, etc. Luego volvió."),
            "Fruta, pan, etcétera. Luego volvió."
        );
        assert_eq!(
            normalizar_hablado_es("compró pág. 12 y art. 4"),
            "compró página 12 y artículo 4"
        );
    }

    #[test]
    fn el_charset_viejo_no_rompe_tildes() {
        // «Año de canción» en ISO-8859-1, con el charset SOLO en el meta.
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=iso-8859-1\"></head><body>");
        bytes.extend_from_slice(&[
            b'A', 0xF1, b'o', b' ', b'd', b'e', b' ', b'c', b'a', b'n', b'c', b'i', 0xF3, b'n',
        ]);
        bytes.extend_from_slice(b"</body></html>");
        let html = decodificar_html(&bytes, "text/html");
        assert!(html.contains("Año de canción"), "{html}");
        // Y con el charset en el content-type manda el header.
        let html2 = decodificar_html(&bytes, "text/html; charset=iso-8859-1");
        assert!(html2.contains("Año de canción"));
    }

    #[test]
    fn los_documentos_remotos_se_reconocen() {
        assert_eq!(
            extension_de_documento(b"%PDF-1.7 x", "text/html", "https://x.com/a"),
            Some("pdf")
        );
        assert_eq!(
            extension_de_documento(b"<html>", "application/pdf", "https://x.com/a"),
            Some("pdf")
        );
        assert_eq!(
            extension_de_documento(
                b"PK\x03\x04",
                "application/octet-stream",
                "https://x.com/libro.epub"
            ),
            Some("epub")
        );
        assert_eq!(
            extension_de_documento(
                b"PK\x03\x04",
                "application/octet-stream",
                "https://x.com/doc.docx"
            ),
            Some("docx")
        );
        assert_eq!(
            extension_de_documento(b"<html>", "text/html; charset=utf-8", "https://x.com/a"),
            None
        );
        assert_eq!(
            extension_de_documento(b"hola", "text/plain", "https://x.com/notas.txt"),
            Some("txt")
        );
    }

    #[test]
    fn mastodon_se_reconoce_por_la_forma() {
        assert!(es_url_mastodon(
            "https://mastodon.social/@Gargron/109381219708671001"
        ));
        assert!(!es_url_mastodon("https://mastodon.social/@Gargron"));
        assert!(!es_url_mastodon("https://elpais.com/@autor/seccion"));
        assert!(!es_url_mastodon("https://x.com/elonmusk/status/123"));
    }

    #[test]
    fn el_enlace_amp_se_encuentra() {
        let html = r#"<html><head><link rel="amphtml" href="https://ejemplo.com/amp/articulo"></head><body></body></html>"#;
        assert_eq!(
            enlace_amp(html, "https://ejemplo.com/articulo"),
            Some("https://ejemplo.com/amp/articulo".into())
        );
        let rel = r#"<html><head><link rel="amphtml" href="/amp/art"></head></html>"#;
        assert_eq!(
            enlace_amp(rel, "https://ejemplo.com/art"),
            Some("https://ejemplo.com/amp/art".into())
        );
        assert_eq!(enlace_amp("<html></html>", "https://x.com"), None);
    }

    #[test]
    fn las_adaptaciones_hablan_el_idioma_del_texto() {
        // En inglés: marcador inglés y normalización inglesa.
        let en = limpiar_markdown_hablado(
            "The war of 1936-1939 changed everything, and the price rose by 3% before the market settled down for good.\n\n```rust\nlet x = 1;\nlet y = 2;\nlet z = 3;\n```\n\nMore prose follows here.",
        );
        assert!(en.contains("1936 to 1939"), "{en}");
        assert!(en.contains("3 percent"), "{en}");
        assert!(en.contains("(There is a code example.)"), "{en}");
        assert!(!en.contains("ejemplo de código"));
        // En español: lo de siempre.
        let es = limpiar_markdown_hablado(
            "La guerra de 1936-1939 lo cambió todo, y el precio subió un 3 % antes de que el mercado se calmara del todo para siempre.",
        );
        assert!(es.contains("1936 a 1939"), "{es}");
        assert!(es.contains("3 por ciento"), "{es}");
        // En francés: conservador, nada de palabras metidas.
        let fr = limpiar_markdown_hablado(
            "La guerre de 1936-1939 a tout changé, et les prix ont augmenté de 3 % avant que le marché ne se calme durablement pour de bon.",
        );
        assert!(fr.contains("1936-1939"), "{fr}");
        assert!(fr.contains("3 %"), "{fr}");
    }

    #[test]
    #[ignore = "red real: un ARTÍCULO de X entero (el caso reportado)"]
    fn articulo_de_x_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_tweet(
            &cliente,
            "https://x.com/fi56622380/status/2093040177711329673?s=46",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?} · {} chars", md.chars().count());
        println!("{}", &md[..md.len().min(500)]);
        assert!(md.chars().count() > 1000, "esperaba el artículo entero");
    }

    #[test]
    #[ignore = "experimento: qué hace Readability con versos"]
    fn experimento_versos() {
        let con_saltos = r#"<html><body><article><h1>Poema</h1><div class="body">Primer verso del poema
segundo verso que sigue
tercer verso sin puntos
y el cuarto que cierra</div><p>Prosa normal aparte para que el artículo pese lo suficiente y Readability lo tome en serio como cuerpo del documento con contenido de verdad.</p></article></body></html>"#;
        let (_, md) = extraer_articulo(con_saltos, "https://x.test/poema").unwrap();
        println!("── \n literales:\n{md}\n");
        let con_br = con_saltos.replace("\n", "<br>");
        let (_, md2) = extraer_articulo(&con_br, "https://x.test/poema").unwrap();
        println!("── con <br>:\n{md2}");
    }

    #[test]
    #[ignore = "red real: un hilo de Reddit como conversación"]
    fn hilo_de_reddit_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_reddit(
            &cliente,
            "https://www.reddit.com/r/rust/comments/1cdqdsi/announcing_rust_178/",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?}\n{}", &md[..md.len().min(700)]);
        assert!(
            md.contains(" says: ") || md.contains(" replies: ") || md.contains(" dice: "),
            "conectores ausentes"
        );
    }

    #[test]
    #[ignore = "red real: un hilo de Bluesky por la API pública"]
    fn hilo_de_bluesky_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_bluesky(
            &cliente,
            "https://bsky.app/profile/bsky.app/post/3l6oveex3ii2l",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?}\n{}", &md[..md.len().min(700)]);
        assert!(titulo.unwrap().contains("Bluesky"));
        assert!(md.chars().count() > 40);
    }

    #[test]
    #[ignore = "red real: un estado de Mastodon con sus respuestas"]
    fn estado_de_mastodon_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_mastodon(
            &cliente,
            "https://mastodon.social/@Mastodon/117156549508722805",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?}\n{}", &md[..md.len().min(700)]);
        assert!(md.chars().count() > 40);
    }

    #[test]
    #[ignore = "red real: el texto completo de un libro de archive.org"]
    fn libro_de_archive_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_archive(
            &cliente,
            "https://archive.org/details/elingeniosohidal01cerv",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?} · {} chars", md.chars().count());
        assert!(md.chars().count() > 5000);
    }

    #[test]
    #[ignore = "red real: un PDF remoto se clasifica como documento"]
    fn pdf_remoto_se_clasifica() {
        let cliente = cliente_de_prueba();
        let (bytes, ct) = tauri::async_runtime::block_on(descargar_crudo(
            &cliente,
            "https://www.boe.es/buscar/pdf/1978/BOE-A-1978-31229-consolidado.pdf",
        ))
        .unwrap();
        println!("{} bytes, content-type {ct}", bytes.len());
        assert_eq!(extension_de_documento(&bytes, &ct, "x.pdf"), Some("pdf"));
    }

    #[test]
    #[ignore = "red real: el hilo de Hacker News como conversación"]
    fn hilo_de_hn_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_hn(
            &cliente,
            "https://news.ycombinator.com/item?id=38865518",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?}\n{}", &md[..md.len().min(900)]);
        assert!(titulo.is_some());
        assert!(
            md.contains(" says: ") || md.contains(" replies: ") || md.contains(" dice: "),
            "conectores ausentes"
        );
        assert!(!md.contains("<p>"));
    }

    #[test]
    #[ignore = "red real: un tuit por la API de sindicación"]
    fn tuit_real() {
        let cliente = cliente_de_prueba();
        let (titulo, md) = tauri::async_runtime::block_on(extraer_tweet(
            &cliente,
            "https://x.com/elonmusk/status/1585341984679469056",
        ))
        .unwrap();
        println!("TÍTULO: {titulo:?}\n{md}");
        assert_eq!(titulo.as_deref(), Some("Elon Musk en X"));
        assert!(md.contains("sink in"));
        assert!(!md.contains("t.co"));
    }

    #[test]
    #[ignore = "harness del corpus: CORPUS=/dir cargo test corpus_real -- --ignored --nocapture"]
    fn corpus_real() {
        let dir = std::env::var("CORPUS").unwrap_or_else(|_| "/tmp/corpus".into());
        let salidas = std::path::Path::new(&dir).join("salidas");
        std::fs::create_dir_all(&salidas).unwrap();
        let mut entradas: Vec<_> = std::fs::read_dir(&dir)
            .expect("directorio del corpus")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "html").unwrap_or(false))
            .collect();
        entradas.sort_by_key(|e| e.file_name());
        for e in entradas {
            let nombre = e.file_name().to_string_lossy().to_string();
            let html = match std::fs::read_to_string(e.path()) {
                Ok(h) => h,
                Err(_) => continue,
            };
            // La URL de origen viaja en un fichero .url hermano (opcional).
            let url = std::fs::read_to_string(e.path().with_extension("url"))
                .map(|u| u.trim().to_string())
                .unwrap_or_else(|_| format!("https://example.com/{nombre}"));
            let podado = podar_html(&html, &url);
            match extraer_articulo(&podado, &url) {
                Ok((titulo, md)) => {
                    let destino = salidas.join(format!("{nombre}.md"));
                    let cabecera = format!("<!-- url: {url} | titulo: {titulo:?} -->\n");
                    std::fs::write(&destino, cabecera + &md).unwrap();
                    println!("OK {nombre}: {} líneas", md.lines().count());
                }
                Err(err) => println!("FALLO {nombre}: {err}"),
            }
        }
    }

    #[test]
    #[ignore = "necesita /tmp/wiki-real.html descargada a mano"]
    fn wikipedia_real_queda_limpia() {
        let html =
            std::fs::read_to_string("/tmp/wiki-real.html").expect("descarga /tmp/wiki-real.html");
        let podado = podar_html(&html, "https://es.wikipedia.org/wiki/Loro");
        let (titulo, md) = extraer_articulo(&podado, "https://es.wikipedia.org/wiki/Loro").unwrap();
        println!("TÍTULO: {titulo:?}");
        println!(
            "PRIMERAS LÍNEAS:\n{}",
            md.lines().take(14).collect::<Vec<_>>().join("\n")
        );
        println!(
            "ÚLTIMAS LÍNEAS:\n{}",
            md.lines()
                .rev()
                .take(10)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(!md.contains("[editar]"));
        assert!(!md.to_lowercase().contains("véase también"));
    }

    #[test]
    fn el_selector_de_ios_entrega_url_y_se_normaliza() {
        let dir = std::env::temp_dir().join("yappy prueba selector");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("mi documento.txt");
        std::fs::write(&f, "hola").unwrap();
        let url = format!("file://{}", f.to_string_lossy().replace(' ', "%20"));
        assert_eq!(ruta_de_selector(&url), f.to_string_lossy());
        // Una ruta normal no se toca.
        assert_eq!(ruta_de_selector(&f.to_string_lossy()), f.to_string_lossy());
    }

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
        assert_eq!(
            id_video_youtube("https://youtu.be/dQw4w9WgXcQ?t=1").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            id_video_youtube("https://www.youtube.com/watch?v=abc123&x=1").as_deref(),
            Some("abc123")
        );
        assert!(es_youtube("https://m.youtube.com/watch?v=x"));
        assert!(!es_youtube("https://example.com/watch"));
    }
}
