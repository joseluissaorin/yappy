//! Active-document extraction per known app, via AppleScript.

use anyhow::Result;

pub fn doc_kind_for(app_name: &str) -> &'static str {
    match app_name {
        "Safari" | "Google Chrome" | "Google Chrome Canary" | "Brave Browser" | "Arc"
        | "Microsoft Edge" | "Vivaldi" | "Firefox" | "Orion" => "browser",
        "Preview" | "Adobe Acrobat" | "Adobe Reader" | "Skim" | "PDF Expert" => "pdf",
        "TextEdit" | "Notes" | "Pages" | "Microsoft Word" | "Ulysses" | "iA Writer"
        | "Bear" | "Obsidian" | "Typora" | "MarkdownEdit" => "editor",
        _ => "document",
    }
}

/// Returns the visible text of the currently focused app's frontmost document.
pub fn active_document_text(app_name: &str) -> Result<Option<String>> {
    let script = match app_name {
        "Safari" => Some(SAFARI),
        "Google Chrome" | "Google Chrome Canary" | "Brave Browser" | "Arc"
        | "Microsoft Edge" | "Vivaldi" => Some(CHROME),
        "Firefox" => Some(FIREFOX),
        "TextEdit" => Some(TEXTEDIT),
        "Notes" => Some(NOTES),
        "Pages" => Some(PAGES),
        "Microsoft Word" => Some(WORD),
        "Ulysses" => Some(ULYSSES),
        "iA Writer" => Some(IA_WRITER),
        "Bear" => Some(BEAR),
        "Obsidian" => Some(OBSIDIAN),
        "Preview" => Some(PREVIEW),
        _ => None,
    };
    let Some(script) = script else {
        return Ok(None);
    };
    let out = std::process::Command::new("osascript")
        .args(["-e", script])
        .output()?;
    if !out.status.success() {
        tracing::debug!(
            "active_document_text({app_name}) osascript stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

// --- Browser fallbacks (used only when defuddle injection fails) ---

const SAFARI: &str = r#"
tell application "Safari"
  if (count of documents) is 0 then return ""
  do JavaScript "document.body && document.body.innerText || ''" in current tab of front window
end tell
"#;

const CHROME: &str = r#"
tell application "Google Chrome"
  if (count of windows) is 0 then return ""
  set ttab to active tab of front window
  execute ttab javascript "document.body && document.body.innerText || ''"
end tell
"#;

const FIREFOX: &str = r#"
tell application "Firefox" to activate
delay 0.2
tell application "System Events"
  keystroke "a" using {command down}
  delay 0.1
  keystroke "c" using {command down}
end tell
delay 0.2
return (the clipboard as text)
"#;

const TEXTEDIT: &str = r#"
tell application "TextEdit"
  if (count of documents) is 0 then return ""
  get text of front document
end tell
"#;

const NOTES: &str = r#"
tell application "Notes"
  if (count of notes) is 0 then return ""
  set theSel to selection
  if (count of theSel) > 0 then
    set theNote to first item of theSel
  else
    set theNote to first note
  end if
  return plaintext of theNote
end tell
"#;

const PAGES: &str = r#"
tell application "Pages"
  if (count of documents) is 0 then return ""
  get body text of front document
end tell
"#;

const WORD: &str = r#"
tell application "Microsoft Word"
  if (count of documents) is 0 then return ""
  get content of text object of active document
end tell
"#;

// Ulysses — robust extraction.
// Strategy:
//   1) If exactly one sheet is selected → return its text.
//   2) Otherwise try `front document` → text of all selected sheets concatenated.
//   3) Last resort: try the title + content of the focused sheet.
const ULYSSES: &str = r#"
tell application "Ulysses"
  try
    set sel to selected sheets
    if (count of sel) is 1 then
      return text of item 1 of sel
    end if
    if (count of sel) > 1 then
      set out to ""
      repeat with s in sel
        set out to out & (text of s) & linefeed & linefeed
      end repeat
      return out
    end if
  end try
  -- Fall back to focused sheet of front window via the title path.
  try
    return text of first sheet of first group
  end try
  return ""
end tell
"#;

const IA_WRITER: &str = r#"
tell application "iA Writer"
  if (count of documents) is 0 then return ""
  return text of front document
end tell
"#;

const BEAR: &str = r#"
tell application "Bear" to activate
delay 0.15
tell application "System Events"
  keystroke "a" using {command down}
  delay 0.1
  keystroke "c" using {command down}
end tell
delay 0.15
return (the clipboard as text)
"#;

const OBSIDIAN: &str = r#"
tell application "Obsidian" to activate
delay 0.15
tell application "System Events"
  keystroke "a" using {command down}
  delay 0.1
  keystroke "c" using {command down}
end tell
delay 0.15
return (the clipboard as text)
"#;

const PREVIEW: &str = r#"
tell application "Preview"
  if (count of documents) is 0 then return ""
  set thePath to path of front document
end tell
do shell script "/usr/bin/mdls -name kMDItemTextContent -raw " & quoted form of thePath
"#;
