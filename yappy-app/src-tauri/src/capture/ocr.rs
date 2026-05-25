//! Screen OCR — picks the best engine for the platform.
//!
//! macOS  → Apple Vision (via the bundled `yappy-ocr.swift` helper)
//! Other  → PaddleOCR via the bundled ONNX models
//! `Auto` setting on macOS uses Vision; everywhere else it uses PaddleOCR.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};

use crate::settings::OcrEngine;

// ---------- screen capture (focused window or whole screen) ----------

#[cfg(target_os = "ios")]
pub fn screen_ocr_focused() -> Result<String> {
    // iOS apps cannot capture other apps' screens — the OS forbids it without
    // a Broadcast Upload Extension and a private entitlement Apple doesn't
    // grant for general use. The frontend hides this entrypoint on mobile;
    // returning an explicit error so anything that slips through fails loud.
    Err(anyhow::anyhow!(
        "screen capture is not available on iOS; share text or a URL via the Share Sheet instead"
    ))
}

#[cfg(not(target_os = "ios"))]
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
            .args(["-x", "-R", &format!("{x},{y},{w},{h}"), out.to_str().unwrap()])
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
    // PowerShell-based screen capture using .NET's System.Drawing API.
    // The clipboard + Ctrl+C paths got replaced with native Win32 calls
    // (no console flash) but screen capture would need GDI BitBlt +
    // GdiplusBitmap which is substantially more Rust unsafe than just
    // hiding the PowerShell window. CREATE_NO_WINDOW does that.
    use std::os::windows::process::CommandExt;
    /// Equivalent to winapi's CREATE_NO_WINDOW. Suppresses console window
    /// creation for the spawned PowerShell process so users don't see a
    /// flash on Ctrl+Alt+R → OCR fallback.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let ps = format!(
        r#"Add-Type -AssemblyName System.Drawing
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
$bmp.Save("{}", [System.Drawing.Imaging.ImageFormat]::Png)"#,
        out.display()
    );
    let res = std::process::Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .output()?;
    if res.status.success() && out.exists() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "windows screen capture failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ))
    }
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
    let out = std::process::Command::new("osascript").args(["-e", script]).output().ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 { return None; }
    let x = parts[0].trim().parse().ok()?;
    let y = parts[1].trim().parse().ok()?;
    let w = parts[2].trim().parse().ok()?;
    let h = parts[3].trim().parse().ok()?;
    if w <= 0 || h <= 0 { return None; }
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
    Err(anyhow::anyhow!("full-screen capture not implemented for this platform"))
}

// ---------- OCR engine selection ----------

pub fn ocr_image(image_path: &Path, lang: &str) -> Result<String> {
    let engine = active_engine();
    match engine {
        OcrEngine::AppleVision => {
            #[cfg(target_os = "macos")]
            {
                return vision_ocr(image_path, lang);
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Fall through to PaddleOCR.
            }
        }
        OcrEngine::Paddle => return paddle_ocr(image_path),
        OcrEngine::Auto => {
            #[cfg(target_os = "macos")]
            {
                if let Ok(t) = vision_ocr(image_path, lang) {
                    return Ok(t);
                }
            }
            return paddle_ocr(image_path);
        }
    }
    // fallback
    paddle_ocr(image_path)
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
                        "paddle" => OcrEngine::Paddle,
                        _ => OcrEngine::Auto,
                    };
                }
            }
        }
        OcrEngine::Auto
    })
}

// ---------- Apple Vision (macOS only) ----------

#[cfg(target_os = "macos")]
pub fn vision_ocr(image_path: &Path, lang: &str) -> Result<String> {
    let helper_path = find_helper_script()?;
    let out = std::process::Command::new("swift")
        .arg(helper_path)
        .arg(image_path)
        .arg(lang)
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
        std::env::current_dir().ok().map(|d| d.join("macos/yappy-ocr.swift")),
        std::env::current_dir().ok().map(|d| d.join("yappy-app/macos/yappy-ocr.swift")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.exists() {
            return Ok(c);
        }
    }
    Err(anyhow::anyhow!("could not locate yappy-ocr.swift helper script"))
}

#[cfg(not(target_os = "macos"))]
pub fn vision_ocr(_image_path: &Path, _lang: &str) -> Result<String> {
    Err(anyhow::anyhow!("Apple Vision only available on macOS"))
}

// ---------- PaddleOCR (cross-platform) ----------

pub fn paddle_ocr(image_path: &Path) -> Result<String> {
    use paddle_ocr_rs::ocr_lite::OcrLite;

    let dir = paddle_models_dir()?;
    let det = dir.join("ch_PP-OCRv4_det_infer.onnx");
    let cls = dir.join("ch_ppocr_mobile_v2.0_cls_infer.onnx");
    let rec = dir.join("ch_PP-OCRv4_rec_infer.onnx");
    let keys = dir.join("ppocr_keys_v1.txt");

    if !det.exists() || !cls.exists() || !rec.exists() || !keys.exists() {
        return Err(anyhow::anyhow!(
            "PaddleOCR models not found at {} — please reinstall Yappy",
            dir.display()
        ));
    }

    let mut ocr = OcrLite::new();
    ocr.init_models(
        det.to_str().unwrap(),
        cls.to_str().unwrap(),
        rec.to_str().unwrap(),
        1,
    )
    .map_err(|e| anyhow::anyhow!("paddle init: {e:?}"))?;
    let _ = keys;
    let img = image::open(image_path)?.to_rgb8();
    let results = ocr
        .detect(&img, 50, 1024, 0.5, 0.3, 1.6, true, false)
        .map_err(|e| anyhow::anyhow!("paddle detect: {e:?}"))?;
    // Re-order blocks into reading order: top→bottom rows, then left→right within each row.
    Ok(reorder_blocks_to_text(&results.text_blocks))
}

fn reorder_blocks_to_text(blocks: &[paddle_ocr_rs::ocr_result::TextBlock]) -> String {
    // Collect each block with its centroid y and left-x.
    let mut items: Vec<(f32, f32, &str)> = blocks
        .iter()
        .map(|b| {
            let ys: Vec<i32> = b.box_points.iter().map(|p| p.y as i32).collect();
            let xs: Vec<i32> = b.box_points.iter().map(|p| p.x as i32).collect();
            let cy = (ys.iter().sum::<i32>() as f32) / (ys.len().max(1) as f32);
            let cx = (xs.iter().min().copied().unwrap_or(0) as f32);
            (cy, cx, b.text.as_str())
        })
        .collect();
    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Cluster into rows: y within ~half-line-height tolerance.
    if items.is_empty() { return String::new(); }
    let mut rows: Vec<Vec<(f32, f32, &str)>> = vec![vec![items[0]]];
    let mut last_y = items[0].0;
    let line_h = estimate_line_height(blocks);
    let tol = (line_h * 0.6).max(8.0);
    for it in items.into_iter().skip(1) {
        if (it.0 - last_y).abs() <= tol {
            rows.last_mut().unwrap().push(it);
        } else {
            rows.push(vec![it]);
            last_y = it.0;
        }
    }
    let mut buf = String::new();
    for row in rows.iter_mut() {
        row.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let line: Vec<&str> = row.iter().map(|(_, _, t)| *t).collect();
        buf.push_str(&line.join(" "));
        buf.push('\n');
    }
    buf
}

fn estimate_line_height(blocks: &[paddle_ocr_rs::ocr_result::TextBlock]) -> f32 {
    let mut hs: Vec<f32> = blocks
        .iter()
        .map(|b| {
            let ys: Vec<i32> = b.box_points.iter().map(|p| p.y as i32).collect();
            let max = ys.iter().max().copied().unwrap_or(0) as f32;
            let min = ys.iter().min().copied().unwrap_or(0) as f32;
            max - min
        })
        .collect();
    if hs.is_empty() { return 12.0; }
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    hs[hs.len() / 2]
}

fn paddle_models_dir() -> Result<PathBuf> {
    // First, look next to the running binary (Tauri bundles resources here in production).
    if let Ok(exe) = std::env::current_exe() {
        let candidates = [
            exe.parent().map(|p| p.join("paddleocr")),
            exe.parent().and_then(|p| p.parent()).map(|p| p.join("Resources/paddleocr")),
            exe.parent().and_then(|p| p.parent()).map(|p| p.join("Resources/_up_/resources/paddleocr")),
        ];
        for c in candidates.into_iter().flatten() {
            if c.join("ch_PP-OCRv4_det_infer.onnx").exists() {
                return Ok(c);
            }
        }
    }
    // Dev: from the workspace.
    let cwd = std::env::current_dir()?;
    for sub in &[
        "yappy-app/resources/paddleocr",
        "resources/paddleocr",
        "../yappy-app/resources/paddleocr",
        "../../yappy-app/resources/paddleocr",
    ] {
        let p = cwd.join(sub);
        if p.join("ch_PP-OCRv4_det_infer.onnx").exists() {
            return Ok(p);
        }
    }
    Err(anyhow::anyhow!("PaddleOCR models not found"))
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
