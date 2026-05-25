//! Cross-platform clipboard read / write / change-count.
//!
//! macOS:   NSPasteboard (with the real macOS change-count integer)
//! Linux:   xclip / xsel / wl-paste (whichever is present)
//! Windows: PowerShell Get-Clipboard / Set-Clipboard
//!
//! `change_count` is used to tell whether a synthetic ⌘C / Ctrl+C produced new contents.

use anyhow::Result;

// ---------- macOS ----------

#[cfg(target_os = "macos")]
pub fn read_text() -> Result<Option<String>> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        let nstype = NSString::from_str("public.utf8-plain-text");
        if let Some(s) = pb.stringForType(&nstype) {
            return Ok(Some(s.to_string()));
        }
        let nstype2 = NSString::from_str("NSStringPboardType");
        if let Some(s) = pb.stringForType(&nstype2) {
            return Ok(Some(s.to_string()));
        }
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
pub fn write_text(text: &str) -> Result<()> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let ns = NSString::from_str(text);
        let ty = NSString::from_str("public.utf8-plain-text");
        let _ = pb.setString_forType(&ns, &ty);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn snapshot() -> Result<Option<String>> { read_text() }

#[cfg(target_os = "macos")]
pub fn change_count() -> i64 {
    use objc2_app_kit::NSPasteboard;
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.changeCount() as i64
    }
}

// ---------- Linux ----------

#[cfg(target_os = "linux")]
pub fn read_text() -> Result<Option<String>> {
    for (tool, args) in [
        ("wl-paste", &["--no-newline"][..]),
        ("xclip", &["-o", "-selection", "clipboard"]),
        ("xsel", &["--output", "--clipboard"]),
    ] {
        if let Some(_) = which(tool) {
            let out = std::process::Command::new(tool).args(args).output().ok();
            if let Some(out) = out {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).to_string();
                    return Ok(if s.is_empty() { None } else { Some(s) });
                }
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "linux")]
pub fn write_text(text: &str) -> Result<()> {
    use std::io::Write;
    for (tool, args) in [
        ("wl-copy", &[][..]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--input", "--clipboard"]),
    ] {
        if which(tool).is_some() {
            let mut child = std::process::Command::new(tool)
                .args(args)
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()?;
            return Ok(());
        }
    }
    Err(anyhow::anyhow!(
        "no clipboard tool available — install wl-clipboard (wayland), xclip, or xsel"
    ))
}

#[cfg(target_os = "linux")]
pub fn snapshot() -> Result<Option<String>> { read_text() }

#[cfg(target_os = "linux")]
pub fn change_count() -> i64 {
    // Linux clipboards don't expose a change-count; use a hash of current contents.
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    read_text().ok().flatten().unwrap_or_default().hash(&mut h);
    h.finish() as i64
}

#[cfg(target_os = "linux")]
fn which(name: &str) -> Option<std::path::PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for d in std::env::split_paths(&path_env) {
        let c = d.join(name);
        if c.exists() {
            return Some(c);
        }
    }
    None
}

// ---------- Windows ----------

#[cfg(target_os = "windows")]
pub fn read_text() -> Result<Option<String>> {
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", "Get-Clipboard -Raw"])
        .output()?;
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    Ok(if s.is_empty() { None } else { Some(s) })
}

#[cfg(target_os = "windows")]
pub fn write_text(text: &str) -> Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", "Set-Clipboard -Value $input"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    child.wait()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn snapshot() -> Result<Option<String>> { read_text() }

#[cfg(target_os = "windows")]
pub fn change_count() -> i64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    read_text().ok().flatten().unwrap_or_default().hash(&mut h);
    h.finish() as i64
}

// ---------- iOS ----------
//
// UIPasteboard requires UIKit linkage and triggers a "X pasted from Y" banner
// on every read on iOS 14+. Phase-1 stubs return empty; Phase 4 wires
// `tauri-plugin-clipboard-manager` (which uses UIPasteboard under the hood)
// for the explicit "Read clipboard" command only — we deliberately avoid
// the change-count polling that the desktop code does.

#[cfg(target_os = "ios")]
pub fn read_text() -> Result<Option<String>> {
    Ok(None)
}
#[cfg(target_os = "ios")]
pub fn write_text(_text: &str) -> Result<()> {
    Ok(())
}
#[cfg(target_os = "ios")]
pub fn snapshot() -> Result<Option<String>> {
    Ok(None)
}
#[cfg(target_os = "ios")]
pub fn change_count() -> i64 {
    0
}
