//! Cross-platform "copy current selection" capture.
//!
//! All three platforms use the same pattern: snapshot the clipboard, synthesise the
//! copy keystroke, read the new clipboard, and restore the previous contents.

use anyhow::Result;

// iOS apps run sandboxed and cannot synthesise a Cmd+C / Ctrl+C against
// other applications. The "capture text the user has selected in another
// app" feature simply doesn't exist on iOS — the same payload arrives via
// the Share Sheet extension instead (see mobile::pickup_shared_payload).
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn capture_selection() -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
use std::time::Duration;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use super::clipboard;

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn capture_selection() -> Result<Option<String>> {
    // On Windows we snapshot EVERY clipboard format so that HTML / RTF /
    // images the user had on the clipboard survive the Ctrl+C trick. On
    // macOS / Linux we currently still do a text-only snapshot (the
    // platforms can carry richer NSPasteboard / XCLIPBOARD targets but
    // capturing every type would need much more code; text-only matches
    // the pre-bundle behaviour and is non-regressing).
    #[cfg(target_os = "windows")]
    let full_snapshot = crate::os_win::clipboard_snapshot_all();
    #[cfg(not(target_os = "windows"))]
    let prev = clipboard::snapshot().ok().flatten();

    let prev_change = clipboard::change_count();

    if let Err(e) = send_copy_key() {
        tracing::debug!("send_copy_key failed: {e:?}");
        #[cfg(target_os = "windows")]
        crate::os_win::clipboard_restore_all(&full_snapshot);
        return Ok(None);
    }

    let deadline = std::time::Instant::now() + Duration::from_millis(380);
    let mut captured: Option<String> = None;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
        let now = clipboard::change_count();
        if now != prev_change {
            captured = clipboard::read_text().ok().flatten();
            break;
        }
    }

    #[cfg(target_os = "windows")]
    crate::os_win::clipboard_restore_all(&full_snapshot);
    #[cfg(not(target_os = "windows"))]
    if let Some(prev_text) = prev {
        let _ = clipboard::write_text(&prev_text);
    }

    Ok(captured.filter(|s| !s.trim().is_empty()))
}

#[cfg(target_os = "macos")]
fn send_copy_key() -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| anyhow::anyhow!("CGEventSource"))?;
    let c_key: CGKeyCode = 8; // virtual keycode for 'C' on US layout
    let cmd = CGEventFlags::CGEventFlagCommand;
    let down = CGEvent::new_keyboard_event(source.clone(), c_key, true)
        .map_err(|_| anyhow::anyhow!("CGEvent down"))?;
    down.set_flags(cmd);
    let up = CGEvent::new_keyboard_event(source, c_key, false)
        .map_err(|_| anyhow::anyhow!("CGEvent up"))?;
    up.set_flags(cmd);
    down.post(CGEventTapLocation::HID);
    up.post(CGEventTapLocation::HID);
    Ok(())
}

#[cfg(target_os = "linux")]
fn send_copy_key() -> Result<()> {
    // Try wtype (wayland) → ydotool → xdotool.
    for tool in ["wtype", "ydotool", "xdotool"] {
        if which(tool).is_some() {
            let args: &[&str] = match tool {
                "xdotool" => &["key", "ctrl+c"],
                "wtype" => &["-M", "ctrl", "c", "-m", "ctrl"],
                "ydotool" => &["key", "29:1", "46:1", "46:0", "29:0"], // ctrl+c down/up
                _ => unreachable!(),
            };
            let res = std::process::Command::new(tool).args(args).output();
            if let Ok(out) = res {
                if out.status.success() {
                    return Ok(());
                }
            }
        }
    }
    Err(anyhow::anyhow!(
        "no key-sender available — install wtype (wayland) or xdotool (x11)"
    ))
}

#[cfg(target_os = "windows")]
fn send_copy_key() -> Result<()> {
    // Native SendInput via the `windows` crate — no PowerShell flash,
    // no process spawn, sub-millisecond. See os_win.rs::send_ctrl_c.
    if !crate::os_win::send_ctrl_c() {
        return Err(anyhow::anyhow!("SendInput Ctrl+C returned 0 events"));
    }
    Ok(())
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
