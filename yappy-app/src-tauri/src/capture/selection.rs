//! Cross-platform "copy current selection" capture.
//!
//! All three platforms use the same pattern: snapshot the clipboard, synthesise the
//! copy keystroke, read the new clipboard, and restore the previous contents.

use anyhow::Result;

// iOS apps run sandboxed and cannot synthesise a Cmd+C / Ctrl+C against
// other applications. The "capture text the user has selected in another
// app" feature simply doesn't exist on iOS — the same payload arrives via
// the Share Sheet extension instead (see mobile::pickup_shared_payload).
#[cfg(target_os = "ios")]
pub fn capture_selection() -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(target_os = "ios"))]
use std::time::Duration;
#[cfg(not(target_os = "ios"))]
use super::clipboard;

#[cfg(not(target_os = "ios"))]
pub fn capture_selection() -> Result<Option<String>> {
    let prev = clipboard::snapshot().ok().flatten();
    let prev_change = clipboard::change_count();

    if let Err(e) = send_copy_key() {
        tracing::debug!("send_copy_key failed: {e:?}");
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
    let ps = r#"
$wshell = New-Object -ComObject wscript.shell;
$wshell.SendKeys("^c")
"#;
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", ps])
        .output()?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "powershell SendKeys ^c failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
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
