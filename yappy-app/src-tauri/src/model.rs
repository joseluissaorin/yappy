//! Manage the Supertonic 3 model assets — locate, download (with progress), verify.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Filenames + sizes (in bytes) we expect under `<root>/onnx/` and `<root>/voice_styles/`.
/// Sizes are sourced from the public HF repo `Supertone/supertonic-3`.
pub const REQUIRED_ONNX: &[(&str, u64)] = &[
    ("duration_predictor.onnx", 3_700_147),
    ("text_encoder.onnx", 36_416_150),
    ("vector_estimator.onnx", 256_534_781),
    ("vocoder.onnx", 101_424_195),
];
pub const REQUIRED_AUX: &[&str] = &["tts.json", "unicode_indexer.json"];
pub const VOICE_STYLE_FILES: &[&str] = &[
    "M1.json", "M2.json", "M3.json", "M4.json", "M5.json", "F1.json", "F2.json", "F3.json",
    "F4.json", "F5.json",
];

const HF_BASE: &str = "https://huggingface.co/Supertone/supertonic-3/resolve/main";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub file: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub stage: String,
    pub overall_done: u64,
    pub overall_total: u64,
}

pub fn model_root(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<PathBuf> {
    let mut p = handle.path().app_data_dir()?;
    std::fs::create_dir_all(&p)?;
    p.push("models");
    p.push("supertonic-3");
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn is_model_ready(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<bool> {
    let root = model_root(handle)?;
    for (name, _) in REQUIRED_ONNX {
        let p = root.join("onnx").join(name);
        if !p.exists() || std::fs::metadata(&p)?.len() < 100_000 {
            return Ok(false);
        }
    }
    for aux in REQUIRED_AUX {
        if !root.join("onnx").join(aux).exists() {
            return Ok(false);
        }
    }
    for vs in VOICE_STYLE_FILES {
        if !root.join("voice_styles").join(vs).exists() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Stream-download all required files into the model directory, emitting progress events.
pub async fn download_model(handle: &tauri::AppHandle<impl tauri::Runtime>, emit: impl Fn(DownloadProgress) + Send + Sync + 'static) -> Result<()> {
    use futures_util::StreamExt;
    let root = model_root(handle)?;
    std::fs::create_dir_all(root.join("onnx"))?;
    std::fs::create_dir_all(root.join("voice_styles"))?;

    // Build the file list with target sizes.
    let mut files: Vec<(String, PathBuf, Option<u64>)> = Vec::new();
    for (name, size) in REQUIRED_ONNX {
        files.push((format!("onnx/{name}"), root.join("onnx").join(name), Some(*size)));
    }
    for aux in REQUIRED_AUX {
        files.push((format!("onnx/{aux}"), root.join("onnx").join(aux), None));
    }
    for vs in VOICE_STYLE_FILES {
        files.push((format!("voice_styles/{vs}"), root.join("voice_styles").join(vs), None));
    }

    // Pre-compute total expected bytes for files we know.
    let overall_total: u64 = files.iter().filter_map(|(_, _, s)| *s).sum::<u64>().max(1);
    let mut overall_done: u64 = 0;

    let client = reqwest::Client::builder()
        .user_agent("Yappy/0.1 (https://yappy.app)")
        .build()?;

    for (remote, local, _expected) in files {
        if local.exists() && std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0) > 100 {
            // Already there. (Voice style / json files don't have expected sizes; we trust existence.)
            tracing::debug!("model file already present: {}", local.display());
            continue;
        }
        let url = format!("{HF_BASE}/{remote}");
        tracing::info!("downloading {} -> {}", url, local.display());
        emit(DownloadProgress {
            file: remote.clone(),
            bytes_done: 0,
            bytes_total: 0,
            stage: "start".into(),
            overall_done,
            overall_total,
        });

        let resp = client.get(&url).send().await
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
            // Emit progress at most every ~256KB.
            if bytes_done % (256 * 1024) < chunk.len() as u64 {
                emit(DownloadProgress {
                    file: remote.clone(),
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
            file: remote.clone(),
            bytes_done,
            bytes_total,
            stage: "done".into(),
            overall_done,
            overall_total,
        });
    }
    if !is_model_ready(handle)? {
        return Err(anyhow!("model files missing after download"));
    }
    Ok(())
}
