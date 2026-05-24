use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::bridge::Bridge;
use crate::playback::PlaybackController;
use crate::settings::Settings;
use yappy_core::TtsEngine;

/// What the document window should render. Lives in AppState because we hand off
/// across the open-window race (backend emits `document_loaded` faster than the
/// new window's JS can register listeners — without this, the window opens empty).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentDocument {
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub paragraphs: Vec<String>,
    pub char_count: usize,
    /// True while the backend is still parsing this file. Frontend renders a
    /// "parsing…" placeholder instead of the empty state, so the user sees
    /// progress even on a multi-second pdf-extract call.
    #[serde(default)]
    pub loading: bool,
    /// Reading-rhythm hints derived from markdown structure (when applicable).
    /// Parallel arrays — `paragraphs[i]`, `paragraph_pauses[i]`, `paragraph_kinds[i]`
    /// describe the same paragraph. For unstructured formats these are 0.0 and "paragraph".
    #[serde(default)]
    pub paragraph_pauses: Vec<f32>,
    #[serde(default)]
    pub paragraph_speed_mult: Vec<f32>,
    #[serde(default)]
    pub paragraph_kinds: Vec<String>,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    /// Serializes settings-save calls so concurrent updates don't race the disk write.
    pub save_lock: Mutex<()>,
    pub engine: Mutex<Option<Arc<TtsEngine>>>,
    pub playback: Arc<PlaybackController>,
    pub bridge: Bridge,
    /// One document per open window. Keyed by the window's label (e.g. "document-1234").
    /// This makes multi-window possible — each doc window has its own state, and emits
    /// are targeted at the specific label so events don't cross-pollute.
    pub documents: Mutex<HashMap<String, CurrentDocument>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            save_lock: Mutex::new(()),
            engine: Mutex::new(None),
            playback: Arc::new(PlaybackController::new()),
            bridge: Bridge::default(),
            documents: Mutex::new(HashMap::new()),
        }
    }

    pub fn engine_or_load(
        &self,
        model_root: &std::path::Path,
    ) -> anyhow::Result<Arc<TtsEngine>> {
        let mut slot = self.engine.lock().unwrap();
        if let Some(e) = slot.as_ref() {
            return Ok(e.clone());
        }
        let cfg = yappy_core::engine::engine_config(model_root);
        let engine = Arc::new(TtsEngine::new(cfg)?);
        *slot = Some(engine.clone());
        Ok(engine)
    }
}
