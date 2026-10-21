//! Amanuensis Ligero en Rust sobre imágenes sueltas, para compararlo con el Python original:
//!   cargo run --release -p yappy-core --example amanuensis -- SALIDA página.png …
//! Pesos: AMANUENSIS_DET (por omisión ~/.cache/amanuensis/amanuensis-det-minimo-1) y AMANUENSIS_LIGERO
//! (~/.cache/amanuensis/amanuensis-ligero-3). Escribe SALIDA/<carpeta>_<nombre>.json como `ref.py`.
use std::path::PathBuf;
use yappy_core::ocr::{abrir_imagen, Ligero, Opciones};

fn main() -> anyhow::Result<()> {
    let cache = dirs_home().join(".cache/amanuensis");
    let det = std::env::var("AMANUENSIS_DET")
        .map(PathBuf::from)
        .unwrap_or(cache.join("amanuensis-det-minimo-1"));
    let lec = std::env::var("AMANUENSIS_LIGERO")
        .map(PathBuf::from)
        .unwrap_or(cache.join("amanuensis-ligero-3"));
    let hilos = std::env::var("HILOS")
        .ok()
        .and_then(|h| h.parse().ok())
        .unwrap_or(0);
    let mut args = std::env::args().skip(1);
    let salida = PathBuf::from(args.next().expect("SALIDA"));
    std::fs::create_dir_all(&salida)?;
    let t0 = std::time::Instant::now();
    let mut m = Ligero::desde_carpetas(
        &det,
        &lec,
        Opciones {
            hilos,
            lote: std::env::var("LOTE")
                .ok()
                .and_then(|h| h.parse().ok())
                .unwrap_or(64),
            arena: std::env::var("ARENA").map(|v| v != "0").unwrap_or(true),
            patron: std::env::var("PATRON").map(|v| v != "0").unwrap_or(true),
            ..Default::default()
        },
    )?;
    eprintln!("modelos cargados en {:.2} s", t0.elapsed().as_secs_f64());
    for r in args {
        let ruta = PathBuf::from(&r);
        let g = abrir_imagen(&ruta)?;
        let t = std::time::Instant::now();
        let p = m.leer_pagina(&g)?;
        let dt = t.elapsed().as_secs_f64();
        let carpeta = ruta
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let nombre = format!(
            "{carpeta}_{}",
            ruta.file_stem().and_then(|s| s.to_str()).unwrap_or("x")
        );
        let j = serde_json::json!({
            "segundos": dt, "giro": p.giro, "inclinacion": p.inclinacion,
            "lineas": p.lineas, "parrafos": p.parrafos(), "calidad": p.calidad(),
        });
        std::fs::write(
            salida.join(format!("{nombre}.json")),
            serde_json::to_string_pretty(&j)?,
        )?;
        println!(
            "{nombre} {dt:.2}s {} líneas, giro {}, incl {}",
            p.lineas.len(),
            p.giro,
            p.inclinacion
        );
    }
    Ok(())
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}
