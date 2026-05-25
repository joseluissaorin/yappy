//! Smart capture: pick the best text source for the focused context.

pub mod active_doc;
pub mod browser;
pub mod clipboard;
pub mod doc_loader;
pub mod ocr;
pub mod selection;

use anyhow::Result;
use serde::Serialize;

/// What was captured, with enough context for the UI to show a clear pill.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureSource {
    Selection { app_name: Option<String> },
    ActiveDocument { app_name: String, doc_kind: String },
    /// Browser tab whose HTML was parsed by defuddle.
    Webpage { app_name: String, url: Option<String>, title: Option<String> },
    Ocr { app_name: Option<String> },
    Manual,
    Clipboard,
    File { path: String, extension: String },
    History,
}

impl CaptureSource {
    pub fn short_kind(&self) -> &'static str {
        match self {
            CaptureSource::Selection { .. } => "selection",
            CaptureSource::ActiveDocument { .. } => "document",
            CaptureSource::Webpage { .. } => "webpage",
            CaptureSource::Ocr { .. } => "screen ocr",
            CaptureSource::Manual => "manual",
            CaptureSource::Clipboard => "clipboard",
            CaptureSource::File { .. } => "file",
            CaptureSource::History => "replay",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureResult {
    pub text: String,
    pub source: CaptureSource,
}

/// Fast-path output: either a captured CaptureResult (selection), or a delegation
/// marker (the extension will deliver the text asynchronously), or nothing (use blocking path).
pub enum FastCapture {
    Done(CaptureResult),
    Delegated(CaptureSource),
}

/// Try the fast paths on the calling async thread.
///
/// Order — selection ALWAYS wins, even if a paired browser is focused. This lets the
/// user say "read THIS specific paragraph" by highlighting it inside any web page.
pub async fn fast_capture(
    state: &std::sync::Arc<crate::state::AppState>,
) -> Option<FastCapture> {
    let app_name = front_app_name();

    // 1. Selection — always first, always wins.
    //    Selection runs synthetic ⌘C/Ctrl+C which is fast (~50ms wait window).
    let sel = tokio::task::spawn_blocking(selection::capture_selection)
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();
    if let Some(sel) = sel {
        if !sel.trim().is_empty() {
            return Some(FastCapture::Done(CaptureResult {
                text: sel,
                source: CaptureSource::Selection { app_name: app_name.clone() },
            }));
        }
    }

    // 2. Paired browser extension — ask it for the clean current tab.
    if let Some(name) = &app_name {
        if browser::is_browser(name) && crate::bridge::is_browser_paired(state, name).await {
            let _ = crate::bridge::request_fetch_current_tab(state, name).await;
            tracing::info!("fast_capture: delegated to {name} extension via bridge");
            return Some(FastCapture::Delegated(CaptureSource::Webpage {
                app_name: name.clone(),
                url: None,
                title: None,
            }));
        }
    }
    None
}

/// Blocking capture chain: selection → browser AppleScript → active doc → OCR.
/// Called from `spawn_blocking`. The bridge fast-path is handled by `fast_capture`.
pub fn smart_capture_blocking() -> Result<CaptureResult> {
    let app_name = front_app_name();
    if let Some(sel) = selection::capture_selection().ok().flatten() {
        if !sel.trim().is_empty() {
            return Ok(CaptureResult {
                text: sel,
                source: CaptureSource::Selection { app_name: app_name.clone() },
            });
        }
    }
    smart_capture_core(app_name)
}

fn smart_capture_core(app_name: Option<String>) -> Result<CaptureResult> {
    // 2. Browser — try defuddle on the active tab, fall back to "select all + copy"
    if let Some(name) = &app_name {
        if browser::is_browser(name) {
            match browser::capture_browser_via_defuddle(name) {
                Ok(Some(b)) if !b.text.trim().is_empty() => {
                    return Ok(CaptureResult {
                        text: b.text,
                        source: CaptureSource::Webpage {
                            app_name: name.clone(),
                            url: b.url,
                            title: b.title,
                        },
                    });
                }
                Ok(_) => tracing::debug!("defuddle returned empty for {name}"),
                Err(e) => tracing::debug!("defuddle({name}) failed: {e:?}"),
            }
            // Fallback for browsers without 'Allow JavaScript from AppleScript' enabled.
            if let Ok(Some(text)) = browser::select_all_then_copy_in_browser(name) {
                if !text.trim().is_empty() {
                    return Ok(CaptureResult {
                        text,
                        source: CaptureSource::Webpage {
                            app_name: name.clone(),
                            url: None,
                            title: None,
                        },
                    });
                }
            }
        }
    }

    // 3. Active document
    if let Some(name) = &app_name {
        match active_doc::active_document_text(name) {
            Ok(Some(text)) if !text.trim().is_empty() => {
                return Ok(CaptureResult {
                    text,
                    source: CaptureSource::ActiveDocument {
                        app_name: name.clone(),
                        doc_kind: active_doc::doc_kind_for(name).to_string(),
                    },
                });
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("active_document_text({name}) failed: {e:?}"),
        }
    }

    // 4. Windows-only: UI Automation can pull text directly from the
    //    foreground app's accessibility tree — the Windows analogue of
    //    macOS's AppleScript active-doc path. Works for Word, Edge, Chrome,
    //    VS Code, Notion, Reader, Notepad, etc. Apps without an
    //    accessibility tree (some games, broken Electron) fall through to
    //    OCR. Fast (sub-100ms typically) vs OCR's seconds.
    #[cfg(target_os = "windows")]
    {
        if let Some(text) = crate::os_win::active_window_text() {
            if !text.trim().is_empty() {
                return Ok(CaptureResult {
                    text,
                    source: CaptureSource::ActiveDocument {
                        app_name: app_name.clone().unwrap_or_default(),
                        doc_kind: "uia".to_string(),
                    },
                });
            }
        }
    }

    // 5. OCR fallback
    let ocr_text = ocr::screen_ocr_focused()?;
    Ok(CaptureResult {
        text: ocr_text,
        source: CaptureSource::Ocr { app_name },
    })
}

#[cfg(target_os = "macos")]
pub fn front_app_name() -> Option<String> {
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to get name of first application process whose frontmost is true"#)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(target_os = "windows")]
pub fn front_app_name() -> Option<String> {
    crate::os_win::front_app_name()
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub fn front_app_name() -> Option<String> {
    None
}
