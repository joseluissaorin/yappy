//! Página derecha y enderezada (`amanuensis/orientacion.py`, la orientación «clásica»): inclinación de ±5° por la
//! nitidez del perfil horizontal de tinta en una miniatura de 600 px, y giro de 90, 180 o 270° decidido por el propio
//! lector (se leen los renglones más largos en cada sentido y gana la lectura más verosímil).

use super::numpy::suma_pares_f32;
use super::pil::{Filtro, FiltroGiro, Gris};

/// `np.arange(ini, fin, paso)` con el relleno de numpy (`ini + i * ((ini + paso) - ini)`).
fn arange(ini: f64, fin: f64, paso: f64) -> Vec<f64> {
    let n = ((fin - ini) / paso).ceil().max(0.0) as usize;
    let delta = (ini + paso) - ini;
    (0..n)
        .map(|i| if i == 0 { ini } else { ini + i as f64 * delta })
        .collect()
}

fn nitidez(tinta: &Gris, a: f64) -> f32 {
    let r = tinta.rotar(a, FiltroGiro::Bilineal, None);
    let p: Vec<f32> = (0..r.alto)
        .map(|y| {
            let fila: Vec<f32> = r.px[y * r.ancho..(y + 1) * r.ancho]
                .iter()
                .map(|&v| v as f32)
                .collect();
            suma_pares_f32(&fila)
        })
        .collect();
    let d: Vec<f32> = p
        .windows(2)
        .map(|w| (w[1] - w[0]) * (w[1] - w[0]))
        .collect();
    suma_pares_f32(&d)
}

/// El primero de los máximos, como `max(..., key=nitidez)`.
fn mejor_de(tinta: &Gris, angulos: &[f64]) -> f64 {
    let mut mejor = angulos[0];
    let mut v = nitidez(tinta, mejor);
    for &a in &angulos[1..] {
        let n = nitidez(tinta, a);
        if n > v {
            v = n;
            mejor = a;
        }
    }
    mejor
}

/// `orientacion.inclinacion(img, lado=600, maximo=5)`.
pub fn inclinacion(g: &Gris) -> f64 {
    const LADO: f64 = 600.0;
    const MAXIMO: f64 = 5.0;
    let s = LADO / g.ancho.max(g.alto) as f64;
    let m = if s < 1.0 {
        g.redimensionar(
            ((g.ancho as f64 * s) as usize).max(1),
            ((g.alto as f64 * s) as usize).max(1),
            Filtro::Bilineal,
        )
    } else {
        g.clone()
    };
    let tinta = Gris::desde(m.ancho, m.alto, m.px.iter().map(|&v| 255 - v).collect());
    if tinta.media() < 1.0 {
        return 0.0;
    }
    let mejor = mejor_de(&tinta, &arange(-MAXIMO, MAXIMO + 1e-6, 1.0));
    let mejor = mejor_de(&tinta, &arange(mejor - 0.75, mejor + 0.75 + 1e-6, 0.25));
    mejor_de(&tinta, &arange(mejor - 0.2, mejor + 0.2 + 1e-6, 0.1))
}

/// `orientacion.enderezar(img, minimo=0.3)` -> (imagen, grados corregidos redondeados a 2 decimales).
pub fn enderezar(g: &Gris) -> (Gris, f64) {
    let a = inclinacion(g);
    if a.abs() < 0.3 {
        return (g.clone(), 0.0);
    }
    (
        g.rotar(a, FiltroGiro::Bicubico, Some(255)),
        (a * 100.0).round_ties_even() / 100.0,
    )
}

const VACIAS: &str = "de la que el en y a los se del las un por con no una su para es al lo como más o pero sus le ha me si sin sobre
este ya entre cuando todo esta ser son dos también fue había era muy años hasta desde está mi porque qué sólo han yo
the of and to in is that for it as was with be by on not he this are or his from at which but have an they
et in est non ad cum quod qui ut sed per de le les des du et la un une est pour dans que qui il ne pas
der die und in den von zu das mit sich des auf für ist im dem nicht ein eine als auch es an werden aus er
e o da do em um para com não uma os no se na por mais as dos como mas foi ao ele das à seu sua ou ser";

/// `orientacion.verosimil(textos, confs)`: confianza media + 2 x proporción de palabras vacías frecuentes +
/// proporción de letras.
pub fn verosimil(textos: &[&str], confs: &[f64]) -> f64 {
    if confs.is_empty() {
        return 0.0;
    }
    let vacias: std::collections::HashSet<&str> = VACIAS.split_whitespace().collect();
    let quitar: &[char] = &[
        '.', ',', ';', ':', '¡', '!', '¿', '?', '(', ')', '«', '»', '"', '\'',
    ];
    let mut n = 0usize;
    let mut vac = 0usize;
    for t in textos {
        for w in t.split_whitespace() {
            n += 1;
            let p = w.trim_matches(quitar).to_lowercase();
            if vacias.contains(p.as_str()) {
                vac += 1;
            }
        }
    }
    let letras: usize = textos
        .iter()
        .map(|t| t.chars().filter(|c| c.is_alphabetic()).count())
        .sum();
    let total: usize = textos
        .iter()
        .map(|t| t.chars().filter(|&c| c != ' ').count())
        .sum();
    confs.iter().sum::<f64>() / confs.len() as f64
        + 2.0 * vac as f64 / n.max(1) as f64
        + letras as f64 / total.max(1) as f64
}

pub fn tumbada(cajas: &[[f64; 4]]) -> bool {
    if cajas.len() < 8 {
        return true;
    }
    let r: Vec<f64> = cajas
        .iter()
        .map(|c| (c[2] - c[0]) / (c[3] - c[1]).max(1.0))
        .collect();
    super::numpy::mediana_f64(&r) < 1.0
}

pub fn girar_cajas_180(cs: &[[f64; 4]], w: f64, h: f64) -> Vec<[f64; 4]> {
    cs.iter()
        .map(|c| [w - c[2], h - c[3], w - c[0], h - c[1]])
        .collect()
}

/// Los `n` renglones más largos (orden estable, como `sorted(..., key=-(ancho))[:n]`).
pub fn mas_largas(cs: &[[f64; 4]], n: usize) -> Vec<[f64; 4]> {
    let mut v = cs.to_vec();
    v.sort_by(|a, b| {
        (b[2] - b[0])
            .partial_cmp(&(a[2] - a[0]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    v.truncate(n);
    v
}
