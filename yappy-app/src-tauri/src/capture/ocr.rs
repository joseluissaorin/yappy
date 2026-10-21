//! OCR de capturas, imágenes y PDF escaneados.
//!
//! `Auto` (por omisión): Amanuensis Ligero, el OCR propio (capture/amanuensis.rs), con Apple Vision de
//! respaldo en macOS e iOS para lo que Ligero no lee con seguridad (otras escrituras: Ligero solo conoce el
//! alfabeto latino). `Amanuensis`: solo Ligero. `AppleVision`: solo Vision (macOS; en el resto, `Auto`).
//! PaddleOCR se quitó el 21-10-2026 (su modelo era el chino, sin tildes; los ajustes que lo guardaban leen
//! ahora `Auto`).

// En el móvil no hay captura de pantalla ni motor elegible: el OCR entra por capture/amanuensis.rs.
#![cfg_attr(any(target_os = "ios", target_os = "android"), allow(dead_code))]

use std::path::Path;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::Result;

use crate::settings::OcrEngine;

// ---------- screen capture (focused window or whole screen) ----------

#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn screen_ocr_focused() -> Result<String> {
    // iOS apps cannot capture other apps' screens — the OS forbids it without
    // a Broadcast Upload Extension and a private entitlement Apple doesn't
    // grant for general use. The frontend hides this entrypoint on mobile;
    // returning an explicit error so anything that slips through fails loud.
    Err(anyhow::anyhow!(
        "screen capture is not available on iOS; share text or a URL via the Share Sheet instead"
    ))
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn screen_ocr_focused() -> Result<String> {
    let tmp = std::env::temp_dir().join(format!(
        "yappy-screen-{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    capture_focused_to(&tmp)?;
    let text = ocr_image(&tmp, "en-US")?;
    let _ = std::fs::remove_file(&tmp);
    Ok(text)
}

#[cfg(target_os = "macos")]
fn capture_focused_to(out: &Path) -> Result<()> {
    let bounds = focused_window_bounds();
    if let Some((x, y, w, h)) = bounds {
        let res = std::process::Command::new("screencapture")
            .args([
                "-x",
                "-R",
                &format!("{x},{y},{w},{h}"),
                out.to_str().unwrap(),
            ])
            .output()?;
        if res.status.success() && out.exists() {
            return Ok(());
        }
    }
    full_screen_capture(out)
}

#[cfg(target_os = "linux")]
fn capture_focused_to(out: &Path) -> Result<()> {
    // Try `grim` first (wayland), then `scrot` (x11).
    for cmd in &["grim", "scrot"] {
        if which(cmd).is_some() {
            let res = std::process::Command::new(cmd).arg(out).output();
            if let Ok(out) = res {
                if out.status.success() {
                    return Ok(());
                }
            }
        }
    }
    Err(anyhow::anyhow!(
        "no screen-capture tool available; install grim (wayland) or scrot (x11)"
    ))
}

#[cfg(target_os = "windows")]
fn capture_focused_to(out: &Path) -> Result<()> {
    // Native GDI BitBlt — no PowerShell process spawn, no .NET startup,
    // no console window at all (not even briefly). Captures the
    // foreground window's bounds for tighter OCR (matching macOS); falls
    // back to the full virtual screen if no window rect is available.
    // See os_win.rs::capture_foreground_window_png.
    crate::os_win::capture_foreground_window_png(out)
}

#[cfg(target_os = "macos")]
fn focused_window_bounds() -> Option<(i64, i64, i64, i64)> {
    let script = r#"
tell application "System Events"
  set frontApp to first application process whose frontmost is true
  if (count of windows of frontApp) is 0 then return ""
  set w to first window of frontApp
  set p to position of w
  set s to size of w
  return (item 1 of p as text) & "," & (item 2 of p as text) & "," & (item 1 of s as text) & "," & (item 2 of s as text)
end tell
"#;
    let out = std::process::Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let x = parts[0].trim().parse().ok()?;
    let y = parts[1].trim().parse().ok()?;
    let w = parts[2].trim().parse().ok()?;
    let h = parts[3].trim().parse().ok()?;
    if w <= 0 || h <= 0 {
        return None;
    }
    Some((x, y, w, h))
}

#[cfg(target_os = "macos")]
fn full_screen_capture(out: &Path) -> Result<()> {
    let res = std::process::Command::new("screencapture")
        .args(["-x", out.to_str().unwrap()])
        .output()?;
    if !res.status.success() {
        return Err(anyhow::anyhow!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn full_screen_capture(_out: &Path) -> Result<()> {
    Err(anyhow::anyhow!(
        "full-screen capture not implemented for this platform"
    ))
}

// ---------- OCR engine selection ----------

pub fn ocr_image(image_path: &Path, _lang: &str) -> Result<String> {
    match active_engine() {
        OcrEngine::Amanuensis => {
            crate::capture::amanuensis::leer_ruta_solo_ligero(image_path).map(|p| p.texto())
        }
        #[cfg(target_os = "macos")]
        OcrEngine::AppleVision => {
            crate::capture::amanuensis::leer_ruta_solo_vision(image_path).map(|p| p.texto())
        }
        _ => crate::capture::amanuensis::leer_ruta(image_path).map(|p| p.texto()),
    }
}

fn active_engine() -> OcrEngine {
    // Read settings from disk once. We don't have access to the AppState here without
    // making this stateful; settings.json is cheap to read.
    static CACHED: OnceLock<OcrEngine> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let path = dirs::config_dir()
            .map(|p| p.join("com.yappy.app/settings.json"))
            .unwrap_or_default();
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(e) = v.get("ocr_engine").and_then(|x| x.as_str()) {
                    return match e {
                        "applevision" | "apple-vision" | "apple_vision" => OcrEngine::AppleVision,
                        "amanuensis" => OcrEngine::Amanuensis,
                        _ => OcrEngine::Auto,
                    };
                }
            }
        }
        OcrEngine::Auto
    })
}

// ---------- Apple Vision (macOS only) ----------

/// Renglones de Vision con su caja, en JSON (`[{"t", "c", "x0", "y0", "x1", "y1"}]`, px con origen arriba a
/// la izquierda): el respaldo de Amanuensis rehace con ellos los párrafos.
#[cfg(target_os = "macos")]
pub fn vision_lineas_json(image_path: &Path) -> Result<String> {
    let helper_path = find_helper_script()?;
    let out = std::process::Command::new("swift")
        .arg(helper_path)
        .arg(image_path)
        .arg("es-ES")
        .arg("json")
        .output()
        .context("swift command failed (Xcode CLT may not be installed)")?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "vision_ocr exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(target_os = "macos")]
fn find_helper_script() -> Result<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let candidates = [
            exe.parent().unwrap().join("yappy-ocr.swift"),
            exe.parent()
                .unwrap()
                .parent()
                .map(|p| p.join("Resources/yappy-ocr.swift"))
                .unwrap_or_default(),
        ];
        for c in candidates.iter() {
            if c.exists() {
                return Ok(c.clone());
            }
        }
    }
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|d| d.join("macos/yappy-ocr.swift")),
        std::env::current_dir()
            .ok()
            .map(|d| d.join("yappy-app/macos/yappy-ocr.swift")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.exists() {
            return Ok(c);
        }
    }
    Err(anyhow::anyhow!(
        "could not locate yappy-ocr.swift helper script"
    ))
}

#[allow(dead_code)]
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
