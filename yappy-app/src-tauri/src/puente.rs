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
//!   → una línea JSON: {token, titulo, texto, voz, velocidad, steps}
//!   ← líneas JSON de progreso: {"tipo":"sintetizando","hecho":n,"total":m}
//!     … {"tipo":"codificando"} … {"tipo":"listo","bytes":N}\n + N bytes
//!     crudos del .m4b. Errores: {"tipo":"error","mensaje":"…"}.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use iroh::endpoint::presets;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    std::fs::write(ruta_config(app, "puente-servidor.json")?, serde_json::to_string_pretty(c)?)?;
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
    std::fs::write(ruta_config(app, "puente-movil.json")?, serde_json::to_string_pretty(c)?)?;
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

#[derive(Debug, Deserialize)]
struct Peticion {
    token: String,
    titulo: String,
    texto: String,
    voz: Option<String>,
    velocidad: Option<f32>,
    steps: Option<usize>,
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

async fn atender(app: AppHandle, conn: iroh::endpoint::Connection) -> Result<()> {
    let (mut tx, mut rx) = conn.accept_bi().await?;

    // La petición: una línea JSON.
    let linea = leer_linea(&mut rx, 4 * 1024 * 1024).await?;
    let peticion: Peticion = serde_json::from_str(&linea).context("petición ilegible")?;

    let admitidos = leer_servidor(&app).tokens;
    if !admitidos.contains(&peticion.token) {
        enviar_json(&mut tx, &serde_json::json!({"tipo":"error","mensaje":"token no emparejado"})).await?;
        return Ok(());
    }
    tracing::info!(
        "puente: trabajo aceptado «{}» ({} chars)",
        peticion.titulo,
        peticion.texto.chars().count()
    );

    // Sintetizar con el guionizador completo, en un hilo aparte.
    let estado = app.state::<Arc<crate::state::AppState>>();
    let root = crate::model::model_root(&app).map_err(|e| anyhow!(e))?;
    let engine = estado.engine_or_load(&root)?;
    let (voz, velocidad, steps) = {
        let s = estado.settings.lock().unwrap();
        (
            peticion.voz.unwrap_or_else(|| s.voice.clone()),
            peticion.velocidad.unwrap_or(s.speed),
            peticion.steps.unwrap_or_else(|| s.quality.total_steps()),
        )
    };

    let (progreso_tx, mut progreso_rx) = tokio::sync::mpsc::unbounded_channel::<(usize, usize)>();
    let texto = peticion.texto.clone();
    let sintesis = tokio::task::spawn_blocking(move || -> Result<(Vec<f32>, u32, Vec<(f64, String)>)> {
        let guion = yappy_core::guion::construir_desde_texto(&texto, "en");
        let opts = yappy_core::engine::SynthesisOptions {
            voice: voz,
            speed: velocidad,
            default_lang: guion.idioma_base.clone(),
            total_steps: steps,
            seed: None,
            detectar_idioma: true,
            pausa_entre_parrafos_s: 0.0,
        };
        let mut samples: Vec<f32> = Vec::new();
        let mut sample_rate = 44_100u32;
        let mut capitulos: Vec<(f64, String)> = Vec::new();
        let mut pieza_previa = usize::MAX;
        engine.synthesize_guion(&guion, &opts, |chunk| {
            sample_rate = chunk.sample_rate as u32;
            // Los títulos del guion se convierten en capítulos del .m4b.
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
            samples.extend_from_slice(&chunk.samples);
            let _ = progreso_tx.send((chunk.index + 1, chunk.total));
            Ok(())
        })?;
        Ok((samples, sample_rate, capitulos))
    });

    // Reenviar el progreso mientras sintetiza.
    let mut ultimo = 0usize;
    loop {
        tokio::select! {
            avance = progreso_rx.recv() => {
                match avance {
                    Some((hecho, total)) => {
                        if hecho.saturating_sub(ultimo) >= 3 || hecho == total {
                            ultimo = hecho;
                            enviar_json(&mut tx, &serde_json::json!({
                                "tipo":"sintetizando","hecho":hecho,"total":total
                            })).await?;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    let (samples, sample_rate, capitulos) = sintesis.await??;
    enviar_json(&mut tx, &serde_json::json!({"tipo":"codificando"})).await?;

    // Codificar el .m4b a un temporal y mandarlo entero.
    let tmp = std::env::temp_dir().join(format!("yappy-puente-{}.m4b", std::process::id()));
    {
        let chapters: Vec<crate::audiobook::Chapter> = if capitulos.is_empty() {
            vec![crate::audiobook::Chapter { start_secs: 0.0, title: peticion.titulo.clone() }]
        } else {
            capitulos
                .into_iter()
                .map(|(s, t)| crate::audiobook::Chapter { start_secs: s, title: t })
                .collect()
        };
        let meta = crate::audiobook::M4bMetadata {
            title: peticion.titulo.clone(),
            author: "Yappy".into(),
            album: peticion.titulo.clone(),
        };
        let tmp2 = tmp.clone();
        tokio::task::spawn_blocking(move || {
            crate::audiobook::encode_m4b(&samples, sample_rate, &chapters, &meta, &tmp2)
        })
        .await??;
    }

    let bytes = tokio::fs::read(&tmp).await?;
    let _ = tokio::fs::remove_file(&tmp).await;
    enviar_json(&mut tx, &serde_json::json!({"tipo":"listo","bytes": bytes.len()})).await?;
    tx.write_all(&bytes).await?;
    tx.finish()?;
    // Esperar a que el otro lado cierre para no cortar los últimos bytes.
    let _ = conn.closed().await;
    tracing::info!("puente: «{}» servido ({} bytes)", peticion.titulo, bytes.len());
    Ok(())
}

// ── El cliente (móvil, y también escritorio si algún día quiere) ─────────

async fn convertir(app: AppHandle, titulo: String, texto: String) -> Result<String> {
    let cfg = leer_movil(&app);
    let (addr_json, token) = match (cfg.addr, cfg.token) {
        (Some(a), Some(t)) => (a, t),
        _ => return Err(anyhow!("no hay ningún ordenador emparejado")),
    };
    let addr: iroh::EndpointAddr = serde_json::from_str(&addr_json).context("dirección corrupta")?;

    let endpoint = iroh::Endpoint::builder(presets::N0)
        .bind()
        .await
        .map_err(|e| anyhow!("iroh bind: {e}"))?;
    let conn = endpoint
        .connect(addr, ALPN)
        .await
        .map_err(|e| anyhow!("no llego al ordenador ({e}); ¿está Yappy abierto allí?"))?;
    let (mut tx, mut rx) = conn.open_bi().await?;

    let (voz, velocidad) = {
        let estado = app.state::<Arc<crate::state::AppState>>();
        let s = estado.settings.lock().unwrap();
        (s.voice.clone(), s.speed)
    };
    let peticion = serde_json::json!({
        "token": token,
        "titulo": titulo,
        "texto": texto,
        "voz": voz,
        "velocidad": velocidad,
        "steps": 12,
    });
    tx.write_all(format!("{peticion}\n").as_bytes()).await?;

    // Progreso + resultado.
    let total_bytes: usize;
    loop {
        let linea = leer_linea(&mut rx, 1024 * 1024).await?;
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
                let _ = app.emit("puente_progreso", serde_json::json!({"etapa":"codificando"}));
            }
            "listo" => {
                total_bytes = v["bytes"].as_u64().unwrap_or(0) as usize;
                break;
            }
            "error" => {
                return Err(anyhow!(v["mensaje"].as_str().unwrap_or("error remoto").to_string()));
            }
            otro => return Err(anyhow!("respuesta desconocida: {otro}")),
        }
    }

    // El fichero, a la biblioteca de audiolibros (la misma carpeta que los
    // renders locales, así aparece en Biblioteca → Audiolibros).
    let mut cuerpo = vec![0u8; total_bytes];
    rx.read_exact(&mut cuerpo).await.map_err(|e| anyhow!("descarga cortada: {e}"))?;
    conn.close(0u32.into(), b"gracias");

    let dir = app.path().document_dir().context("document_dir")?;
    std::fs::create_dir_all(&dir)?;
    let nombre_limpio: String = titulo
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '-' })
        .take(60)
        .collect();
    let destino = dir.join(format!("{}.m4b", nombre_limpio.trim()));
    tokio::fs::write(&destino, &cuerpo).await?;
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
        rx.read_exact(&mut byte).await.map_err(|e| anyhow!("stream cortado: {e}"))?;
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&buf).to_string());
        }
        buf.push(byte[0]);
    }
    Err(anyhow!("línea demasiado larga"))
}

async fn enviar_json(
    tx: &mut iroh::endpoint::SendStream,
    v: &serde_json::Value,
) -> Result<()> {
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
    (0..32).map(|_| format!("{:x}", rng.gen_range(0..16u8))).collect()
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
        tokens: cfg.tokens.iter().map(|t| t.chars().take(8).collect()).collect(),
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
    let json = B64.decode(b64.as_bytes()).map_err(|_| "código ilegible".to_string())?;
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
    convertir(app, titulo, texto).await.map_err(|e| {
        tracing::warn!("puente_convertir: {e:#}");
        e.to_string()
    })
}
