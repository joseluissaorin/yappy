//! LA IMPRENTA: el gestor de encargos de audiolibros (v0.3.0).
//!
//! Cada render es un ENCARGO con vida propia: se encola, se sintetiza por
//! párrafos con CHECKPOINT a disco (pausar, reanudar o morir la app cuesta
//! como mucho un párrafo), se codifica, se empaqueta como .yappy (con el
//! .m4b extraíble al instante) y se publica en la biblioteca. El motor
//! puede ser ESTE aparato o el ordenador emparejado (el puente).
//!
//! Persistencia: {app_data}/imprenta/indice.json + una carpeta por encargo
//! con el texto, las piezas sintetizadas (p00001.bin + tiempos parciales)
//! y nada más: al terminar, el artefacto vive en la biblioteca y la
//! carpeta se limpia. Todo por NOMBRE relativo (iOS migra el contenedor).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotorEncargo {
    Local,
    Ordenador,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoEncargo {
    EnCola,
    Sintetizando,
    Codificando,
    Empaquetando,
    Descargando,
    Pausado,
    Hecho,
    Error,
    Cancelado,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encargo {
    pub id: String,
    pub titulo: String,
    pub voz: String,
    pub velocidad: f32,
    pub steps: usize,
    /// Idioma fijado (None = detectar).
    pub idioma: Option<String>,
    pub motor: MotorEncargo,
    pub estado: EstadoEncargo,
    #[serde(default)]
    pub error: Option<String>,
    pub creado_unix: u64,
    pub orden: u32,
    // ── Progreso ──
    #[serde(default)]
    pub piezas_hechas: usize,
    #[serde(default)]
    pub piezas_total: usize,
    #[serde(default)]
    pub segundos_audio: f32,
    /// Ritmo MEDIDO (segundos de reloj por pieza) para el ETA honesto.
    #[serde(default)]
    pub segundos_por_pieza: f32,
    /// La frase que la voz dice ahora mismo (teletipo del detalle).
    #[serde(default)]
    pub frase_actual: String,
    /// Bytes descargados / totales (encargos remotos).
    #[serde(default)]
    pub bytes_hechos: u64,
    #[serde(default)]
    pub bytes_total: u64,
    /// Nombre del artefacto en la biblioteca (relativo a document_dir).
    #[serde(default)]
    pub artefacto: Option<String>,
    /// De dónde vino el encargo («» = de esta app). Un encargo que llega
    /// por el puente se ESPEJA aquí para verse, pero lo gestiona el
    /// propio puente: el runner no lo toca.
    #[serde(default)]
    pub origen: String,
    #[serde(default)]
    pub espejo: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Indice {
    encargos: Vec<Encargo>,
}

/// Las banderas vivas de control (no se persisten).
#[derive(Default)]
struct Control {
    pausar: AtomicBool,
    cancelar: AtomicBool,
}

#[derive(Default)]
pub struct Imprenta {
    controles: Mutex<std::collections::HashMap<String, Arc<Control>>>,
    /// El runner solo puede existir una vez.
    runner_vivo: AtomicBool,
    /// Despertador del runner: se avisa al encolar o reanudar.
    aviso: tokio::sync::Notify,
}

// ── Persistencia ─────────────────────────────────────────────────────────

fn dir_imprenta(app: &AppHandle) -> Result<PathBuf> {
    let d = app
        .path()
        .app_data_dir()
        .context("app_data_dir")?
        .join("imprenta");
    fs::create_dir_all(&d)?;
    Ok(d)
}

fn dir_encargo(app: &AppHandle, id: &str) -> Result<PathBuf> {
    let d = dir_imprenta(app)?.join(id);
    fs::create_dir_all(&d)?;
    Ok(d)
}

fn leer_indice(app: &AppHandle) -> Indice {
    dir_imprenta(app)
        .ok()
        .map(|d| d.join("indice.json"))
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn guardar_indice(app: &AppHandle, indice: &Indice) {
    if let Ok(d) = dir_imprenta(app) {
        let tmp = d.join("indice.json.tmp");
        if fs::write(
            &tmp,
            serde_json::to_string_pretty(indice).unwrap_or_default(),
        )
        .is_ok()
        {
            let _ = fs::rename(&tmp, d.join("indice.json"));
        }
    }
}

fn emitir(app: &AppHandle) {
    let indice = leer_indice(app);
    let _ = app.emit("imprenta_actualizada", &indice.encargos);
}

/// Muta UN encargo del índice y avisa a las pantallas.
fn mutar_encargo(app: &AppHandle, id: &str, f: impl FnOnce(&mut Encargo)) {
    let mut indice = leer_indice(app);
    if let Some(e) = indice.encargos.iter_mut().find(|e| e.id == id) {
        f(e);
    }
    guardar_indice(app, &indice);
    emitir(app);
}

fn ahora_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── El alta de encargos ──────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn encargar(
    app: &AppHandle,
    titulo: String,
    texto: String,
    voz: Option<String>,
    velocidad: Option<f32>,
    steps: Option<usize>,
    idioma: Option<String>,
    motor: MotorEncargo,
) -> Result<Encargo> {
    let estado = app.state::<Arc<AppState>>();
    let (voz, velocidad, steps) = {
        let s = estado.settings.lock().unwrap();
        (
            voz.filter(|v| !v.is_empty())
                .unwrap_or_else(|| s.voice.clone()),
            velocidad.unwrap_or(s.speed),
            steps.unwrap_or_else(|| s.quality.total_steps()),
        )
    };
    let id = format!("{}-{:04}", ahora_unix(), rand_suffix());
    let dir = dir_encargo(app, &id)?;
    fs::write(dir.join("texto.md"), &texto)?;

    let mut indice = leer_indice(app);
    let orden = indice.encargos.iter().map(|e| e.orden).max().unwrap_or(0) + 1;
    let encargo = Encargo {
        id: id.clone(),
        titulo,
        voz,
        velocidad,
        steps,
        idioma: idioma.filter(|l| !l.is_empty() && l != "auto"),
        motor,
        estado: EstadoEncargo::EnCola,
        error: None,
        creado_unix: ahora_unix(),
        orden,
        piezas_hechas: 0,
        piezas_total: 0,
        segundos_audio: 0.0,
        segundos_por_pieza: 0.0,
        frase_actual: String::new(),
        bytes_hechos: 0,
        bytes_total: 0,
        artefacto: None,
        origen: String::new(),
        espejo: false,
    };
    indice.encargos.push(encargo.clone());
    guardar_indice(app, &indice);
    emitir(app);
    despertar(app);
    Ok(encargo)
}

fn rand_suffix() -> u32 {
    use std::sync::atomic::AtomicU32;
    static N: AtomicU32 = AtomicU32::new(0);
    N.fetch_add(1, Ordering::Relaxed) % 10000
}

// ── Los mandos ───────────────────────────────────────────────────────────

pub fn pausar(app: &AppHandle, id: &str) {
    let estado = app.state::<Arc<AppState>>();
    if let Some(c) = estado.imprenta.controles.lock().unwrap().get(id) {
        c.pausar.store(true, Ordering::SeqCst);
    }
    // Si estaba en cola sin arrancar, queda pausado directamente.
    mutar_encargo(app, id, |e| {
        if e.estado == EstadoEncargo::EnCola {
            e.estado = EstadoEncargo::Pausado;
        }
    });
}

pub fn reanudar(app: &AppHandle, id: &str) {
    mutar_encargo(app, id, |e| {
        if matches!(e.estado, EstadoEncargo::Pausado | EstadoEncargo::Error) {
            e.estado = EstadoEncargo::EnCola;
            e.error = None;
        }
    });
    despertar(app);
}

pub fn cancelar(app: &AppHandle, id: &str) {
    // Un encargo REMOTO además avisa al ordenador para que no siga
    // sintetizando en balde (best effort).
    if let Some(e) = listar(app).into_iter().find(|e| e.id == id) {
        if e.motor == MotorEncargo::Ordenador {
            if let Ok(d) = dir_encargo(app, id) {
                if let Ok(texto) = fs::read_to_string(d.join("texto.md")) {
                    let app2 = app.clone();
                    let titulo = e.titulo.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::puente::cancelar_remoto(app2, titulo, texto).await;
                    });
                }
            }
        }
    }
    let estado = app.state::<Arc<AppState>>();
    if let Some(c) = estado.imprenta.controles.lock().unwrap().get(id) {
        c.cancelar.store(true, Ordering::SeqCst);
        return; // el runner hará la limpieza al soltar el encargo
    }
    // No está corriendo: limpieza directa.
    if let Ok(d) = dir_encargo(app, id) {
        let _ = fs::remove_dir_all(d);
    }
    mutar_encargo(app, id, |e| e.estado = EstadoEncargo::Cancelado);
}

pub fn quitar(app: &AppHandle, id: &str) {
    if let Ok(d) = dir_encargo(app, id) {
        let _ = fs::remove_dir_all(d);
    }
    let mut indice = leer_indice(app);
    indice.encargos.retain(|e| e.id != id);
    guardar_indice(app, &indice);
    emitir(app);
}

/// Editar un encargo parado. Cambiar voz/calidad/velocidad/idioma con
/// piezas ya sintetizadas INVALIDA el checkpoint (rehacer = true lo
/// asume; sin él, la edición se rechaza para que la UI avise).
#[allow(clippy::too_many_arguments)]
pub fn editar(
    app: &AppHandle,
    id: &str,
    voz: Option<String>,
    velocidad: Option<f32>,
    steps: Option<usize>,
    idioma: Option<String>,
    motor: Option<MotorEncargo>,
    rehacer: bool,
) -> Result<()> {
    let indice = leer_indice(app);
    let e = indice
        .encargos
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| anyhow!("encargo desaparecido"))?;
    if !matches!(
        e.estado,
        EstadoEncargo::EnCola | EstadoEncargo::Pausado | EstadoEncargo::Error
    ) {
        return Err(anyhow!("solo se puede editar un encargo parado"));
    }
    let cambia_sintesis = voz.as_ref().map(|v| *v != e.voz).unwrap_or(false)
        || velocidad.map(|v| v != e.velocidad).unwrap_or(false)
        || steps.map(|v| v != e.steps).unwrap_or(false)
        || idioma
            .as_ref()
            .map(|l| Some(l.clone()) != e.idioma)
            .unwrap_or(false);
    if cambia_sintesis && e.piezas_hechas > 0 && !rehacer {
        return Err(anyhow!(
            "cambiar la voz o la calidad rehace lo ya sintetizado"
        ));
    }
    if cambia_sintesis && e.piezas_hechas > 0 {
        // Invalida el checkpoint: fuera las piezas.
        if let Ok(d) = dir_encargo(app, id) {
            if let Ok(entradas) = fs::read_dir(&d) {
                for ent in entradas.flatten() {
                    let nombre = ent.file_name().to_string_lossy().to_string();
                    if nombre.starts_with('p') && nombre != "texto.md" {
                        let _ = fs::remove_file(ent.path());
                    }
                }
            }
        }
    }
    mutar_encargo(app, id, |e| {
        if let Some(v) = voz {
            e.voz = v;
        }
        if let Some(v) = velocidad {
            e.velocidad = v;
        }
        if let Some(v) = steps {
            e.steps = v;
        }
        if let Some(l) = idioma {
            e.idioma = if l.is_empty() || l == "auto" {
                None
            } else {
                Some(l)
            };
        }
        if let Some(m) = motor {
            e.motor = m;
        }
        if cambia_sintesis && e.piezas_hechas > 0 {
            e.piezas_hechas = 0;
            e.segundos_audio = 0.0;
        }
    });
    Ok(())
}

pub fn reordenar(app: &AppHandle, id: &str, delta: i32) {
    let mut indice = leer_indice(app);
    indice.encargos.sort_by_key(|e| e.orden);
    let Some(pos) = indice.encargos.iter().position(|e| e.id == id) else {
        return;
    };
    let destino = (pos as i32 + delta).clamp(0, indice.encargos.len() as i32 - 1) as usize;
    let e = indice.encargos.remove(pos);
    indice.encargos.insert(destino, e);
    for (i, e) in indice.encargos.iter_mut().enumerate() {
        e.orden = i as u32 + 1;
    }
    guardar_indice(app, &indice);
    emitir(app);
}

pub fn listar(app: &AppHandle) -> Vec<Encargo> {
    let mut v = leer_indice(app).encargos;
    v.sort_by_key(|e| e.orden);
    v
}

// ── El runner ────────────────────────────────────────────────────────────

fn despertar(app: &AppHandle) {
    let estado = app.state::<Arc<AppState>>();
    estado.imprenta.aviso.notify_one();
}

/// Al arrancar la app: los encargos que murieron trabajando quedan
/// PAUSADOS con su checkpoint intacto, los que descargaban vuelven a la
/// cola (la descarga se reanuda sola), y el runner se pone en marcha.
pub fn arrancar(app: AppHandle) {
    {
        let mut indice = leer_indice(&app);
        for e in indice.encargos.iter_mut() {
            match e.estado {
                EstadoEncargo::Sintetizando
                | EstadoEncargo::Codificando
                | EstadoEncargo::Empaquetando => e.estado = EstadoEncargo::Pausado,
                EstadoEncargo::Descargando => e.estado = EstadoEncargo::EnCola,
                _ => {}
            }
        }
        guardar_indice(&app, &indice);
    }
    let estado = app.state::<Arc<AppState>>();
    if estado.imprenta.runner_vivo.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            let siguiente = listar(&app)
                .into_iter()
                .find(|e| e.estado == EstadoEncargo::EnCola && !e.espejo);
            let Some(encargo) = siguiente else {
                // Nada que hacer: dormir hasta el próximo aviso.
                let estado = app.state::<Arc<AppState>>();
                let aviso = &estado.imprenta.aviso;
                let _ = tokio::time::timeout(std::time::Duration::from_secs(300), aviso.notified())
                    .await;
                continue;
            };
            let id = encargo.id.clone();
            let control = Arc::new(Control::default());
            {
                let estado = app.state::<Arc<AppState>>();
                estado
                    .imprenta
                    .controles
                    .lock()
                    .unwrap()
                    .insert(id.clone(), control.clone());
            }
            let resultado = match encargo.motor {
                MotorEncargo::Local => procesar_local(&app, &encargo, &control).await,
                MotorEncargo::Ordenador => procesar_remoto(&app, &encargo, &control).await,
            };
            {
                let estado = app.state::<Arc<AppState>>();
                estado.imprenta.controles.lock().unwrap().remove(&id);
            }
            match resultado {
                Ok(Desenlace::Hecho) => {}
                Ok(Desenlace::Pausado) => {
                    mutar_encargo(&app, &id, |e| e.estado = EstadoEncargo::Pausado);
                }
                Ok(Desenlace::Cancelado) => {
                    if let Ok(d) = dir_encargo(&app, &id) {
                        let _ = fs::remove_dir_all(d);
                    }
                    mutar_encargo(&app, &id, |e| e.estado = EstadoEncargo::Cancelado);
                }
                Err(e) => {
                    tracing::warn!("imprenta: encargo {id} falló: {e:#}");
                    mutar_encargo(&app, &id, |enc| {
                        enc.estado = EstadoEncargo::Error;
                        enc.error = Some(e.to_string().chars().take(300).collect());
                    });
                }
            }
        }
    });
}

enum Desenlace {
    Hecho,
    Pausado,
    Cancelado,
}

// ── El motor LOCAL con checkpoints ───────────────────────────────────────

/// Una pieza sintetizada en disco: cabecera (sr, nº muestras) + f32le.
fn escribir_pieza(
    dir: &Path,
    n: usize,
    sr: u32,
    samples: &[f32],
    tiempos: &[crate::yappy_pack::TiempoFrase],
) -> Result<()> {
    use std::io::Write as _;
    let tmp = dir.join(format!("p{n:05}.bin.tmp"));
    {
        let mut f = std::io::BufWriter::new(fs::File::create(&tmp)?);
        f.write_all(&sr.to_le_bytes())?;
        f.write_all(&(samples.len() as u64).to_le_bytes())?;
        for s in samples {
            f.write_all(&s.to_le_bytes())?;
        }
    }
    fs::rename(&tmp, dir.join(format!("p{n:05}.bin")))?;
    fs::write(
        dir.join(format!("p{n:05}.tiempos.json")),
        serde_json::to_string(tiempos)?,
    )?;
    Ok(())
}

fn leer_pieza(
    dir: &Path,
    n: usize,
) -> Result<(u32, Vec<f32>, Vec<crate::yappy_pack::TiempoFrase>)> {
    let bytes = fs::read(dir.join(format!("p{n:05}.bin")))?;
    if bytes.len() < 12 {
        return Err(anyhow!("pieza corrupta"));
    }
    let sr = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let cuantas = u64::from_le_bytes(bytes[4..12].try_into().unwrap()) as usize;
    let mut samples = Vec::with_capacity(cuantas);
    for ch in bytes[12..].chunks_exact(4).take(cuantas) {
        samples.push(f32::from_le_bytes(ch.try_into().unwrap()));
    }
    let tiempos = fs::read_to_string(dir.join(format!("p{n:05}.tiempos.json")))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Ok((sr, samples, tiempos))
}

/// ¿Cuántas piezas contiguas desde 0 hay ya en disco?
fn piezas_en_disco(dir: &Path) -> usize {
    let mut n = 0;
    while dir.join(format!("p{n:05}.bin")).exists() {
        n += 1;
    }
    n
}

async fn procesar_local(
    app: &AppHandle,
    encargo: &Encargo,
    control: &Arc<Control>,
) -> Result<Desenlace> {
    let dir = dir_encargo(app, &encargo.id)?;
    let texto = fs::read_to_string(dir.join("texto.md"))?;

    let estado = app.state::<Arc<AppState>>();
    let root = crate::model::model_root(app).map_err(|e| anyhow!(e))?;
    let engine = estado.engine_or_load(&root)?;
    let silencio = estado.settings.lock().unwrap().silence_secs.clamp(0.0, 5.0);

    // El guion completo, con el idioma fijado o detectado.
    let idioma = match &encargo.idioma {
        Some(l) => l.clone(),
        None => yappy_core::lang_detect::detect_document_lang(&texto, "es"),
    };
    let guion = yappy_core::guion::construir_desde_texto(&texto, &idioma);
    let total_piezas = guion.piezas.len();
    let desde = piezas_en_disco(&dir).min(total_piezas);
    mutar_encargo(app, &encargo.id, |e| {
        e.estado = EstadoEncargo::Sintetizando;
        e.piezas_total = total_piezas;
        e.piezas_hechas = desde;
    });
    tracing::info!(
        "imprenta: «{}» sintetizando ({} piezas, desde {desde})",
        encargo.titulo,
        total_piezas
    );

    // iOS: el proceso sigue vivo con la pantalla apagada, con la isla.
    #[cfg(target_os = "ios")]
    let _keepalive = crate::mobile::BackgroundAudioGuard::begin();
    #[cfg(target_os = "ios")]
    crate::mobile::activity_start(&encargo.titulo, total_piezas as i32);

    // Sintetizar DESDE el checkpoint: el subguion re-indexa, así que la
    // primera pieza retomada recibe su suelo de silencio a mano.
    let resultado_sintesis = if desde < total_piezas {
        let mut piezas = guion.piezas[desde..].to_vec();
        if desde > 0 {
            if let Some(p) = piezas.first_mut() {
                p.pausa_antes_s = p.pausa_antes_s.max(silencio);
            }
        }
        let subguion = yappy_core::guion::Guion {
            idioma_base: guion.idioma_base.clone(),
            piezas,
        };
        let opts = yappy_core::engine::SynthesisOptions {
            voice: encargo.voz.clone(),
            speed: encargo.velocidad,
            default_lang: idioma.clone(),
            total_steps: encargo.steps,
            seed: None,
            detectar_idioma: encargo.idioma.is_none(),
            pausa_entre_parrafos_s: silencio,
        };
        let app2 = app.clone();
        let id2 = encargo.id.clone();
        let dir2 = dir.clone();
        let control2 = control.clone();
        let engine2 = engine.clone();
        #[cfg(target_os = "ios")]
        let total_act = total_piezas as i32;
        let estado_arc = app.state::<Arc<AppState>>().inner().clone();
        tokio::task::spawn_blocking(move || -> Result<&'static str> {
            // Una sola síntesis en el proceso (compartido con la lectura).
            let _motor = estado_arc.candado_motor.lock().unwrap();
            let mut pieza_actual: usize = 0; // índice en el subguion
            let mut samples: Vec<f32> = Vec::new();
            let mut tiempos: Vec<crate::yappy_pack::TiempoFrase> = Vec::new();
            let mut sr = 44_100u32;
            let mut reloj = std::time::Instant::now();
            let volcar = |n_global: usize,
                          sr: u32,
                          samples: &mut Vec<f32>,
                          tiempos: &mut Vec<crate::yappy_pack::TiempoFrase>,
                          reloj: &mut std::time::Instant|
             -> Result<()> {
                escribir_pieza(&dir2, n_global, sr, samples, tiempos)?;
                let tardo = reloj.elapsed().as_secs_f32();
                *reloj = std::time::Instant::now();
                let seg_audio = samples.len() as f32 / sr.max(1) as f32;
                mutar_encargo(&app2, &id2, |e| {
                    e.piezas_hechas = n_global + 1;
                    e.segundos_audio += seg_audio;
                    e.segundos_por_pieza = if e.segundos_por_pieza > 0.0 {
                        e.segundos_por_pieza * 0.7 + tardo * 0.3
                    } else {
                        tardo
                    };
                });
                #[cfg(target_os = "ios")]
                crate::mobile::activity_update((n_global + 1) as i32, total_act, "synth", None);
                samples.clear();
                tiempos.clear();
                Ok(())
            };
            let r = engine2.synthesize_guion(&subguion, &opts, |chunk| {
                if control2.cancelar.load(Ordering::SeqCst) {
                    return Err(anyhow!("__cancelado__"));
                }
                if chunk.paragraph_index != pieza_actual {
                    // La pieza anterior está completa: checkpoint.
                    volcar(
                        desde + pieza_actual,
                        sr,
                        &mut samples,
                        &mut tiempos,
                        &mut reloj,
                    )?;
                    pieza_actual = chunk.paragraph_index;
                    if control2.pausar.load(Ordering::SeqCst) {
                        return Err(anyhow!("__pausado__"));
                    }
                }
                sr = chunk.sample_rate as u32;
                let ini = samples.len();
                samples.extend_from_slice(&chunk.samples);
                if !chunk.es_pausa && !chunk.text.trim().is_empty() {
                    tiempos.push(crate::yappy_pack::TiempoFrase {
                        ini_s: ini as f32 / sr as f32,
                        fin_s: samples.len() as f32 / sr as f32,
                        parrafo: desde + chunk.paragraph_index,
                        origen_ini: chunk.origen_ini,
                        origen_fin: chunk.origen_fin,
                        texto: chunk.text.clone(),
                    });
                    let frase = chunk.text.clone();
                    mutar_encargo(&app2, &id2, |e| e.frase_actual = frase);
                }
                Ok(())
            });
            match r {
                Ok(()) => {
                    // La última pieza del subguion.
                    volcar(
                        desde + pieza_actual,
                        sr,
                        &mut samples,
                        &mut tiempos,
                        &mut reloj,
                    )?;
                    Ok("hecho")
                }
                Err(e) if e.to_string().contains("__pausado__") => Ok("pausado"),
                Err(e) if e.to_string().contains("__cancelado__") => Ok("cancelado"),
                Err(e) => Err(e),
            }
        })
        .await?
    } else {
        Ok("hecho")
    };

    #[cfg(target_os = "ios")]
    crate::mobile::activity_end(&encargo.titulo);

    match resultado_sintesis? {
        "pausado" => return Ok(Desenlace::Pausado),
        "cancelado" => return Ok(Desenlace::Cancelado),
        _ => {}
    }

    // ── Codificar: juntar las piezas y hacer el m4b ────────────────────
    mutar_encargo(app, &encargo.id, |e| {
        e.estado = EstadoEncargo::Codificando;
        e.frase_actual = String::new();
    });
    let cuantas = piezas_en_disco(&dir);
    let mut combinado: Vec<f32> = Vec::new();
    let mut tiempos: Vec<crate::yappy_pack::TiempoFrase> = Vec::new();
    let mut sr_final = 44_100u32;
    let mut capitulos: Vec<crate::audiobook::Chapter> = Vec::new();
    for n in 0..cuantas {
        let (sr, samples, ts) = leer_pieza(&dir, n)?;
        sr_final = sr;
        let offset = combinado.len() as f32 / sr.max(1) as f32;
        // Los capítulos: los títulos del guion.
        if let Some(p) = guion.piezas.get(n) {
            use yappy_core::guion::ClasePieza::*;
            if matches!(p.clase, Titulo1 | Titulo2 | Titulo3) {
                capitulos.push(crate::audiobook::Chapter {
                    title: p.texto.chars().take(80).collect(),
                    start_secs: offset as f64,
                });
            }
        }
        for t in ts {
            tiempos.push(crate::yappy_pack::TiempoFrase {
                ini_s: t.ini_s + offset,
                fin_s: t.fin_s + offset,
                ..t
            });
        }
        combinado.extend(samples);
    }
    if capitulos.is_empty() {
        capitulos.push(crate::audiobook::Chapter {
            title: encargo.titulo.clone(),
            start_secs: 0.0,
        });
    }
    let duracion = combinado.len() as f32 / sr_final.max(1) as f32;
    let m4b = dir.join("salida.m4b");
    {
        let meta = crate::audiobook::M4bMetadata {
            title: encargo.titulo.clone(),
            author: "Yappy".into(),
            album: encargo.titulo.clone(),
        };
        let capitulos2 = capitulos.clone();
        let m4b2 = m4b.clone();
        let combinado2 = std::mem::take(&mut combinado);
        tokio::task::spawn_blocking(move || {
            crate::audiobook::encode_m4b(&combinado2, sr_final, &capitulos2, &meta, &m4b2)
        })
        .await??;
    }
    if control.cancelar.load(Ordering::SeqCst) {
        return Ok(Desenlace::Cancelado);
    }

    // ── Empaquetar el .yappy (con el título dicho) y publicar ──────────
    mutar_encargo(app, &encargo.id, |e| e.estado = EstadoEncargo::Empaquetando);
    let titulo_wav = sintetizar_titulo(&engine, &encargo.voz, &encargo.titulo, &idioma, &dir);
    let manifiesto = crate::yappy_pack::ManifiestoPack {
        version: crate::yappy_pack::VERSION,
        titulo: encargo.titulo.clone(),
        autor: String::new(),
        voz: encargo.voz.clone(),
        velocidad: encargo.velocidad,
        idioma,
        duracion_secs: duracion,
        capitulos: capitulos
            .iter()
            .map(|c| crate::yappy_pack::CapituloPack {
                titulo: c.title.clone(),
                inicio_s: c.start_secs as f32,
            })
            .collect(),
        creado_unix: ahora_unix(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let pack_tmp = dir.join("salida.yappy");
    crate::yappy_pack::escribir(
        &pack_tmp,
        &manifiesto,
        &m4b,
        &tiempos,
        &texto,
        titulo_wav.as_deref(),
    )?;

    let nombre = publicar(app, &pack_tmp, &encargo.titulo)?;
    // El título dicho, a la caché de dichos (suena al instante).
    if let Ok(wav) = crate::commands::dicho_path(app, &encargo.voz, &encargo.titulo) {
        if let Some(t) = &titulo_wav {
            let _ = fs::copy(t, wav);
        }
    }
    let _ = fs::remove_dir_all(&dir);
    mutar_encargo(app, &encargo.id, |e| {
        e.estado = EstadoEncargo::Hecho;
        e.artefacto = Some(nombre.clone());
    });
    avisar_terminado(app, &encargo.titulo, duracion);
    Ok(Desenlace::Hecho)
}

/// El clip del título, cortito, para el manifiesto.
fn sintetizar_titulo(
    engine: &Arc<yappy_core::TtsEngine>,
    voz: &str,
    titulo: &str,
    idioma: &str,
    dir: &Path,
) -> Option<PathBuf> {
    let opts = yappy_core::engine::SynthesisOptions {
        voice: voz.to_string(),
        speed: 1.0,
        default_lang: idioma.to_string(),
        total_steps: 8,
        seed: None,
        detectar_idioma: true,
        pausa_entre_parrafos_s: 0.0,
    };
    let mut muestras: Vec<f32> = Vec::new();
    let mut sr = 44_100u32;
    engine
        .synthesize_streaming(titulo, &opts, |chunk| {
            sr = chunk.sample_rate as u32;
            muestras.extend_from_slice(&chunk.samples);
            Ok(())
        })
        .ok()?;
    if muestras.is_empty() {
        return None;
    }
    let ruta = dir.join("titulo.wav");
    crate::playback::write_wav_file(ruta.to_string_lossy().as_ref(), &muestras, sr).ok()?;
    Some(ruta)
}

/// Publica el .yappy terminado en la biblioteca con el nombre del título.
fn publicar(app: &AppHandle, pack: &Path, titulo: &str) -> Result<String> {
    let dir = app.path().document_dir().context("document_dir")?;
    fs::create_dir_all(&dir)?;
    let base: String = titulo
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { ' ' } else { c })
        .collect::<String>()
        .trim()
        .chars()
        .take(80)
        .collect();
    let base = if base.is_empty() {
        "Audiolibro".into()
    } else {
        base
    };
    let mut destino = dir.join(format!("{base}.yappy"));
    let mut i = 2;
    while destino.exists() {
        destino = dir.join(format!("{base} ({i}).yappy"));
        i += 1;
    }
    fs::rename(pack, &destino).or_else(|_| fs::copy(pack, &destino).map(|_| ()))?;
    Ok(destino
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(base))
}

fn avisar_terminado(app: &AppHandle, titulo: &str, duracion: f32) {
    let mins = (duracion / 60.0).round().max(1.0) as i64;
    #[cfg(target_os = "ios")]
    crate::mobile::notify(
        "yappy.imprenta.hecho",
        "Audiolibro listo",
        &format!("{titulo}: {mins} min de audio, en tu biblioteca"),
    );
    #[cfg(desktop)]
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Audiolibro listo")
            .body(format!("{titulo}: {mins} min de audio, en tu biblioteca"))
            .show();
    }
    #[cfg(not(any(target_os = "ios", desktop)))]
    let _ = (app, titulo);
}

// ── El motor REMOTO (el ordenador emparejado) ────────────────────────────

async fn procesar_remoto(
    app: &AppHandle,
    encargo: &Encargo,
    control: &Arc<Control>,
) -> Result<Desenlace> {
    let dir = dir_encargo(app, &encargo.id)?;
    let texto = fs::read_to_string(dir.join("texto.md"))?;
    mutar_encargo(app, &encargo.id, |e| e.estado = EstadoEncargo::Sintetizando);

    // El progreso del puente alimenta la ficha del encargo.
    let id2 = encargo.id.clone();
    let app2 = app.clone();
    let escucha = app.listen("puente_progreso", move |ev| {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(ev.payload()) else {
            return;
        };
        let etapa = v["etapa"].as_str().unwrap_or("");
        match etapa {
            "sintetizando" => {
                let hecho = v["hecho"].as_u64().unwrap_or(0) as usize;
                let total = v["total"].as_u64().unwrap_or(0) as usize;
                mutar_encargo(&app2, &id2, |e| {
                    e.estado = EstadoEncargo::Sintetizando;
                    e.piezas_hechas = hecho;
                    e.piezas_total = total;
                });
            }
            "codificando" => {
                mutar_encargo(&app2, &id2, |e| e.estado = EstadoEncargo::Codificando);
            }
            "descargando" => {
                let hecho = v["hecho"].as_u64().unwrap_or(0);
                let total = v["total"].as_u64().unwrap_or(0);
                mutar_encargo(&app2, &id2, |e| {
                    e.estado = EstadoEncargo::Descargando;
                    e.bytes_hechos = hecho;
                    e.bytes_total = total;
                });
            }
            _ => {}
        }
    });

    // La cancelación mientras corre: el puente aborta su conexión al ver
    // la bandera (comprobada por sondeo aquí, con la conversión en task).
    let convertir = tauri::async_runtime::spawn(crate::puente::convertir_publico(
        app.clone(),
        encargo.titulo.clone(),
        texto,
    ));
    let resultado = loop {
        if control.cancelar.load(Ordering::SeqCst) || control.pausar.load(Ordering::SeqCst) {
            convertir.abort();
            break None;
        }
        if convertir.inner().is_finished() {
            break Some(convertir.await);
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    };
    app.unlisten(escucha);

    let Some(resultado) = resultado else {
        return if control.cancelar.load(Ordering::SeqCst) {
            Ok(Desenlace::Cancelado)
        } else {
            // Pausar un remoto = dejar de descargar; el Mac termina solo y
            // el .part + la caché del servidor retoman al reanudar.
            Ok(Desenlace::Pausado)
        };
    };
    let ruta = resultado??;
    let nombre = std::path::Path::new(&ruta)
        .file_name()
        .map(|n| n.to_string_lossy().to_string());
    let _ = fs::remove_dir_all(&dir);
    let duracion = crate::yappy_pack::leer_manifiesto(std::path::Path::new(&ruta))
        .map(|m| m.duracion_secs)
        .unwrap_or(0.0);
    mutar_encargo(app, &encargo.id, |e| {
        e.estado = EstadoEncargo::Hecho;
        e.artefacto = nombre.clone();
    });
    avisar_terminado(app, &encargo.titulo, duracion);
    Ok(Desenlace::Hecho)
}

// ── El espejo de los trabajos del puente (transparencia en el Mac) ──────

/// Alta de un encargo ESPEJO: llegó por el puente y lo gestiona el
/// propio puente; aquí solo se VE.
pub fn espejo_alta(app: &AppHandle, id: &str, titulo: &str) {
    let mut indice = leer_indice(app);
    if indice.encargos.iter().any(|e| e.id == id) {
        return;
    }
    let orden = indice.encargos.iter().map(|e| e.orden).max().unwrap_or(0) + 1;
    indice.encargos.push(Encargo {
        id: id.to_string(),
        titulo: titulo.to_string(),
        voz: String::new(),
        velocidad: 0.0,
        steps: 0,
        idioma: None,
        motor: MotorEncargo::Local,
        estado: EstadoEncargo::Sintetizando,
        error: None,
        creado_unix: ahora_unix(),
        orden,
        piezas_hechas: 0,
        piezas_total: 0,
        segundos_audio: 0.0,
        segundos_por_pieza: 0.0,
        frase_actual: String::new(),
        bytes_hechos: 0,
        bytes_total: 0,
        artefacto: None,
        origen: "puente".into(),
        espejo: true,
    });
    guardar_indice(app, &indice);
    emitir(app);
}

pub fn espejo_progreso(app: &AppHandle, id: &str, hecho: usize, total: usize) {
    mutar_encargo(app, id, |e| {
        e.piezas_hechas = hecho;
        e.piezas_total = total;
    });
}

pub fn espejo_cierre(app: &AppHandle, id: &str, resultado: Result<(), String>) {
    mutar_encargo(app, id, |e| match &resultado {
        Ok(()) => e.estado = EstadoEncargo::Hecho,
        Err(m) => {
            e.estado = EstadoEncargo::Error;
            e.error = Some(m.clone());
        }
    });
}

// ── Comandos ─────────────────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn imprenta_encargar_cmd(
    app: AppHandle,
    titulo: String,
    texto: String,
    voz: Option<String>,
    velocidad: Option<f32>,
    steps: Option<usize>,
    idioma: Option<String>,
    motor: Option<String>,
) -> Result<Encargo, String> {
    let motor = match motor.as_deref() {
        Some("ordenador") => MotorEncargo::Ordenador,
        _ => MotorEncargo::Local,
    };
    encargar(&app, titulo, texto, voz, velocidad, steps, idioma, motor).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn imprenta_listar_cmd(app: AppHandle) -> Vec<Encargo> {
    listar(&app)
}

#[tauri::command]
pub fn imprenta_pausar_cmd(app: AppHandle, id: String) {
    pausar(&app, &id);
}

#[tauri::command]
pub fn imprenta_reanudar_cmd(app: AppHandle, id: String) {
    reanudar(&app, &id);
}

#[tauri::command]
pub fn imprenta_cancelar_cmd(app: AppHandle, id: String) {
    cancelar(&app, &id);
}

#[tauri::command]
pub fn imprenta_quitar_cmd(app: AppHandle, id: String) {
    quitar(&app, &id);
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn imprenta_editar_cmd(
    app: AppHandle,
    id: String,
    voz: Option<String>,
    velocidad: Option<f32>,
    steps: Option<usize>,
    idioma: Option<String>,
    motor: Option<String>,
    rehacer: Option<bool>,
) -> Result<(), String> {
    let motor = motor.map(|m| {
        if m == "ordenador" {
            MotorEncargo::Ordenador
        } else {
            MotorEncargo::Local
        }
    });
    editar(
        &app,
        &id,
        voz,
        velocidad,
        steps,
        idioma,
        motor,
        rehacer.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn imprenta_reordenar_cmd(app: AppHandle, id: String, delta: i32) {
    reordenar(&app, &id, delta);
}

/// El .m4b del artefacto de un encargo (extraído del .yappy al momento).
#[tauri::command]
pub fn imprenta_m4b_cmd(app: AppHandle, nombre: String) -> Result<String, String> {
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    let pack = dir.join(&nombre);
    let cache = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("biblioteca-cache");
    let m4b = crate::yappy_pack::extraer_audio(&pack, &cache).map_err(|e| e.to_string())?;
    // Con nombre humano al lado, para compartir bonito.
    let bonito = cache.join(format!(
        "{}.m4b",
        pack.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    ));
    let _ = fs::copy(&m4b, &bonito);
    Ok(bonito.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_piezas_del_checkpoint_van_y_vuelven_exactas() {
        let dir = std::env::temp_dir().join("yappy-imprenta-prueba");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let muestras: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.01).sin() * 0.3).collect();
        let tiempos = vec![crate::yappy_pack::TiempoFrase {
            ini_s: 0.1,
            fin_s: 0.2,
            parrafo: 3,
            origen_ini: 5,
            origen_fin: 25,
            texto: "Una frase con tilde: canción".into(),
        }];
        escribir_pieza(&dir, 0, 44_100, &muestras, &tiempos).unwrap();
        escribir_pieza(&dir, 1, 44_100, &muestras[..100], &[]).unwrap();
        assert_eq!(piezas_en_disco(&dir), 2);
        let (sr, leidas, ts) = leer_pieza(&dir, 0).unwrap();
        assert_eq!(sr, 44_100);
        assert_eq!(leidas, muestras);
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].texto, "Una frase con tilde: canción");
        // Un hueco en la numeración corta el checkpoint contiguo.
        escribir_pieza(&dir, 3, 44_100, &muestras[..10], &[]).unwrap();
        assert_eq!(piezas_en_disco(&dir), 2);
    }
}
