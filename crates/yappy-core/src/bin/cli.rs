//! yappy-cli — small CLI to test the yappy-core pipeline end-to-end without a UI.
//! Usage: yappy-cli --root /path/to/supertonic-3 --voice Jessica --text "Hello." --out out.wav

use std::path::PathBuf;

use anyhow::Result;
use yappy_core::engine::{engine_config, SynthesisOptions, TtsEngine};
use yappy_core::supertonic::write_wav;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // `yappy-cli transcribe --root <asr-model-dir> --in <16k-mono.wav>` — fast
    // correctness loop for the Parakeet ASR engine without spinning up Tauri.
    let mut raw = std::env::args().skip(1).peekable();
    if raw.peek().map(|s| s.as_str()) == Some("transcribe") {
        let _ = raw.next();
        return transcribe_cmd(raw.collect());
    }
    // `yappy-cli lote --root <supertonic-3> --in <lote.jsonl>`: muchas frases
    // con UNA sola carga del modelo (las voces pregrabadas del paseo). Cada
    // línea: {"texto","lang","voz","velocidad","pasos","semilla","out"}.
    if raw.peek().map(|s| s.as_str()) == Some("lote") {
        let _ = raw.next();
        return lote_cmd(raw.collect());
    }

    let mut args = std::env::args().skip(1);
    let mut root: PathBuf = PathBuf::from("/tmp/supertonic-3");
    let mut voice = String::from("Jessica");
    let mut text =
        String::from("Yappy is now alive, and reading your text aloud with Supertonic 3.");
    let mut out = PathBuf::from("out.wav");
    let mut speed = 1.05f32;
    let mut lang = String::from("en");
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = args.next().unwrap().into(),
            "--voice" => voice = args.next().unwrap(),
            "--text" => text = args.next().unwrap(),
            "--out" => out = args.next().unwrap().into(),
            "--speed" => speed = args.next().unwrap().parse()?,
            "--lang" => lang = args.next().unwrap(),
            "--list-voices" => {
                for v in yappy_core::voices::VOICES {
                    println!(
                        "{:<8} {:<4} {:?}  — {}",
                        v.name, v.id, v.gender, v.description
                    );
                }
                return Ok(());
            }
            _ => eprintln!("ignoring unknown arg: {a}"),
        }
    }

    let cfg = engine_config(&root);
    let engine = TtsEngine::new(cfg)?;
    let opts = SynthesisOptions {
        voice,
        speed,
        default_lang: lang,
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let chunks = engine.synthesize(&text, &opts)?;
    let elapsed = started.elapsed();

    let sr = engine.sample_rate();
    let mut combined: Vec<f32> = Vec::new();
    let silence = vec![0.0f32; (sr as f32 * 0.3) as usize];
    for (i, c) in chunks.iter().enumerate() {
        println!(
            "chunk {} of {}  ({} samples, lang={})  → {:.40}",
            i + 1,
            c.total,
            c.samples.len(),
            c.lang,
            c.text.replace('\n', " ")
        );
        if i > 0 {
            combined.extend_from_slice(&silence);
        }
        combined.extend_from_slice(&c.samples);
    }
    write_wav(&out, &combined, sr)?;
    println!(
        "Saved {} ({:.2}s of audio, generated in {:.2}s)",
        out.display(),
        combined.len() as f32 / sr as f32,
        elapsed.as_secs_f32()
    );
    Ok(())
}

/// Transcribe a 16 kHz mono WAV with the Parakeet TDT engine.
fn transcribe_cmd(args: Vec<String>) -> Result<()> {
    use yappy_core::asr::{ParakeetAsr, TranscribeOptions, Transcriber};

    let mut root = PathBuf::from("/tmp/parakeet-tdt");
    let mut input: Option<PathBuf> = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = it.next().expect("--root needs a path").into(),
            "--in" => input = Some(it.next().expect("--in needs a path").into()),
            _ => eprintln!("ignoring unknown arg: {a}"),
        }
    }
    let input = input.expect("usage: yappy-cli transcribe --root <dir> --in <16k-mono.wav>");

    // Read the WAV (expects 16 kHz mono; resampling/decoding of other formats
    // lives in the Tauri layer, not in core).
    let mut reader = hound::WavReader::open(&input)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / (1i64 << (spec.bits_per_sample - 1)) as f32))
            .collect::<std::result::Result<_, _>>()?,
    };
    let mono: Vec<f32> = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|c| c.iter().sum::<f32>() / c.len() as f32)
            .collect()
    } else {
        samples
    };
    if spec.sample_rate != 16_000 {
        eprintln!(
            "warning: input is {} Hz, Parakeet expects 16000 Hz — transcript may be garbage",
            spec.sample_rate
        );
    }

    let mut asr = ParakeetAsr::from_dir(&root)?;
    let started = std::time::Instant::now();
    let result = asr.transcribe_mono16k(&mono, &TranscribeOptions::default())?;
    let elapsed = started.elapsed();
    println!("\n─── transcript ───\n{}\n", result.text);
    println!(
        "{:.2}s audio transcribed in {:.2}s ({:.1}× realtime), {} segments",
        result.audio_secs,
        elapsed.as_secs_f32(),
        result.audio_secs / elapsed.as_secs_f32().max(1e-3),
        result.segments.len()
    );
    Ok(())
}

/// Sintetiza un lote de frases (JSON por línea) con una sola carga del
/// modelo. Los trozos de una frase se unen con un respiro corto, como en la
/// app; escribe WAV y una línea `ok <out> <segundos>` por frase.
fn lote_cmd(args: Vec<String>) -> Result<()> {
    let mut root = PathBuf::from("/tmp/supertonic-3");
    let mut input: Option<PathBuf> = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = it.next().expect("--root necesita ruta").into(),
            "--in" => input = Some(it.next().expect("--in necesita ruta").into()),
            _ => eprintln!("ignoring unknown arg: {a}"),
        }
    }
    let input = input.expect("uso: yappy-cli lote --root <dir> --in <lote.jsonl>");
    let engine = TtsEngine::new(engine_config(&root))?;
    let sr = engine.sample_rate();
    let respiro = vec![0.0f32; (sr as f32 * 0.22) as usize];
    for linea in std::fs::read_to_string(&input)?.lines() {
        if linea.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(linea)?;
        let texto = v["texto"].as_str().unwrap_or_default();
        let out = PathBuf::from(v["out"].as_str().expect("falta out"));
        let opts = SynthesisOptions {
            voice: v["voz"].as_str().unwrap_or("Alex").to_string(),
            speed: v["velocidad"].as_f64().unwrap_or(1.0) as f32,
            default_lang: v["lang"].as_str().unwrap_or("en").to_string(),
            total_steps: v["pasos"].as_u64().unwrap_or(12) as usize,
            seed: v["semilla"].as_u64().or(Some(7)),
            // El idioma lo marca el diccionario: sin adivinarlo.
            detectar_idioma: false,
            ..Default::default()
        };
        let chunks = engine.synthesize(texto, &opts)?;
        let mut todo: Vec<f32> = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            if i > 0 {
                todo.extend_from_slice(&respiro);
            }
            todo.extend_from_slice(&c.samples);
        }
        write_wav(&out, &todo, sr)?;
        println!("ok {} {:.2}", out.display(), todo.len() as f32 / sr as f32);
    }
    Ok(())
}
