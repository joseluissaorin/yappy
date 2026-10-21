//! Amanuensis Ligero en Rust: el OCR propio de José Luis Saorín (https://github.com/joseluissaorin/amanuensis,
//! código Apache 2.0) portado sin Python para leer fotos, capturas y PDF escaneados en el dispositivo.
//!
//! Tubería (la de `amanu -m ligero` con el detector Mínimo y la orientación clásica, `AMANUENSIS_MAQUETA_MINIMA=0
//! AMANUENSIS_ORIENTACION=clasica`):
//!   1. página en gris (Pillow `L`), enderezada ±5° (`orientacion::enderezar`);
//!   2. detector Mínimo (`det.onnx`, 0,13 M) -> cajas de renglón en orden de lectura (orden ligero);
//!   3. giro de 90/180/270° si la lectura de prueba lo pide (`orientar`);
//!   4. lector Ligero 3 (`mini.onnx`, 3,5 M, CTC de 616 caracteres) sobre cada renglón;
//!   5. párrafos (`parrafos`), que es la unidad con la que Yappy lee y guarda el progreso.
//!
//! Todo en CPU: las sesiones NO heredan los proveedores globales de ONNX Runtime (Core ML/DirectML), porque con
//! formas variables por lote recompilarían el grafo a cada renglón; no se registra ningún proveedor por sesión.
//!
//! Pesos: Ligero y su maquetación son de USO NO COMERCIAL (parte de los datos de entrenamiento es CC BY-NC 4.0);
//! el detector Mínimo y el orden ligero son Apache 2.0. Ver MODEL_CARD.md de Amanuensis.

pub mod detector;
pub mod lector;
pub mod numpy;
pub mod orden;
pub mod orientacion;
pub mod parrafos;
pub mod pil;

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ndarray::Array4;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Value;

pub use detector::ConfigDetector;
pub use lector::{ConfigLector, LectorMini, LineaLeida};
pub use pil::Gris;

/// Umbrales de la sonda de impreso moderno latino de Amanuensis (`motor.py`, calibrados en su examen DEV).
pub const SONDA_CONF: f64 = 0.90;
pub const SONDA_P10: f64 = 0.95;
pub const SONDA_FLOJAS: f64 = 0.08;
pub const UMBRAL_FLOJA: f64 = 0.75;
pub const SONDA_MIN_LINEAS: usize = 3;

/// Un renglón leído: caja en px de la página derecha (redondeada a 0,1 como en Python), texto y confianza.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Linea {
    pub caja: [f64; 4],
    pub texto: String,
    pub conf: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PaginaLeida {
    pub ancho: usize,
    pub alto: usize,
    /// giro aplicado (grados antihorarios: 0, 90, 180, 270)
    pub giro: i32,
    /// inclinación corregida (grados)
    pub inclinacion: f64,
    pub lineas: Vec<Linea>,
}

/// Resumen de la confianza de una página: lo que la sonda de Amanuensis mira para decidir si es impreso moderno
/// latino (Ligero lee bien) o si conviene otro lector.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Calidad {
    pub lineas: usize,
    pub media: f64,
    pub p10: f64,
    pub flojas: f64,
}

impl Calidad {
    /// La sonda de `motor.py` (sin el filtro de tablas): ¿impreso moderno en alfabeto latino leído con seguridad?
    pub fn impreso_latino(&self) -> bool {
        self.lineas >= SONDA_MIN_LINEAS
            && self.media >= SONDA_CONF
            && self.p10 >= SONDA_P10
            && self.flojas <= SONDA_FLOJAS
    }

    /// Criterio más laxo para quedarse con la lectura cuando no hay otro lector mejor a mano.
    pub fn aceptable(&self) -> bool {
        self.lineas >= 1 && self.media >= 0.80 && self.flojas <= 0.25
    }
}

impl PaginaLeida {
    fn corridas(&self) -> Vec<&Linea> {
        self.lineas
            .iter()
            .filter(|l| !l.texto.trim().is_empty())
            .collect()
    }

    /// Párrafos de texto corrido, en orden de lectura (`salidas.parrafos`).
    pub fn parrafos(&self) -> Vec<String> {
        let ls = self.corridas();
        parrafos::partir(&ls)
            .into_iter()
            .map(|g| parrafos::unir(g.iter().map(|&i| ls[i].texto.as_str())))
            .filter(|t| !t.is_empty())
            .collect()
    }

    pub fn calidad(&self) -> Calidad {
        let ls = self.corridas();
        if ls.is_empty() {
            return Calidad {
                lineas: 0,
                media: 0.0,
                p10: 0.0,
                flojas: 1.0,
            };
        }
        let mut c: Vec<f32> = ls.iter().map(|l| l.conf as f32).collect();
        let media = c.iter().map(|&v| v as f64).sum::<f64>() / c.len() as f64;
        let flojas =
            c.iter().filter(|&&v| (v as f64) < UMBRAL_FLOJA).count() as f64 / c.len() as f64;
        c.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p10 = numpy::percentil_ordenado_f32(&c, 10.0) as f64;
        Calidad {
            lineas: ls.len(),
            media,
            p10,
            flojas,
        }
    }
}

/// Opciones de la tubería.
#[derive(Clone, Debug)]
pub struct Opciones {
    /// enderezar la inclinación y probar giros (por omisión sí, como `amanu -m ligero`)
    pub enderezar: bool,
    /// hilos de ONNX Runtime por sesión (0 = los que decida ORT)
    pub hilos: usize,
    /// renglones por lote del lector (64 en Amanuensis; menos = menos memoria pico, misma lectura)
    pub lote: usize,
    /// arena de memoria de CPU de ONNX Runtime (más rápida; retiene memoria entre lotes)
    pub arena: bool,
    /// «memory pattern» de ONNX Runtime: guarda el plan de búferes por forma de entrada. Con páginas de alto
    /// distinto el detector acumulaba cientos de MB (de 299 a 415 MB en seis vueltas); apagado, el pico se queda.
    pub patron: bool,
}

impl Opciones {
    /// Lo que usa la app en el dispositivo: misma lectura, menos memoria (medido el 21-10-2026 en el Mac: pico
    /// de unos 380 MB por página frente a 500 MB y creciendo).
    pub fn ahorro() -> Self {
        Self {
            lote: 16,
            arena: false,
            patron: false,
            ..Self::default()
        }
    }
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            enderezar: true,
            hilos: 0,
            lote: 64,
            arena: true,
            patron: true,
        }
    }
}

/// Amanuensis Ligero: detector Mínimo + lector Ligero 3.
pub struct Ligero {
    det: Session,
    cfg_det: ConfigDetector,
    lector: LectorMini,
    pub opciones: Opciones,
}

fn sesion(bytes: &[u8], hilos: usize, arena: bool, patron: bool) -> Result<Session> {
    let mut b = Session::builder()?
        // CPU a propósito: ver la cabecera del módulo (y la memoria ort-ep-registration: nunca se REGISTRA un
        // proveedor por sesión; esto solo evita heredar los globales).
        .with_no_environment_execution_providers()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_inter_threads(1)?;
    if hilos > 0 {
        b = b.with_intra_threads(hilos)?;
    }
    // Arena de CPU opcional (`enable_cpu_mem_arena`); ort rc.10 no lo expone en el builder: API C.
    if !arena {
        use ort::AsPointer;
        let st = unsafe { (ort::api().DisableCpuMemArena)(b.ptr_mut()) };
        if !st.0.is_null() {
            unsafe { (ort::api().ReleaseStatus)(st.0) };
            tracing::warn!("ocr: no se pudo desactivar la arena de CPU de ONNX Runtime");
        }
    }
    if !patron {
        b = b.with_memory_pattern(false)?;
    }
    Ok(b.commit_from_memory(bytes)?)
}

impl Ligero {
    /// Desde los ficheros en memoria (empaquetados en la app).
    pub fn desde_bytes(
        det_onnx: &[u8],
        det_cfg: &str,
        lector_onnx: &[u8],
        lector_cfg: &str,
        opciones: Opciones,
    ) -> Result<Self> {
        let det = sesion(det_onnx, opciones.hilos, opciones.arena, opciones.patron)
            .context("cargar el detector Mínimo")?;
        let lec = sesion(lector_onnx, opciones.hilos, opciones.arena, opciones.patron)
            .context("cargar el lector Ligero")?;
        Ok(Self {
            det,
            cfg_det: ConfigDetector::desde_json(det_cfg),
            lector: LectorMini::nuevo(lec, ConfigLector::desde_json(lector_cfg)?, opciones.lote),
            opciones,
        })
    }

    /// Desde las carpetas del catálogo de Amanuensis (`~/.cache/amanuensis/amanuensis-det-minimo-1` y
    /// `.../amanuensis-ligero-3`).
    pub fn desde_carpetas(det: &Path, lector: &Path, opciones: Opciones) -> Result<Self> {
        let leer = |p: &Path| std::fs::read(p).with_context(|| format!("leer {}", p.display()));
        let det_cfg = std::fs::read_to_string(det.join("config.json")).unwrap_or_default();
        let lec_cfg = std::fs::read_to_string(lector.join("config.json"))
            .context("config.json del lector")?;
        Self::desde_bytes(
            &leer(&det.join("det.onnx"))?,
            &det_cfg,
            &leer(&lector.join("mini.onnx"))?,
            &lec_cfg,
            opciones,
        )
    }

    /// `DetectorMini.detectar(pagina)`: cajas en px de la página, en orden de lectura.
    pub fn detectar(&mut self, g: &Gris) -> Result<Vec<[f64; 4]>> {
        let (x, hh, esc) = detector::lienzo(g, &self.cfg_det);
        let w = self.cfg_det.ancho;
        let arr = Array4::from_shape_vec((1, 1, hh, w), x)?;
        let out = self
            .det
            .run(ort::inputs! { "x" => Value::from_array(arr)? })?;
        let (forma, mapas) = out[0].try_extract_tensor::<f32>()?;
        if forma.len() != 4 {
            return Err(anyhow!("salida del detector inesperada: {forma:?}"));
        }
        let (h4, w4) = (forma[2] as usize, forma[3] as usize);
        let nuc: Vec<f32> = mapas[..h4 * w4]
            .iter()
            .map(|&v| 1.0 / (1.0 + (-v).exp()))
            .collect();
        let (w0, h0) = (g.ancho as f64, g.alto as f64);
        let cs: Vec<[f64; 4]> = detector::cajas_desde_mapa(
            &nuc,
            w4,
            h4,
            self.cfg_det.umbral,
            4.0,
            self.cfg_det.escala_alto,
        )
        .into_iter()
        .map(|b| {
            [
                (b[0] / esc).max(0.0),
                (b[1] / esc).max(0.0),
                (b[2] / esc).min(w0),
                (b[3] / esc).min(h0),
            ]
        })
        .collect();
        let orden = detector::orden_lectura(&cs, (w0, h0));
        Ok(orden.into_iter().map(|i| cs[i]).collect())
    }

    /// Lee recortes de renglón ya hechos.
    pub fn leer_renglones(&mut self, recortes: &[Gris]) -> Result<Vec<LineaLeida>> {
        if recortes.is_empty() {
            return Ok(Vec::new());
        }
        self.lector.leer(recortes)
    }

    fn puntuar(&mut self, im: &Gris, cs: &[[f64; 4]]) -> Result<f64> {
        let largas = orientacion::mas_largas(cs, 8);
        if largas.is_empty() {
            return Ok(0.0);
        }
        let rec: Vec<Gris> = largas
            .iter()
            .map(|c| lector::recorte_de_caja(im, c))
            .collect();
        let ls = self.leer_renglones(&rec)?;
        let textos: Vec<&str> = ls.iter().map(|l| l.texto.as_str()).collect();
        let confs: Vec<f64> = ls.iter().map(|l| l.conf).collect();
        Ok(orientacion::verosimil(&textos, &confs))
    }

    /// `orientacion.orientar(img, cajas, detectar, leer)` -> (imagen derecha, giro, cajas).
    fn orientar(&mut self, img: Gris, cajas: Vec<[f64; 4]>) -> Result<(Gris, i32, Vec<[f64; 4]>)> {
        if !cajas.is_empty() {
            let alturas: Vec<f64> = cajas.iter().map(|c| c[3] - c[1]).collect();
            if numpy::mediana_f64(&alturas) < 14.0 && !orientacion::tumbada(&cajas) {
                return Ok((img, 0, cajas));
            }
        }
        let mut cands: Vec<(i32, Gris, Vec<[f64; 4]>)> = Vec::new();
        let tumbada = orientacion::tumbada(&cajas);
        if tumbada {
            for g in [90, 270] {
                let im = img.rotar_cuartos(g);
                let cs = self.detectar(&im)?;
                cands.push((g, im, cs));
            }
        }
        // (p, giro, índice del candidato, media vuelta)
        let mut mejor: Option<(f64, i32, Option<usize>, bool)> = None;
        let total = 1 + cands.len();
        for k in 0..total {
            for extra in [0, 180] {
                let (g, im, cs): (i32, &Gris, &Vec<[f64; 4]>) = if k == 0 {
                    (0, &img, &cajas)
                } else {
                    (cands[k - 1].0, &cands[k - 1].1, &cands[k - 1].2)
                };
                let p = if extra == 0 {
                    self.puntuar(im, cs)?
                } else {
                    let im2 = im.rotar180();
                    let cs2 = orientacion::girar_cajas_180(cs, im.ancho as f64, im.alto as f64);
                    self.puntuar(&im2, &cs2)? - 0.15
                };
                if mejor.map(|m| p > m.0).unwrap_or(true) {
                    mejor = Some((
                        p,
                        (g + extra) % 360,
                        if k == 0 { None } else { Some(k - 1) },
                        extra != 0,
                    ));
                }
            }
        }
        let (_, giro, cual, media_vuelta) = mejor.expect("al menos un candidato");
        if giro == 0 {
            return Ok((img, 0, cajas));
        }
        let base = match cual {
            None => img,
            Some(k) => cands.swap_remove(k).1,
        };
        let im = if media_vuelta { base.rotar180() } else { base };
        let cs = self.detectar(&im)?;
        Ok((im, giro, cs))
    }

    /// La página entera: enderezar, detectar, orientar y leer.
    pub fn leer_pagina(&mut self, g: &Gris) -> Result<PaginaLeida> {
        let (img, incl) = if self.opciones.enderezar {
            orientacion::enderezar(g)
        } else {
            (g.clone(), 0.0)
        };
        let cajas = self.detectar(&img)?;
        let (img, giro, cajas) = if self.opciones.enderezar {
            self.orientar(img, cajas)?
        } else {
            (img, 0, cajas)
        };
        let rec: Vec<Gris> = cajas
            .iter()
            .map(|c| lector::recorte_de_caja(&img, c))
            .collect();
        let leidas = self.leer_renglones(&rec)?;
        let r1 = |v: f64| format!("{v:.1}").parse::<f64>().unwrap_or(v);
        let lineas = cajas
            .iter()
            .zip(leidas)
            .map(|(c, l)| Linea {
                caja: [r1(c[0]), r1(c[1]), r1(c[2]), r1(c[3])],
                texto: l.texto,
                conf: l.conf,
            })
            .collect();
        Ok(PaginaLeida {
            ancho: img.ancho,
            alto: img.alto,
            giro,
            inclinacion: incl,
            lineas,
        })
    }
}

/// Abre una imagen como la abre Amanuensis (`geometria_pagina.abrir` + `convert("RGB").convert("L")`): con la
/// orientación EXIF aplicada (fotos de móvil) y el gris de Pillow.
pub fn abrir_imagen(ruta: &Path) -> Result<Gris> {
    use image::ImageDecoder;
    let lector = image::ImageReader::open(ruta)?.with_guessed_format()?;
    let mut dec = lector.into_decoder()?;
    let orient = dec
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(dec)?;
    img.apply_orientation(orient);
    Ok(gris_de(&img))
}

/// Imagen ya decodificada -> gris de Pillow (RGB -> L con la fórmula de Pillow; el alfa se descarta).
pub fn gris_de(img: &image::DynamicImage) -> Gris {
    let rgb = img.to_rgb8();
    Gris::desde_rgb(rgb.width() as usize, rgb.height() as usize, rgb.as_raw())
}
