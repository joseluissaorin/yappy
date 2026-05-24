//! Local WebSocket bridge.
//!
//! Yappy listens on `127.0.0.1:47898`. Browser extensions connect, identify
//! themselves with a shared token (auto-paired on first connection), and
//! forward the active tab's clean content. Yappy can also push
//! `fetch_current_tab` requests so a hotkey press triggers the extension.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;

pub const BRIDGE_PORT: u16 = 47898;

#[derive(Default)]
pub struct Bridge {
    pub connections: Arc<Mutex<HashMap<String, ConnectionHandle>>>,
}

impl Clone for Bridge {
    fn clone(&self) -> Self {
        Self { connections: self.connections.clone() }
    }
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge").finish()
    }
}

/// Live per-browser handle. `tx` writes a JSON string to that connection.
pub struct ConnectionHandle {
    pub info: ConnectionInfo,
    pub tx: mpsc::UnboundedSender<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub browser: String,
    pub connected_at: i64,
    pub last_seen: i64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InMsg {
    Hello { token: String, browser: Option<String> },
    Page { url: Option<String>, title: Option<String>, markdown: String },
    Ping,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutMsg {
    Welcome { paired: bool, version: &'static str },
    Ack { ok: bool, message: Option<String> },
    Pong,
    NotPaired,
    FetchCurrentTab,
}

pub fn start<R: Runtime + 'static>(app: AppHandle<R>, state: Arc<crate::state::AppState>) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app, state).await {
            tracing::error!("bridge: stopped with error: {e:?}");
        }
    });
}

async fn run<R: Runtime + 'static>(
    app: AppHandle<R>,
    state: Arc<crate::state::AppState>,
) -> Result<()> {
    let addr: SocketAddr = format!("127.0.0.1:{}", BRIDGE_PORT).parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("bridge: listening on ws://{}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::debug!("bridge: accepted {peer}");
        let app2 = app.clone();
        let state2 = state.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = handle_conn(stream, app2, state2).await {
                tracing::debug!("bridge: connection ended: {e:?}");
            }
        });
    }
}

async fn handle_conn<R: Runtime + 'static>(
    stream: tokio::net::TcpStream,
    app: AppHandle<R>,
    state: Arc<crate::state::AppState>,
) -> Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    use futures::{SinkExt, StreamExt};
    let (mut write, mut read) = ws.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Forward outbound messages from `rx` to the websocket.
    let writer_task = tauri::async_runtime::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if write.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
        // Best-effort close.
        let _ = write.close().await;
    });

    let mut paired = false;
    let mut browser_name = "unknown".to_string();

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        if !msg.is_text() {
            continue;
        }
        let text = msg.into_text().unwrap_or_default();
        let parsed: Result<InMsg, _> = serde_json::from_str(&text);
        let Ok(in_msg) = parsed else { continue; };
        match in_msg {
            InMsg::Hello { token, browser } => {
                // Auto-pair: if no token has been claimed yet, the first connection
                // claims it. Subsequent connections must match.
                let mut token_to_store: Option<String> = None;
                let accepted = {
                    let s = state.settings.lock().unwrap();
                    if s.bridge_token.is_empty() {
                        token_to_store = Some(token.clone());
                        true
                    } else {
                        s.bridge_token == token
                    }
                };
                if accepted && !token.is_empty() {
                    if let Some(tk) = token_to_store {
                        // Persist auto-pair via the central settings::update path so we
                        // serialize with any concurrent user-driven setter.
                        let app_for_save = app.clone();
                        let state_for_save = state.clone();
                        let tk2 = tk.clone();
                        if let Err(e) = crate::settings::update(&app_for_save, &state_for_save, |s| {
                            s.bridge_token = tk2;
                        }) {
                            tracing::warn!("bridge: failed to persist token: {e:?}");
                        } else {
                            let _ = app.emit("bridge_token_changed", &tk);
                        }
                    }
                    paired = true;
                    browser_name = browser.unwrap_or_else(|| "unknown".into());
                    let info = ConnectionInfo {
                        browser: browser_name.clone(),
                        connected_at: now_unix(),
                        last_seen: now_unix(),
                    };
                    {
                        let app_state = app.state::<Arc<crate::state::AppState>>();
                        let mut conns = app_state.bridge.connections.lock().await;
                        conns.insert(
                            browser_name.clone(),
                            ConnectionHandle { info: info.clone(), tx: tx.clone() },
                        );
                    }
                    let _ = app.emit("bridge_paired", &browser_name);
                    let _ = tx.send(serde_json::to_string(&OutMsg::Welcome {
                        paired: true,
                        version: env!("CARGO_PKG_VERSION"),
                    })?);
                } else {
                    let _ = tx.send(serde_json::to_string(&OutMsg::NotPaired)?);
                    break;
                }
            }
            InMsg::Page { url, title, markdown } => {
                if !paired { let _ = tx.send(serde_json::to_string(&OutMsg::NotPaired)?); continue; }
                let prefix = title.as_deref().map(|t| format!("{t}.\n\n")).unwrap_or_default();
                let combined = format!("{prefix}{}", clean_markdown_for_speech(&markdown));
                let app_for_read = app.clone();
                let state_for_read = state.clone();
                let url_for_event = url.clone();
                let title_for_event = title.clone();
                let browser_for_event = browser_name.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = app_for_read.emit(
                        "capture_info",
                        serde_json::json!({
                            "source": {
                                "kind": "webpage",
                                "app_name": browser_for_event,
                                "url": url_for_event,
                                "title": title_for_event,
                            },
                            "len": combined.chars().count(),
                        }),
                    );
                    let _ = crate::commands::read_text(
                        &app_for_read,
                        state_for_read,
                        combined,
                        format!("extension:{}", browser_for_event),
                    ).await;
                });
                let _ = tx.send(serde_json::to_string(&OutMsg::Ack { ok: true, message: None })?);
            }
            InMsg::Ping => { let _ = tx.send(serde_json::to_string(&OutMsg::Pong)?); }
        }
    }

    drop(tx);
    let _ = writer_task.await;

    if paired {
        let conns = {
            let app_state = app.state::<Arc<crate::state::AppState>>();
            app_state.bridge.connections.clone()
        };
        let bname = browser_name.clone();
        tauri::async_runtime::spawn(async move {
            let mut g = conns.lock().await;
            g.remove(&bname);
        });
        let _ = app.emit("bridge_disconnected", &browser_name);
    }
    Ok(())
}

/// Ask the named browser's extension to push us the current tab. Returns true if a
/// request was sent (i.e. a matching paired connection exists).
pub async fn request_fetch_current_tab(
    state: &Arc<crate::state::AppState>,
    browser: &str,
) -> bool {
    let conns = state.bridge.connections.lock().await;
    // Try exact match first, then case-insensitive / family-friendly.
    let key = conns
        .keys()
        .find(|k| k.eq_ignore_ascii_case(browser))
        .cloned()
        .or_else(|| {
            // family-friendly: "Brave Browser" ~ "Brave", "Google Chrome" ~ "Chromium" etc.
            conns
                .keys()
                .find(|k| browser.to_lowercase().contains(&k.to_lowercase())
                    || k.to_lowercase().contains(&browser.to_lowercase()))
                .cloned()
        });
    let Some(key) = key else { return false; };
    let Some(h) = conns.get(&key) else { return false; };
    h.tx
        .send(serde_json::to_string(&OutMsg::FetchCurrentTab).unwrap_or_default())
        .is_ok()
}

pub async fn is_browser_paired(state: &Arc<crate::state::AppState>, browser: &str) -> bool {
    let conns = state.bridge.connections.lock().await;
    conns.keys().any(|k| {
        k.eq_ignore_ascii_case(browser)
            || browser.to_lowercase().contains(&k.to_lowercase())
            || k.to_lowercase().contains(&browser.to_lowercase())
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn clean_markdown_for_speech(md: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // Patterns we strip to make TTS speech less noisy. Wikipedia (the common defuddle
    // source) produces a LOT of markdown link syntax that, when read literally, says
    // every linked phrase twice — once as visible text, once as the URL/article name.
    // We keep the link's display text and drop the rest.
    static IMG: Lazy<Regex> = Lazy::new(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap());
    static LINK_WITH_TITLE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"\[([^\]]+)\]\([^)]*\s+"[^"]*"\s*\)"#).unwrap());
    static LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([^\]]+)\]\([^)]*\)").unwrap());
    static REF_LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([^\]]+)\]\[[^\]]*\]").unwrap());
    static BARE_URL: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"\bhttps?://[^\s<>"']+|\bwww\.[^\s<>"']+"#).unwrap());
    static HTML_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]*>").unwrap());
    // Defuddle on Wikipedia often produces "Word (Word)" or "Word [Word]" — same word
    // repeated immediately as parenthetical. Collapse to one. Allow simple variations
    // of accents/case for robustness.
    static DUP_PAREN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(\b\p{L}[\p{L}\p{M}0-9'’\-]{1,40})\s*\(\s*\1\s*\)").unwrap());
    static DUP_BRACK: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(\b\p{L}[\p{L}\p{M}0-9'’\-]{1,40})\s*\[\s*\1\s*\]").unwrap());
    // Footnote markers like [1], [12], [edit].
    static FOOTNOTE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\d+\]|\[edit\]").unwrap());

    let mut out = String::with_capacity(md.len());
    let mut in_code = false;
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "***" {
            continue;
        }
        let stripped_prefix = line.trim_start_matches(|c: char| {
            c == '#' || c == '>' || c == '-' || c == '*' || c.is_whitespace()
        });
        out.push_str(stripped_prefix);
        out.push('\n');
    }

    let s = IMG.replace_all(&out, "").into_owned();
    let s = LINK_WITH_TITLE.replace_all(&s, "$1").into_owned();
    let s = LINK.replace_all(&s, "$1").into_owned();
    let s = REF_LINK.replace_all(&s, "$1").into_owned();
    let s = HTML_TAG.replace_all(&s, "").into_owned();
    let s = FOOTNOTE.replace_all(&s, "").into_owned();
    let s = BARE_URL.replace_all(&s, "").into_owned();
    let s = DUP_PAREN.replace_all(&s, "$1").into_owned();
    let s = DUP_BRACK.replace_all(&s, "$1").into_owned();

    // Collapse runs of whitespace introduced by the strips.
    let mut compact = String::with_capacity(s.len());
    let mut prev_blank = false;
    let mut prev_ws = false;
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !prev_blank {
                compact.push('\n');
                prev_blank = true;
            }
            continue;
        }
        prev_blank = false;
        prev_ws = false;
        for c in t.chars() {
            if c.is_whitespace() {
                if !prev_ws {
                    compact.push(' ');
                    prev_ws = true;
                }
            } else {
                compact.push(c);
                prev_ws = false;
            }
        }
        compact.push('\n');
    }
    compact
}
