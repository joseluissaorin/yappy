//! La doble página: un libro escaneado abierto (dos hojas lado a lado en una imagen apaisada) se parte por el
//! canal antes del OCR. Leída entera, el orden de lectura mezcla los renglones de las dos hojas; partida, cada
//! hoja se lee como lo que es.
//!
//! La medida es el número de cambios entre tinta y papel por columna: alto donde hay texto, casi nulo en el canal
//! (sea blanco o la sombra oscura del lomo, que tampoco alterna). Calibrado con un libro escaneado de 18 doble
//! páginas: el canal tiene entre el 1 % y el 6 % de los cambios de las hojas; una página de texto corrido que
//! cruza el centro no baja nunca de ahí.

use super::pil::Gris;

/// Ancho / alto a partir del cual una imagen puede ser una doble página.
const APAISADO: f64 = 1.2;
/// Franja central donde se busca el canal (fracción del ancho).
const CANAL: (f64, f64) = (0.38, 0.62);
/// El canal tiene que bajar de esta fracción de los cambios de la hoja más pobre.
const VALLE: f64 = 0.15;
/// Cada hoja necesita al menos estos cambios por columna, en fracción del alto (si no, es una hoja en blanco y no
/// hay nada que separar).
const TINTA_MIN: f64 = 0.01;
/// Columnas muestreadas como mucho.
const MUESTRA: usize = 1200;

/// Si `g` es una doble página, la columna por la que partirla.
pub fn canal(g: &Gris) -> Option<usize> {
    if g.alto == 0 || (g.ancho as f64) < g.alto as f64 * APAISADO {
        return None;
    }
    let paso = g.ancho.div_ceil(MUESTRA).max(1);
    let umbral = otsu(g, paso);
    let cols: Vec<usize> = (0..g.ancho).step_by(paso).collect();
    let filas = g.alto.div_ceil(paso);
    let cambios: Vec<f64> = cols
        .iter()
        .map(|&x| {
            let mut n = 0u32;
            let mut antes = g.at(x, 0) <= umbral;
            for y in (paso..g.alto).step_by(paso) {
                let ahora = g.at(x, y) <= umbral;
                n += (ahora != antes) as u32;
                antes = ahora;
            }
            n as f64
        })
        .collect();
    let n = cambios.len();
    let ventana = ((n as f64 * 0.02) as usize).max(3);
    let suave = media_movil(&cambios, ventana);

    let franja = |a: f64, b: f64| ((n as f64 * a) as usize, (n as f64 * b) as usize);
    let (lo, hi) = franja(CANAL.0, CANAL.1);
    let (i_canal, valle) = suave[lo..hi]
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, v)| (lo + i, *v))?;
    let (a, b) = franja(0.10, 0.40);
    let izq = mediana(&suave[a..b]);
    let (a, b) = franja(0.60, 0.90);
    let der = mediana(&suave[a..b]);
    let hoja = izq.min(der);
    if hoja < filas as f64 * TINTA_MIN || valle > hoja * VALLE {
        return None;
    }
    Some(cols[i_canal])
}

/// Parte una doble página en (izquierda, derecha); `None` si no lo es.
pub fn partir(g: &Gris) -> Option<(Gris, Gris)> {
    let x = canal(g)? as i64;
    let (w, h) = (g.ancho as i64, g.alto as i64);
    Some((g.recortar(0, 0, x, h), g.recortar(x, 0, w, h)))
}

/// Umbral de Otsu sobre una muestra de la imagen.
fn otsu(g: &Gris, paso: usize) -> u8 {
    let mut hist = [0u64; 256];
    for y in (0..g.alto).step_by(paso) {
        for x in (0..g.ancho).step_by(paso) {
            hist[g.at(x, y) as usize] += 1;
        }
    }
    let total: u64 = hist.iter().sum();
    let suma: f64 = hist
        .iter()
        .enumerate()
        .map(|(i, &c)| i as f64 * c as f64)
        .sum();
    let (mut peso_b, mut suma_b, mut mejor, mut umbral) = (0u64, 0f64, 0f64, 128u8);
    for (t, &c) in hist.iter().enumerate() {
        peso_b += c;
        if peso_b == 0 || peso_b == total {
            continue;
        }
        suma_b += t as f64 * c as f64;
        let mb = suma_b / peso_b as f64;
        let mf = (suma - suma_b) / (total - peso_b) as f64;
        let v = peso_b as f64 * (total - peso_b) as f64 * (mb - mf).powi(2);
        if v > mejor {
            mejor = v;
            umbral = t as u8;
        }
    }
    umbral
}

fn media_movil(v: &[f64], k: usize) -> Vec<f64> {
    let m = k / 2;
    (0..v.len())
        .map(|i| {
            let a = i.saturating_sub(m);
            let b = (i + m + 1).min(v.len());
            v[a..b].iter().sum::<f64>() / (b - a) as f64
        })
        .collect()
}

fn mediana(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una «hoja» de renglones: franjas negras de 3 px cada 8 px, con huecos entre palabras.
    fn renglones(g: &mut Gris, x0: usize, x1: usize) {
        for y in (40..g.alto - 40).step_by(8) {
            for dy in 0..3 {
                for x in x0..x1 {
                    if (x / 23) % 5 != 4 {
                        g.px[(y + dy) * g.ancho + x] = 0;
                    }
                }
            }
        }
    }

    #[test]
    fn la_doble_pagina_se_parte_por_el_canal() {
        let mut g = Gris::nueva(1600, 1000, 255);
        renglones(&mut g, 80, 740);
        renglones(&mut g, 860, 1520);
        let x = canal(&g).expect("es una doble página");
        assert!((740..=860).contains(&x), "canal en {x}");
        let (i, d) = partir(&g).unwrap();
        assert_eq!(i.ancho + d.ancho, 1600);
    }

    #[test]
    fn la_sombra_del_lomo_tambien_es_canal() {
        let mut g = Gris::nueva(1600, 1000, 255);
        renglones(&mut g, 80, 740);
        renglones(&mut g, 860, 1520);
        for y in 0..1000 {
            for x in 770..830 {
                g.px[y * 1600 + x] = 30;
            }
        }
        assert!(canal(&g).is_some());
    }

    #[test]
    fn el_texto_que_cruza_el_centro_no_se_parte() {
        let mut g = Gris::nueva(1600, 1000, 255);
        renglones(&mut g, 80, 1520);
        assert_eq!(canal(&g), None);
    }

    #[test]
    fn una_pagina_vertical_no_se_parte() {
        let mut g = Gris::nueva(1000, 1400, 255);
        renglones(&mut g, 60, 460);
        renglones(&mut g, 540, 940);
        assert_eq!(canal(&g), None);
    }

    #[test]
    fn con_una_hoja_en_blanco_no_hay_nada_que_separar() {
        let mut g = Gris::nueva(1600, 1000, 255);
        renglones(&mut g, 860, 1520);
        assert_eq!(canal(&g), None);
    }
}
