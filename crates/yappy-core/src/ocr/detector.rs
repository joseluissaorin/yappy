//! Detector de líneas Mínimo de Amanuensis (`amanuensis/detector.py`, `DetectorMini`): U-Net diminuta (0,13 M) que
//! da un mapa de núcleo de línea a paso 4 sobre la página a 768 px de ancho; las cajas salen de sus componentes
//! conexas (alto por la mediana de las columnas, centro por el centro de masas) y se ordenan con el orden ligero.

use super::numpy::{mediana_f32, suma_pares_f32};
use super::pil::{Filtro, Gris};

pub const NUCLEO_V: f64 = 0.5;

/// Configuración del detector (`config.json` de `amanuensis-det-minimo-1`).
#[derive(Clone, Debug)]
pub struct ConfigDetector {
    pub ancho: usize,
    pub multiplo: usize,
    pub umbral: f32,
    pub escala_alto: f64,
}

impl Default for ConfigDetector {
    fn default() -> Self {
        Self {
            ancho: 768,
            multiplo: 32,
            umbral: 0.5,
            escala_alto: 0.8,
        }
    }
}

impl ConfigDetector {
    pub fn desde_json(s: &str) -> Self {
        let v: serde_json::Value = serde_json::from_str(s).unwrap_or_default();
        let d = Self::default();
        Self {
            ancho: v["ancho"].as_u64().map(|x| x as usize).unwrap_or(d.ancho),
            multiplo: v["multiplo"]
                .as_u64()
                .map(|x| x as usize)
                .unwrap_or(d.multiplo),
            umbral: v["umbral"].as_f64().map(|x| x as f32).unwrap_or(d.umbral),
            escala_alto: v["escala_alto"].as_f64().unwrap_or(d.escala_alto),
        }
    }
}

/// `_lienzo(pagina, fondo_blanco_1=False)`: tinta en [0, 1] (1 - gris/255) a `ancho` px, alto múltiplo de 32
/// rellenado con 0. -> (datos [H, ancho], H, escala).
pub fn lienzo(g: &Gris, cfg: &ConfigDetector) -> (Vec<f32>, usize, f64) {
    let esc = cfg.ancho as f64 / g.ancho as f64;
    let h = (g.alto as f64 * esc) as usize;
    let hh = ((g.alto as f64 * esc) / cfg.multiplo as f64).ceil() as usize * cfg.multiplo;
    let r = g.redimensionar(cfg.ancho, h.max(1), Filtro::Bilineal);
    let mut x = vec![0f32; hh * cfg.ancho];
    for (i, &v) in r.px.iter().enumerate().take(h * cfg.ancho) {
        x[i] = 1.0 - (v as f32 / 255.0);
    }
    (x, hh, esc)
}

/// Componentes conexas de 8 vecinos (`cv2.connectedComponentsWithStats`): -> (etiquetas, cajas [x0, y0, w, h]
/// por etiqueta 1..n; el índice 0 es el fondo).
fn componentes(masc: &[bool], w: usize, h: usize) -> (Vec<u32>, Vec<[usize; 4]>) {
    let mut lab = vec![0u32; w * h];
    let mut padre: Vec<u32> = vec![0];
    fn raiz(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for y in 0..h {
        for x in 0..w {
            if !masc[y * w + x] {
                continue;
            }
            let mut vecinos = [0u32; 4];
            let mut nv = 0;
            if x > 0 && lab[y * w + x - 1] != 0 {
                vecinos[nv] = lab[y * w + x - 1];
                nv += 1;
            }
            if y > 0 {
                for dx in [-1i64, 0, 1] {
                    let xx = x as i64 + dx;
                    if xx >= 0 && (xx as usize) < w && lab[(y - 1) * w + xx as usize] != 0 {
                        vecinos[nv] = lab[(y - 1) * w + xx as usize];
                        nv += 1;
                    }
                }
            }
            if nv == 0 {
                let n = padre.len() as u32;
                padre.push(n);
                lab[y * w + x] = n;
            } else {
                let mut m = raiz(&mut padre, vecinos[0]);
                for &v in &vecinos[1..nv] {
                    m = m.min(raiz(&mut padre, v));
                }
                for &v in &vecinos[..nv] {
                    let r = raiz(&mut padre, v);
                    padre[r as usize] = m;
                }
                lab[y * w + x] = m;
            }
        }
    }
    // etiquetas definitivas consecutivas en el orden de aparición de las raíces
    let mut final_ = vec![0u32; padre.len()];
    let mut n = 0u32;
    for (i, f) in final_.iter_mut().enumerate().skip(1) {
        let r = raiz(&mut padre, i as u32);
        if r == i as u32 {
            n += 1;
            *f = n;
        }
    }
    for i in 1..padre.len() {
        let r = raiz(&mut padre, i as u32);
        final_[i] = final_[r as usize];
    }
    let mut cajas = vec![[usize::MAX, usize::MAX, 0usize, 0usize]; n as usize + 1];
    for y in 0..h {
        for x in 0..w {
            let l = lab[y * w + x];
            if l == 0 {
                continue;
            }
            let f = final_[l as usize];
            lab[y * w + x] = f;
            let c = &mut cajas[f as usize];
            c[0] = c[0].min(x);
            c[1] = c[1].min(y);
            c[2] = c[2].max(x);
            c[3] = c[3].max(y);
        }
    }
    let cajas = cajas
        .into_iter()
        .map(|c| {
            if c[0] == usize::MAX {
                [0, 0, 0, 0]
            } else {
                [c[0], c[1], c[2] - c[0] + 1, c[3] - c[1] + 1]
            }
        })
        .collect();
    (lab, cajas)
}

/// `cajas_desde_mapa(nucleo, umbral, escala_inv=4, escala_alto)`: cajas en px del lienzo.
pub fn cajas_desde_mapa(
    nuc: &[f32],
    w: usize,
    h: usize,
    umbral: f32,
    escala_inv: f64,
    escala_alto: f64,
) -> Vec<[f64; 4]> {
    let masc: Vec<bool> = nuc.iter().map(|&v| v > umbral).collect();
    let (lab, stats) = componentes(&masc, w, h);
    let mut out = Vec::new();
    for (k, s) in stats.iter().enumerate().skip(1) {
        let [x0, y0, cw, ch] = *s;
        let (x1, y1) = (x0 + cw, y0 + ch);
        if cw * ch < 3 {
            continue;
        }
        let mut hh = ch as f64 / NUCLEO_V;
        let mut cy = (y0 + y1) as f64 / 2.0;
        let r0 = y0.saturating_sub(2);
        let r1 = (y1 + 2).min(h);
        let (nf, nc) = (r1 - r0, x1 - x0);
        let mut pm = vec![0f32; nf * nc];
        for (fi, yy) in (r0..r1).enumerate() {
            for (ci, xx) in (x0..x1).enumerate() {
                let l = lab[yy * w + xx];
                if l == 0 || l == k as u32 {
                    pm[fi * nc + ci] = nuc[yy * w + xx];
                }
            }
        }
        // pm.sum(0): acumulación fila a fila en float32
        let mut alto_cols = vec![0f32; nc];
        for fi in 0..nf {
            for ci in 0..nc {
                alto_cols[ci] += pm[fi * nc + ci];
            }
        }
        let maxc = alto_cols.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        if maxc > 0.0 {
            let sel: Vec<f32> = alto_cols
                .iter()
                .cloned()
                .filter(|&v| v > 0.5 * maxc)
                .collect();
            hh = mediana_f32(&sel) as f64 / NUCLEO_V;
            let filas: Vec<f32> = (0..nf)
                .map(|fi| suma_pares_f32(&pm[fi * nc..(fi + 1) * nc]))
                .collect();
            let pesado: Vec<f64> = filas
                .iter()
                .enumerate()
                .map(|(i, &f)| f as f64 * i as f64)
                .collect();
            let num = suma_pares_f64(&pesado);
            let den = suma_pares_f32(&filas).max(1e-6) as f64;
            cy = r0 as f64 + 0.5 + num / den;
        }
        hh *= escala_alto;
        let ins = 0.15 * hh;
        out.push([
            (x0 as f64 - ins) * escala_inv,
            (cy - hh / 2.0) * escala_inv,
            (x1 as f64 + ins) * escala_inv,
            (cy + hh / 2.0) * escala_inv,
        ]);
    }
    out
}

/// Suma por parejas de numpy en float64 (misma estructura que la de float32).
pub fn suma_pares_f64(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0;
        for &v in a {
            r += v;
        }
        r
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for k in 0..8 {
                r[k] += a[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        suma_pares_f64(&a[..n2]) + suma_pares_f64(&a[n2..])
    }
}

/// `orden_lectura(cajas, "aprendido", tam)`: orden ligero (perceptrón) hasta 400 líneas; si no, por columnas.
pub fn orden_lectura(cajas: &[[f64; 4]], tam: (f64, f64)) -> Vec<usize> {
    if cajas.len() > 1 && cajas.len() <= 400 {
        return super::orden::ordenar(cajas, tam);
    }
    orden_columnas(cajas)
}

/// Rama «columnas» de `orden_lectura` (corte XY con canales entre columnas), para páginas de más de 400 líneas.
fn orden_columnas(c: &[[f64; 4]]) -> Vec<usize> {
    if c.len() <= 1 {
        return (0..c.len()).collect();
    }
    let alturas: Vec<f64> = c.iter().map(|b| b[3] - b[1]).collect();
    let hmed = super::numpy::mediana_f64(&alturas);

    fn cortar(c: &[[f64; 4]], idx: &[usize], eje: usize, hueco: Option<f64>) -> Vec<Vec<usize>> {
        let (a0, a1) = if eje == 0 { (1, 3) } else { (0, 2) };
        let mut iv = idx.to_vec();
        iv.sort_by(|&p, &q| {
            c[p][a0]
                .partial_cmp(&c[q][a0])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut grupos = Vec::new();
        let mut actual = vec![iv[0]];
        let mut fin = c[iv[0]][a1];
        for &i in &iv[1..] {
            let corta = match hueco {
                None => c[i][a0] > fin - 0.1 * (c[i][a1] - c[i][a0]),
                Some(hg) => c[i][a0] > fin + hg,
            };
            if corta {
                grupos.push(std::mem::take(&mut actual));
            }
            actual.push(i);
            fin = fin.max(c[i][a1]);
        }
        grupos.push(actual);
        grupos
    }

    fn por_y(c: &[[f64; 4]], idx: &[usize]) -> Vec<usize> {
        let mut v = idx.to_vec();
        v.sort_by(|&p, &q| {
            (c[p][1], c[p][0])
                .partial_cmp(&(c[q][1], c[q][0]))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    }

    fn buscar_canal(c: &[[f64; 4]], idx: &[usize], hmed: f64) -> Option<(f64, Vec<usize>)> {
        let n = idx.len();
        if n < 6 {
            return None;
        }
        let x0 = idx.iter().map(|&i| c[i][0]).fold(f64::INFINITY, f64::min);
        let x1 = idx
            .iter()
            .map(|&i| c[i][2])
            .fold(f64::NEG_INFINITY, f64::max);
        let m = 0.25 * hmed;
        let paso = (hmed / 6.0).max(2.0);
        let (ini, fin) = (x0 + hmed, x1 - hmed);
        let mut mejor: Option<((usize, f64), f64, Vec<usize>)> = None;
        let pasos = ((fin - ini) / paso).ceil().max(0.0) as usize;
        for k in 0..pasos {
            let xc = ini + k as f64 * paso;
            let cruzan: Vec<usize> = idx
                .iter()
                .copied()
                .filter(|&i| c[i][0] < xc - m && c[i][2] > xc + m)
                .collect();
            let izq = idx.iter().filter(|&&i| c[i][2] <= xc + m).count();
            let der = n as i64 - cruzan.len() as i64 - izq as i64;
            let menor = (izq as i64).min(der);
            if (menor as f64) < 0.2 * n as f64 || cruzan.len() as f64 > (0.08 * n as f64).max(1.0) {
                continue;
            }
            let clave = (cruzan.len(), -(menor as f64));
            if mejor.as_ref().map(|b| clave < b.0).unwrap_or(true) {
                mejor = Some((clave, xc, cruzan));
            }
        }
        mejor.map(|(_, xc, cr)| (xc, cr))
    }

    fn rec(c: &[[f64; 4]], idx: Vec<usize>, p: usize, hmed: f64) -> Vec<usize> {
        if idx.len() <= 1 || p > 40 {
            return por_y(c, &idx);
        }
        let g = cortar(c, &idx, 1, Some(0.5 * hmed));
        if g.len() > 1 {
            return g.into_iter().flat_map(|x| rec(c, x, p + 1, hmed)).collect();
        }
        if let Some((xc, mut cruzan)) = buscar_canal(c, &idx, hmed) {
            let cy = |i: usize| (c[i][1] + c[i][3]) / 2.0;
            cruzan.sort_by(|&a, &b| {
                cy(a)
                    .partial_cmp(&cy(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut resto: Vec<usize> = idx
                .iter()
                .copied()
                .filter(|i| !cruzan.contains(i))
                .collect();
            resto.sort_by(|&a, &b| {
                cy(a)
                    .partial_cmp(&cy(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let volcar = |fr: &[usize]| -> Vec<usize> {
                let izq: Vec<usize> = fr
                    .iter()
                    .copied()
                    .filter(|&i| (c[i][0] + c[i][2]) / 2.0 < xc)
                    .collect();
                let der: Vec<usize> = fr.iter().copied().filter(|i| !izq.contains(i)).collect();
                let mut o = rec(c, izq, p + 1, hmed);
                o.extend(rec(c, der, p + 1, hmed));
                o
            };
            let (mut out, mut franja, mut k) = (Vec::new(), Vec::new(), 0);
            for &i in &resto {
                while k < cruzan.len() && cy(cruzan[k]) < cy(i) {
                    out.extend(volcar(&franja));
                    out.push(cruzan[k]);
                    franja.clear();
                    k += 1;
                }
                franja.push(i);
            }
            out.extend(volcar(&franja));
            out.extend_from_slice(&cruzan[k..]);
            return out;
        }
        let g = cortar(c, &idx, 0, Some(0.7 * hmed));
        if g.len() > 1 {
            return g.into_iter().flat_map(|x| rec(c, x, p + 1, hmed)).collect();
        }
        let filas = cortar(c, &idx, 0, None);
        filas
            .into_iter()
            .flat_map(|mut f| {
                f.sort_by(|&a, &b| {
                    c[a][0]
                        .partial_cmp(&c[b][0])
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                f
            })
            .collect()
    }

    rec(c, (0..c.len()).collect(), 0, hmed)
}
