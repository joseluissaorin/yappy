//! El puente: el móvil usa el ordenador para convertir.
//!
//! El caso de uso es el del pitch: «¿quieres un libro para tus paseos? Lo
//! dejas renderizando la noche anterior en el ordenador y por la mañana lo
//! escuchas en el móvil». El teléfono ya sintetiza solo; el puente aporta
//! los libros ENTEROS sin gastar batería, con la calidad alta.
//!
//! Transporte: iroh (QUIC cifrado de extremo a extremo, marcando por clave
//! pública; en la misma wifi conecta directo, fuera pasa por los relays
//! públicos de n0). Sin cuentas, sin dominios, sin binarios ajenos.
//!
//! Emparejamiento: el escritorio enseña un QR con
//!   yappy://pair?d=<base64url(json{addr, token, nombre})>
//! que la CÁMARA DEL SISTEMA del iPhone abre como deep link (sin cámara
//! dentro de la app). También se puede copiar el enlace y pegarlo.
//!
//! Protocolo (ALPN yappy/puente/1), un bi-stream por trabajo:
//!   → una línea JSON: {token, titulo, texto, voz, velocidad, steps, desde?}
//!   ← líneas JSON de progreso: {"tipo":"sintetizando","hecho":n,"total":m}
//!     … {"tipo":"codificando"} … {"tipo":"listo","bytes":N,"desde":D}\n
//!     + los bytes del .yappy desde el offset D (por trozos, desde disco).
//!     Errores: {"tipo":"error","mensaje":"…"}.
//!
//! REANUDACIÓN: cada trabajo tiene un job_id determinista (hash de la
//! petición). El servidor CACHEA el .yappy terminado por job_id: si la
//! descarga se corta, el cliente reconecta con `desde` = bytes que ya
//! tiene y el servidor sirve el resto SIN resintetizar nada.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use iroh::endpoint::presets;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};

const ALPN: &[u8] = b"yappy/puente/1";

const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

// ── Persistencia ─────────────────────────────────────────────────────────

/// Config del ESCRITORIO: la clave secreta del nodo y los tokens admitidos.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConfigServidor {
    /// Clave secreta del endpoint (hex), para conservar la identidad.
    clave_secreta: Option<String>,
    /// Tokens de emparejamiento admitidos.
    tokens: Vec<String>,
}

/// Config del MÓVIL: a qué ordenador llama y con qué token.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigMovil {
    pub addr: Option<String>,
    pub token: Option<String>,
    pub nombre: Option<String>,
}

fn ruta_config<R: Runtime>(app: &AppHandle<R>, fichero: &str) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().context("app_config_dir")?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(fichero))
}

fn leer_servidor<R: Runtime>(app: &AppHandle<R>) -> ConfigServidor {
    ruta_config(app, "puente-servidor.json")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn guardar_servidor<R: Runtime>(app: &AppHandle<R>, c: &ConfigServidor) -> Result<()> {
    std::fs::write(
        ruta_config(app, "puente-servidor.json")?,
        serde_json::to_string_pretty(c)?,
    )?;
    Ok(())
}

pub fn leer_movil<R: Runtime>(app: &AppHandle<R>) -> ConfigMovil {
    ruta_config(app, "puente-movil.json")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn guardar_movil<R: Runtime>(app: &AppHandle<R>, c: &ConfigMovil) -> Result<()> {
    std::fs::write(
        ruta_config(app, "puente-movil.json")?,
        serde_json::to_string_pretty(c)?,
    )?;
    Ok(())
}

// ── El servidor (escritorio) ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EstadoPuente {
    pub activo: bool,
    pub addr: Option<String>,
    pub enlace: Option<String>,
    pub tokens: Vec<String>,
}

static ADDR_ACTUAL: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
    std::sync::OnceLock::new();

fn addr_actual() -> &'static std::sync::Mutex<Option<String>> {
    ADDR_ACTUAL.get_or_init(|| std::sync::Mutex::new(None))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peticion {
    pub token: String,
    /// Si viene, no es un trabajo: es la orden de CANCELAR ese job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancelar: Option<String>,
    pub titulo: String,
    pub texto: String,
    pub voz: Option<String>,
    pub velocidad: Option<f32>,
    pub steps: Option<usize>,
    /// Reanudación: desde qué byte del .yappy terminado seguir (0 = entero).
    #[serde(default)]
    pub desde: u64,
    /// Idioma del texto fijado por el encargo (None = detectar allí).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idioma: Option<String>,
    /// La SONDA: no es un trabajo, es «¿estás ahí?». El servidor contesta
    /// {"tipo":"pong","nombre":…} y cierra. (Los servidores viejos, sin
    /// este campo, lo ignoran y lo tratarían como trabajo: por eso la
    /// sonda viaja ADEMÁS como `cancelar: "sonda"`, que cualquier versión
    /// responde sin sintetizar nada.)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sonda: bool,
}

/// Trabajos cancelados a distancia: sintetizar_pack los consulta y
/// aborta. (El cliente manda {token, cancelar: job_id}.)
static CANCELADOS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

fn cancelados() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    CANCELADOS.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

/// El identificador determinista de un trabajo: la MISMA petición produce
/// el MISMO id, y con él la caché de packs terminados hace la reanudación.
fn job_id_de(p: &Peticion) -> String {
    let mut h: u64 = 1469598103934665603;
    for b in p
        .titulo
        .bytes()
        .chain(p.texto.bytes())
        .chain(p.voz.clone().unwrap_or_default().bytes())
        .chain(p.idioma.clone().unwrap_or_default().bytes())
    {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h ^= (p.velocidad.unwrap_or(0.0).to_bits() as u64) ^ ((p.steps.unwrap_or(0) as u64) << 32);
    h = h.wrapping_mul(1099511628211);
    format!("{h:016x}")
}

/// El directorio de packs terminados (la caché de reanudación). Se limpia
/// de trabajos viejos (>24 h) al arrancar el servidor.
fn dir_cache_puente() -> PathBuf {
    std::env::temp_dir().join("yappy-puente-cache")
}

fn limpiar_cache_puente() {
    let dir = dir_cache_puente();
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return;
    };
    let ahora = std::time::SystemTime::now();
    for e in entradas.flatten() {
        let vieja = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| ahora.duration_since(t).ok())
            .map(|d| d.as_secs() > 24 * 3600)
            .unwrap_or(true);
        if vieja {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// La voz pedida, o la del ajuste si la petición no traía.
fn peticion_voz_o(pedida: &Option<String>, ajuste: &str) -> String {
    pedida
        .clone()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| ajuste.to_string())
}

/// Arranca el endpoint iroh y el bucle de aceptación. Solo escritorio.
pub fn iniciar_servidor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = servidor(app).await {
            tracing::error!("puente: servidor caído: {e:#}");
        }
    });
}

async fn servidor(app: AppHandle) -> Result<()> {
    let mut cfg = leer_servidor(&app);
    let secreta = match cfg.clave_secreta.as_deref() {
        Some(hex) => iroh::SecretKey::from_bytes(
            &<[u8; 32]>::try_from(hex_a_bytes(hex)?.as_slice())
                .map_err(|_| anyhow!("clave secreta corrupta"))?,
        ),
        None => {
            let clave = iroh::SecretKey::generate();
            cfg.clave_secreta = Some(bytes_a_hex(&clave.to_bytes()));
            guardar_servidor(&app, &cfg)?;
            clave
        }
    };

    let endpoint = iroh::Endpoint::builder(presets::N0)
        .secret_key(secreta)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| anyhow!("iroh bind: {e}"))?;

    // Esperar a estar «online» (relay contactado) para que la dirección del
    // QR lleve el relay y las direcciones directas: la primera conexión del
    // móvil no depende así de ningún descubrimiento.
    endpoint.online().await;
    let addr = endpoint.addr();
    let addr_json = serde_json::to_string(&addr)?;
    *addr_actual().lock().unwrap() = Some(addr_json.clone());
    tracing::info!("puente: escuchando como {}", endpoint.id());
    limpiar_cache_puente();
    // La dirección se REFRESCA periódicamente: al cambiar de red, el QR
    // y el estado no sirven direcciones muertas.
    {
        let endpoint2 = endpoint.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                if let Ok(json) = serde_json::to_string(&endpoint2.addr()) {
                    *addr_actual().lock().unwrap() = Some(json);
                }
            }
        });
    }

    while let Some(entrante) = endpoint.accept().await {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            match entrante.await {
                Ok(conn) => {
                    if let Err(e) = atender(app2, conn).await {
                        tracing::warn!("puente: conexión terminó con error: {e:#}");
                    }
                }
                Err(e) => tracing::warn!("puente: handshake falló: {e}"),
            }
        });
    }
    Ok(())
}

/// LA SÍNTESIS DEL PUENTE, sin tauri: recibe el motor y los parámetros ya
/// resueltos, produce el .yappy TERMINADO en la caché de trabajos y
/// devuelve su ruta. Si el trabajo ya estaba cacheado (reanudación), no
/// sintetiza nada. Es la pieza que los tests ejercitan con el motor real.
pub fn sintetizar_pack(
    engine: Arc<yappy_core::TtsEngine>,
    peticion: &Peticion,
    voz: String,
    velocidad: f32,
    steps: usize,
    progreso: tokio::sync::mpsc::UnboundedSender<(usize, usize)>,
) -> Result<PathBuf> {
    let job = job_id_de(peticion);
    let dir = dir_cache_puente();
    std::fs::create_dir_all(&dir)?;
    let pack = dir.join(format!("{job}.yappy"));
    if pack.exists() && crate::yappy_pack::leer_manifiesto(&pack).is_ok() {
        tracing::info!("puente: trabajo {job} ya cacheado, sin resintetizar");
        return Ok(pack);
    }

    let guion = yappy_core::guion::construir_desde_texto(
        &peticion.texto,
        peticion.idioma.as_deref().unwrap_or("en"),
    );
    let opts = yappy_core::engine::SynthesisOptions {
        voice: voz.clone(),
        speed: velocidad,
        default_lang: peticion
            .idioma
            .clone()
            .unwrap_or_else(|| guion.idioma_base.clone()),
        total_steps: steps,
        seed: None,
        detectar_idioma: peticion.idioma.is_none(),
        pausa_entre_parrafos_s: 0.0,
    };
    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate = 44_100u32;
    let mut capitulos: Vec<(f64, String)> = Vec::new();
    // EL KARAOKE del render remoto: los tiempos por frase viajan dentro
    // del .yappy de vuelta.
    let mut tiempos: Vec<crate::yappy_pack::TiempoFrase> = Vec::new();
    let mut pieza_previa = usize::MAX;
    let job_para_cancelar = job.clone();
    engine.synthesize_guion(&guion, &opts, |chunk| {
        if cancelados().lock().unwrap().remove(&job_para_cancelar) {
            return Err(anyhow!("trabajo cancelado a distancia"));
        }
        sample_rate = chunk.sample_rate as u32;
        if chunk.paragraph_index != pieza_previa {
            pieza_previa = chunk.paragraph_index;
            if let Some(p) = guion.piezas.get(chunk.paragraph_index) {
                use yappy_core::guion::ClasePieza::*;
                if matches!(p.clase, Titulo1 | Titulo2 | Titulo3) {
                    capitulos.push((
                        samples.len() as f64 / sample_rate as f64,
                        p.texto.chars().take(80).collect(),
                    ));
                }
            }
        }
        let ini = samples.len();
        samples.extend_from_slice(&chunk.samples);
        if !chunk.es_pausa && !chunk.text.trim().is_empty() {
            tiempos.push(crate::yappy_pack::TiempoFrase {
                ini_s: ini as f32 / sample_rate as f32,
                fin_s: samples.len() as f32 / sample_rate as f32,
                parrafo: chunk.paragraph_index,
                origen_ini: chunk.origen_ini,
                origen_fin: chunk.origen_fin,
                texto: chunk.text.clone(),
            });
        }
        let _ = progreso.send((chunk.index + 1, chunk.total));
        Ok(())
    })?;
    let texto_guion = guion
        .piezas
        .iter()
        .map(|p| p.texto.clone())
        .collect::<Vec<_>>()
        .join("\n\n");
    let idioma = guion.idioma_base.clone();
    let duracion_secs = samples.len() as f32 / sample_rate.max(1) as f32;

    // El título DICHO, cortito, para que la pieza llegue sonando.
    let titulo_wav = {
        let opts_titulo = yappy_core::engine::SynthesisOptions {
            voice: voz.clone(),
            speed: 1.0,
            default_lang: idioma.clone(),
            total_steps: 8,
            seed: None,
            detectar_idioma: true,
            pausa_entre_parrafos_s: 0.0,
        };
        let mut muestras: Vec<f32> = Vec::new();
        let mut sr = 44_100u32;
        let ok = engine
            .synthesize_streaming(&peticion.titulo, &opts_titulo, |chunk| {
                sr = chunk.sample_rate as u32;
                muestras.extend_from_slice(&chunk.samples);
                Ok(())
            })
            .is_ok();
        if ok && !muestras.is_empty() {
            let ruta = dir.join(format!("{job}-titulo.wav"));
            crate::playback::write_wav_file(ruta.to_string_lossy().as_ref(), &muestras, sr)
                .ok()
                .map(|_| ruta)
        } else {
            None
        }
    };

    let chapters: Vec<crate::audiobook::Chapter> = if capitulos.is_empty() {
        vec![crate::audiobook::Chapter {
            start_secs: 0.0,
            title: peticion.titulo.clone(),
        }]
    } else {
        capitulos
            .into_iter()
            .map(|(s, t)| crate::audiobook::Chapter {
                start_secs: s,
                title: t,
            })
            .collect()
    };
    let capitulos_pack: Vec<crate::yappy_pack::CapituloPack> = chapters
        .iter()
        .map(|c| crate::yappy_pack::CapituloPack {
            titulo: c.title.clone(),
            inicio_s: c.start_secs as f32,
        })
        .collect();

    let m4b = dir.join(format!("{job}.m4b"));
    let meta = crate::audiobook::M4bMetadata {
        title: peticion.titulo.clone(),
        author: "Yappy".into(),
        album: peticion.titulo.clone(),
    };
    crate::audiobook::encode_m4b(&samples, sample_rate, &chapters, &meta, &m4b)?;

    let manifiesto = crate::yappy_pack::ManifiestoPack {
        version: crate::yappy_pack::VERSION,
        titulo: peticion.titulo.clone(),
        autor: String::new(),
        voz,
        velocidad,
        idioma,
        duracion_secs,
        capitulos: capitulos_pack,
        creado_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    // Escritura ATÓMICA del pack en la caché: nada de trabajos a medias.
    let tmp_pack = dir.join(format!("{job}.yappy.tmp"));
    crate::yappy_pack::escribir(
        &tmp_pack,
        &manifiesto,
        &m4b,
        &tiempos,
        &texto_guion,
        titulo_wav.as_deref(),
    )?;
    std::fs::rename(&tmp_pack, &pack)?;
    let _ = std::fs::remove_file(&m4b);
    if let Some(w) = &titulo_wav {
        let _ = std::fs::remove_file(w);
    }
    Ok(pack)
}

/// El productor de packs: recibe la petición validada y el canal de
/// progreso, devuelve la ruta del .yappy terminado. En producción sale de
/// la app; en los tests, de un motor directo o de un fake.
pub type Productor = std::sync::Arc<
    dyn Fn(
            Peticion,
            tokio::sync::mpsc::UnboundedSender<(usize, usize)>,
        ) -> tauri::async_runtime::JoinHandle<Result<PathBuf>>
        + Send
        + Sync,
>;

/// EL PROTOCOLO del lado servidor, sin tauri: leer la petición, validar
/// el token, reenviar el progreso, y mandar el .yappy POR TROZOS desde
/// disco, desde el offset pedido (la reanudación). Testeable de verdad.
pub async fn atender_conexion(
    conn: iroh::endpoint::Connection,
    tokens_admitidos: Vec<String>,
    productor: Productor,
) -> Result<()> {
    let (mut tx, mut rx) = conn.accept_bi().await?;

    let linea = leer_linea(&mut rx, 4 * 1024 * 1024).await?;
    let peticion: Peticion = serde_json::from_str(&linea).context("petición ilegible")?;

    if !tokens_admitidos.contains(&peticion.token) {
        enviar_json(
            &mut tx,
            &serde_json::json!({"tipo":"error","mensaje":"token no emparejado"}),
        )
        .await?;
        return Ok(());
    }
    if peticion.sonda || peticion.cancelar.as_deref() == Some("sonda") {
        enviar_json(
            &mut tx,
            &serde_json::json!({"tipo":"pong","nombre": nombre_maquina()}),
        )
        .await?;
        tx.finish()?;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), conn.closed()).await;
        return Ok(());
    }
    if let Some(job) = &peticion.cancelar {
        cancelados().lock().unwrap().insert(job.clone());
        enviar_json(&mut tx, &serde_json::json!({"tipo":"cancelado"})).await?;
        tracing::info!("puente: trabajo {job} cancelado a distancia");
        return Ok(());
    }
    tracing::info!(
        "puente: trabajo aceptado «{}» ({} chars, desde {})",
        peticion.titulo,
        peticion.texto.chars().count(),
        peticion.desde
    );

    let (progreso_tx, mut progreso_rx) = tokio::sync::mpsc::unbounded_channel::<(usize, usize)>();
    let tarea = productor(peticion.clone(), progreso_tx);

    // Reenviar el progreso mientras se produce el pack.
    let mut ultimo = 0usize;
    while let Some((hecho, total)) = progreso_rx.recv().await {
        if hecho.saturating_sub(ultimo) >= 3 || hecho == total {
            ultimo = hecho;
            enviar_json(
                &mut tx,
                &serde_json::json!({"tipo":"sintetizando","hecho":hecho,"total":total}),
            )
            .await?;
        }
    }
    let pack = match tarea.await? {
        Ok(p) => p,
        Err(e) => {
            enviar_json(
                &mut tx,
                &serde_json::json!({"tipo":"error","mensaje": e.to_string()}),
            )
            .await?;
            return Ok(());
        }
    };
    enviar_json(&mut tx, &serde_json::json!({"tipo":"codificando"})).await?;

    // El fichero, POR TROZOS desde disco y desde el offset pedido: los
    // libros de horas ya no viven enteros en la RAM de nadie.
    let total_bytes = std::fs::metadata(&pack)?.len();
    let desde = peticion.desde.min(total_bytes);
    enviar_json(
        &mut tx,
        &serde_json::json!({
            "tipo":"listo","bytes": total_bytes, "desde": desde, "formato": "yappy"
        }),
    )
    .await?;
    {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut f = tokio::fs::File::open(&pack).await?;
        f.seek(std::io::SeekFrom::Start(desde)).await?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let leidos = f.read(&mut buf).await?;
            if leidos == 0 {
                break;
            }
            tx.write_all(&buf[..leidos]).await?;
        }
    }
    tx.finish()?;
    // Esperar a que el otro lado cierre para no cortar los últimos bytes.
    let _ = conn.closed().await;
    tracing::info!(
        "puente: «{}» servido ({total_bytes} bytes)",
        peticion.titulo
    );
    Ok(())
}

/// El productor REAL: el motor y los ajustes salen de la app. El trabajo
/// se ESPEJA en la imprenta del Mac para que también se vea aquí.
fn productor_de_app(app: AppHandle) -> Productor {
    std::sync::Arc::new(move |mut peticion, progreso| {
        let app = app.clone();
        // El espejo: alta + reenvío del progreso a la imprenta local.
        let job = job_id_de(&peticion);
        crate::imprenta::espejo_alta(&app, &job, &peticion.titulo);
        let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel::<(usize, usize)>();
        {
            let app2 = app.clone();
            let job2 = job.clone();
            let progreso = progreso.clone();
            tauri::async_runtime::spawn(async move {
                let mut ultimo = 0usize;
                while let Some((hecho, total)) = rx2.recv().await {
                    let _ = progreso.send((hecho, total));
                    if hecho.saturating_sub(ultimo) >= 5 || hecho == total {
                        ultimo = hecho;
                        crate::imprenta::espejo_progreso(&app2, &job2, hecho, total);
                    }
                }
            });
        }
        tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf> {
            let estado = app.state::<Arc<crate::state::AppState>>();
            let root = crate::model::model_root(&app).map_err(|e| anyhow!(e))?;
            let engine = estado.engine_or_load(&root)?;
            let (voz, velocidad, steps) = {
                let s = estado.settings.lock().unwrap();
                (
                    peticion_voz_o(&peticion.voz, &s.voice),
                    peticion.velocidad.unwrap_or(s.speed),
                    peticion.steps.unwrap_or_else(|| s.quality.total_steps()),
                )
            };
            peticion.voz = Some(voz.clone());
            let r = sintetizar_pack(engine, &peticion, voz, velocidad, steps, tx2);
            crate::imprenta::espejo_cierre(
                &app,
                &job,
                r.as_ref().map(|_| ()).map_err(|e| e.to_string()),
            );
            r
        })
    })
}

/// Cancela un trabajo A DISTANCIA (best effort): reconstruye la petición
/// para derivar su job_id y manda la orden por el puente.
pub async fn cancelar_remoto(
    app: AppHandle,
    titulo: String,
    texto: String,
    receta: RecetaRemota,
) -> Result<()> {
    let cfg = leer_movil(&app);
    let (addr_json, token) = match (cfg.addr, cfg.token) {
        (Some(a), Some(t)) => (a, t),
        _ => return Ok(()),
    };
    let addr: iroh::EndpointAddr = serde_json::from_str(&addr_json)?;
    let (voz, velocidad, steps) = {
        let estado = app.state::<Arc<crate::state::AppState>>();
        let s = estado.settings.lock().unwrap();
        // La MISMA receta que el encargo, o el job_id no coincide y la
        // cancelación no encuentra su trabajo.
        (
            receta
                .voz
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| s.voice.clone()),
            receta.velocidad.unwrap_or(s.speed),
            receta.steps.unwrap_or_else(|| s.quality.total_steps()),
        )
    };
    let base = Peticion {
        token: token.clone(),
        cancelar: None,
        titulo,
        texto,
        voz: Some(voz),
        velocidad: Some(velocidad),
        steps: Some(steps),
        desde: 0,
        idioma: receta
            .idioma
            .clone()
            .filter(|l| !l.is_empty() && l != "auto"),
        sonda: false,
    };
    let orden = Peticion {
        cancelar: Some(job_id_de(&base)),
        texto: String::new(),
        ..base
    };
    let endpoint = iroh::Endpoint::builder(presets::N0).bind().await?;
    let conn = endpoint.connect(addr, ALPN).await?;
    let (mut tx, mut rx) = conn.open_bi().await?;
    tx.write_all(format!("{}\n", serde_json::to_string(&orden)?).as_bytes())
        .await?;
    let _ = leer_linea(&mut rx, 64 * 1024).await;
    conn.close(0u32.into(), b"fin");
    Ok(())
}

async fn atender(app: AppHandle, conn: iroh::endpoint::Connection) -> Result<()> {
    let tokens = leer_servidor(&app).tokens;
    let productor = productor_de_app(app);
    atender_conexion(conn, tokens, productor).await
}

// ── El cliente (móvil, y también escritorio si algún día quiere) ─────────

/// Un intento de conversión remota: conecta, pide (con `desde` para
/// reanudar) y ESCRIBE por trozos en el .part. Devuelve los bytes totales
/// del pack (los ya escritos + los que llegaron).
async fn intento_convertir(
    app: &AppHandle,
    addr: iroh::EndpointAddr,
    peticion: &Peticion,
    part: &std::path::Path,
) -> Result<u64> {
    let endpoint = iroh::Endpoint::builder(presets::N0)
        .bind()
        .await
        .map_err(|e| anyhow!("iroh bind: {e}"))?;
    let conn = endpoint
        .connect(addr, ALPN)
        .await
        .map_err(|e| anyhow!("no llego al ordenador ({e}); ¿está Yappy abierto allí?"))?;
    let (mut tx, mut rx) = conn.open_bi().await?;

    tx.write_all(format!("{}\n", serde_json::to_string(peticion)?).as_bytes())
        .await?;

    // Progreso hasta el «listo» (con guardián de inactividad: una síntesis
    // muda más de tres minutos es una síntesis muerta).
    let total_bytes: u64;
    let desde_confirmado: u64;
    loop {
        let linea = tokio::time::timeout(
            std::time::Duration::from_secs(180),
            leer_linea(&mut rx, 1024 * 1024),
        )
        .await
        .map_err(|_| anyhow!("el ordenador dejó de responder"))??;
        let v: serde_json::Value = serde_json::from_str(&linea).context("respuesta ilegible")?;
        match v["tipo"].as_str().unwrap_or("") {
            "sintetizando" => {
                let _ = app.emit(
                    "puente_progreso",
                    serde_json::json!({
                        "etapa": "sintetizando",
                        "hecho": v["hecho"], "total": v["total"],
                    }),
                );
            }
            "codificando" => {
                let _ = app.emit(
                    "puente_progreso",
                    serde_json::json!({"etapa":"codificando"}),
                );
            }
            "listo" => {
                total_bytes = v["bytes"].as_u64().unwrap_or(0);
                desde_confirmado = v["desde"].as_u64().unwrap_or(0);
                break;
            }
            "error" => {
                return Err(anyhow!(v["mensaje"]
                    .as_str()
                    .unwrap_or("error remoto")
                    .to_string()));
            }
            otro => return Err(anyhow!("respuesta desconocida: {otro}")),
        }
    }

    // El cuerpo, POR TROZOS al .part (nada en RAM), con timeout por trozo.
    use tokio::io::AsyncWriteExt;
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(part)
        .await?;
    f.set_len(desde_confirmado).await?;
    use tokio::io::AsyncSeekExt;
    f.seek(std::io::SeekFrom::Start(desde_confirmado)).await?;
    let mut escritos = desde_confirmado;
    let mut buf = vec![0u8; 256 * 1024];
    while escritos < total_bytes {
        let leidos = tokio::time::timeout(std::time::Duration::from_secs(90), rx.read(&mut buf))
            .await
            .map_err(|_| anyhow!("descarga estancada"))?
            .map_err(|e| anyhow!("descarga cortada: {e}"))?;
        let Some(leidos) = leidos else {
            break;
        };
        if leidos == 0 {
            break;
        }
        f.write_all(&buf[..leidos]).await?;
        escritos += leidos as u64;
        let _ = app.emit(
            "puente_progreso",
            serde_json::json!({
                "etapa": "descargando",
                "hecho": escritos, "total": total_bytes,
            }),
        );
    }
    f.flush().await?;
    conn.close(0u32.into(), b"gracias");
    if escritos < total_bytes {
        return Err(anyhow!(
            "descarga incompleta ({escritos}/{total_bytes} bytes)"
        ));
    }
    Ok(total_bytes)
}

/// La receta de un encargo remoto: lo que el encargo pidió (voz, velocidad,
/// pasos, idioma). Lo que venga vacío lo rellenan los ajustes del teléfono.
#[derive(Debug, Clone, Default)]
pub struct RecetaRemota {
    pub voz: Option<String>,
    pub velocidad: Option<f32>,
    pub steps: Option<usize>,
    pub idioma: Option<String>,
}

/// La conversión remota, expuesta para la imprenta.
pub async fn convertir_publico(
    app: AppHandle,
    titulo: String,
    texto: String,
    receta: RecetaRemota,
) -> Result<String> {
    convertir(app, titulo, texto, receta).await
}

/// La dirección y el token del ordenador emparejado (o el porqué de que no).
fn destino_movil(app: &AppHandle) -> Result<(iroh::EndpointAddr, String)> {
    let cfg = leer_movil(app);
    let (addr_json, token) = match (cfg.addr, cfg.token) {
        (Some(a), Some(t)) => (a, t),
        _ => return Err(anyhow!("no hay ningún ordenador emparejado")),
    };
    let addr: iroh::EndpointAddr =
        serde_json::from_str(&addr_json).context("dirección corrupta")?;
    Ok((addr, token))
}

/// LA SONDA: conecta con el ordenador emparejado, se identifica con el
/// token y espera el «pong». Devuelve el nombre del ordenador y los ms.
pub async fn sondear(app: &AppHandle) -> Result<(String, u128)> {
    let (addr, token) = destino_movil(app)?;
    let endpoint = iroh::Endpoint::builder(presets::N0)
        .bind()
        .await
        .map_err(|e| anyhow!("iroh bind: {e}"))?;
    // El reloj arranca al llamar (el bind de iroh no es latencia al Mac).
    let t0 = std::time::Instant::now();
    let conn = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        endpoint.connect(addr, ALPN),
    )
    .await
    .map_err(|_| anyhow!("no llego al ordenador (sin respuesta); ¿está Yappy abierto allí?"))?
    .map_err(|e| anyhow!("no llego al ordenador ({e}); ¿está Yappy abierto allí?"))?;
    let (mut tx, mut rx) = conn.open_bi().await?;
    let peticion = Peticion {
        token,
        cancelar: Some("sonda".into()),
        titulo: String::new(),
        texto: String::new(),
        voz: None,
        velocidad: None,
        steps: None,
        desde: 0,
        idioma: None,
        sonda: true,
    };
    tx.write_all(format!("{}\n", serde_json::to_string(&peticion)?).as_bytes())
        .await?;
    let linea = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        leer_linea(&mut rx, 64 * 1024),
    )
    .await
    .map_err(|_| anyhow!("el ordenador no contesta"))??;
    let v: serde_json::Value = serde_json::from_str(&linea).context("respuesta ilegible")?;
    let nombre = match v["tipo"].as_str().unwrap_or("") {
        "pong" => v["nombre"].as_str().unwrap_or("ordenador").to_string(),
        // Un servidor anterior a la sonda contesta «cancelado»: está ahí.
        "cancelado" => leer_movil(app).nombre.unwrap_or_else(|| "ordenador".into()),
        "error" => {
            return Err(anyhow!(v["mensaje"]
                .as_str()
                .unwrap_or("el ordenador rechazó el token")
                .to_string()))
        }
        otro => return Err(anyhow!("respuesta desconocida: {otro}")),
    };
    conn.close(0u32.into(), b"sonda");
    Ok((nombre, t0.elapsed().as_millis()))
}

async fn convertir(
    app: AppHandle,
    titulo: String,
    texto: String,
    receta: RecetaRemota,
) -> Result<String> {
    let (addr, token) = destino_movil(&app)?;

    let (voz, velocidad, steps) = {
        let estado = app.state::<Arc<crate::state::AppState>>();
        let s = estado.settings.lock().unwrap();
        // El encargo manda; lo que no fijó, lo ponen los ajustes del
        // teléfono (la CALIDAD también viaja al render remoto).
        (
            receta
                .voz
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| s.voice.clone()),
            receta.velocidad.unwrap_or(s.speed),
            receta.steps.unwrap_or_else(|| s.quality.total_steps()),
        )
    };
    let mut peticion = Peticion {
        token,
        cancelar: None,
        titulo: titulo.clone(),
        texto,
        voz: Some(voz),
        velocidad: Some(velocidad),
        steps: Some(steps),
        desde: 0,
        idioma: receta
            .idioma
            .clone()
            .filter(|l| !l.is_empty() && l != "auto"),
        sonda: false,
    };

    // El destino y su .part (la reanudación vive en el .part).
    let dir = app.path().document_dir().context("document_dir")?;
    std::fs::create_dir_all(&dir)?;
    let nombre_limpio: String = titulo
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(60)
        .collect();
    let destino = dir.join(format!(
        "{}.{}",
        nombre_limpio.trim(),
        crate::yappy_pack::EXTENSION
    ));
    let part = dir.join(format!("{}.part", nombre_limpio.trim()));

    // REINTENTOS con reanudación: cada intento sigue desde lo que el .part
    // ya tenga (el servidor cachea el trabajo terminado y no resintetiza).
    let mut ultimo_error: Option<anyhow::Error> = None;
    for intento in 0..4 {
        if intento > 0 {
            let espera = 2u64 << (intento - 1);
            tracing::info!("puente: reintento {intento} en {espera}s");
            let _ = app.emit(
                "puente_progreso",
                serde_json::json!({"etapa":"reintentando","intento":intento}),
            );
            tokio::time::sleep(std::time::Duration::from_secs(espera)).await;
        }
        peticion.desde = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        match intento_convertir(&app, addr.clone(), &peticion, &part).await {
            Ok(_) => {
                ultimo_error = None;
                break;
            }
            Err(e) => {
                tracing::warn!("puente: intento {intento} falló: {e:#}");
                ultimo_error = Some(e);
            }
        }
    }
    if let Some(e) = ultimo_error {
        return Err(e);
    }

    // VALIDAR el pack entero antes de darlo por bueno (el zip se abre y el
    // manifiesto se lee o no hay trato), y colocarlo con rename atómico.
    crate::yappy_pack::leer_manifiesto(&part)
        .map_err(|e| anyhow!("el archivo llegó corrupto: {e}"))?;
    let _ = std::fs::remove_file(&destino);
    std::fs::rename(&part, &destino)?;

    // El título DICHO que viaja dentro, a la caché de dichos.
    if let Ok(mani) = crate::yappy_pack::leer_manifiesto(&destino) {
        if let Ok(wav) = crate::commands::dicho_path(&app, &mani.voz, &mani.titulo) {
            let _ = crate::yappy_pack::extraer_titulo_wav(&destino, &wav);
        }
    }
    let _ = app.emit(
        "puente_progreso",
        serde_json::json!({"etapa":"hecho","ruta": destino.to_string_lossy()}),
    );
    Ok(destino.to_string_lossy().to_string())
}

// ── Utilidades ───────────────────────────────────────────────────────────

async fn leer_linea(rx: &mut iroh::endpoint::RecvStream, max: usize) -> Result<String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while buf.len() < max {
        rx.read_exact(&mut byte)
            .await
            .map_err(|e| anyhow!("stream cortado: {e}"))?;
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&buf).to_string());
        }
        buf.push(byte[0]);
    }
    Err(anyhow!("línea demasiado larga"))
}

async fn enviar_json(tx: &mut iroh::endpoint::SendStream, v: &serde_json::Value) -> Result<()> {
    tx.write_all(format!("{v}\n").as_bytes()).await?;
    Ok(())
}

fn bytes_a_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hex_a_bytes(s: &str) -> Result<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow!("hex: {e}")))
        .collect()
}

fn token_nuevo() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| format!("{:x}", rng.gen_range(0..16u8)))
        .collect()
}

fn nombre_maquina() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().trim_end_matches(".local").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mi ordenador".into())
}

// ── Comandos Tauri ───────────────────────────────────────────────────────

#[tauri::command]
pub fn puente_estado_cmd(app: AppHandle) -> Result<EstadoPuente, String> {
    let cfg = leer_servidor(&app);
    let addr = addr_actual().lock().unwrap().clone();
    let enlace = addr.as_ref().and_then(|a| {
        let dato = serde_json::json!({
            "addr": serde_json::from_str::<serde_json::Value>(a).ok()?,
            "token": cfg.tokens.last()?,
            "nombre": nombre_maquina(),
        });
        Some(format!("yappy://pair?d={}", B64.encode(dato.to_string())))
    });
    Ok(EstadoPuente {
        activo: addr.is_some(),
        addr,
        enlace,
        tokens: cfg
            .tokens
            .iter()
            .map(|t| t.chars().take(8).collect())
            .collect(),
    })
}

/// Genera un token nuevo y devuelve el enlace de emparejamiento + su QR SVG.
#[tauri::command]
pub fn puente_emparejar_nuevo_cmd(app: AppHandle) -> Result<serde_json::Value, String> {
    let addr = addr_actual()
        .lock()
        .unwrap()
        .clone()
        .ok_or("el puente aún no está escuchando")?;
    let mut cfg = leer_servidor(&app);
    let token = token_nuevo();
    cfg.tokens.push(token.clone());
    guardar_servidor(&app, &cfg).map_err(|e| e.to_string())?;

    let dato = serde_json::json!({
        "addr": serde_json::from_str::<serde_json::Value>(&addr).map_err(|e| e.to_string())?,
        "token": token,
        "nombre": nombre_maquina(),
    });
    let enlace = format!("yappy://pair?d={}", B64.encode(dato.to_string()));
    let qr = qrcode::QrCode::new(enlace.as_bytes())
        .map_err(|e| e.to_string())?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build();
    Ok(serde_json::json!({"enlace": enlace, "qr_svg": qr}))
}

#[tauri::command]
pub fn puente_revocar_cmd(app: AppHandle, prefijo: String) -> Result<(), String> {
    let mut cfg = leer_servidor(&app);
    cfg.tokens.retain(|t| !t.starts_with(&prefijo));
    guardar_servidor(&app, &cfg).map_err(|e| e.to_string())
}

/// El móvil recibe el deep link (o el enlace pegado) y guarda el vínculo.
#[tauri::command]
pub fn puente_vincular_cmd(app: AppHandle, dato: String) -> Result<ConfigMovil, String> {
    let b64 = dato
        .trim()
        .trim_start_matches("yappy://pair?d=")
        .trim_start_matches("d=");
    let json = B64
        .decode(b64.as_bytes())
        .map_err(|_| "código ilegible".to_string())?;
    let v: serde_json::Value =
        serde_json::from_slice(&json).map_err(|_| "código ilegible".to_string())?;
    let cfg = ConfigMovil {
        addr: Some(v["addr"].to_string()),
        token: v["token"].as_str().map(String::from),
        nombre: v["nombre"].as_str().map(String::from),
    };
    if cfg.addr.is_none() || cfg.token.is_none() {
        return Err("al código le faltan piezas".into());
    }
    guardar_movil(&app, &cfg).map_err(|e| e.to_string())?;
    Ok(cfg)
}

#[tauri::command]
pub fn puente_movil_estado_cmd(app: AppHandle) -> Result<ConfigMovil, String> {
    Ok(leer_movil(&app))
}

#[tauri::command]
pub fn puente_desvincular_cmd(app: AppHandle) -> Result<(), String> {
    guardar_movil(&app, &ConfigMovil::default()).map_err(|e| e.to_string())
}

/// «Convertir en el ordenador»: manda el texto y recibe el .m4b acabado.
#[tauri::command]
pub async fn puente_convertir_cmd(
    app: AppHandle,
    titulo: String,
    texto: String,
) -> Result<String, String> {
    // El puente es cosa de parlanchines (solo muerde donde hay tienda).
    if !crate::compras::es_pro() {
        return Err("parlanchin".into());
    }
    convertir(app, titulo, texto, RecetaRemota::default())
        .await
        .map_err(|e| {
            tracing::warn!("puente_convertir: {e:#}");
            e.to_string()
        })
}

/// La sonda desde la trastienda: ¿responde el ordenador emparejado?
#[tauri::command]
pub async fn puente_probar_cmd(app: AppHandle) -> Result<serde_json::Value, String> {
    sondear(&app)
        .await
        .map(|(nombre, ms)| serde_json::json!({"nombre": nombre, "ms": ms}))
        .map_err(|e| {
            tracing::warn!("puente_probar: {e:#}");
            e.to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un .yappy pequeño y VÁLIDO para las pruebas de protocolo.
    fn pack_de_prueba(dir: &std::path::Path, titulo: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let m4b = dir.join("falso.m4b");
        std::fs::write(&m4b, vec![7u8; 300_000]).unwrap();
        let pack = dir.join("prueba.yappy");
        let manifiesto = crate::yappy_pack::ManifiestoPack {
            version: crate::yappy_pack::VERSION,
            titulo: titulo.into(),
            autor: "Prueba".into(),
            voz: "Alex".into(),
            velocidad: 1.0,
            idioma: "es".into(),
            duracion_secs: 12.0,
            capitulos: vec![],
            creado_unix: 0,
            app_version: "test".into(),
        };
        let tiempos = vec![crate::yappy_pack::TiempoFrase {
            ini_s: 0.0,
            fin_s: 2.0,
            parrafo: 0,
            origen_ini: 0,
            origen_fin: 10,
            texto: "Hola prueba".into(),
        }];
        crate::yappy_pack::escribir(&pack, &manifiesto, &m4b, &tiempos, "Hola.", None).unwrap();
        pack
    }

    /// Arranca un SERVIDOR iroh real de una conexión con un productor dado
    /// y devuelve su dirección para conectar.
    async fn servidor_de_prueba(
        productor: Productor,
        tokens: Vec<String>,
    ) -> (iroh::EndpointAddr, tauri::async_runtime::JoinHandle<()>) {
        let endpoint = iroh::Endpoint::builder(presets::N0)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .unwrap();
        endpoint.online().await;
        let addr = endpoint.addr();
        let tarea = tauri::async_runtime::spawn(async move {
            while let Some(entrante) = endpoint.accept().await {
                let Ok(conn) = entrante.await else { continue };
                let _ = atender_conexion(conn, tokens.clone(), productor.clone()).await;
            }
        });
        (addr, tarea)
    }

    /// Cliente de protocolo puro para las pruebas: pide y descarga a un
    /// fichero, devolviendo (bytes_totales, bytes_recibidos).
    async fn cliente_de_prueba(
        addr: iroh::EndpointAddr,
        peticion: &Peticion,
        destino: &std::path::Path,
    ) -> Result<(u64, u64)> {
        let endpoint = iroh::Endpoint::builder(presets::N0).bind().await.unwrap();
        let conn = endpoint.connect(addr, ALPN).await?;
        let (mut tx, mut rx) = conn.open_bi().await?;
        tx.write_all(format!("{}\n", serde_json::to_string(peticion)?).as_bytes())
            .await?;
        let total;
        let desde;
        loop {
            let linea = leer_linea(&mut rx, 1024 * 1024).await?;
            let v: serde_json::Value = serde_json::from_str(&linea)?;
            match v["tipo"].as_str().unwrap_or("") {
                "listo" => {
                    total = v["bytes"].as_u64().unwrap();
                    desde = v["desde"].as_u64().unwrap();
                    break;
                }
                "error" => return Err(anyhow!(v["mensaje"].as_str().unwrap_or("?").to_string())),
                _ => {}
            }
        }
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(destino)
            .await?;
        f.seek(std::io::SeekFrom::Start(desde)).await?;
        let mut escritos = desde;
        let mut buf = vec![0u8; 64 * 1024];
        while escritos < total {
            match rx.read(&mut buf).await? {
                Some(0) | None => break,
                Some(k) => {
                    f.write_all(&buf[..k]).await?;
                    escritos += k as u64;
                }
            }
        }
        f.flush().await?;
        conn.close(0u32.into(), b"fin");
        Ok((total, escritos))
    }

    #[test]
    #[ignore = "red: dos endpoints iroh REALES hablan el protocolo entero"]
    fn el_puente_entero_con_dos_endpoints_reales() {
        tauri::async_runtime::block_on(async {
            let dir = std::env::temp_dir().join("yappy-puente-prueba");
            let _ = std::fs::remove_dir_all(&dir);
            let pack = pack_de_prueba(&dir, "La pieza del puente");
            let pack2 = pack.clone();
            let productor: Productor = std::sync::Arc::new(move |_p, progreso| {
                let pack = pack2.clone();
                let _ = progreso.send((1, 2));
                let _ = progreso.send((2, 2));
                tauri::async_runtime::spawn_blocking(move || Ok(pack))
            });
            let (addr, _tarea) = servidor_de_prueba(productor, vec!["token-prueba".into()]).await;

            // 1. El token malo se rechaza.
            let mala = Peticion {
                token: "impostor".into(),
                cancelar: None,
                titulo: "x".into(),
                texto: "x".into(),
                voz: None,
                velocidad: None,
                steps: None,
                desde: 0,
                idioma: None,
                sonda: false,
            };
            let destino_mal = dir.join("mal.yappy");
            let err = cliente_de_prueba(addr.clone(), &mala, &destino_mal).await;
            assert!(err.is_err(), "el token impostor debió rechazarse");

            // 2. La descarga entera llega y el zip es válido.
            let buena = Peticion {
                token: "token-prueba".into(),
                cancelar: None,
                titulo: "La pieza del puente".into(),
                texto: "Hola.".into(),
                voz: None,
                velocidad: None,
                steps: None,
                desde: 0,
                idioma: None,
                sonda: false,
            };
            let destino = dir.join("recibido.yappy");
            let (total, recibidos) = cliente_de_prueba(addr.clone(), &buena, &destino)
                .await
                .unwrap();
            assert_eq!(total, recibidos, "descarga completa");
            let mani = crate::yappy_pack::leer_manifiesto(&destino).unwrap();
            assert_eq!(mani.titulo, "La pieza del puente");

            // 3. LA REANUDACIÓN: se corta a la mitad y se retoma con
            //    `desde`, quedando el fichero entero y válido.
            let mitad = total / 2;
            let destino2 = dir.join("reanudado.yappy");
            let entero = std::fs::read(&destino).unwrap();
            std::fs::write(&destino2, &entero[..mitad as usize]).unwrap();
            let reanuda = Peticion {
                desde: mitad,
                ..buena.clone()
            };
            let (total2, recibidos2) = cliente_de_prueba(addr, &reanuda, &destino2).await.unwrap();
            assert_eq!(total2, total);
            assert_eq!(recibidos2, total, "el resto llegó desde el offset");
            let mani2 = crate::yappy_pack::leer_manifiesto(&destino2).unwrap();
            assert_eq!(mani2.titulo, "La pieza del puente");
            assert_eq!(
                std::fs::read(&destino2).unwrap(),
                entero,
                "byte a byte idéntico tras reanudar"
            );
            println!(
                "PUENTE OK: {total} bytes, token rechazado, descarga entera y reanudación byte a byte"
            );
        });
    }

    #[test]
    #[ignore = "red + motor real: el puente sintetiza de verdad y entrega un .yappy"]
    fn el_puente_con_el_motor_real() {
        tauri::async_runtime::block_on(async {
            let root = dirs::data_dir()
                .unwrap()
                .join("com.yappy.app/models/supertonic-3");
            assert!(root.exists(), "no está el modelo en {root:?}");
            let engine = std::sync::Arc::new(
                yappy_core::TtsEngine::new(yappy_core::engine::engine_config(&root)).unwrap(),
            );
            let productor: Productor = std::sync::Arc::new(move |peticion, progreso| {
                let engine = engine.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    sintetizar_pack(engine, &peticion, "Daniel".into(), 1.05, 4, progreso)
                })
            });
            let (addr, _tarea) = servidor_de_prueba(productor, vec!["t".into()]).await;
            let peticion = Peticion {
                token: "t".into(),
                cancelar: None,
                titulo: "La prueba real del puente".into(),
                texto: "Hola desde el puente. Esta frase viajó por iroh de verdad.".into(),
                voz: Some("Daniel".into()),
                velocidad: Some(1.05),
                steps: Some(4),
                desde: 0,
                idioma: None,
                sonda: false,
            };
            let dir = std::env::temp_dir().join("yappy-puente-prueba-real");
            let _ = std::fs::create_dir_all(&dir);
            let destino = dir.join("real.yappy");
            let (total, recibidos) = cliente_de_prueba(addr, &peticion, &destino).await.unwrap();
            assert_eq!(total, recibidos);
            let contenido = crate::yappy_pack::leer(&destino).unwrap();
            assert_eq!(contenido.manifiesto.titulo, "La prueba real del puente");
            assert!(contenido.tiempos.len() >= 2, "karaoke presente");
            assert!(contenido.manifiesto.duracion_secs > 1.0);
            println!(
                "PUENTE REAL OK: {} bytes, {:.1}s de audio, {} frases de karaoke",
                total,
                contenido.manifiesto.duracion_secs,
                contenido.tiempos.len()
            );
        });
    }
}
