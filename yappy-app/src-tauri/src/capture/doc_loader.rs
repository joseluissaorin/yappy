//! Load a document file (.txt .md .rtf .docx .pptx .xlsx .odt .pdf .html .epub) to plain text.
//!
//! Strategy, in order:
//!   1. If `pandoc` is on PATH, use it for any office-y format (best quality).
//!   2. Otherwise, pure-Rust crates:
//!         .txt / .md / .markdown / .csv / .log  → fs::read_to_string + light markdown stripping
//!         .docx / .pptx / .xlsx / .html         → anytomd
//!         .pdf                                  → pdf-extract; if empty → rasterise + OCR
//!         .rtf                                  → strip RTF control words (lightweight)
//!         .epub                                 → epub crate, chapters joined
//!         .odt                                  → unzip + parse content.xml
//!
//! Pandoc is treated as the "premium" path because it understands footnotes, formulas,
//! footers, etc. better than any individual Rust parser.

use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Load a document AND return reading-rhythm paragraphs when possible.
/// For .md (and .pdf via pdf_oxide's markdown output) we parse structure so
/// headings, lists, and rule breaks get appropriate pauses. For everything else
/// we fall back to plain paragraph splitting (text only, default pauses).
pub fn load_rich_from_file(path: &Path) -> Result<Vec<RichParagraph>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // For native markdown files: parse structure directly.
    if ext == "md" || ext == "markdown" {
        let raw = read_utf8(path)?;
        return Ok(parse_markdown_rhythm(&raw));
    }
    // For PDFs: pdf_oxide outputs proper markdown with headings, so route through
    // the markdown rhythm parser. Falls back to plain text if to_markdown fails.
    if ext == "pdf" {
        if let Ok(md) = pdf_oxide_to_markdown(path) {
            if md.trim().chars().count() > 40 {
                return Ok(parse_markdown_rhythm(&md));
            }
        }
    }

    // Default: load plain text and split by blank lines.
    let text = load_text_from_file(path)?;
    let paragraphs = text
        .split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|t| RichParagraph {
            text: t,
            pause_before: 0.0,
            speed_mult: 1.0,
            kind: "paragraph".into(),
        })
        .collect();
    Ok(paragraphs)
}

/// pdf_oxide's `to_markdown` per-page, joined. Used to feed PDFs through
/// the markdown rhythm parser so headings get pauses.
fn pdf_oxide_to_markdown(path: &Path) -> Result<String> {
    use pdf_oxide::converters::ConversionOptions;
    let mut doc = pdf_oxide::PdfDocument::open(path).map_err(|e| anyhow!("pdf_oxide open: {e:?}"))?;
    let pages = doc.page_count().map_err(|e| anyhow!("pdf_oxide page_count: {e:?}"))?;
    let opts = ConversionOptions::default();
    let mut buf = String::with_capacity(pages * 1024);
    for i in 0..pages {
        if let Ok(md) = doc.to_markdown(i, &opts) {
            buf.push_str(md.trim());
            buf.push_str("\n\n");
        }
    }
    Ok(buf)
}

pub fn load_text_from_file(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // 1) Pandoc when available — covers nearly every text format.
    if PANDOC_SUPPORTED.contains(&ext.as_str()) {
        if let Some(pd) = which("pandoc") {
            tracing::debug!("doc_loader: using pandoc for .{ext}");
            return pandoc_to_text(&pd, path);
        }
    }

    match ext.as_str() {
        "txt" | "log" | "csv" => Ok(read_utf8(path)?),
        "md" | "markdown" => Ok(strip_md_markup(&read_utf8(path)?)),
        "html" | "htm" | "xml" => {
            // anytomd handles HTML well.
            anytomd_convert(path, &ext)
        }
        "docx" | "pptx" | "xlsx" => anytomd_convert(path, &ext),
        "odt" => odt_to_text(path),
        "rtf" => Ok(strip_rtf(&read_utf8(path)?)),
        "pdf" => pdf_to_text(path),
        "epub" => epub_to_text(path),
        other => Err(anyhow!("unsupported document extension: .{}", other)),
    }
}

const PANDOC_SUPPORTED: &[&str] = &[
    "docx", "doc", "rtf", "odt", "html", "htm", "epub", "tex", "latex", "rst", "org",
    "fb2", "djvu", "pptx",
];

fn read_utf8(path: &Path) -> Result<String> {
    // Read as bytes so we can be tolerant of encodings + BOMs.
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // Strip UTF-8 BOM if present (some editors add it to .md files; the BOM
    // character renders as a stray glyph otherwise).
    let bytes = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { &bytes[..] };
    // Try strict UTF-8 first. If that fails, fall back to UTF-8 with replacement
    // characters (best-effort) and log a warning. Most modern editors save UTF-8
    // so the fallback is rare — but legacy Latin-1 files would otherwise hard-fail.
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            tracing::warn!(
                "read_utf8: {} is not valid UTF-8; decoding as windows-1252",
                path.display()
            );
            // windows-1252 covers Latin-1 + the European code-page extensions.
            // We use the `encoding_rs` crate which is already a dep.
            let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            Ok(cow.into_owned())
        }
    }
}

fn pandoc_to_text(pandoc: &std::path::Path, path: &Path) -> Result<String> {
    // Convert to plain text (strip formatting). Markdown gives us nicer reading flow than -t plain
    // but markers are noisy for TTS; -t plain is best.
    let out = std::process::Command::new(pandoc)
        .args(["-t", "plain", "--wrap=none"])
        .arg(path)
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "pandoc failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn anytomd_convert(path: &Path, _ext: &str) -> Result<String> {
    use anytomd::{convert_file, ConversionOptions};
    let opts = ConversionOptions::default();
    let res = convert_file(path.to_str().unwrap(), &opts)
        .map_err(|e| anyhow!("anytomd: {e:?}"))?;
    Ok(strip_md_markup(&res.markdown))
}

fn odt_to_text(path: &Path) -> Result<String> {
    use std::io::Read;
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let mut entry = z.by_name("content.xml").context(".odt missing content.xml")?;
    let mut xml = String::new();
    entry.read_to_string(&mut xml)?;
    Ok(strip_xml(&xml))
}

fn pdf_to_text(path: &Path) -> Result<String> {
    // Stage 1: pdf_oxide for digital PDFs. ~0.8ms mean on 3,830-PDF corpus, 5×
    // faster than pdf-extract, and emits real markdown (headings preserved).
    tracing::info!("pdf_to_text: opening with pdf_oxide: {}", path.display());
    let oxide_text = pdf_oxide_extract(path)
        .map_err(|e| tracing::warn!("pdf_oxide failed: {e:?}"))
        .unwrap_or_default();
    if oxide_text.trim().chars().count() > 40 {
        tracing::info!(
            "pdf_to_text: pdf_oxide extracted {} chars",
            oxide_text.trim().chars().count()
        );
        return Ok(oxide_text);
    }

    // Stage 2: scanned PDF → pdfium rasterizes each page, PaddleOCR transcribes.
    // Both libpdfium.dylib and the PaddleOCR ONNX models are bundled in the .app
    // — no system deps required. iOS doesn't bundle the pdfium XCFramework
    // yet, so for now scanned-PDF OCR falls through to an error on iOS;
    // born-digital PDFs still work via pdf_oxide above.
    tracing::info!("pdf_to_text: no text from pdf_oxide; rasterizing via pdfium for OCR");
    #[cfg(not(target_os = "ios"))]
    return pdf_to_text_via_pdfium_ocr(path);
    #[cfg(target_os = "ios")]
    return Err(anyhow!(
        "scanned PDF OCR is not yet available on iOS — pdfium XCFramework bundling pending"
    ));
}

fn pdf_oxide_extract(path: &Path) -> Result<String> {
    use pdf_oxide::converters::ConversionOptions;
    let t0 = std::time::Instant::now();
    tracing::info!("pdf_oxide_extract: opening {}", path.display());
    let mut doc = pdf_oxide::PdfDocument::open(path).map_err(|e| anyhow!("pdf_oxide open: {e:?}"))?;
    tracing::info!("pdf_oxide_extract: open in {:?}", t0.elapsed());

    let pages = doc.page_count().map_err(|e| anyhow!("pdf_oxide page_count: {e:?}"))?;
    tracing::info!("pdf_oxide_extract: {pages} pages");

    let mut buf = String::with_capacity(pages * 1024);
    let opts = ConversionOptions::default();
    // Prefer extract_text — it's the fastest path. to_markdown adds parsing overhead
    // and was producing minutes-long stalls for some users; the editor splits paragraphs
    // by double-newline regardless of source.
    for i in 0..pages {
        let p_t0 = std::time::Instant::now();
        let result = doc.extract_text(i);
        match result {
            Ok(t) => {
                tracing::info!(
                    "pdf_oxide_extract: page {}/{} extract_text in {:?} ({} chars)",
                    i + 1,
                    pages,
                    p_t0.elapsed(),
                    t.chars().count()
                );
                buf.push_str(t.trim());
                buf.push_str("\n\n");
            }
            Err(e) => {
                tracing::warn!(
                    "pdf_oxide_extract: page {}/{} failed in {:?}: {e:?} — trying to_markdown",
                    i + 1,
                    pages,
                    p_t0.elapsed()
                );
                if let Ok(md) = doc.to_markdown(i, &opts) {
                    buf.push_str(md.trim());
                    buf.push_str("\n\n");
                }
            }
        }
    }
    tracing::info!(
        "pdf_oxide_extract: done in {:?}, total {} chars",
        t0.elapsed(),
        buf.chars().count()
    );
    Ok(buf)
}

#[cfg(not(target_os = "ios"))]
fn pdf_to_text_via_pdfium_ocr(path: &Path) -> Result<String> {
    use pdfium_render::prelude::*;

    let dylib = find_pdfium_dylib()
        .ok_or_else(|| anyhow!("libpdfium.dylib not found in bundle — scanned-PDF OCR is unavailable"))?;
    tracing::info!("pdfium dylib: {}", dylib.display());
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(&dylib).map_err(|e| anyhow!("pdfium bind: {e:?}"))?,
    );
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow!("pdfium open: {e:?}"))?;
    let n_pages = document.pages().len();
    tracing::info!("pdf_to_text_via_pdfium_ocr: {n_pages} pages");

    let tmpdir = tempdir_for_pdf()?;
    let render_config = PdfRenderConfig::new()
        .set_target_width(2200)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);
    let mut buf = String::new();
    for (i, page) in document.pages().iter().enumerate() {
        let img_path = tmpdir.join(format!("page-{:04}.png", i + 1));
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| anyhow!("pdfium render page {}: {e:?}", i + 1))?;
        let image = bitmap.as_image();
        image
            .save(&img_path)
            .map_err(|e| anyhow!("save page png: {e}"))?;
        match crate::capture::ocr::ocr_image(&img_path, "en-US") {
            Ok(text) if !text.trim().is_empty() => {
                buf.push_str(text.trim());
                buf.push_str("\n\n");
            }
            Ok(_) => tracing::warn!("ocr returned empty text for page {}", i + 1),
            Err(e) => tracing::warn!("ocr failed for page {}: {e:?}", i + 1),
        }
        // Best-effort cleanup so the temp dir doesn't accumulate large PNGs.
        let _ = std::fs::remove_file(&img_path);
    }
    if buf.trim().is_empty() {
        return Err(anyhow!(
            "this PDF appears to be image-only and OCR returned no text. \
             Try a clearer scan, or pre-OCR the file in Preview.app (Tools → \
             Annotate → Text)."
        ));
    }
    Ok(buf)
}

/// Locate the bundled pdfium shared library. The filename varies per OS, as do
/// the bundle layouts:
///   - macOS:   libpdfium.dylib inside the .app's Contents/Resources/_up_/resources/pdfium/
///   - Windows: pdfium.dll next to the exe (Tauri MSI/NSIS layout)
///   - Linux:   libpdfium.so in the AppImage's usr/lib or alongside the binary
#[cfg(not(target_os = "ios"))]
fn find_pdfium_dylib() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    let lib_name = "libpdfium.dylib";
    #[cfg(target_os = "windows")]
    let lib_name = "pdfium.dll";
    #[cfg(all(unix, not(target_os = "macos")))]
    let lib_name = "libpdfium.so";

    let exe = std::env::current_exe().ok()?;
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // Production bundle paths.
    #[cfg(target_os = "macos")]
    {
        if let Some(parent2) = exe.parent().and_then(|p| p.parent()) {
            candidates.push(parent2.join(format!("Resources/_up_/resources/pdfium/{}", lib_name)));
            candidates.push(parent2.join(format!("Resources/pdfium/{}", lib_name)));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(parent) = exe.parent() {
            // Tauri Windows/Linux: resources/pdfium/ relative to the binary.
            candidates.push(parent.join(format!("resources/pdfium/{}", lib_name)));
            candidates.push(parent.join(format!("pdfium/{}", lib_name)));
            candidates.push(parent.join(lib_name));
        }
        // AppImage layouts vary — Tauri Linux AppImage places resources under
        // either $APPDIR/usr/lib/yappy/resources/ OR
        // $APPDIR/usr/share/yappy/resources/. Check both.
        #[cfg(target_os = "linux")]
        if let Ok(appdir) = std::env::var("APPDIR") {
            let appdir = std::path::PathBuf::from(appdir);
            candidates.push(appdir.join(format!("usr/lib/yappy/resources/pdfium/{}", lib_name)));
            candidates.push(appdir.join(format!("usr/share/yappy/resources/pdfium/{}", lib_name)));
            candidates.push(appdir.join(format!("usr/lib/yappy/{}", lib_name)));
        }
        // Linux .deb / .rpm install paths.
        #[cfg(target_os = "linux")]
        {
            candidates.push(std::path::PathBuf::from(format!("/usr/lib/yappy/resources/pdfium/{}", lib_name)));
            candidates.push(std::path::PathBuf::from(format!("/usr/share/yappy/resources/pdfium/{}", lib_name)));
            candidates.push(std::path::PathBuf::from(format!("/usr/lib/yappy/{}", lib_name)));
        }
    }

    // Dev paths (cargo run / cargo test).
    for prefix in [
        "yappy-app/resources/pdfium/",
        "resources/pdfium/",
        "../yappy-app/resources/pdfium/",
        "../../yappy-app/resources/pdfium/",
    ] {
        candidates.push(std::path::PathBuf::from(format!("{prefix}{lib_name}")));
    }

    candidates.into_iter().find(|p| p.exists())
}

fn epub_to_text(path: &Path) -> Result<String> {
    let mut doc = epub::doc::EpubDoc::new(path).map_err(|e| anyhow!("epub open: {e:?}"))?;
    let mut buf = String::new();
    while {
        if let Some((html, _mime)) = doc.get_current_str() {
            buf.push_str(&strip_xml(&html));
            buf.push_str("\n\n");
        }
        doc.go_next()
    } {}
    Ok(buf)
}

fn tempdir_for_pdf() -> Result<std::path::PathBuf> {
    let d = std::env::temp_dir().join(format!("yappy-pdf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for d in std::env::split_paths(&path_env) {
        let c = d.join(name);
        if c.exists() {
            return Some(c);
        }
    }
    for prefix in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let c = std::path::Path::new(prefix).join(name);
        if c.exists() {
            return Some(c);
        }
    }
    None
}

// ---------- text cleanup helpers ----------

/// Strip basic markdown for cleaner TTS.
fn strip_md_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_code = false;
    for line in s.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code { continue; }
        let l = line.trim_start_matches(|c: char| c == '#' || c == '>' || c.is_whitespace());
        out.push_str(&strip_md_links(l));
        out.push('\n');
    }
    out
}

/// A paragraph with reading-rhythm hints. Used for .md / pdf_oxide-markdown input.
#[derive(Debug, Clone)]
pub struct RichParagraph {
    pub text: String,
    /// Default pause (seconds) inserted BEFORE this paragraph when reading.
    pub pause_before: f32,
    /// Speed multiplier for this paragraph (1.0 = global).
    pub speed_mult: f32,
    /// "paragraph" | "heading1" | "heading2" | "heading3" | "list" | "hr" | "quote" | "code".
    pub kind: String,
}

/// Parse a markdown document into reading-aware paragraphs.
/// Recognises:
///   - `# Heading` levels 1-3 → bigger pause + slightly slower speed
///   - `---` / `***` / `___` → silence break
///   - `> blockquote` → small pause, kept as text
///   - `- item` / `1. item` → joined into one paragraph block (each item gets a small pause)
///   - ``` ``` ``` code fences → kept but flagged "code" (currently we skip the body
///     for TTS sanity; future versions could read code differently)
///   - blank line → paragraph boundary
pub fn parse_markdown_rhythm(md: &str) -> Vec<RichParagraph> {
    let mut paragraphs: Vec<RichParagraph> = Vec::new();
    let mut buf = String::new();
    let mut in_code = false;
    let mut current_kind = "paragraph".to_string();

    let flush = |paragraphs: &mut Vec<RichParagraph>, buf: &mut String, kind: &mut String| {
        let trimmed = buf.trim().to_string();
        if !trimmed.is_empty() {
            let (pause_before, speed_mult) = match kind.as_str() {
                "heading1" => (1.2, 0.92),
                "heading2" => (0.9, 0.95),
                "heading3" => (0.6, 0.97),
                "heading4" => (0.45, 0.98),
                "heading5" => (0.35, 0.99),
                "heading6" => (0.25, 1.00),
                "hr" => (1.5, 1.0),
                "quote" => (0.45, 0.97),
                "list" => (0.25, 1.0),
                _ => (0.0, 1.0),
            };
            paragraphs.push(RichParagraph {
                text: trimmed,
                pause_before,
                speed_mult,
                kind: kind.clone(),
            });
        }
        buf.clear();
        *kind = "paragraph".to_string();
    };

    for raw_line in md.lines() {
        let line = raw_line.trim_end();
        let stripped = line.trim_start();

        // Code fences toggle. Skip code blocks for TTS.
        if stripped.starts_with("```") {
            in_code = !in_code;
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            continue;
        }
        if in_code { continue; }

        // Horizontal rule.
        if matches!(stripped, "---" | "***" | "___" | "----" | "*****" | "_____") {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            paragraphs.push(RichParagraph {
                text: String::from("(pause)"), // not actually read; pause_before is what matters
                pause_before: 1.5,
                speed_mult: 1.0,
                kind: "hr".into(),
            });
            // Pop the "(pause)" placeholder text — we just want the pause marker.
            if let Some(last) = paragraphs.last_mut() { last.text = String::new(); }
            continue;
        }

        // Headings (h1-h6). Check longest first so '######' isn't matched as '#'.
        let heading_match: Option<(&'static str, &'static str)> = [
            ("###### ", "heading6"),
            ("##### ", "heading5"),
            ("#### ", "heading4"),
            ("### ", "heading3"),
            ("## ", "heading2"),
            ("# ", "heading1"),
        ]
        .into_iter()
        .find(|(p, _)| stripped.starts_with(p));
        if let Some((prefix, kind)) = heading_match {
            let rest = &stripped[prefix.len()..];
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            current_kind = kind.to_string();
            buf.push_str(&strip_md_links(rest));
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            continue;
        }

        // Blockquote.
        if let Some(rest) = stripped.strip_prefix("> ") {
            if current_kind != "quote" {
                flush(&mut paragraphs, &mut buf, &mut current_kind);
                current_kind = "quote".into();
            }
            if !buf.is_empty() { buf.push(' '); }
            buf.push_str(&strip_md_inline(rest));
            continue;
        }

        // Unordered / ordered list item — each item becomes its own paragraph.
        let list_match = if let Some(rest) = stripped.strip_prefix("- ").or_else(|| stripped.strip_prefix("* ")) {
            Some(rest)
        } else {
            // Numbered list "1. xxx"
            let mut chars = stripped.chars();
            let mut digits = String::new();
            while let Some(c) = chars.next() {
                if c.is_ascii_digit() { digits.push(c); } else { break; }
            }
            if !digits.is_empty() && stripped.get(digits.len()..).map(|s| s.starts_with(". ")).unwrap_or(false) {
                Some(&stripped[digits.len() + 2..])
            } else { None }
        };
        if let Some(item) = list_match {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            current_kind = "list".into();
            buf.push_str(&strip_md_inline(item));
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            continue;
        }

        // Blank line → paragraph boundary.
        if stripped.is_empty() {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            continue;
        }

        // Plain text line — accumulate into the current paragraph (or quote).
        if !buf.is_empty() { buf.push(' '); }
        buf.push_str(&strip_md_inline(stripped));
    }
    flush(&mut paragraphs, &mut buf, &mut current_kind);

    paragraphs
}

/// Strip inline markdown (links, bold, italic, code) for clean TTS speech.
fn strip_md_inline(s: &str) -> String {
    let s = strip_md_links(s);
    // Strip ** and * around bold/italic; keep the inner text.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '_' => {
                // Skip another marker for bold (** or __).
                if chars.peek() == Some(&c) { chars.next(); }
                // Otherwise it's italic — already skipped.
            }
            '`' => {
                // Inline code: keep the content, skip the backticks.
            }
            _ => out.push(c),
        }
    }
    out
}

fn strip_md_links(s: &str) -> String {
    // Regex-based — safe on UTF-8 because regex operates on char boundaries.
    // The previous byte-level implementation mangled multi-byte chars
    // (á → Ã¡) which broke both rendering and TTS output for Spanish/French/…
    use once_cell::sync::Lazy;
    use regex::Regex;
    static IMG: Lazy<Regex> = Lazy::new(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap());
    static LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([^\]]+)\]\([^)]*\)").unwrap());
    let no_img = IMG.replace_all(s, "");
    let no_link = LINK.replace_all(&no_img, "$1");
    no_link.into_owned()
}

/// Naïve XML/HTML tag stripper that handles entities and skips <style>/<script>.
/// Regex-based so it's UTF-8 safe — the previous byte-level walk corrupted
/// multi-byte chars (á, ñ, é …) by pushing raw bytes as `char`.
fn strip_xml(s: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static STYLE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap());
    static SCRIPT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap());
    static TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]*>").unwrap());

    let s = STYLE.replace_all(s, "");
    let s = SCRIPT.replace_all(&s, "");
    let s = TAG.replace_all(&s, "");
    let s = s
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    // Collapse whitespace.
    let mut compact = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
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
    compact.trim().to_string()
}

/// Strip RTF control words. Good enough for plain-text reading.
fn strip_rtf(rtf: &str) -> String {
    let mut out = String::with_capacity(rtf.len());
    let mut chars = rtf.chars().peekable();
    let mut depth = 0i32;
    while let Some(c) = chars.next() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            '\\' => {
                // skip the control word
                let mut hex = String::new();
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_alphabetic() || n == '*' || n == '\'' || n.is_ascii_digit() || n == '-' {
                        hex.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Hex char escape like \'e9
                if hex.starts_with('\'') && hex.len() >= 3 {
                    if let Ok(v) = u8::from_str_radix(&hex[1..3], 16) {
                        out.push(v as char);
                    }
                }
                // skip exactly one whitespace if any
                if let Some(&n) = chars.peek() {
                    if n == ' ' {
                        chars.next();
                    }
                }
            }
            other if depth > 0 => out.push(other),
            _ => {}
        }
    }
    out
}
