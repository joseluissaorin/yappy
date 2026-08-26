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
