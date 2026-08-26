//! Manage the Parakeet TDT (speech-to-text) ONNX model assets — locate,
//! download (with progress), verify. Mirrors `model.rs` (the TTS model manager)
//! so the frontend reuses the same `DownloadProgress` event shape.
//!
//! We ship the **int8** variant of `istupakov/parakeet-tdt-0.6b-v3-onnx`
//! (~670 MB total, self-contained — no `.onnx.data` sidecar) as the default
//! download: a sensible size/quality tradeoff next to Yappy's 380 MB TTS model,
//! and Parakeet TDT v3 is multilingual (25 languages) so it transcribes "any
//! audio". The loader in `yappy-core::asr::parakeet` also accepts the
//! full-precision filenames if a power user drops them in.
//!
//! On iOS this ONNX model is unused — transcription runs natively on CoreML in
//! Swift — but the file compiles everywhere; iOS simply never calls the download.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tauri::Manager;

pub use crate::model::DownloadProgress;

const HF_BASE: &str = "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main";

/// `(remote filename, approx size in bytes)` for the int8 model set.
pub const REQUIRED_FILES: &[(&str, u64)] = &[
    ("encoder-model.int8.onnx", 652_183_999),
    ("decoder_joint-model.int8.onnx", 18_202_004),
    ("vocab.txt", 93_939),
];

pub fn asr_model_root(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<PathBuf> {
    let mut p = handle.path().app_data_dir()?;
    std::fs::create_dir_all(&p)?;
    p.push("models");
    p.push("parakeet-tdt-v3");
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn is_asr_model_ready(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<bool> {
    let root = asr_model_root(handle)?;
    // vocab.txt + at least one encoder + one decoder_joint (int8 OR full).
    if !root.join("vocab.txt").exists() {
        return Ok(false);
    }
    let has_encoder = [
        "encoder-model.int8.onnx",
        "encoder-model.onnx",
        "encoder.onnx",
    ]
    .iter()
    .any(|f| file_big_enough(&root.join(f), 1_000_000));
    let has_decoder = [
        "decoder_joint-model.int8.onnx",
        "decoder_joint-model.onnx",
        "decoder_joint.onnx",
    ]
    .iter()
    .any(|f| file_big_enough(&root.join(f), 100_000));
    Ok(has_encoder && has_decoder)
}

fn file_big_enough(p: &std::path::Path, min: u64) -> bool {
    std::fs::metadata(p)
        .map(|m| m.len() >= min)
        .unwrap_or(false)
}

/// Stream-download the int8 model set, emitting `DownloadProgress` events.
pub async fn download_asr_model(
    handle: &tauri::AppHandle<impl tauri::Runtime>,
    emit: impl Fn(DownloadProgress) + Send + Sync + 'static,
) -> Result<()> {
    use futures_util::StreamExt;
    let root = asr_model_root(handle)?;

    let overall_total: u64 = REQUIRED_FILES.iter().map(|(_, s)| *s).sum::<u64>().max(1);
    let mut overall_done: u64 = 0;

    let client = reqwest::Client::builder()
        .user_agent("Yappy/0.1 (https://yappy.app)")
        .build()?;

    for (name, _expected) in REQUIRED_FILES {
        let local = root.join(name);
        if file_big_enough(&local, 100) {
            tracing::debug!("asr model file already present: {}", local.display());
            overall_done += std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
            continue;
        }
        let url = format!("{HF_BASE}/{name}");
        tracing::info!("downloading {} -> {}", url, local.display());
        emit(DownloadProgress {
            file: (*name).into(),
            bytes_done: 0,
            bytes_total: 0,
            stage: "start".into(),
            overall_done,
            overall_total,
        });

        let resp = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?
            .error_for_status()?;
        let bytes_total = resp.content_length().unwrap_or(0);
        let mut stream = resp.bytes_stream();
        let tmp = local.with_extension("part");
        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut bytes_done: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            bytes_done += chunk.len() as u64;
            if bytes_done % (512 * 1024) < chunk.len() as u64 {
                emit(DownloadProgress {
                    file: (*name).into(),
                    bytes_done,
                    bytes_total,
                    stage: "downloading".into(),
                    overall_done: overall_done + bytes_done,
                    overall_total,
                });
            }
        }
        drop(file);
        tokio::fs::rename(&tmp, &local).await?;
        overall_done += bytes_done;
        emit(DownloadProgress {
            file: (*name).into(),
            bytes_done,
            bytes_total,
            stage: "done".into(),
            overall_done,
            overall_total,
        });
    }

    if !is_asr_model_ready(handle)? {
        return Err(anyhow!("asr model files missing after download"));
    }
    Ok(())
}
