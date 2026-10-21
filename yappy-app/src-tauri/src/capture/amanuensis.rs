//! El OCR de Yappy: Amanuensis Ligero (el lector propio de José Luis Saorín, portado a Rust en
//! `yappy_core::ocr`) por omisión, y el OCR del sistema de respaldo.
//!
//! - Pesos EMPOTRADOS en el binario (`resources/amanuensis/`, 14,5 MB: detector Mínimo 0,5 MB + Ligero 3
//!   14 MB): no hay descarga ni rutas de recursos que resolver (en Android los recursos viven dentro del APK).
//! - Política: Ligero lee la página; si la sonda de Amanuensis dice que es impreso moderno latino leído con
//!   seguridad, se queda esa lectura. Si no (otra escritura, foto muy mala, manuscrito), en macOS e iOS se
//!   prueba Apple Vision, que sí lee cirílico, griego, chino, japonés, coreano… (Ligero solo conoce el alfabeto
//!   latino). En Windows, Linux y Android no hay respaldo: Ligero siempre (PaddleOCR se quitó el 21-10-2026).
//! - Siempre se devuelven PÁRRAFOS (Yappy lee y guarda el progreso por párrafo): los de Ligero salen de su
//!   geometría (`salidas.parrafos` de Amanuensis) y los del respaldo se rehacen con la misma regla sobre las
//!   cajas de sus renglones.
//! - GRATIS en todas las plataformas: el OCR no pasa por ninguna puerta de Parlanchín (ni cuota, ni
//!   Err("parlanchin"), ni paywall). La percha de la cola (`cuota::aforo`) cuenta documentos, no el OCR.
//! - Licencia de los pesos de Ligero: etiqueta de USO NO COMERCIAL (datos CC BY-NC; ver MODEL_CARD y
//!   credits.rs). José Luis, autor de los pesos, decidió el 21-10-2026 mantenerlos en Yappy: el OCR no se cobra.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use yappy_core::ocr::{self, Gris, Ligero, Opciones, PaginaLeida};

static DET_ONNX: &[u8] = include_bytes!("../../../resources/amanuensis/det-minimo.onnx");
static DET_CFG: &str = include_str!("../../../resources/amanuensis/det-minimo.json");
static LIGERO_ONNX: &[u8] = include_bytes!("../../../resources/amanuensis/ligero-3.onnx");
static LIGERO_CFG: &str = include_str!("../../../resources/amanuensis/ligero-3.json");

/// Versión de la lectura (entra en la clave de la caché: cambiarla invalida lo guardado).
const VERSION: &str = "ligero3-detmin1-v1";
/// Lado máximo de la página que se lee (las fotos de 48 MP no aportan nada y cuestan memoria).
const LADO_MAX: usize = 4000;
/// Resolución a la que se rasterizan los PDF escaneados en el móvil (la de Amanuensis, `--dpi 200`).
#[cfg(any(target_os = "ios", target_os = "android"))]
const DPI_PDF: i32 = 200;

/// El motor y el último uso. Se suelta tras `OCIOSO` sin OCR: en el móvil no merece retener sus sesiones
/// (recargarlo desde los bytes empotrados cuesta unos 0,05 s).
static MOTOR: Lazy<Mutex<Option<(Ligero, std::time::Instant)>>> = Lazy::new(|| Mutex::new(None));
const OCIOSO: std::time::Duration = std::time::Duration::from_secs(30);

fn con_motor<T>(f: impl FnOnce(&mut Ligero) -> Result<T>) -> Result<T> {
    let mut g = MOTOR.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        let t = std::time::Instant::now();
        // Opciones::ahorro: lotes de 16 renglones, sin arena ni «memory pattern» de ORT (misma lectura, unos
        // 300 MB menos de pico en 77 páginas).
        let m = Ligero::desde_bytes(
            DET_ONNX,
            DET_CFG,
            LIGERO_ONNX,
            LIGERO_CFG,
            Opciones::ahorro(),
        )?;
        tracing::info!("amanuensis: Ligero cargado en {:?}", t.elapsed());
        *g = Some((m, t));
        vigilar_ocio();
    }
    let (m, uso) = g.as_mut().expect("motor recién cargado");
    let r = f(m);
    *uso = std::time::Instant::now();
    r
}

/// Un hilo que suelta el motor cuando lleva `OCIOSO` sin usarse (uno solo, mientras haya motor).
fn vigilar_ocio() {
    static VIGILANDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if VIGILANDO.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("amanuensis-ocio".into())
        .spawn(|| loop {
            std::thread::sleep(OCIOSO / 2);
            let Ok(mut g) = MOTOR.try_lock() else {
                continue;
            };
            match g.as_ref() {
                Some((_, uso)) if uso.elapsed() >= OCIOSO => {
                    *g = None;
                    tracing::info!("amanuensis: motor soltado tras {:?} sin OCR", OCIOSO);
                    VIGILANDO.store(false, std::sync::atomic::Ordering::SeqCst);
                    return;
                }
                None => {
                    VIGILANDO.store(false, std::sync::atomic::Ordering::SeqCst);
                    return;
                }
                _ => {}
            }
        });
}

/// Quién leyó la página.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lector {
    Amanuensis,
    Sistema,
}

#[derive(Clone, Debug)]
pub struct PaginaOcr {
    pub parrafos: Vec<String>,
    pub lector: Lector,
}

impl PaginaOcr {
    pub fn texto(&self) -> String {
        self.parrafos.join("\n\n")
    }
}

/// Extensiones de imagen que se abren como documento (se leen con OCR).
pub const EXT_IMAGEN: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif", "heic", "heif",
];

pub fn es_imagen(ext: &str) -> bool {
    EXT_IMAGEN.contains(&ext.to_ascii_lowercase().as_str())
}

// ---------------------------------------------------------------- imágenes

/// Abre una imagen como gris de Pillow con la orientación EXIF aplicada. Lo que el decodificador de Rust no lee
/// (HEIC de las fotos del iPhone, sobre todo) lo convierte antes el sistema a PNG. -> (gris, ruta legible por
/// el OCR del sistema, temporal a borrar).
fn cargar(ruta: &Path) -> Result<(Gris, PathBuf, Option<PathBuf>)> {
    match ocr::abrir_imagen(ruta) {
        Ok(g) => Ok((acotar(g), ruta.to_path_buf(), None)),
        Err(e) => {
            let tmp = temporal("png");
            if convertir_a_png(ruta, &tmp) {
                let g =
                    ocr::abrir_imagen(&tmp).with_context(|| format!("abrir {}", tmp.display()))?;
                Ok((acotar(g), tmp.clone(), Some(tmp)))
            } else {
                Err(anyhow!(
                    "no se pudo abrir la imagen {}: {e}",
                    ruta.display()
                ))
            }
        }
    }
}

fn acotar(g: Gris) -> Gris {
    let lado = g.ancho.max(g.alto);
    if lado <= LADO_MAX {
        return g;
    }
    let s = LADO_MAX as f64 / lado as f64;
    let (w, h) = (
        ((g.ancho as f64 * s) as usize).max(1),
        ((g.alto as f64 * s) as usize).max(1),
    );
    g.redimensionar(w, h, ocr::pil::Filtro::Lanczos)
}

fn temporal(ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("yappy-ocr-{}.{ext}", uuid::Uuid::new_v4().simple()))
}

#[cfg(target_os = "macos")]
fn convertir_a_png(origen: &Path, destino: &Path) -> bool {
    std::process::Command::new("/usr/bin/sips")
        .args(["-s", "format", "png"])
        .arg(origen)
        .arg("--out")
        .arg(destino)
        .output()
        .map(|o| o.status.success() && destino.exists())
        .unwrap_or(false)
}

#[cfg(target_os = "ios")]
fn convertir_a_png(origen: &Path, destino: &Path) -> bool {
    nativo::imagen_a_png(origen, destino, LADO_MAX as i32)
}

#[cfg(target_os = "android")]
fn convertir_a_png(origen: &Path, destino: &Path) -> bool {
    crate::mobile::imagen_a_png(origen, destino, LADO_MAX as i32)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
fn convertir_a_png(_origen: &Path, _destino: &Path) -> bool {
    false
}

// ---------------------------------------------------------------- lectura de una página

/// Ligero sobre una imagen ya cargada.
pub fn leer_gris(g: &Gris) -> Result<PaginaLeida> {
    con_motor(|m| m.leer_pagina(g))
}

/// Una página (imagen en disco) con la política completa: Ligero y, si no convence, el OCR del sistema.
pub fn leer_ruta(ruta: &Path) -> Result<PaginaOcr> {
    let (g, legible, tmp) = cargar(ruta)?;
    let r = leer_con_respaldo(&g, &legible);
    if let Some(t) = tmp {
        let _ = std::fs::remove_file(t);
    }
    r
}

/// Solo Ligero (el motor «amanuensis» de los ajustes de escritorio).
#[cfg_attr(any(target_os = "ios", target_os = "android"), allow(dead_code))]
pub fn leer_ruta_solo_ligero(ruta: &Path) -> Result<PaginaOcr> {
    let (g, _, tmp) = cargar(ruta)?;
    let p = leer_gris(&g);
    if let Some(t) = tmp {
        let _ = std::fs::remove_file(t);
    }
    Ok(PaginaOcr {
        parrafos: p?.parrafos(),
        lector: Lector::Amanuensis,
    })
}

fn leer_con_respaldo(g: &Gris, legible: &Path) -> Result<PaginaOcr> {
    let t = std::time::Instant::now();
    let ligero = leer_gris(g);
    let (pag, cal) = match &ligero {
        Ok(p) => (Some(p), Some(p.calidad())),
        Err(e) => {
            tracing::warn!("amanuensis: Ligero falló: {e:?}");
            (None, None)
        }
    };
    if let (Some(p), Some(c)) = (pag, cal) {
        tracing::info!(
            "amanuensis: {} renglones en {:?} (media {:.3}, p10 {:.3}, flojas {:.2}; giro {}, incl. {})",
            c.lineas,
            t.elapsed(),
            c.media,
            c.p10,
            c.flojas,
            p.giro,
            p.inclinacion
        );
        if c.impreso_latino() {
            return Ok(PaginaOcr {
                parrafos: p.parrafos(),
                lector: Lector::Amanuensis,
            });
        }
    }
    // Respaldo: el OCR del sistema.
    let aceptable = cal.map(|c| c.aceptable()).unwrap_or(false);
    if let Some(sistema) = respaldo(legible, aceptable) {
        if !sistema.lineas.is_empty() {
            tracing::info!(
                "amanuensis: la página la lee el OCR del sistema ({} renglones)",
                sistema.lineas.len()
            );
            return Ok(PaginaOcr {
                parrafos: sistema.parrafos(),
                lector: Lector::Sistema,
            });
        }
    }
    match ligero {
        Ok(p) => Ok(PaginaOcr {
            parrafos: p.parrafos(),
            lector: Lector::Amanuensis,
        }),
        Err(e) => Err(e),
    }
}

/// Renglones del OCR del sistema como página de Amanuensis (para partir en párrafos con la misma regla).
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn pagina_de(lineas: Vec<ocr::Linea>) -> PaginaLeida {
    PaginaLeida {
        ancho: 0,
        alto: 0,
        giro: 0,
        inclinacion: 0.0,
        lineas,
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn respaldo(legible: &Path, _ligero_aceptable: bool) -> Option<PaginaLeida> {
    #[cfg(target_os = "macos")]
    let json = crate::capture::ocr::vision_lineas_json(legible).ok()?;
    #[cfg(target_os = "ios")]
    let json = nativo::vision_lineas(legible)?;
    Some(pagina_de(lineas_vision(&json)))
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn respaldo(_legible: &Path, _ligero_aceptable: bool) -> Option<PaginaLeida> {
    None
}

/// Solo Vision (el motor «applevision» de los ajustes de macOS), con los párrafos rehechos por geometría.
#[cfg(target_os = "macos")]
pub fn leer_ruta_solo_vision(ruta: &Path) -> Result<PaginaOcr> {
    let json = crate::capture::ocr::vision_lineas_json(ruta)?;
    Ok(PaginaOcr {
        parrafos: pagina_de(lineas_vision(&json)).parrafos(),
        lector: Lector::Sistema,
    })
}

/// JSON de los renglones de Vision (`[{"t", "c", "x0", "y0", "x1", "y1"}]`, px con origen arriba a la izquierda).
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn lineas_vision(json: &str) -> Vec<ocr::Linea> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|l| {
                    let f = |k: &str| l[k].as_f64();
                    Some(ocr::Linea {
                        caja: [f("x0")?, f("y0")?, f("x1")?, f("y1")?],
                        texto: l["t"].as_str()?.to_string(),
                        conf: f("c").unwrap_or(1.0),
                    })
                })
                .filter(|l| !l.texto.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------- documentos

/// Una imagen abierta como documento: sus párrafos separados por línea en blanco (con caché).
pub fn texto_de_imagen(ruta: &Path) -> Result<String> {
    if let Some(t) = cache_leer(ruta) {
        return Ok(t);
    }
    let p = leer_ruta(ruta)?;
    tracing::info!(
        "amanuensis: imagen leída por {:?} ({} párrafos)",
        p.lector,
        p.parrafos.len()
    );
    let texto = p.texto();
    if texto.trim().is_empty() {
        return Err(anyhow!("no se encontró texto en la imagen"));
    }
    cache_guardar(ruta, &texto);
    Ok(texto)
}

/// PDF escaneado en el móvil: el sistema rasteriza cada página (PDFKit en iOS, PdfRenderer en Android) y
/// Amanuensis la lee. Párrafos de todas las páginas separados por línea en blanco (con caché).
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn pdf_escaneado(ruta: &Path) -> Result<String> {
    if let Some(t) = cache_leer(ruta) {
        return Ok(t);
    }
    let n = paginas_pdf(ruta);
    if n <= 0 {
        return Err(anyhow!("no se pudo abrir el PDF para leerlo con OCR"));
    }
    let t0 = std::time::Instant::now();
    let mut buf = String::new();
    for i in 0..n {
        let png = temporal("png");
        if !pagina_pdf_png(ruta, i, &png) {
            tracing::warn!("amanuensis: no se pudo rasterizar la página {}", i + 1);
            continue;
        }
        match leer_ruta(&png) {
            Ok(p) if !p.parrafos.is_empty() => {
                buf.push_str(&p.texto());
                buf.push_str("\n\n");
            }
            Ok(_) => tracing::warn!("amanuensis: página {} sin texto", i + 1),
            Err(e) => tracing::warn!("amanuensis: página {}: {e:?}", i + 1),
        }
        let _ = std::fs::remove_file(&png);
    }
    tracing::info!(
        "amanuensis: PDF escaneado de {n} páginas leído en {:?}",
        t0.elapsed()
    );
    if buf.trim().is_empty() {
        return Err(anyhow!(
            "este PDF solo tiene imágenes y el OCR no encontró texto"
        ));
    }
    cache_guardar(ruta, &buf);
    Ok(buf)
}

#[cfg(target_os = "ios")]
fn paginas_pdf(ruta: &Path) -> i32 {
    nativo::pdf_paginas(ruta)
}
#[cfg(target_os = "ios")]
fn pagina_pdf_png(ruta: &Path, i: i32, destino: &Path) -> bool {
    nativo::pdf_pagina_png(ruta, i, DPI_PDF, destino)
}
#[cfg(target_os = "android")]
fn paginas_pdf(ruta: &Path) -> i32 {
    crate::mobile::pdf_paginas(ruta)
}
#[cfg(target_os = "android")]
fn pagina_pdf_png(ruta: &Path, i: i32, destino: &Path) -> bool {
    crate::mobile::pdf_pagina_png(ruta, destino, i, DPI_PDF)
}

// ---------------------------------------------------------------- caché de lecturas

/// Clave por CONTENIDO (tamaño + huella de los primeros y últimos 256 KB): no depende de la ruta, que en iOS
/// cambia con cada actualización (el contenedor se muda de UUID).
fn clave(ruta: &Path) -> Option<String> {
    use std::hash::{Hash, Hasher};
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(ruta).ok()?;
    let tam = f.metadata().ok()?.len();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    VERSION.hash(&mut h);
    tam.hash(&mut h);
    let mut b = vec![0u8; 256 * 1024];
    let n = f.read(&mut b).ok()?;
    b[..n].hash(&mut h);
    if tam > 512 * 1024 {
        f.seek(SeekFrom::End(-(256 * 1024))).ok()?;
        let n = f.read(&mut b).ok()?;
        b[..n].hash(&mut h);
    }
    Some(format!("{:016x}", h.finish()))
}

fn dir_cache() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("yappy-amanuensis")
}

fn cache_leer(ruta: &Path) -> Option<String> {
    let k = clave(ruta)?;
    let t = std::fs::read_to_string(dir_cache().join(format!("{k}.txt"))).ok()?;
    (!t.trim().is_empty()).then(|| {
        tracing::info!("amanuensis: lectura en caché para {}", ruta.display());
        t
    })
}

fn cache_guardar(ruta: &Path, texto: &str) {
    let Some(k) = clave(ruta) else { return };
    let d = dir_cache();
    if std::fs::create_dir_all(&d).is_ok() {
        let _ = std::fs::write(d.join(format!("{k}.txt")), texto);
    }
}

// ---------------------------------------------------------------- puente nativo de iOS

#[cfg(target_os = "ios")]
mod nativo {
    //! `gen/apple/Sources/yappy-app/Amanuensis.swift`: PDFKit, ImageIO y Vision.
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;
    use std::path::Path;

    extern "C" {
        fn yappy_pdf_paginas(ruta: *const c_char) -> i32;
        fn yappy_pdf_pagina_png(
            ruta: *const c_char,
            indice: i32,
            dpi: i32,
            destino: *const c_char,
        ) -> bool;
        fn yappy_imagen_a_png(origen: *const c_char, destino: *const c_char, lado_max: i32)
            -> bool;
        fn yappy_vision_lineas(ruta: *const c_char) -> *mut c_char;
        fn yappy_free_string(ptr: *mut c_char);
    }

    fn c(p: &Path) -> CString {
        CString::new(p.to_string_lossy().as_bytes()).unwrap_or_default()
    }

    pub fn pdf_paginas(ruta: &Path) -> i32 {
        let r = c(ruta);
        unsafe { yappy_pdf_paginas(r.as_ptr()) }
    }

    pub fn pdf_pagina_png(ruta: &Path, indice: i32, dpi: i32, destino: &Path) -> bool {
        let (r, d) = (c(ruta), c(destino));
        unsafe { yappy_pdf_pagina_png(r.as_ptr(), indice, dpi, d.as_ptr()) && destino.exists() }
    }

    pub fn imagen_a_png(origen: &Path, destino: &Path, lado_max: i32) -> bool {
        let (o, d) = (c(origen), c(destino));
        unsafe { yappy_imagen_a_png(o.as_ptr(), d.as_ptr(), lado_max) && destino.exists() }
    }

    pub fn vision_lineas(ruta: &Path) -> Option<String> {
        let r = c(ruta);
        let raw = unsafe { yappy_vision_lineas(r.as_ptr()) };
        if raw.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        unsafe { yappy_free_string(raw) };
        Some(s)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Ligero empotrado sobre una página del banco de Amanuensis (si está en este Mac; si no, se omite).
    #[test]
    fn ligero_empotrado_lee_una_pagina_del_banco() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let p = PathBuf::from(home).join("Developer/eg2-ocr/datos/banco/paginas_limpio/000.png");
        if !p.exists() {
            eprintln!("(sin el banco de Amanuensis: prueba omitida)");
            return;
        }
        let r = leer_ruta_solo_ligero(&p).unwrap();
        assert!(r.parrafos.len() >= 3, "{:?}", r.parrafos);
        assert!(r.parrafos[0].starts_with("Teoría general de Saussure"));
        assert!(r.texto().contains("lingüísticos"));
    }

    #[test]
    fn el_ajuste_paddle_guardado_pasa_a_auto() {
        let e: crate::settings::OcrEngine = serde_json::from_str("\"paddle\"").unwrap();
        assert_eq!(e, crate::settings::OcrEngine::Auto);
        assert_eq!(serde_json::to_string(&e).unwrap(), "\"auto\"");
    }

    #[test]
    fn extensiones_de_imagen() {
        assert!(es_imagen("JPG"));
        assert!(es_imagen("heic"));
        assert!(!es_imagen("pdf"));
    }
}
