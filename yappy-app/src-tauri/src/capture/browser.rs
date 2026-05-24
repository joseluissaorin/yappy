//! Browser readability via embedded defuddle.js.
//!
//! For each supported browser we:
//!   1. Read the current tab's URL and title (AppleScript).
//!   2. Inject the bundled defuddle.js into the page.
//!   3. Run `new Defuddle(document).parse()` and return clean text + markdown.

use anyhow::Result;
use base64::Engine;
use serde_json;

const DEFUDDLE_JS: &str = include_str!("../../../resources/defuddle.js");

pub struct BrowserCapture {
    pub text: String,
    pub url: Option<String>,
    pub title: Option<String>,
}

pub fn is_browser(app_name: &str) -> bool {
    matches!(
        app_name,
        "Safari"
            | "Google Chrome"
            | "Google Chrome Canary"
            | "Brave Browser"
            | "Arc"
            | "Microsoft Edge"
            | "Vivaldi"
            | "Firefox"
            | "Orion"
    )
}

#[cfg(target_os = "macos")]
pub fn capture_browser_via_defuddle(app_name: &str) -> Result<Option<BrowserCapture>> {
    // Build the JS payload — defuddle bundle exposes a global `Defuddle` constructor (UMD).
    // After running, we serialize {content, title, markdown} as JSON and stringify so AppleScript
    // gets a single string back.
    let js = format!(
        r#"
(function() {{
  try {{
    {bundle}
    var DefuddleCtor = (typeof Defuddle !== 'undefined') ? Defuddle : (window && window.Defuddle);
    if (!DefuddleCtor) {{ return JSON.stringify({{ error: 'defuddle not exposed' }}); }}
    var result = new DefuddleCtor(document, {{ markdown: true, debug: false }}).parse();
    return JSON.stringify({{
      title: result.title || document.title || '',
      content: result.markdownContent || result.content || '',
      url: location.href,
      excerpt: result.description || ''
    }});
  }} catch (e) {{
    return JSON.stringify({{ error: String(e && e.message ? e.message : e) }});
  }}
}})()
"#,
        bundle = DEFUDDLE_JS,
    );

    // Pass the JS through a temp file because it's >64KB.
    let tmp = std::env::temp_dir().join(format!("yappy-defuddle-{}.js", std::process::id()));
    std::fs::write(&tmp, &js)?;

    // Map app -> AppleScript template for "execute javascript in current tab".
    // All scripts are wrapped in `with timeout` so a hung browser doesn't block Yappy.
    let script = match app_name {
        "Safari" => format!(
            r#"with timeout of 8 seconds
set jsPath to "{path}"
set theJs to read POSIX file jsPath as «class utf8»
tell application "Safari"
  if (count of documents) is 0 then return ""
  return do JavaScript theJs in current tab of front window
end tell
end timeout
"#,
            path = tmp.display(),
        ),
        // Chromium-family: use `execute javascript`
        "Google Chrome" | "Google Chrome Canary" | "Brave Browser" | "Arc"
        | "Microsoft Edge" | "Vivaldi" => format!(
            r#"with timeout of 8 seconds
set jsPath to "{path}"
set theJs to read POSIX file jsPath as «class utf8»
tell application "{app}"
  if (count of windows) is 0 then return ""
  set ttab to active tab of front window
  return execute ttab javascript theJs
end tell
end timeout
"#,
            app = app_name,
            path = tmp.display(),
        ),
        // Firefox / Orion don't expose execute-javascript over AppleScript reliably; fall back.
        _ => return Ok(None),
    };

    let out = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()?;
    let _ = std::fs::remove_file(&tmp);

    // Surface AppleEvent timeout (-1712) or JS-disabled hints clearly.
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("-1712") {
        tracing::warn!(
            "defuddle({}): AppleEvent timed out — enable \"Allow JavaScript from Apple Events\" in the browser's Develop menu",
            app_name
        );
        return Ok(None);
    }
    if stderr.contains("JavaScript") && stderr.contains("not allowed") {
        tracing::warn!(
            "defuddle({}): JavaScript-from-Apple-Events disabled — enable it in the browser's Develop menu",
            app_name
        );
        return Ok(None);
    }

    if !out.status.success() {
        tracing::debug!(
            "defuddle osascript failed for {app_name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return Ok(None);
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return Ok(None);
    }
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            // AppleScript sometimes double-quotes the string; try once-decoded.
            let unq = raw.trim_matches('"').replace("\\\"", "\"");
            match serde_json::from_str::<serde_json::Value>(&unq) {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!("defuddle: could not parse JSON ({e}). raw_head={}", &raw[..raw.len().min(120)]);
                    return Ok(None);
                }
            }
        }
    };
    if parsed.get("error").is_some() {
        tracing::debug!("defuddle reported error: {}", parsed["error"]);
        return Ok(None);
    }
    let title = parsed.get("title").and_then(|v| v.as_str()).map(str::to_string);
    let url = parsed.get("url").and_then(|v| v.as_str()).map(str::to_string);
    let content = parsed
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let text = clean_markdown_for_speech(&content);
    if text.trim().is_empty() {
        return Ok(None);
    }
    let prefix = title.as_deref().map(|t| format!("{t}.\n\n")).unwrap_or_default();
    Ok(Some(BrowserCapture {
        text: prefix + &text,
        url,
        title,
    }))
}

#[cfg(not(target_os = "macos"))]
pub fn capture_browser_via_defuddle(_app_name: &str) -> Result<Option<BrowserCapture>> {
    Ok(None)
}

/// Fallback when defuddle injection isn't possible — synthesise Cmd+A then Cmd+C,
/// read the clipboard, restore. Works for any focused browser tab.
#[cfg(target_os = "macos")]
pub fn select_all_then_copy_in_browser(app_name: &str) -> Result<Option<String>> {
    use std::thread::sleep;
    use std::time::Duration;
    let prev = super::clipboard::snapshot().ok().flatten();
    let prev_change = super::clipboard::change_count();

    // Make sure the browser is frontmost before keystroking.
    let activate = format!(r#"tell application "{}" to activate"#, app_name);
    let _ = std::process::Command::new("osascript").args(["-e", &activate]).status();
    sleep(Duration::from_millis(160));

    // Cmd+A then Cmd+C via System Events.
    let select_copy = r#"tell application "System Events"
  keystroke "a" using {command down}
  delay 0.15
  keystroke "c" using {command down}
end tell"#;
    let _ = std::process::Command::new("osascript")
        .args(["-e", select_copy])
        .status();

    // Wait up to ~600ms for the clipboard to change.
    let deadline = std::time::Instant::now() + Duration::from_millis(700);
    let mut captured: Option<String> = None;
    while std::time::Instant::now() < deadline {
        sleep(Duration::from_millis(40));
        let now = super::clipboard::change_count();
        if now != prev_change {
            captured = super::clipboard::read_text().ok().flatten();
            break;
        }
    }
    if let Some(prev_text) = prev {
        let _ = super::clipboard::write_text(&prev_text);
    }
    Ok(captured.filter(|s| !s.trim().is_empty()))
}

#[cfg(not(target_os = "macos"))]
pub fn select_all_then_copy_in_browser(_app_name: &str) -> Result<Option<String>> {
    Ok(None)
}

/// Strip markdown noise (image refs, link targets, code fences) so the TTS reads cleanly.
fn clean_markdown_for_speech(md: &str) -> String {
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
        // Skip horizontal rules and empty image lines.
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "***" {
            continue;
        }
        // Replace markdown links [text](url) with just text.
        let cleaned = strip_md_links(line);
        // Strip leading "# ", "## ", "- ", "* " markers for readability.
        let cleaned = cleaned
            .trim_start_matches(|c: char| c == '#' || c == '>' || c == '-' || c == '*' || c.is_whitespace());
        out.push_str(cleaned);
        out.push('\n');
    }
    // Collapse 3+ blank lines.
    let mut prev_blank = false;
    let mut compact = String::with_capacity(out.len());
    for line in out.lines() {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        compact.push_str(line);
        compact.push('\n');
        prev_blank = blank;
    }
    compact.trim().to_string()
}

fn strip_md_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // image: skip up to the closing ')'
            if let Some(end) = find_link_end(&bytes[i..]) {
                i += end + 1;
                continue;
            }
        }
        if bytes[i] == b'[' {
            // try parse [text](url)
            if let Some(text_end) = bytes[i + 1..].iter().position(|&b| b == b']') {
                let after = i + 1 + text_end + 1;
                if after < bytes.len() && bytes[after] == b'(' {
                    if let Some(close) = bytes[after..].iter().position(|&b| b == b')') {
                        let text = &s[i + 1..i + 1 + text_end];
                        out.push_str(text);
                        i = after + close + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn find_link_end(slice: &[u8]) -> Option<usize> {
    // Find matching ')' assuming structure ![alt](url)
    let mut depth = 0;
    for (i, &b) in slice.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// `base64` import kept so cargo doesn't complain on unused features (we may use it later for
// passing the bundle inline rather than via a tempfile).
#[allow(dead_code)]
fn _unused() -> String {
    base64::engine::general_purpose::STANDARD.encode("noop")
}
