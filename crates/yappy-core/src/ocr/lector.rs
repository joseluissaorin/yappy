//! Lector de Amanuensis Ligero (`amanuensis/lector.py`, `LectorMini`, e `imagen.py`): cada renglón se recorta
//! (0,06 del alto arriba y abajo, 0,2 a los lados), se normaliza (fondo claro, contraste entre los percentiles 1 y
//! 99,5), se lleva a 40 px de alto (LANCZOS, como mucho 160 columnas de 16 px), se estrecha al 0,8 (BILINEAR) y entra
//! como mapa de tinta; la salida CTC (617 clases: blanco + 616 caracteres del `config.json`) se lee en voraz.

use anyhow::{anyhow, Result};
use ndarray::{Array1, Array4};
use ort::{session::Session, value::Value};

use super::numpy::{mediana_f32, percentil_ordenado_f32};
use super::pil::{Filtro, Gris};

pub const MARGEN_V: f64 = 0.25;
pub const MARGEN_H: f64 = 0.6;

#[derive(Clone, Debug)]
pub struct LineaLeida {
    pub texto: String,
    pub conf: f64,
    /// probabilidad de cada carácter (pico de la CTC)
    pub confs: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct ConfigLector {
    pub chars: Vec<String>,
    pub alto: usize,
    pub escala_h: f64,
    pub recorte_v: f64,
    pub recorte_h: f64,
    pub max_cols: usize,
}

impl ConfigLector {
    pub fn desde_json(s: &str) -> Result<Self> {
        let v: serde_json::Value = serde_json::from_str(s)?;
        let chars = v["chars"]
            .as_array()
            .ok_or_else(|| anyhow!("config del lector sin «chars»"))?
            .iter()
            .map(|c| c.as_str().unwrap_or("").to_string())
            .collect();
        Ok(Self {
            chars,
            alto: v["alto"].as_u64().unwrap_or(40) as usize,
            escala_h: v["escala_h"].as_f64().unwrap_or(0.8),
            recorte_v: v["recorte_v"].as_f64().unwrap_or(0.06),
            recorte_h: v["recorte_h"].as_f64().unwrap_or(0.2),
            max_cols: v["max_cols"].as_u64().unwrap_or(160) as usize,
        })
    }
}

/// `round()` de Python (mitades al par).
pub fn redondear_par(x: f64) -> f64 {
    x.round_ties_even()
}

/// `imagen.recorte_de_caja`: la caja con márgenes de 0,6 alturas a los lados y 0,25 arriba y abajo (int() trunca).
pub fn recorte_de_caja(pagina: &Gris, caja: &[f64; 4]) -> Gris {
    let [x0, y0, x1, y1] = *caja;
    let h = (y1 - y0).max(1.0);
    pagina.recortar(
        (x0 - MARGEN_H * h) as i64,
        (y0 - MARGEN_V * h) as i64,
        (x1 + MARGEN_H * h) as i64,
        (y1 + MARGEN_V * h) as i64,
    )
}

/// `imagen.recortar_fijo(img, v, hz)` (sin truncar).
fn recortar_fijo(g: &Gris, v: f64, hz: f64) -> Gris {
    if v == 0.0 && hz == 0.0 {
        return g.clone();
    }
    let (w, h) = (g.ancho as i64, g.alto as i64);
    let dy = redondear_par(h as f64 * v) as i64;
    let mut dx = redondear_par(h as f64 * hz) as i64;
    if w - 2 * dx < 8.max(w / 2) {
        dx = 0;
    }
    g.recortar(dx, dy, w - dx, h - dy)
}

/// `imagen.normalizar_linea`.
fn normalizar_linea(g: &Gris) -> Gris {
    if g.px.is_empty() {
        return g.clone();
    }
    let (w, h) = (g.ancho, g.alto);
    let mut a: Vec<f32> = g.px.iter().map(|&v| v as f32).collect();
    let mut borde = Vec::new();
    // a[:2].ravel(), a[-2:].ravel(), a[:, :2].ravel(), a[:, -2:].ravel()
    for y in 0..h.min(2) {
        borde.extend_from_slice(&a[y * w..(y + 1) * w]);
    }
    for y in h.saturating_sub(2)..h {
        borde.extend_from_slice(&a[y * w..(y + 1) * w]);
    }
    for y in 0..h {
        for x in 0..w.min(2) {
            borde.push(a[y * w + x]);
        }
    }
    for y in 0..h {
        for x in w.saturating_sub(2)..w {
            borde.push(a[y * w + x]);
        }
    }
    let fondo = mediana_f32(&borde);
    if fondo < mediana_f32(&a) - 5.0 || fondo < 100.0 {
        for v in a.iter_mut() {
            *v = 255.0 - *v;
        }
    }
    let mut s = a.clone();
    s.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
    let lo = percentil_ordenado_f32(&s, 1.0);
    let hi = percentil_ordenado_f32(&s, 99.5);
    if hi - lo > 8.0 {
        let f = 255.0f32 / (hi - lo);
        for v in a.iter_mut() {
            *v = (*v - lo) * f;
        }
    }
    Gris::desde(w, h, a.iter().map(|&v| v.clamp(0.0, 255.0) as u8).collect())
}

/// `imagen.preparar2(img, alto, max_cols)`.
fn preparar2(g: &Gris, alto: usize, max_cols: usize) -> Gris {
    let (w, h) = (g.ancho as f64, g.alto as f64);
    let nw = (redondear_par(w * alto as f64 / h.max(1.0)) as usize)
        .max(16)
        .min(max_cols * 16);
    let r = g.redimensionar(nw, alto, Filtro::Lanczos);
    let cols = nw.div_ceil(16);
    let fondo = if r.media() > 128.0 { 255 } else { 0 };
    let mut lienzo = Gris::nueva(cols * 16, alto, fondo);
    lienzo.pegar(&r);
    lienzo
}

/// `lector.voraz(logits, chars, logits=True)`: argmax, colapso de repeticiones y blancos (clase 0); confianza de cada
/// carácter = softmax en su pico.
fn voraz(lg: &[f32], t: usize, c: usize, chars: &[String]) -> LineaLeida {
    let mut texto = String::new();
    let mut confs: Vec<f32> = Vec::new();
    let mut prev = 0usize;
    for f in 0..t {
        let fila = &lg[f * c..(f + 1) * c];
        let mut id = 0;
        let mut mx = fila[0];
        for (k, &v) in fila.iter().enumerate().skip(1) {
            if v > mx {
                mx = v;
                id = k;
            }
        }
        let ez: Vec<f32> = fila.iter().map(|&v| (v - mx).exp()).collect();
        let p = 1.0 / super::numpy::suma_pares_f32(&ez);
        if id != prev && id != 0 {
            if let Some(ch) = chars.get(id - 1) {
                texto.push_str(ch);
            }
            confs.push(p);
        } else if id == prev && id != 0 {
            if let Some(u) = confs.last_mut() {
                *u = u.max(p);
            }
        }
        prev = id;
    }
    let conf = if confs.is_empty() {
        0.0
    } else {
        confs.iter().map(|&v| v as f64).sum::<f64>() / confs.len() as f64
    };
    LineaLeida { texto, conf, confs }
}

pub struct LectorMini {
    pub cfg: ConfigLector,
    sesion: Session,
    lote: usize,
}

impl LectorMini {
    pub fn nuevo(sesion: Session, cfg: ConfigLector, lote: usize) -> Self {
        Self {
            cfg,
            sesion,
            lote: lote.max(1),
        }
    }

    /// `LectorMini.leer(recortes)` con lotes de hasta 64 renglones de ancho parecido y tope de área 64 x 1600.
    pub fn leer(&mut self, recortes: &[Gris]) -> Result<Vec<LineaLeida>> {
        let lote = self.lote;
        let max_area = lote * 1600;
        let a = self.cfg.alto;
        let prep: Vec<(Vec<f32>, usize)> = recortes
            .iter()
            .map(|g| {
                let g2 =
                    normalizar_linea(&recortar_fijo(g, self.cfg.recorte_v, self.cfg.recorte_h));
                let p = preparar2(&g2, a, self.cfg.max_cols);
                let nw = (redondear_par(p.ancho as f64 * self.cfg.escala_h) as usize).max(8);
                let p = p.redimensionar(nw, a, Filtro::Bilineal);
                (p.px.iter().map(|&v| 1.0 - v as f32 / 255.0).collect(), nw)
            })
            .collect();
        let mut orden: Vec<usize> = (0..prep.len()).collect();
        orden.sort_by_key(|&i| prep[i].1);
        let mut grupos: Vec<Vec<usize>> = Vec::new();
        let mut act: Vec<usize> = Vec::new();
        for &i in &orden {
            let w = prep[i].1;
            if !act.is_empty()
                && (act.len() >= lote
                    || (act.len() + 1) * w > max_area
                    || w as f64 > 1.3 * (prep[act[0]].1.max(64)) as f64)
            {
                grupos.push(std::mem::take(&mut act));
            }
            act.push(i);
        }
        if !act.is_empty() {
            grupos.push(act);
        }
        let mut out: Vec<Option<LineaLeida>> = vec![None; prep.len()];
        for idx in grupos {
            let wmax = idx.iter().map(|&i| prep[i].1).max().unwrap_or(32);
            let wl = wmax.div_ceil(32) * 32;
            let mut x = Array4::<f32>::zeros((idx.len(), 1, a, wl));
            let mut an = Array1::<i64>::zeros(idx.len());
            for (j, &i) in idx.iter().enumerate() {
                let (ref d, w) = prep[i];
                for y in 0..a {
                    for xx in 0..w {
                        x[[j, 0, y, xx]] = d[y * w + xx];
                    }
                }
                an[j] = w as i64;
            }
            let salidas = self.sesion.run(ort::inputs! {
                "x" => Value::from_array(x)?,
                "anchos" => Value::from_array(an)?,
            })?;
            let (forma, logits) = salidas[0].try_extract_tensor::<f32>()?;
            let (_, masc) = salidas[1].try_extract_tensor::<f32>()?;
            let (tt, cc) = (forma[1] as usize, forma[2] as usize);
            for (j, &i) in idx.iter().enumerate() {
                let fila = &masc[j * tt..(j + 1) * tt];
                let t = super::numpy::suma_pares_f32(fila) as usize;
                let lg = &logits[j * tt * cc..(j * tt + t) * cc];
                out[i] = Some(voraz(lg, t, cc, &self.cfg.chars));
            }
        }
        Ok(out
            .into_iter()
            .map(|o| {
                o.unwrap_or(LineaLeida {
                    texto: String::new(),
                    conf: 0.0,
                    confs: vec![],
                })
            })
            .collect())
    }
}
