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
    // API mejor): Hacker News (el hilo como conversación) y X/Twitter.
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

    // LA VÍA CON SESIÓN: si el compartir trajo el HTML que el usuario VEÍA
    // (Safari + preprocesado JS: con su suscripción, sin muro), está en
    // cola/{id}.html y no hay nada que descargar.
    let ruta_html = dir_cola(app)?.join(format!("{}.html", item.id));
    let html = if ruta_html.exists() {
        fs::read_to_string(&ruta_html)?
    } else {
        cliente
            .get(&item.origen)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?
    };

    let html_podado = podar_html(&html, &item.origen);
    let (titulo, markdown) = extraer_articulo(&html_podado, &item.origen)?;
    // El HTML compartido ya sirvió; no ocupa sitio en la cinta.
    let _ = fs::remove_file(&ruta_html);
    terminar_con_markdown(app, &item.id, titulo, markdown)
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
    let mut md = format!("# {titulo}\n\n");
    if let Some(texto) = v["text"].as_str() {
        md.push_str(&texto_de_fragmento_html(texto));
        md.push_str("\n\n");
    }
    // Los comentarios, en orden y con jerarquía hablada: primer nivel
    // «dice», respuestas «responde». Tope generoso para no leer mil.
    let mut cuantos = 0usize;
    fn caminar(nodo: &serde_json::Value, nivel: usize, md: &mut String, cuantos: &mut usize) {
        if *cuantos >= 60 || nivel > 2 {
            return;
        }
        if let Some(hijos) = nodo["children"].as_array() {
            for h in hijos {
                let (Some(autor), Some(texto)) = (h["author"].as_str(), h["text"].as_str()) else {
                    continue;
                };
                let verbo = if nivel == 0 { "dice" } else { "responde" };
                md.push_str(&format!(
                    "{autor} {verbo}: {}\n\n",
                    texto_de_fragmento_html(texto)
                ));
                *cuantos += 1;
                caminar(h, nivel + 1, md, cuantos);
                if *cuantos >= 60 {
                    return;
                }
            }
        }
    }
    caminar(&v, 0, &mut md, &mut cuantos);
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
    // Los tuits largos («artículos») llegan enteros en note_tweet.
    let texto = v["note_tweet"]["text"]
        .as_str()
        .or_else(|| v["text"].as_str())
        .ok_or_else(|| anyhow!("el tuit no se puede leer (¿borrado o privado?)"))?;
    let mut md = format!("# {autor} en X\n\n{texto}\n");
    if let Some(cita) = v["quoted_tweet"].as_object() {
        if let (Some(qn), Some(qt)) = (
            cita.get("user").and_then(|u| u["name"].as_str()),
            cita.get("text").and_then(|t| t.as_str()),
        ) {
            md.push_str(&format!("\nCita de {qn}: {qt}\n"));
        }
    }
    let markdown = limpiar_markdown_hablado(&md);
    Ok((Some(format!("{autor} en X")), markdown))
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
    doc.html().to_string()
}

/// La limpieza HABLADA del markdown final: marcas de cita [1] [nota 2],
/// restos de [editar], líneas de tabla y flechas de referencia. Vale para
/// TODO origen (también documentos y pegados).
pub fn limpiar_markdown_hablado(md: &str) -> String {
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
    for linea in md.lines() {
        let l = linea.trim_end();
        let compacta = l.trim();
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
        assert!(md.contains(" dice: ") || md.contains(" responde: "));
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
