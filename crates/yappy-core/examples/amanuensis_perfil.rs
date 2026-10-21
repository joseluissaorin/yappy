//! Tiempos por fase de Amanuensis Ligero en Rust (enderezar, detectar, recortes, leer).
use std::path::PathBuf;
use std::time::Instant;
use yappy_core::ocr::{abrir_imagen, lector, orientacion, Ligero, Opciones};

fn main() -> anyhow::Result<()> {
    let cache = PathBuf::from(std::env::var("HOME")?).join(".cache/amanuensis");
    let mut m = Ligero::desde_carpetas(
        &cache.join("amanuensis-det-minimo-1"),
        &cache.join("amanuensis-ligero-3"),
        Opciones::default(),
    )?;
    let (mut te, mut td, mut tr, mut tl, mut tt) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut n = 0;
    for r in std::env::args().skip(1) {
        let g = abrir_imagen(&PathBuf::from(&r))?;
        let t = Instant::now();
        let (img, _) = orientacion::enderezar(&g);
        te += t.elapsed().as_secs_f64();
        let t = Instant::now();
        let cs = m.detectar(&img)?;
        td += t.elapsed().as_secs_f64();
        let t = Instant::now();
        let rec: Vec<_> = cs
            .iter()
            .map(|c| lector::recorte_de_caja(&img, c))
            .collect();
        tr += t.elapsed().as_secs_f64();
        let t = Instant::now();
        m.leer_renglones(&rec)?;
        tl += t.elapsed().as_secs_f64();
        let t = Instant::now();
        m.leer_pagina(&g)?;
        tt += t.elapsed().as_secs_f64();
        n += 1;
    }
    let n = n as f64;
    println!("por página: enderezar {:.3} s, detectar {:.3}, recortes {:.3}, leer {:.3}; página entera {:.3}", te / n, td / n, tr / n, tl / n, tt / n);
    Ok(())
}
