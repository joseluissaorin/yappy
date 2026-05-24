//! yappy-cli — small CLI to test the yappy-core pipeline end-to-end without a UI.
//! Usage: yappy-cli --root /path/to/supertonic-3 --voice Jessica --text "Hello." --out out.wav

use std::path::PathBuf;

use anyhow::Result;
use yappy_core::engine::{engine_config, SynthesisOptions, TtsEngine};
use yappy_core::supertonic::write_wav;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut args = std::env::args().skip(1);
    let mut root: PathBuf = PathBuf::from("/tmp/supertonic-3");
    let mut voice = String::from("Jessica");
    let mut text = String::from("Yappy is now alive, and reading your text aloud with Supertonic 3.");
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
                    println!("{:<8} {:<4} {:?}  — {}", v.name, v.id, v.gender, v.description);
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
