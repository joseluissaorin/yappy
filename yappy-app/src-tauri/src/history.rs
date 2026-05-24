//! Reading history — appended to a small JSON file in the config dir.

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub started_at: i64, // unix timestamp seconds
    pub source: String,  // "Selection" / "ActiveDocument" / "Ocr" / "Manual"
    pub app_name: Option<String>,
    pub voice: String,
    pub lang: String,
    pub text: String,
    pub duration_secs: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

pub fn history_path(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<PathBuf> {
    let mut p = handle.path().app_config_dir()?;
    std::fs::create_dir_all(&p)?;
    p.push("history.json");
    Ok(p)
}

pub fn load(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<History> {
    let p = history_path(handle)?;
    if !p.exists() {
        return Ok(History::default());
    }
    let bytes = std::fs::read(&p)?;
    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

pub fn save(handle: &tauri::AppHandle<impl tauri::Runtime>, h: &History) -> Result<()> {
    let p = history_path(handle)?;
    let json = serde_json::to_vec_pretty(h)?;
    std::fs::write(p, json)?;
    Ok(())
}

pub fn append(
    handle: &tauri::AppHandle<impl tauri::Runtime>,
    entry: HistoryEntry,
    max: usize,
) -> Result<()> {
    let mut h = load(handle)?;
    h.entries.insert(0, entry);
    h.entries.truncate(max.max(1));
    save(handle, &h)
}

pub fn clear(handle: &tauri::AppHandle<impl tauri::Runtime>) -> Result<()> {
    save(handle, &History::default())
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
