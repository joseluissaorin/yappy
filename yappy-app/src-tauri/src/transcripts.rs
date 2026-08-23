//! Transcription history — appended to a small JSON file in the config dir.
//! Mirrors `history.rs` (the TTS reading history) but with a transcript-shaped
//! schema (no voice/speed; has the source filename + detected language).

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: String,
    pub created_at: i64, // unix timestamp seconds
    /// "File" / "Shared" / "Manual" — how the audio reached Yappy.
    pub source: String,
    /// Original filename (or app the audio was shared from), best-effort.
    pub filename: Option<String>,
    pub duration_secs: f32,
    pub language: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transcripts {
    pub entries: Vec<Transcript>,
}

pub fn transcripts_path(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<PathBuf> {
    let mut p = handle.path().app_config_dir()?;
    std::fs::create_dir_all(&p)?;
    p.push("transcripts.json");
    Ok(p)
}

pub fn load(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<Transcripts> {
    let p = transcripts_path(handle)?;
    if !p.exists() {
        return Ok(Transcripts::default());
    }
    let bytes = std::fs::read(&p)?;
    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

pub fn save(handle: &tauri::AppHandle<impl tauri::Runtime>, t: &Transcripts) -> Result<()> {
    let p = transcripts_path(handle)?;
    let json = serde_json::to_vec_pretty(t)?;
    std::fs::write(p, json)?;
    Ok(())
}

pub fn append(
    handle: &tauri::AppHandle<impl tauri::Runtime>,
    entry: Transcript,
    max: usize,
) -> Result<()> {
    let mut t = load(handle)?;
    t.entries.insert(0, entry);
    t.entries.truncate(max.max(1));
    save(handle, &t)
}

pub fn delete(handle: &tauri::AppHandle<impl tauri::Runtime>, id: &str) -> Result<()> {
    let mut t = load(handle)?;
    t.entries.retain(|e| e.id != id);
    save(handle, &t)
}

pub fn clear(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<()> {
    save(handle, &Transcripts::default())
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
