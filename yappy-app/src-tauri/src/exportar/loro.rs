//! EL LORO, a pluma en el PDF: el mismo dibujo que brand/loro.svg (dos
//! tintas con desregistro: la voz coral delante, el texto ultramar detrás),
//! trazado con las primitivas de krilla para que viaje como vector.

use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder, Transform};
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, FillRule};
use krilla::surface::Surface;

const CUERPO: &str = "M32 7 C42.5 6.5 50 14 50.5 24 C51 33 49.5 41.5 43 46.4 C39 49.4 25 49.4 21 46.4 C14.5 41.5 13 33 13.5 24 C14 14 21.5 6.5 32 7 Z";
const COPETE: &str = "M29.2 7.4 C26.8 1.8 32 -0.6 37.8 1.6 C34.6 3.1 33.7 5 34 7 Z";
const PICO: &str = "M28 29 Q32 27.8 36 29 Q36.2 35.4 32.6 38.9 Q32 39.4 31.5 38.9 Q28 35.4 28 29 Z";

pub const CORAL: (u8, u8, u8) = (0xe0, 0x50, 0x2a);
pub const ULTRAMAR: (u8, u8, u8) = (0x2f, 0x4b, 0xc4);
pub const PAPEL: (u8, u8, u8) = (0xf7, 0xf2, 0xe7);
pub const TINTA: (u8, u8, u8) = (0x2b, 0x24, 0x18);

pub fn relleno(c: (u8, u8, u8), opacidad: f32) -> Fill {
    Fill {
        paint: rgb::Color::new(c.0, c.1, c.2).into(),
        opacity: NormalizedF32::new(opacidad.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE),
        rule: FillRule::NonZero,
    }
}

/// Un intérprete mínimo de trazados SVG absolutos (M, L, C, Q, Z): todo lo
/// que usa el loro.
fn fichas(d: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in d.chars() {
        if c.is_ascii_alphabetic() {
            if !cur.is_empty() {
                v.push(std::mem::take(&mut cur));
            }
            v.push(c.to_string());
        } else if c == ' ' || c == ',' {
            if !cur.is_empty() {
                v.push(std::mem::take(&mut cur));
            }
        } else if c == '-' && !cur.is_empty() {
            v.push(std::mem::take(&mut cur));
            cur.push(c);
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        v.push(cur);
    }
    v
}

fn trazado(d: &str) -> Option<Path> {
    let f = fichas(d);
    let mut i = 0usize;
    let es_num = |i: usize| f.get(i).map(|t| t.parse::<f32>().is_ok()).unwrap_or(false);
    let n = |i: &mut usize| -> Option<f32> {
        let v = f.get(*i)?.parse().ok();
        *i += 1;
        v
    };
    let mut pb = PathBuilder::new();
    while i < f.len() {
        let op = f[i].clone();
        i += 1;
        match op.as_str() {
            "M" => {
                let (x, y) = (n(&mut i)?, n(&mut i)?);
                pb.move_to(x, y);
            }
            "L" => {
                let (x, y) = (n(&mut i)?, n(&mut i)?);
                pb.line_to(x, y);
            }
            "C" => loop {
                let (a, b, c, d2, e, g) = (
                    n(&mut i)?,
                    n(&mut i)?,
                    n(&mut i)?,
                    n(&mut i)?,
                    n(&mut i)?,
                    n(&mut i)?,
                );
                pb.cubic_to(a, b, c, d2, e, g);
                if !es_num(i) {
                    break;
                }
            },
            "Q" => loop {
                let (a, b, c, d2) = (n(&mut i)?, n(&mut i)?, n(&mut i)?, n(&mut i)?);
                pb.quad_to(a, b, c, d2);
                if !es_num(i) {
                    break;
                }
            },
            "Z" | "z" => pb.close(),
            _ => return None,
        }
    }
    pb.finish()
}

/// Elipse con cuatro cúbicas (k = 0,5523).
fn elipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    const K: f32 = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy + ry * K, cx + rx * K, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - rx * K, cy + ry, cx - rx, cy + ry * K, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - ry * K, cx - rx * K, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + rx * K, cy - ry, cx + rx, cy - ry * K, cx + rx, cy);
    pb.close();
    pb.finish()
}

fn pinta(s: &mut Surface<'_>, p: Option<Path>, f: Fill) {
    if let Some(p) = p {
        s.set_fill(Some(f));
        s.set_stroke(None);
        s.draw_path(&p);
    }
}

/// Dibuja el loro con su esquina superior izquierda en (x, y) y `lado`
/// puntos de ancho (el dibujo original mide 64×64). `opacidad` < 1 para la
/// marca de agua.
pub fn dibujar(s: &mut Surface<'_>, x: f32, y: f32, lado: f32, opacidad: f32) {
    let k = lado / 64.0;
    let base = Transform::from_row(k, 0.0, 0.0, k, x, y);
    s.push_transform(&base);
    s.push_transform(&Transform::from_rotate_at(-3.0, 32.0, 32.0));
    if opacidad < 1.0 {
        if let Some(o) = NormalizedF32::new(opacidad) {
            s.push_opacity(o);
        }
    }
    // El texto (ultramar), detrás y corrido.
    s.push_transform(&Transform::from_translate(1.8, 2.0));
    for p in [
        trazado(CUERPO),
        trazado(COPETE),
        elipse(23.5, 49.6, 5.6, 4.2),
        elipse(40.5, 50.8, 5.6, 4.2),
    ] {
        pinta(s, p, relleno(ULTRAMAR, 1.0));
    }
    s.pop();
    // La voz (coral), delante.
    for p in [
        trazado(CUERPO),
        trazado(COPETE),
        elipse(23.5, 49.6, 5.6, 4.2),
        elipse(40.5, 50.8, 5.6, 4.2),
    ] {
        pinta(s, p, relleno(CORAL, 1.0));
    }
    pinta(s, elipse(19.8, 29.5, 2.9, 2.9), relleno(PAPEL, 0.55));
    pinta(s, elipse(44.2, 29.5, 2.9, 2.9), relleno(PAPEL, 0.55));
    pinta(s, elipse(25.5, 23.5, 2.0, 3.7), relleno(TINTA, 1.0));
    pinta(s, elipse(38.5, 23.5, 2.0, 3.7), relleno(TINTA, 1.0));
    pinta(s, trazado(PICO), relleno(PAPEL, 1.0));
    if opacidad < 1.0 && NormalizedF32::new(opacidad).is_some() {
        s.pop();
    }
    s.pop();
    s.pop();
}

#[cfg(test)]
mod tests {
    #[test]
    fn el_loro_se_traza() {
        assert!(super::trazado(super::CUERPO).is_some());
        assert!(super::trazado(super::COPETE).is_some());
        assert!(super::trazado(super::PICO).is_some());
    }
}
