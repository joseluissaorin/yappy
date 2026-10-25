//! Carga un documento (.txt .md .rtf .docx .odt .pptx .xlsx .pdf .html .epub,
//! imágenes) como PÁRRAFOS de lectura con ritmo (títulos, listas, citas).
//!
//! Regla de la casa: el mismo fichero da los MISMOS párrafos en el Mac, en
//! iOS y en Android, porque el progreso se guarda por índice de párrafo.
//! Por eso todo va en Rust puro y nada depende de pandoc:
//! ```text
//! .md / .markdown                 → parse_markdown_rhythm
//! .txt / .log                     → txt_a_rich (CRLF, Gutenberg, texto envuelto)
//! .docx / .odt / .rtf (.doc)      → capture/ofimatica.rs (estilos, notas, tablas, versos)
//! .html / .htm                    → html_a_rich (web: extractor de la cola; documento: DOM)
//! .epub                           → epub_to_rich (DOM de cada fichero del spine)
//! .pdf con texto                  → capture/pdf_maqueta.rs (caracteres + geometría)
//! .pdf escaneado / imágenes       → OCR (capture/amanuensis.rs)
//! .pptx / .xlsx                   → anytomd → markdown
//! .tex / .rst / .org / .fb2       → pandoc si está (solo escritorio)
//! ```
//! Al final, `partir_gigantes` trocea los párrafos de miles de caracteres.

use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Load a document AND return reading-rhythm paragraphs when possible.
/// For .md (and .pdf via pdf_oxide's markdown output) we parse structure so
/// headings, lists, and rule breaks get appropriate pauses. For everything else
/// we fall back to plain paragraph splitting (text only, default pauses).
pub fn load_rich_from_file(path: &Path) -> Result<Vec<RichParagraph>> {
    Ok(partir_gigantes(cargar_rico(path)?))
}

/// Un párrafo de miles de caracteres (un libro de gramática escaneado, un
/// TXT sin líneas en blanco) es una isla para el progreso, que se guarda por
/// párrafo, y para el lector, que no puede saltar dentro de él. Se parte en
/// trozos de ~1.500 caracteres, siempre en final de frase y sin perder ni un
/// byte (cada trozo es una rebanada exacta del original).
pub fn partir_gigantes(parrafos: Vec<RichParagraph>) -> Vec<RichParagraph> {
    use once_cell::sync::Lazy;
    use regex::Regex;
    const LIMITE: usize = 2500;
    const OBJETIVO: usize = 1500;
    static FIN_FRASE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"[.!?…][»"”’)\]]*\s+[¿¡«"“‘(\[]?\p{Lu}"#).unwrap());
    let mut out = Vec::with_capacity(parrafos.len());
    for p in parrafos {
        let largo = p.text.chars().count();
        if largo <= LIMITE || !matches!(p.kind.as_str(), "paragraph" | "quote" | "list") {
            out.push(p);
            continue;
        }
        // Cortes posibles: justo después del espacio que sigue al signo.
        let cortes: Vec<usize> = FIN_FRASE
            .find_iter(&p.text)
            .map(|m| {
                let tramo = m.as_str();
                let antes_mayus = tramo
                    .char_indices()
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let tras_espacio = tramo[antes_mayus..]
                    .char_indices()
                    .find(|(_, c)| !c.is_whitespace())
                    .map(|(i, _)| antes_mayus + i)
                    .unwrap_or(tramo.len());
                m.start() + tras_espacio
            })
            .collect();
        let mut ini = 0usize;
        let mut primero = true;
        for &c in &cortes {
            if p.text[ini..c].chars().count() >= OBJETIVO && p.text[c..].chars().count() >= 400 {
                let mut trozo = p.clone();
                trozo.text = p.text[ini..c].trim_end().to_string();
                if !primero {
                    trozo.pause_before = 0.0;
                }
                out.push(trozo);
                primero = false;
                ini = c;
            }
        }
        let mut resto = p.clone();
        resto.text = p.text[ini..].to_string();
        if !primero {
            resto.pause_before = 0.0;
        }
        out.push(resto);
    }
    out
}

fn cargar_rico(path: &Path) -> Result<Vec<RichParagraph>> {
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
    // PDF con capa de texto: maquetación propia a partir de los caracteres y
    // su posición (capture/pdf_maqueta.rs): orden de columnas, cabeceras y
    // folios fuera, notas al final, párrafos cosidos entre páginas. Es Rust
    // puro: el mismo PDF da los mismos párrafos en el Mac y en el móvil.
    // Si la capa de texto es basura (OCR viejo), se trata como escaneado y
    // sigue al OCR por load_text_from_file → pdf_to_text.
    if ext == "pdf" {
        match crate::capture::pdf_maqueta::leer(path) {
            Ok(ps) => {
                let todo: String = ps
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if crate::capture::pdf_maqueta::capa_util(&todo) {
                    return Ok(ps);
                }
                tracing::info!("doc_loader: la capa de texto del PDF no sirve; se intenta OCR");
            }
            Err(e) => {
                tracing::warn!("pdf_maqueta falló: {e:?}; se prueba to_markdown");
                if let Ok(md) = pdf_oxide_to_markdown(path) {
                    if crate::capture::pdf_maqueta::capa_util(&md) {
                        return Ok(refinar_parrafos_pdf(parse_markdown_rhythm(&md)));
                    }
                }
            }
        }
    }

    // EPUB: estructura del XHTML (párrafos, títulos, listas, citas) en el
    // orden del spine, en TODAS las plataformas. Antes el camino sin pandoc
    // (iOS, Android) colapsaba cada fichero del spine en UN párrafo: el libro
    // salía como un bloque y el progreso, que va por párrafo, no avanzaba.
    // Tampoco se usa pandoc aquí: el mismo libro debe dar los MISMOS
    // párrafos en el Mac y en el móvil (el progreso guardado es un índice).
    if ext == "epub" {
        let parrafos = epub_to_rich(path)?;
        if parrafos.iter().any(|p| !p.text.trim().is_empty()) {
            return Ok(parrafos);
        }
    }

    // Ofimática, HTML y texto plano: estructura propia en Rust puro, sin
    // pandoc (que solo existía en el Mac y daba otros párrafos que el móvil).
    match ext.as_str() {
        "docx" | "docm" | "dotx" => return crate::capture::ofimatica::docx(path),
        "odt" | "ott" => return crate::capture::ofimatica::odt(path),
        "rtf" => return crate::capture::ofimatica::rtf(&std::fs::read(path)?),
        // Un «.doc» suele ser RTF o DOCX con otro nombre; el Word 97 binario
        // de verdad no se sabe leer.
        "doc" => {
            let bytes = std::fs::read(path)?;
            if bytes.starts_with(b"{\\rtf") {
                return crate::capture::ofimatica::rtf(&bytes);
            }
            if bytes.starts_with(b"PK") {
                return crate::capture::ofimatica::docx(path);
            }
            return Err(anyhow!(
                "this is an old binary Word .doc; save it as .docx or .rtf and open it again"
            ));
        }
        "html" | "htm" | "xhtml" => return Ok(html_a_rich(&read_utf8(path)?)),
        "txt" | "text" | "log" => return Ok(txt_a_rich(&read_utf8(path)?)),
        "pptx" | "xlsx" => {
            use anytomd::{convert_file, ConversionOptions};
            let res = convert_file(
                path.to_str().unwrap_or_default(),
                &ConversionOptions::default(),
            )
            .map_err(|e| anyhow!("anytomd: {e:?}"))?;
            return Ok(parse_markdown_rhythm(&res.markdown));
        }
        _ => {}
    }

    // Default: load plain text and split by blank lines.
    let text = load_text_from_file(path)?.replace("\r\n", "\n");
    let paragraphs: Vec<RichParagraph> = text
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
    // Un PDF que llega aquí es un escaneado leído por OCR: trae el mismo
    // mobiliario que uno con texto (cabeceras corrientes, folios, párrafos
    // partidos entre páginas, guiones de final de línea) y se repara igual.
    if ext == "pdf" {
        return Ok(refinar_parrafos_pdf(paragraphs));
    }
    Ok(paragraphs)
}

/// pdf_oxide's `to_markdown` per-page, joined. Used to feed PDFs through
/// the markdown rhythm parser so headings get pauses.
fn pdf_oxide_to_markdown(path: &Path) -> Result<String> {
    use pdf_oxide::converters::ConversionOptions;
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| anyhow!("pdf_oxide open: {e:?}"))?;
    let pages = doc
        .page_count()
        .map_err(|e| anyhow!("pdf_oxide page_count: {e:?}"))?;
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

    // Pandoc solo para formatos que no tienen camino propio (LaTeX, RST,
    // Org, FB2): son de escritorio y en el móvil no se abren. Los formatos
    // de todos los días (DOCX, ODT, RTF, HTML, EPUB) NUNCA pasan por pandoc:
    // darían párrafos distintos en el Mac y en el móvil.
    if PANDOC_SUPPORTED.contains(&ext.as_str()) {
        if let Some(pd) = which("pandoc") {
            tracing::debug!("doc_loader: using pandoc for .{ext}");
            return pandoc_to_text(&pd, path);
        }
    }

    let unir = |ps: Vec<RichParagraph>| -> String {
        ps.into_iter()
            .map(|p| p.text)
            .filter(|t| !t.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    match ext.as_str() {
        "csv" => Ok(read_utf8(path)?),
        "txt" | "text" | "log" | "md" | "markdown" | "html" | "htm" | "xhtml" | "docx" | "odt"
        | "rtf" | "doc" | "pptx" | "xlsx" => Ok(unir(cargar_rico(path)?)),
        "xml" => anytomd_convert(path, &ext),
        "pdf" => pdf_to_text(path),
        "epub" => epub_to_text(path),
        // Fotos, capturas y escaneos: OCR (Amanuensis Ligero; capture/amanuensis.rs).
        e if crate::capture::amanuensis::es_imagen(e) => {
            crate::capture::amanuensis::texto_de_imagen(path)
        }
        other => Err(anyhow!("unsupported document extension: .{}", other)),
    }
}

const PANDOC_SUPPORTED: &[&str] = &["tex", "latex", "rst", "org", "fb2"];

fn read_utf8(path: &Path) -> Result<String> {
    // Read as bytes so we can be tolerant of encodings + BOMs.
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // UTF-16 con BOM (el Bloc de notas de Windows lo guarda así a veces).
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let cod = if bytes[0] == 0xFF {
            encoding_rs::UTF_16LE
        } else {
            encoding_rs::UTF_16BE
        };
        let (cow, _, _) = cod.decode(&bytes);
        return Ok(cow.into_owned());
    }
    // Strip UTF-8 BOM if present (some editors add it to .md files; the BOM
    // character renders as a stray glyph otherwise).
    let bytes = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        &bytes[..]
    };
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
    let res = convert_file(path.to_str().unwrap(), &opts).map_err(|e| anyhow!("anytomd: {e:?}"))?;
    Ok(strip_md_markup(&res.markdown))
}

fn pdf_to_text(path: &Path) -> Result<String> {
    // Stage 1: pdf_oxide for digital PDFs. ~0.8ms mean on 3,830-PDF corpus, 5×
    // faster than pdf-extract, and emits real markdown (headings preserved).
    tracing::info!("pdf_to_text: opening with pdf_oxide: {}", path.display());
    let oxide_text = pdf_oxide_extract(path)
        .map_err(|e| tracing::warn!("pdf_oxide failed: {e:?}"))
        .unwrap_or_default();
    // Una capa de texto de OCR viejo (sopa de signos) no cuenta como texto.
    if crate::capture::pdf_maqueta::capa_util(&oxide_text) {
        tracing::info!(
            "pdf_to_text: pdf_oxide extracted {} chars",
            oxide_text.trim().chars().count()
        );
        return Ok(oxide_text);
    }

    // Stage 2: PDF escaneado (o con una capa de texto que es basura). En el
    // escritorio, pdfium rasteriza cada página y lee Amanuensis Ligero
    // (capture/amanuensis.rs, pesos empotrados); en el móvil rasteriza el
    // sistema (PDFKit / PdfRenderer) y lee Amanuensis.
    tracing::info!("pdf_to_text: no text from pdf_oxide; rasterizing via pdfium for OCR");
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    return pdf_to_text_via_pdfium_ocr(path);
    // En el móvil rasteriza el sistema (PDFKit / PdfRenderer) y lee Amanuensis.
    #[cfg(any(target_os = "ios", target_os = "android"))]
    return crate::capture::amanuensis::pdf_escaneado(path);
}

fn pdf_oxide_extract(path: &Path) -> Result<String> {
    use pdf_oxide::converters::ConversionOptions;
    let t0 = std::time::Instant::now();
    tracing::info!("pdf_oxide_extract: opening {}", path.display());
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| anyhow!("pdf_oxide open: {e:?}"))?;
    tracing::info!("pdf_oxide_extract: open in {:?}", t0.elapsed());

    let pages = doc
        .page_count()
        .map_err(|e| anyhow!("pdf_oxide page_count: {e:?}"))?;
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

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn pdf_to_text_via_pdfium_ocr(path: &Path) -> Result<String> {
    use pdfium_render::prelude::*;

    let dylib = find_pdfium_dylib().ok_or_else(|| {
        anyhow!("libpdfium.dylib not found in bundle — scanned-PDF OCR is unavailable")
    })?;
    tracing::info!("pdfium dylib: {}", dylib.display());
    let pdfium =
        Pdfium::new(Pdfium::bind_to_library(&dylib).map_err(|e| anyhow!("pdfium bind: {e:?}"))?);
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow!("pdfium open: {e:?}"))?;
    let n_pages = document.pages().len();
    tracing::info!("pdf_to_text_via_pdfium_ocr: {n_pages} pages");

    let tmpdir = tempdir_for_pdf()?;
    // Como en el móvil (amanuensis::DPI_PDF): 200 ppp, lado mayor de 4000 px como mucho, y la página tal como
    // es. Nada de girar las apaisadas: Amanuensis endereza solo, y una doble página de libro tiene que llegarle
    // apaisada para que `ocr::doble` la parta por el canal.
    let render_config = PdfRenderConfig::new()
        .scale_page_by_factor(200.0 / 72.0)
        .set_maximum_width(4000)
        .set_maximum_height(4000);
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
#[cfg(not(any(target_os = "ios", target_os = "android")))]
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
            candidates.push(std::path::PathBuf::from(format!(
                "/usr/lib/yappy/resources/pdfium/{}",
                lib_name
            )));
            candidates.push(std::path::PathBuf::from(format!(
                "/usr/share/yappy/resources/pdfium/{}",
                lib_name
            )));
            candidates.push(std::path::PathBuf::from(format!(
                "/usr/lib/yappy/{}",
                lib_name
            )));
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

/// Texto plano del EPUB (para quien pida solo texto): los párrafos de
/// `epub_to_rich` separados por línea en blanco.
fn epub_to_text(path: &Path) -> Result<String> {
    Ok(epub_to_rich(path)?
        .into_iter()
        .map(|p| p.text)
        .filter(|t| !t.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n"))
}

/// El ritmo de lectura de cada clase de bloque (pausa previa, velocidad).
fn ritmo_de(kind: &str) -> (f32, f32) {
    match kind {
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
    }
}

pub(crate) fn rich(text: String, kind: &str) -> RichParagraph {
    let (pause_before, speed_mult) = ritmo_de(kind);
    RichParagraph {
        text,
        pause_before,
        speed_mult,
        kind: kind.to_string(),
    }
}

/// EPUB → párrafos con estructura, en el orden del spine.
///
/// Cada fichero del spine se recorre como árbol (html5ever vía dom_query):
/// los elementos de bloque (p, div, h1-h6, li, blockquote, section…) cierran
/// párrafo, dos <br> seguidos también, y un <br> suelto separa párrafos
/// solo si las «líneas» son prosa larga (los versos se quedan juntos). Al
/// empezar cada fichero nuevo hay pausa de capítulo, y si el fichero no
/// trae título propio se toma el del índice (toc).
pub fn epub_to_rich(path: &Path) -> Result<Vec<RichParagraph>> {
    let mut doc = epub::doc::EpubDoc::new(path).map_err(|e| anyhow!("epub open: {e:?}"))?;

    // Índice: fichero (sin #fragmento) → título, solo las entradas que
    // apuntan al PRINCIPIO del fichero.
    let mut titulos: std::collections::HashMap<String, String> = Default::default();
    fn aplanar(
        puntos: &[epub::doc::NavPoint],
        out: &mut std::collections::HashMap<String, String>,
    ) {
        for p in puntos {
            let c = p.content.to_string_lossy().to_string();
            if !c.contains('#') {
                out.entry(c).or_insert_with(|| p.label.trim().to_string());
            }
            aplanar(&p.children, out);
        }
    }
    aplanar(&doc.toc, &mut titulos);
    // EPUB3 sin NCX: el índice vive en el documento de navegación
    // (properties="nav"), que el crate no lee. Se lee aquí.
    if titulos.is_empty() {
        let nav = doc
            .resources
            .values()
            .find(|r| {
                r.properties
                    .as_deref()
                    .is_some_and(|p| p.split_whitespace().any(|x| x == "nav"))
            })
            .map(|r| r.path.clone());
        if let Some(ruta_nav) = nav {
            if let Some(xhtml) = doc.get_resource_str_by_path(&ruta_nav) {
                let base = ruta_nav
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_default();
                let d = dom_query::Document::from(xhtml.as_str());
                for a in d.select("nav a[href]").nodes() {
                    let href = a.attr("href").map(|h| h.to_string()).unwrap_or_default();
                    if href.is_empty() || href.contains('#') || href.contains("://") {
                        continue;
                    }
                    let destino = normalizar_ruta(&base.join(&href));
                    let etiqueta = compactar(&a.text());
                    if !etiqueta.is_empty() {
                        titulos.entry(destino).or_insert(etiqueta);
                    }
                }
            }
        }
    }

    let mut salida: Vec<RichParagraph> = Vec::new();
    let total = doc.get_num_chapters();
    for i in 0..total {
        if !doc.set_current_chapter(i) {
            continue;
        }
        let ruta = doc
            .get_current_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some((html, mime)) = doc.get_current_str() else {
            continue;
        };
        if !(mime.contains("html") || mime.contains("xml") || mime.is_empty()) {
            continue;
        }
        let mut bloques = bloques_de_xhtml(&html);
        if bloques.is_empty() {
            continue;
        }
        // Título del índice cuando el fichero no trae ninguno.
        if let Some(t) = titulos.get(&ruta).filter(|t| !t.is_empty()) {
            let tiene_titulo = bloques.iter().any(|b| b.kind.starts_with("heading"));
            let normal = |s: &str| {
                s.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
            };
            if !tiene_titulo {
                if normal(&bloques[0].text) == normal(t) {
                    bloques[0] = rich(bloques[0].text.clone(), "heading2");
                } else if bloques.len() > 1 && t.chars().count() <= 120 {
                    bloques.insert(0, rich(t.clone(), "heading2"));
                }
            }
        }
        // Pausa de capítulo al cambiar de fichero.
        if !salida.is_empty() {
            if let Some(b) = bloques.first_mut() {
                b.pause_before = b.pause_before.max(1.2);
            }
        }
        salida.extend(bloques);
    }
    // Ni separadores al principio o al final, ni dos seguidos.
    let mut limpia: Vec<RichParagraph> = Vec::with_capacity(salida.len());
    for p in salida {
        if p.kind == "hr" && limpia.last().is_none_or(|u| u.kind == "hr") {
            continue;
        }
        limpia.push(p);
    }
    while limpia.last().is_some_and(|u| u.kind == "hr") {
        limpia.pop();
    }
    Ok(limpia)
}

/// Un fichero XHTML del spine → bloques de lectura.
fn bloques_de_xhtml(xhtml: &str) -> Vec<RichParagraph> {
    let html = reabrir_autocierres(xhtml);
    let doc = dom_query::Document::from(html.as_ref());
    let cuerpo = doc.select("body");
    let mut st = Recorrido::default();
    if let Some(body) = cuerpo.nodes().first() {
        st.recorrer(body);
    } else if let Some(raiz) = doc.root().first_child() {
        st.recorrer(&raiz);
    }
    st.cerrar();
    st.bloques
}

/// El XHTML se parsea como HTML (html5ever tolera el marcado roto de muchos
/// EPUB), pero en HTML un `<title/>` o `<div/>` NO se cierran solos: el
/// título se tragaría el libro entero. Se reabren a mano.
fn reabrir_autocierres(xhtml: &str) -> std::borrow::Cow<'_, str> {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static AUTOCIERRE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"<([A-Za-z][\w:.-]*)((?:\s[^<>]*?)?)\s*/>").unwrap());
    const VACIOS: &[&str] = &[
        "br", "hr", "img", "meta", "link", "input", "col", "area", "base", "embed", "param",
        "source", "track", "wbr", "image",
    ];
    AUTOCIERRE.replace_all(xhtml, |c: &regex::Captures| {
        let nombre = &c[1];
        if VACIOS.contains(&nombre.to_ascii_lowercase().as_str()) {
            c[0].to_string()
        } else {
            format!("<{}{}></{}>", nombre, &c[2], nombre)
        }
    })
}

#[derive(Default)]
struct Recorrido {
    bloques: Vec<RichParagraph>,
    buf: String,
    /// <br> seguidos sin texto en medio.
    brs: usize,
    /// clase del bloque que se está llenando
    clase: Option<&'static str>,
    en_cita: usize,
    en_lista: usize,
}

const BLOQUES: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "header",
    "footer",
    "aside",
    "main",
    "nav",
    "figure",
    "figcaption",
    "blockquote",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "caption",
    "pre",
    "address",
    "center",
    "details",
    "summary",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "body",
    "html",
    "hgroup",
    "fieldset",
    "form",
    "legend",
    "poem",
    "stanza",
    "epigraph",
];
const SALTAR: &[&str] = &[
    "head", "script", "style", "title", "noscript", "template", "svg", "math", "img", "image",
    "object", "video", "audio", "iframe", "button", "select", "textarea",
];

impl Recorrido {
    fn recorrer(&mut self, nodo: &dom_query::NodeRef) {
        for hijo in nodo.children_it(false) {
            if hijo.is_text() {
                let t = hijo.text();
                if t.trim().is_empty() {
                    if !self.buf.is_empty() && !t.is_empty() {
                        self.buf.push(' ');
                    }
                    continue;
                }
                if self.brs >= 2 {
                    self.cerrar();
                } else if self.brs == 1 && !self.buf.trim().is_empty() {
                    self.buf.push('\n');
                }
                self.brs = 0;
                self.buf.push_str(&t);
                continue;
            }
            if !hijo.is_element() {
                continue;
            }
            let nombre = hijo
                .node_name()
                .map(|n| n.to_ascii_lowercase())
                .unwrap_or_default();
            // «epub:page» o «ns:div» → «div».
            let nombre = nombre.rsplit(':').next().unwrap_or("").to_string();
            if SALTAR.contains(&nombre.as_str()) || self.es_oculto(&hijo) {
                continue;
            }
            match nombre.as_str() {
                "br" => {
                    self.brs += 1;
                    continue;
                }
                "hr" => {
                    self.cerrar();
                    self.bloques.push(rich(String::new(), "hr"));
                    continue;
                }
                "td" | "th" => {
                    if !self.buf.is_empty() {
                        self.buf.push(' ');
                    }
                    self.recorrer(&hijo);
                    self.buf.push(' ');
                    continue;
                }
                "sup" | "a" if es_llamada_de_nota(&hijo) => continue,
                _ => {}
            }
            if BLOQUES.contains(&nombre.as_str()) {
                self.cerrar();
                let clase_antes = self.clase;
                match nombre.as_str() {
                    "h1" => self.clase = Some("heading1"),
                    "h2" => self.clase = Some("heading2"),
                    "h3" => self.clase = Some("heading3"),
                    "h4" => self.clase = Some("heading4"),
                    "h5" => self.clase = Some("heading5"),
                    "h6" => self.clase = Some("heading6"),
                    _ => {}
                }
                let cita = nombre == "blockquote";
                let lista = nombre == "li";
                self.en_cita += cita as usize;
                self.en_lista += lista as usize;
                self.recorrer(&hijo);
                self.cerrar();
                self.en_cita -= cita as usize;
                self.en_lista -= lista as usize;
                self.clase = clase_antes;
            } else {
                self.recorrer(&hijo);
            }
        }
    }

    fn es_oculto(&self, n: &dom_query::NodeRef) -> bool {
        if n.has_attr("hidden") {
            return true;
        }
        let estilo = n
            .attr("style")
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if estilo.replace(' ', "").contains("display:none") {
            return true;
        }
        let tipo = n
            .attr("epub:type")
            .or_else(|| n.attr("role"))
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        // Marcas de página y el índice de puntos de referencia: invisibles
        // para el lector humano, ruido para la voz.
        tipo.contains("pagebreak")
            || tipo.contains("page-list")
            || tipo.contains("landmarks")
            || tipo.contains("doc-pagebreak")
    }

    /// Cierra el bloque en curso. Si sus líneas (separadas por un <br>)
    /// son prosa larga, cada línea es un párrafo; si son cortas (versos,
    /// direcciones), se quedan en uno.
    fn cerrar(&mut self) {
        let crudo = std::mem::take(&mut self.buf);
        self.brs = 0;
        let lineas: Vec<String> = crudo
            .split('\n')
            .map(compactar)
            .filter(|l| !l.is_empty())
            .collect();
        if lineas.is_empty() {
            return;
        }
        let clase = self.clase.unwrap_or(if self.en_cita > 0 {
            "quote"
        } else if self.en_lista > 0 {
            "list"
        } else {
            "paragraph"
        });
        let media = lineas.iter().map(|l| l.chars().count()).sum::<usize>() / lineas.len();
        let partir = lineas.len() > 1 && media >= 120 && !clase.starts_with("heading");
        if partir {
            for l in lineas {
                self.bloques.push(rich(l, clase));
            }
        } else {
            let texto = lineas.join(" ");
            // Un «título» kilométrico es marcado roto: se lee como párrafo.
            let clase = if clase.starts_with("heading") && texto.chars().count() > 300 {
                "paragraph"
            } else {
                clase
            };
            self.bloques.push(rich(texto, clase));
        }
    }
}

/// Llamada a nota (el «1» volado que enlaza a la nota): no se lee.
fn es_llamada_de_nota(n: &dom_query::NodeRef) -> bool {
    let tipo = n
        .attr("epub:type")
        .or_else(|| n.attr("role"))
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if tipo.contains("noteref") {
        return true;
    }
    // <sup><a href="#n1">1</a></sup> o <a href="#n1"><sup>1</sup></a>
    let texto = n.text();
    let t = texto.trim();
    let corto = !t.is_empty()
        && t.chars().count() <= 4
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '*' | '†' | '‡' | '[' | ']' | '(' | ')'));
    if !corto {
        return false;
    }
    let es_sup = n.node_name().is_some_and(|x| x.eq_ignore_ascii_case("sup"));
    let enlace_a_nota = |a: &dom_query::NodeRef| a.attr("href").is_some_and(|h| h.contains('#'));
    if es_sup {
        n.descendants_it().any(|d| {
            d.node_name().is_some_and(|x| x.eq_ignore_ascii_case("a")) && enlace_a_nota(&d)
        })
    } else {
        enlace_a_nota(n)
            && n.descendants_it()
                .any(|d| d.node_name().is_some_and(|x| x.eq_ignore_ascii_case("sup")))
    }
}

/// HTML local → párrafos. Una página web guardada (con menús, cabecera,
/// pie, barra lateral) pasa por el MISMO extractor que un enlace compartido
/// (`cola::podar_html` + `cola::extraer_articulo`): Readability y JSON-LD.
/// Un HTML de documento (exportado de Word, un libro de Gutenberg) se
/// recorre entero con el lector de bloques del EPUB.
pub fn html_a_rich(html: &str) -> Vec<RichParagraph> {
    let url = url_de_html(html);
    let podado = crate::cola::podar_html(html, &url);
    let completo = {
        let doc = dom_query::Document::from(podado.as_str());
        doc.select(
            "nav, [role=navigation], [role=search], form, button, .mw-editsection, #toc, .toc, \
             .navbox, .noprint, .mw-jump-link, #pg-header, #pg-footer, .pg-boilerplate",
        )
        .remove();
        bloques_de_xhtml(&doc.html())
    };
    let con_mobiliario = {
        let doc = dom_query::Document::from(html);
        // Una cabecera sola (la del título en un HTML de pandoc) no hace una
        // web: hace falta un menú, o cabecera y pie, o barra lateral.
        let menus = doc.select("nav, [role=navigation]").length();
        let marcos = doc
            .select("header, footer, aside, [role=banner], [role=contentinfo]")
            .length();
        menus > 0 || marcos >= 2
    };
    if con_mobiliario {
        if let Ok((titulo, md)) = crate::cola::extraer_articulo(&podado, &url) {
            let mut ps = parse_markdown_rhythm(&md);
            if let Some(t) = titulo.filter(|t| !t.trim().is_empty()) {
                let normal = |s: &str| {
                    s.chars()
                        .filter(|c| c.is_alphanumeric())
                        .flat_map(char::to_lowercase)
                        .collect::<String>()
                };
                if ps.first().is_none_or(|p| normal(&p.text) != normal(&t)) {
                    ps.insert(0, rich(compactar(&t), "heading1"));
                }
            }
            // Readability a veces se queda con un trozo: si el artículo no
            // llega a un tercio de la página, se lee la página entera.
            let letras = |v: &[RichParagraph]| {
                v.iter()
                    .map(|p| p.text.chars().filter(|c| c.is_alphanumeric()).count())
                    .sum::<usize>()
            };
            if letras(&ps) * 3 >= letras(&completo) {
                return ps;
            }
        }
    }
    let doc = dom_query::Document::from(podado.as_str());
    doc.select(
        "nav, header, footer, aside, [role=navigation], [role=search], [role=banner], \
         [role=contentinfo], form, button, .mw-editsection, #toc, .toc, .navbox, .noprint, \
         #pg-header, #pg-footer, .pg-boilerplate",
    )
    .remove();
    let ps = bloques_de_xhtml(&doc.html());
    if ps.is_empty() {
        completo
    } else {
        ps
    }
}

/// La URL de origen de un HTML guardado (para resolver enlaces relativos en
/// Readability): la marca del share de Yappy, el canónico o el og:url.
fn url_de_html(html: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static MARCA: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"<!--\s*yappy-url:\s*(\S+)\s*-->").unwrap());
    if let Some(c) = MARCA.captures(html) {
        return c[1].to_string();
    }
    let doc = dom_query::Document::from(html);
    for sel in [r#"link[rel="canonical"]"#, r#"meta[property="og:url"]"#] {
        if let Some(n) = doc.select(sel).nodes().first() {
            if let Some(v) = n.attr("href").or_else(|| n.attr("content")) {
                if v.starts_with("http") {
                    return v.to_string();
                }
            }
        }
    }
    "https://localhost/documento.html".to_string()
}

/// Texto plano → párrafos.
///
/// - Saltos de línea de Windows y Mac antiguo normalizados (antes un TXT
///   con CRLF salía como UN párrafo: el split buscaba «\n\n»).
/// - Proyecto Gutenberg: solo el libro, entre los marcadores START y END.
/// - Un bloque cuyas líneas están cortadas a una anchura fija (texto
///   «envuelto» a 70-80 columnas) es UN párrafo: se cose, quitando el guion
///   de corte. Un bloque de líneas largas o desiguales sin líneas en blanco
///   (un párrafo por línea) da un párrafo por línea.
/// - Las viñetas («- », «* », «• ») son elementos de lista.
pub fn txt_a_rich(texto: &str) -> Vec<RichParagraph> {
    let texto = texto.replace("\r\n", "\n").replace('\r', "\n");
    let texto = recortar_gutenberg_txt(&texto).unwrap_or(&texto).to_string();
    let mut bloques: Vec<Vec<&str>> = Vec::new();
    let mut actual: Vec<&str> = Vec::new();
    for l in texto.lines() {
        if l.trim().is_empty() {
            if !actual.is_empty() {
                bloques.push(std::mem::take(&mut actual));
            }
        } else {
            actual.push(l.trim_end());
        }
    }
    if !actual.is_empty() {
        bloques.push(actual);
    }
    let es_vineta = |l: &str| {
        let t = l.trim_start();
        t.starts_with("- ") || t.starts_with("* ") || t.starts_with("• ") || t.starts_with("· ")
    };
    let mut out: Vec<RichParagraph> = Vec::new();
    let n_bloques = bloques.len();
    for (bi, b) in bloques.into_iter().enumerate() {
        let largos: Vec<usize> = b.iter().map(|l| l.trim().chars().count()).collect();
        let max = largos.iter().copied().max().unwrap_or(0);
        let media = largos.iter().sum::<usize>() / largos.len().max(1);
        // ¿Envuelto a anchura fija? Casi todas las líneas (menos la última)
        // rozan la anchura máxima, y esa anchura es de máquina de escribir.
        let envuelto = b.len() >= 2 && (35..=110).contains(&max) && {
            let cerca = largos[..largos.len() - 1]
                .iter()
                .filter(|&&n| n * 100 >= max * 60)
                .count();
            cerca * 4 >= (largos.len() - 1) * 3
        };
        if b.iter().any(|l| es_vineta(l)) {
            // Lista: cada viñeta, un elemento; las líneas sin viñeta continúan
            // la anterior.
            let mut item = String::new();
            for l in &b {
                if es_vineta(l) {
                    if !item.is_empty() {
                        out.push(rich(compactar(&item), "list"));
                    }
                    item = l.trim_start()[2..].to_string();
                    if l.trim_start().starts_with('•') || l.trim_start().starts_with('·') {
                        item = l.trim_start().chars().skip(2).collect();
                    }
                } else {
                    item.push(' ');
                    item.push_str(l.trim());
                }
            }
            if !item.is_empty() {
                out.push(rich(compactar(&item), "list"));
            }
            continue;
        }
        if b.len() == 1 || envuelto || (b.len() <= 8 && media < 100) {
            // Un párrafo (envuelto, o una estrofa / dirección corta).
            let mut p = String::new();
            for l in &b {
                coser_linea(&mut p, l.trim());
            }
            let p = compactar(&p);
            // Un renglón suelto, corto, sin puntuación final y seguido de
            // más texto: un título («CAPÍTULO I», «The Yellow Wallpaper»).
            let titulo = b.len() == 1
                && p.chars().count() <= 70
                && bi + 1 < n_bloques
                && !p.ends_with(['.', ',', ';', ':', '!', '?', '»', '"', '”', '…', ')'])
                && p.chars()
                    .next()
                    .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit());
            out.push(rich(p, if titulo { "heading3" } else { "paragraph" }));
        } else {
            for l in b {
                let t = compactar(l);
                if !t.is_empty() {
                    out.push(rich(t, "paragraph"));
                }
            }
        }
    }
    out
}

/// Cose una línea envuelta a la anterior: «pala-» + «bra» → «palabra».
fn coser_linea(acum: &mut String, l: &str) {
    if acum.is_empty() {
        acum.push_str(l);
        return;
    }
    let partida = acum.ends_with('-')
        && acum.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic())
        && l.chars().next().is_some_and(|c| c.is_lowercase());
    if partida {
        acum.pop();
    } else {
        acum.push(' ');
    }
    acum.push_str(l);
}

/// El libro de un TXT del Proyecto Gutenberg, sin el preámbulo legal ni la
/// licencia del final.
fn recortar_gutenberg_txt(t: &str) -> Option<&str> {
    let ini = t.find("*** START OF")?;
    let tras = ini + t[ini..].find('\n')? + 1;
    let fin = t[tras..]
        .find("*** END OF")
        .map(|f| tras + f)
        .unwrap_or(t.len());
    Some(&t[tras..fin])
}

/// «OEBPS/nav/../text/a.xhtml» → «OEBPS/text/a.xhtml».
fn normalizar_ruta(p: &Path) -> String {
    let mut partes: Vec<String> = Vec::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                partes.pop();
            }
            std::path::Component::CurDir => {}
            otro => partes.push(otro.as_os_str().to_string_lossy().to_string()),
        }
    }
    partes.join("/")
}

fn compactar(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
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
        if in_code {
            continue;
        }
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
///   - `# Heading` (ATX) and setext headings (`Título` + `====` / `----`)
///   - `---` / `***` / `___` → silence break
///   - `> blockquote` → small pause, kept as text
///   - `- item` / `* item` / `+ item` / `1. item` / `1) item` / `- [ ] tarea`
///   - tables (`| a | b |`) → one paragraph per row, cells joined by commas
///   - YAML front matter, HTML comments, link reference definitions → skipped
///   - footnotes (`[^1]` and `[^1]: text`) → the call is dropped and the
///     note text is read at the end, after a pause
///   - ``` ``` ``` code fences → skipped
///   - blank line → paragraph boundary
pub fn parse_markdown_rhythm(md: &str) -> Vec<RichParagraph> {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static NOTA_DEF: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[\^([^\]]+)\]:\s*(.*)$").unwrap());
    static REF_DEF: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"^\[[^\]^][^\]]*\]:\s*\S+(\s+["'(].*["')])?\s*$"#).unwrap());
    static SEPARADOR_TABLA: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\|?[\s:|-]+\|?$").unwrap());
    static TAREA: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[[ xX]\]\s+").unwrap());

    let mut paragraphs: Vec<RichParagraph> = Vec::new();
    let mut notas: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut lineas_buf = 0usize;
    let mut in_code = false;
    let mut en_comentario = false;
    let mut current_kind = "paragraph".to_string();

    let flush = |paragraphs: &mut Vec<RichParagraph>, buf: &mut String, kind: &mut String| {
        let trimmed = buf.trim().to_string();
        if !trimmed.is_empty() {
            paragraphs.push(rich(trimmed, kind.as_str()));
        }
        buf.clear();
        *kind = "paragraph".to_string();
    };

    // Front matter YAML al principio («---» … «---» o «...»): no se lee.
    let mut cuerpo = md.trim_start_matches('\u{feff}');
    if cuerpo.starts_with("---\n") || cuerpo.starts_with("---\r\n") {
        let resto = &cuerpo[4..];
        for (i, l) in resto.lines().enumerate().take(80) {
            if l.trim_end() == "---" || l.trim_end() == "..." {
                let fin: usize = resto.lines().take(i + 1).map(|x| x.len() + 1).sum();
                cuerpo = resto.get(fin.min(resto.len())..).unwrap_or("");
                break;
            }
        }
    }

    for raw_line in cuerpo.lines() {
        let line = raw_line.trim_end();
        let stripped = line.trim_start();

        // Comentarios HTML de varias líneas.
        if en_comentario {
            if let Some(p) = stripped.find("-->") {
                en_comentario = false;
                if stripped[p + 3..].trim().is_empty() {
                    continue;
                }
            } else {
                continue;
            }
        }
        if stripped.starts_with("<!--") && !stripped.contains("-->") {
            en_comentario = true;
            continue;
        }

        // Code fences toggle. Skip code blocks for TTS.
        if stripped.starts_with("```") || stripped.starts_with("~~~") {
            in_code = !in_code;
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            continue;
        }
        if in_code {
            continue;
        }

        // Setext: la línea de «===» o «---» bajo UNA línea de texto la hace
        // título (antes «---» se leía como pausa y el título como prosa).
        let solo = |c: char| stripped.len() >= 2 && stripped.chars().all(|x| x == c);
        if lineas_buf == 1 && current_kind == "paragraph" && (solo('=') || solo('-')) {
            current_kind = if solo('=') {
                "heading1".into()
            } else {
                "heading2".into()
            };
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            continue;
        }

        // Horizontal rule.
        let sin_espacios: String = stripped.chars().filter(|c| !c.is_whitespace()).collect();
        if sin_espacios.len() >= 3
            && (sin_espacios.chars().all(|c| c == '-')
                || sin_espacios.chars().all(|c| c == '*')
                || sin_espacios.chars().all(|c| c == '_'))
        {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            paragraphs.push(rich(String::new(), "hr"));
            continue;
        }

        // Definiciones de notas y de enlaces por referencia.
        if let Some(c) = NOTA_DEF.captures(stripped) {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            let t = strip_md_inline(&c[2]);
            if !t.trim().is_empty() {
                notas.push(t.trim().to_string());
            }
            continue;
        }
        if REF_DEF.is_match(stripped) {
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
            // «## Título ##»: las almohadillas de cierre tampoco se leen.
            let rest = stripped[prefix.len()..].trim_end_matches('#').trim_end();
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            current_kind = kind.to_string();
            buf.push_str(&strip_md_inline(rest));
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            continue;
        }

        // Tablas: una fila por párrafo, celdas separadas por comas.
        if stripped.starts_with('|') && stripped[1..].contains('|') {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            if SEPARADOR_TABLA.is_match(stripped) {
                continue;
            }
            let celdas: Vec<String> = stripped
                .trim_matches('|')
                .split('|')
                .map(|c| strip_md_inline(c.trim()).trim().to_string())
                .filter(|c| !c.is_empty())
                .collect();
            if !celdas.is_empty() {
                let mut fila = celdas.join(", ");
                if !fila.ends_with(['.', '!', '?', ':', ';']) {
                    fila.push('.');
                }
                paragraphs.push(rich(fila, "paragraph"));
            }
            continue;
        }

        // Blockquote.
        if let Some(rest) = stripped.strip_prefix('>') {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if current_kind != "quote" {
                flush(&mut paragraphs, &mut buf, &mut current_kind);
                lineas_buf = 0;
                current_kind = "quote".into();
            }
            if rest.trim().is_empty() {
                // «>» solo: párrafo nuevo dentro de la cita.
                flush(&mut paragraphs, &mut buf, &mut current_kind);
                current_kind = "quote".into();
                continue;
            }
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(&strip_md_inline(rest));
            continue;
        }

        // Unordered / ordered list item — each item becomes its own paragraph.
        let list_match = if let Some(rest) = stripped
            .strip_prefix("- ")
            .or_else(|| stripped.strip_prefix("* "))
            .or_else(|| stripped.strip_prefix("+ "))
        {
            Some(rest)
        } else {
            // Numbered list "1. xxx" / "1) xxx"
            let digits = stripped.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0
                && digits <= 3
                && stripped
                    .get(digits..)
                    .map(|s| s.starts_with(". ") || s.starts_with(") "))
                    .unwrap_or(false)
            {
                Some(&stripped[digits + 2..])
            } else {
                None
            }
        };
        if let Some(item) = list_match {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            let item = TAREA.replace(item, "");
            current_kind = "list".into();
            buf.push_str(&strip_md_inline(&item));
            lineas_buf = 1;
            continue;
        }

        // Blank line → paragraph boundary.
        if stripped.is_empty() {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
            continue;
        }

        // Plain text line — accumulate into the current paragraph (or quote,
        // or the list item it continues).
        if current_kind == "quote" {
            flush(&mut paragraphs, &mut buf, &mut current_kind);
            lineas_buf = 0;
        }
        // Salto duro con barra invertida al final de línea.
        let texto = stripped.strip_suffix('\\').unwrap_or(stripped);
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(&strip_md_inline(texto));
        lineas_buf += 1;
    }
    flush(&mut paragraphs, &mut buf, &mut current_kind);

    if !notas.is_empty() {
        paragraphs.push(rich(String::new(), "hr"));
        for n in notas {
            paragraphs.push(rich(n, "paragraph"));
        }
    }
    paragraphs
}

/// Strip inline markdown (links, bold, italic, code, footnote calls,
/// strikethrough, inline HTML, escapes) for clean TTS speech. Los asteriscos
/// y guiones bajos solo caen cuando marcan énfasis: «snake_case» y «5 * 3»
/// se quedan como están.
fn strip_md_inline(s: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static LLAMADA: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\^[^\]]+\]").unwrap());
    static REF_LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([^\]]+)\]\[[^\]]*\]").unwrap());
    static AUTOLINK: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"<((?:https?|mailto):[^>\s]+)>").unwrap());
    static COMENTARIO: Lazy<Regex> = Lazy::new(|| Regex::new(r"<!--.*?-->").unwrap());
    static ETIQUETA: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"</?[A-Za-z][A-Za-z0-9-]*(\s[^<>]*)?/?>").unwrap());
    static FUERTE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(\*\*|__)(\S(?:.*?\S)?)(\*\*|__)").unwrap());
    static CURSIVA_AST: Lazy<Regex> = Lazy::new(|| Regex::new(r"\*(\S(?:[^*]*?\S)?)\*").unwrap());
    static CURSIVA_BAJA: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(^|[^\p{L}\p{N}_])_(\S(?:[^_]*?\S)?)_($|[^\p{L}\p{N}_])").unwrap()
    });
    static TACHADO: Lazy<Regex> = Lazy::new(|| Regex::new(r"~~(.+?)~~|==(.+?)==").unwrap());
    static CODIGO: Lazy<Regex> = Lazy::new(|| Regex::new(r"`+([^`]*)`+").unwrap());
    static ESCAPE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\\([\\`*_{}\[\]()#+\-.!|~<>])").unwrap());

    let s = strip_md_links(s);
    let s = LLAMADA.replace_all(&s, "");
    let s = REF_LINK.replace_all(&s, "$1");
    let s = AUTOLINK.replace_all(&s, "$1");
    let s = COMENTARIO.replace_all(&s, "");
    let s = ETIQUETA.replace_all(&s, "");
    let s = CODIGO.replace_all(&s, "$1");
    let s = FUERTE.replace_all(&s, "$2");
    let s = CURSIVA_AST.replace_all(&s, "$1");
    let s = CURSIVA_BAJA.replace_all(&s, "$1$2$3");
    let s = TACHADO.replace_all(&s, "$1$2");
    let s = ESCAPE.replace_all(&s, "$1");
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
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

// ── Refinado de PDFs ─────────────────────────────────────────────────────
//
// Los extractores de PDF devuelven la PÁGINA, no el TEXTO: frases partidas
// en bloques allí donde la maqueta cambió de caja, palabras con guion de
// final de línea, y el mobiliario de página (cabeceras corrientes, folios,
// pies repetidos) mezclado con la prosa. Escuchado, eso suena a tartamudeo.
// Esta pasada lo repara con tres etapas, en este orden:
//   1. fuera el mobiliario (bloques cortos repetidos y números de página),
//   2. fusión de continuaciones (un bloque que empieza en minúscula
//      continúa al anterior; un «heading» que empieza en minúscula es un
//      falso positivo del extractor y se degrada a prosa),
//   3. desguionizado («len- guaje» → «lenguaje»).

pub fn refinar_parrafos_pdf(parrafos: Vec<RichParagraph>) -> Vec<RichParagraph> {
    let sin_mobiliario = quitar_mobiliario(parrafos);
    let fusionados = fusionar_continuaciones(sin_mobiliario);
    fusionados
        .into_iter()
        .map(|mut p| {
            p.text = desguionizar(&p.text);
            p
        })
        .collect()
}

/// Un bloque corto que se repite idéntico tres o más veces es mobiliario de
/// página (cabecera corriente, pie, URL del documento): se quitan TODAS sus
/// apariciones. Los folios («12», «01 / 19», «xii») caen por forma.
fn quitar_mobiliario(parrafos: Vec<RichParagraph>) -> Vec<RichParagraph> {
    use std::collections::HashMap;
    // La clave ignora los espacios: la MISMA cabecera corriente sale con
    // huecos distintos según la página («ESC EN A 3 .1» / «ESC ENA 3.1»).
    let clave = |t: &str| -> String { t.chars().filter(|c| !c.is_whitespace()).collect() };
    let mut veces: HashMap<String, usize> = HashMap::new();
    for p in &parrafos {
        if p.text.chars().count() < 90 && !p.text.is_empty() {
            *veces.entry(clave(&p.text)).or_default() += 1;
        }
    }
    parrafos
        .into_iter()
        .filter(|p| {
            if p.text.is_empty() {
                return true; // separadores hr: solo llevan pausa
            }
            if p.text.chars().count() < 90 && veces.get(&clave(&p.text)).copied().unwrap_or(0) >= 3
            {
                return false;
            }
            // Un párrafo de un solo carácter (la viñeta que el OCR leyó como
            // «a» o «*») nunca es texto que leer.
            let alfanumericos = p.text.chars().filter(|c| c.is_alphanumeric()).count();
            alfanumericos > 1 && !es_folio(&p.text) && !es_letras_esparcidas(&p.text)
        })
        .collect()
}

/// Rótulo decorativo con letra-espaciado que el extractor rompió en fichas
/// («F EC HA DE AUTÓGR AFO», «SUB GÉ NE RO»): corto, sin minúsculas y con
/// la mitad de sus fichas de una o dos letras. Leído en voz alta es sopa de
/// letras; mejor callarlo.
fn es_letras_esparcidas(t: &str) -> bool {
    if t.chars().count() > 60 || t.chars().any(|c| c.is_lowercase()) {
        return false;
    }
    let fichas: Vec<&str> = t
        .split_whitespace()
        .filter(|f| f.chars().any(|c| c.is_alphabetic()))
        .collect();
    if fichas.len() < 2 {
        return false;
    }
    // Letra a letra («L A C O M U N I C A C I Ó N»): aunque alguna letra
    // suelta parezca un romano, cuatro fichas o más de una sola letra en su
    // mayoría son un rótulo espaciado.
    let sueltas = fichas.iter().filter(|f| f.chars().count() == 1).count();
    if fichas.len() >= 4 && sueltas * 2 > fichas.len() {
        return true;
    }
    // Los romanos cortos son legítimos («CAPÍTULO IV»): no cuentan como ficha rota.
    let es_romano = |f: &str| f.chars().all(|c| "IVXLCDM".contains(c));
    let cortas = fichas
        .iter()
        .filter(|f| f.chars().count() <= 2 && !es_romano(f))
        .count();
    // Tres de cada cinco fichas rotas: un título de verdad con palabras
    // cortas («1.1. EN BUSCA DE REGULARIDADES», dos de cuatro) se queda.
    cortas * 5 >= fichas.len() * 3
}

/// ¿Tiene pinta de número de página? «12», «01 / 19», «- 7 -», «xiv».
fn es_folio(t: &str) -> bool {
    let limpio: String = t
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '/' && *c != '-' && *c != '·' && *c != '.')
        .collect();
    if limpio.is_empty() || limpio.chars().count() > 8 {
        return false;
    }
    limpio.chars().all(|c| c.is_ascii_digit())
        || limpio.chars().all(|c| "ivxlcdmIVXLCDM".contains(c))
}

fn fusionar_continuaciones(parrafos: Vec<RichParagraph>) -> Vec<RichParagraph> {
    let mut salida: Vec<RichParagraph> = Vec::new();
    for mut p in parrafos {
        let continua = salida
            .last()
            .map(|a| es_continuacion(a, &p))
            .unwrap_or(false);
        if continua {
            let a = salida.last_mut().unwrap();
            if a.text.ends_with('-') {
                // Palabra partida justo en el borde del bloque.
                a.text.pop();
                a.text.push_str(p.text.trim_start());
            } else {
                a.text.push(' ');
                a.text.push_str(p.text.trim_start());
            }
        } else {
            // Un «heading» que empieza en minúscula es prosa mal clasificada
            // (aunque no continúe nada: no merece pausa ni voz de título).
            if p.kind.starts_with("heading") && empieza_en_minuscula(&p.text) {
                p.kind = "paragraph".into();
                p.pause_before = 0.0;
                p.speed_mult = 1.0;
            }
            salida.push(p);
        }
    }
    salida
}

/// ¿El bloque b continúa la frase del bloque a?
fn es_continuacion(a: &RichParagraph, b: &RichParagraph) -> bool {
    if a.text.is_empty() || b.text.is_empty() {
        return false;
    }
    // Solo la prosa continúa prosa; un título de verdad corta.
    let a_prosa = matches!(a.kind.as_str(), "paragraph" | "quote" | "list");
    if !a_prosa {
        return false;
    }
    let b_prosa = matches!(b.kind.as_str(), "paragraph" | "quote" | "list")
        || (b.kind.starts_with("heading") && empieza_en_minuscula(&b.text));
    if !b_prosa {
        return false;
    }
    // b empieza en minúscula: en prosa ninguna frase nueva lo hace.
    if empieza_en_minuscula(&b.text) {
        return true;
    }
    // a quedó a media frase (sin puntuación de cierre) y encima acaba en
    // guion: la palabra siguió en el bloque de abajo.
    let fin = a.text.chars().last().unwrap_or('.');
    if fin == '-' {
        return true;
    }
    false
}

fn empieza_en_minuscula(t: &str) -> bool {
    t.chars()
        .find(|c| c.is_alphabetic())
        .map(|c| c.is_lowercase())
        .unwrap_or(false)
}

/// «len- guaje» → «lenguaje»: minúscula + guion + espacio + minúscula es la
/// firma del corte de línea tipográfico, no de un compuesto real.
fn desguionizar(t: &str) -> String {
    static RE: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"(\p{Ll})- (\p{Ll})").unwrap());
    RE.replace_all(t, "$1$2").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parrafo(texto: &str, kind: &str) -> RichParagraph {
        RichParagraph {
            text: texto.into(),
            pause_before: 0.0,
            speed_mult: 1.0,
            kind: kind.into(),
        }
    }

    #[test]
    fn fusiona_frase_partida_en_bloques() {
        let v = vec![
            parrafo(
                "Llega el centro de la pieza, porque los actos están separados",
                "paragraph",
            ),
            parrafo(
                "precisamente por esa elipsis y abre el tercer acto.",
                "heading2",
            ),
            parrafo("Otra frase que empieza como debe.", "paragraph"),
        ];
        let r = refinar_parrafos_pdf(v);
        assert_eq!(r.len(), 2);
        assert!(r[0]
            .text
            .ends_with("por esa elipsis y abre el tercer acto."));
        assert_eq!(r[0].kind, "paragraph");
    }

    #[test]
    fn desguioniza_cortes_de_linea() {
        let v = vec![parrafo(
            "El ser del len- guaje y la catás- trofe.",
            "paragraph",
        )];
        let r = refinar_parrafos_pdf(v);
        assert_eq!(r[0].text, "El ser del lenguaje y la catástrofe.");
    }

    #[test]
    fn quita_mobiliario_repetido_y_folios() {
        let mut v = vec![parrafo("La prosa de verdad sigue aquí.", "paragraph")];
        for _ in 0..4 {
            v.push(parrafo("LA DAMA BOBA · LOPE DE VEGA · 1613", "paragraph"));
        }
        v.push(parrafo("03 / 19", "paragraph"));
        v.push(parrafo("xii", "paragraph"));
        let r = refinar_parrafos_pdf(v);
        assert_eq!(r.len(), 1);
        assert!(r[0].text.starts_with("La prosa"));
    }

    #[test]
    fn los_titulos_con_palabras_cortas_no_son_letras_esparcidas() {
        assert!(!es_letras_esparcidas("1.1. EN BUSCA DE REGULARIDADES"));
        assert!(!es_letras_esparcidas("1.2. EL ESQUEMA CLÁSICO"));
        assert!(es_letras_esparcidas("F EC HA DE AUTÓGR AFO"));
        assert!(es_letras_esparcidas("SUB GÉ NE RO"));
        assert!(es_letras_esparcidas("L A C O M U N I C A C I Ó N"));
        assert!(es_letras_esparcidas("LA COM U N I CA C I Ó N"));
    }

    #[test]
    fn no_fusiona_titulos_de_verdad() {
        let v = vec![
            parrafo("La frase anterior termina bien.", "paragraph"),
            parrafo("El capítulo siguiente", "heading1"),
        ];
        let r = refinar_parrafos_pdf(v);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].kind, "heading1");
    }

    #[test]
    fn heading_falso_sin_continuar_se_degrada() {
        let v = vec![parrafo(
            "en cada década, porque cada una articula",
            "heading2",
        )];
        let r = refinar_parrafos_pdf(v);
        assert_eq!(r[0].kind, "paragraph");
        assert_eq!(r[0].pause_before, 0.0);
    }

    #[test]
    fn guion_al_borde_del_bloque_une_la_palabra() {
        let v = vec![
            parrafo("El discurso de la natu-", "paragraph"),
            parrafo("raleza sigue en el bloque de abajo.", "paragraph"),
        ];
        let r = refinar_parrafos_pdf(v);
        assert_eq!(r.len(), 1);
        assert!(r[0].text.contains("naturaleza sigue"));
    }

    /// Diagnóstico manual: vuelca lo que pdf_oxide emite para un PDF dado.
    /// `YAPPY_PDF_PRUEBA=/ruta.pdf cargo test -p yappy-app vuelca_pdf -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn vuelca_pdf() {
        let ruta = std::env::var("YAPPY_PDF_PRUEBA").expect("YAPPY_PDF_PRUEBA sin definir");
        let md = pdf_oxide_to_markdown(std::path::Path::new(&ruta)).expect("pdf_oxide");
        println!("════ PÁRRAFOS REFINADOS ════");
        for (i, p) in refinar_parrafos_pdf(parse_markdown_rhythm(&md))
            .iter()
            .enumerate()
        {
            println!(
                "[{i}] kind={} pausa={} → {:?}",
                p.kind, p.pause_before, p.text
            );
        }
    }

    /// Diagnóstico manual: el camino entero de la app (maqueta → OCR) para un PDF.
    /// `YAPPY_PDF_PRUEBA=/ruta.pdf cargo test -p yappy-app vuelca_documento -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn vuelca_documento() {
        let ruta = std::env::var("YAPPY_PDF_PRUEBA").expect("YAPPY_PDF_PRUEBA sin definir");
        let t0 = std::time::Instant::now();
        match load_rich_from_file(std::path::Path::new(&ruta)) {
            Ok(ps) => {
                println!("{} párrafos en {:?}", ps.len(), t0.elapsed());
                for (i, p) in ps.iter().enumerate() {
                    println!("[{i}] {} → {:?}", p.kind, p.text);
                }
            }
            Err(e) => println!("ERROR en {:?}: {e:?}", t0.elapsed()),
        }
    }
}

#[cfg(test)]
mod tests_epub {
    use super::*;
    use std::io::Write;

    fn fixture(nombre: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(nombre)
    }

    /// Letras y cifras de TODO el <body> de cada fichero del spine: la vara
    /// para comprobar que la extracción no pierde texto.
    fn alfanumericos_del_cuerpo(ruta: &Path) -> usize {
        let mut doc = epub::doc::EpubDoc::new(ruta).unwrap();
        let mut n = 0;
        for i in 0..doc.get_num_chapters() {
            doc.set_current_chapter(i);
            if let Some((html, _)) = doc.get_current_str() {
                let html = reabrir_autocierres(&html);
                let d = dom_query::Document::from(html.as_ref());
                n += d
                    .select("body")
                    .text()
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .count();
            }
        }
        n
    }

    fn alfanumericos(ps: &[RichParagraph]) -> usize {
        ps.iter()
            .map(|p| p.text.chars().filter(|c| c.is_alphanumeric()).count())
            .sum()
    }

    /// Fabrica un EPUB mínimo con los ficheros dados (ruta, contenido).
    fn fabricar_epub(destino: &Path, ficheros: &[(&str, &str)]) {
        let f = std::fs::File::create(destino).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let guardado: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        z.start_file("mimetype", guardado).unwrap();
        z.write_all(b"application/epub+zip").unwrap();
        z.start_file(
            "META-INF/container.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        z.write_all(
            br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        )
        .unwrap();
        for (ruta, contenido) in ficheros {
            z.start_file(*ruta, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(contenido.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn el_papel_amarillo_sale_por_parrafos() {
        // El caso de José Luis (21-10-2026): The Yellow Wallpaper, de
        // Calibre, en .html partidos. Antes: 6 párrafos, uno de 31 181
        // caracteres (el camino sin pandoc de iOS/Android).
        let ruta = fixture("the-yellow-wallpaper.epub");
        let ps = load_rich_from_file(&ruta).unwrap();
        let max = ps.iter().map(|p| p.text.chars().count()).max().unwrap();
        assert!(ps.len() >= 300, "solo {} párrafos", ps.len());
        assert!(max < 2_000, "párrafo de {max} caracteres");
        let primero = ps
            .iter()
            .position(|p| p.text.starts_with("It is very seldom"))
            .unwrap();
        assert_eq!(ps[primero].kind, "paragraph");
        assert!(ps[primero].text.ends_with("for the summer."));
        assert!(ps.iter().any(|p| p
            .text
            .contains("John is a physician, and PERHAPS—(I would not say it to a living soul")));
        // Sin perder texto.
        let cuerpo = alfanumericos_del_cuerpo(&ruta);
        let sacado = alfanumericos(&ps);
        assert!(sacado * 1000 >= cuerpo * 995, "{sacado} de {cuerpo}");
        // Determinista: el progreso guardado es un ÍNDICE de párrafo, así
        // que reabrir el libro debe dar exactamente la misma partición.
        let otra = load_rich_from_file(&ruta).unwrap();
        assert_eq!(
            ps.iter().map(|p| &p.text).collect::<Vec<_>>(),
            otra.iter().map(|p| &p.text).collect::<Vec<_>>()
        );
        // Y el texto plano también va por párrafos.
        let plano = epub_to_text(&ruta).unwrap();
        assert!(plano.split("\n\n").count() >= 300);
    }

    #[test]
    fn el_papel_amarillo_no_dice_the_first() {
        // Banco de la «I»: el cuento está lleno de pronombres en primera
        // persona. Ninguno debe salir «the first».
        let ps = load_rich_from_file(&fixture("the-yellow-wallpaper.epub")).unwrap();
        let mut ies = 0;
        let mut malas = Vec::new();
        for p in &ps {
            ies += p
                .text
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| *w == "I")
                .count();
            let pieza = yappy_core::guion::construir_pieza(
                &p.text,
                yappy_core::guion::ClasePieza::desde_kind(&p.kind),
                "en",
            );
            let h = pieza.texto_hablado();
            if p.text.contains("PERHAPS—(I would not") {
                // La versal de énfasis llega al modelo en minúsculas.
                assert!(h.contains("perhaps—(I would not say it"), "{h}");
            }
            if h.contains("the first") || h.contains("first will") {
                malas.push(h);
            }
        }
        assert!(ies > 250, "solo {ies} «I»");
        assert!(
            malas.is_empty(),
            "{} «the first»: {:?}",
            malas.len(),
            &malas[..malas.len().min(3)]
        );
    }

    #[test]
    fn epub2_con_divs_br_y_notas() {
        let dir = std::env::temp_dir().join(format!("yappy-epub2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("prueba2.epub");
        let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Prueba</dc:title><dc:identifier id="id">x</dc:identifier><dc:language>es</dc:language></metadata>
<manifest>
<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
<item id="c1" href="cap1.xhtml" media-type="application/xhtml+xml"/>
<item id="c2" href="cap2.xhtml" media-type="application/xhtml+xml"/>
</manifest>
<spine toc="ncx"><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>"#;
        let ncx = r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><navMap>
<navPoint id="n1" playOrder="1"><navLabel><text>Capítulo primero</text></navLabel><content src="cap1.xhtml"/></navPoint>
<navPoint id="n2" playOrder="2"><navLabel><text>Capítulo segundo</text></navLabel><content src="cap2.xhtml"/></navPoint>
</navMap></ncx>"#;
        let cap1 = r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title/><link rel="stylesheet" href="s.css"/></head>
<body><div class="calibre">
<h2 class="titulo">Capítulo primero</h2>
<div class="para">Era una mañana fría de enero y el pueblo dormía.<a href="notas.xhtml#n1"><sup>1</sup></a></div>
<div class="para">La campana sonó dos veces &mdash; y nadie acudió.</div>
<div class="para"><span class="ini">E</span>ntonces llegó el cartero.<div class="vacio"/></div>
<blockquote><p>Una cita famosa.</p></blockquote>
<ul><li>Primero</li><li>Segundo</li></ul>
<p>Párrafo uno tras br.<br/><br/>Párrafo dos tras br.</p>
<p class="verso">Verde que te quiero verde.<br/>Verde viento. Verdes ramas.<br/>El barco sobre la mar.</p>
<span epub:type="pagebreak" id="p2" title="2"/>
<hr/>
<div>Último párrafo del capítulo.</div>
</div></body></html>"#;
        let largo = "Esta es una línea de prosa larguísima que en un libro mal convertido ocupa un párrafo entero y que solo separa un br suelto del siguiente";
        let cap2 = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>x</title></head>
<body><div>{largo} uno.<br/>{largo} dos.<br/>{largo} tres.</div></body></html>"#
        );
        fabricar_epub(
            &ruta,
            &[
                ("OEBPS/content.opf", opf),
                ("OEBPS/toc.ncx", ncx),
                ("OEBPS/cap1.xhtml", cap1),
                ("OEBPS/cap2.xhtml", &cap2),
            ],
        );
        let ps = load_rich_from_file(&ruta).unwrap();
        let textos: Vec<(&str, &str)> = ps
            .iter()
            .map(|p| (p.kind.as_str(), p.text.as_str()))
            .collect();
        let esperado = vec![
            ("heading2", "Capítulo primero"),
            (
                "paragraph",
                "Era una mañana fría de enero y el pueblo dormía.",
            ),
            ("paragraph", "La campana sonó dos veces — y nadie acudió."),
            ("paragraph", "Entonces llegó el cartero."),
            ("quote", "Una cita famosa."),
            ("list", "Primero"),
            ("list", "Segundo"),
            ("paragraph", "Párrafo uno tras br."),
            ("paragraph", "Párrafo dos tras br."),
            (
                "paragraph",
                "Verde que te quiero verde. Verde viento. Verdes ramas. El barco sobre la mar.",
            ),
            ("hr", ""),
            ("paragraph", "Último párrafo del capítulo."),
            // cap2 sin título propio: el del índice.
            ("heading2", "Capítulo segundo"),
        ];
        assert_eq!(&textos[..esperado.len()], &esperado[..], "{textos:#?}");
        assert_eq!(textos.len(), esperado.len() + 3, "{textos:#?}");
        assert!(textos[esperado.len()].1.ends_with("uno."));
        // Pausa de capítulo al cambiar de fichero.
        assert!(ps[esperado.len() - 1].pause_before >= 1.2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn epub3_con_nav_y_secciones() {
        let dir = std::env::temp_dir().join(format!("yappy-epub3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("prueba3.epub");
        let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Test</dc:title><dc:identifier id="id">y</dc:identifier><dc:language>en</dc:language></metadata>
<manifest>
<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="a" href="text/a.xhtml" media-type="application/xhtml+xml"/>
<item id="b" href="text/b.xhtml" media-type="application/xhtml+xml"/>
</manifest>
<spine><itemref idref="a"/><itemref idref="b"/></spine>
</package>"#;
        let nav = r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Nav</title></head><body>
<nav epub:type="toc"><ol><li><a href="text/a.xhtml">Chapter I</a></li><li><a href="text/b.xhtml">Chapter II</a></li></ol></nav></body></html>"#;
        let a = r##"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>A</title></head>
<body><section epub:type="chapter"><header><h1>Chapter I</h1><p class="subtitle">In which I begin</p></header>
<div class="p">I think I can, said the engine.</div>
<div class="p">Henry I was king, and I was not.<a epub:type="noteref" href="#fn1">1</a></div>
<aside epub:type="footnote" id="fn1"><p>A footnote that is still text.</p></aside>
</section></body></html>"##;
        let b = r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>B</title></head>
<body><section><div class="p">First paragraph of two.</div><div class="p">Second paragraph of two.</div></section></body></html>"#;
        fabricar_epub(
            &ruta,
            &[
                ("OEBPS/content.opf", opf),
                ("OEBPS/nav.xhtml", nav),
                ("OEBPS/text/a.xhtml", a),
                ("OEBPS/text/b.xhtml", b),
            ],
        );
        let ps = load_rich_from_file(&ruta).unwrap();
        let textos: Vec<(&str, &str)> = ps
            .iter()
            .map(|p| (p.kind.as_str(), p.text.as_str()))
            .collect();
        assert_eq!(
            textos,
            vec![
                ("heading1", "Chapter I"),
                ("paragraph", "In which I begin"),
                ("paragraph", "I think I can, said the engine."),
                ("paragraph", "Henry I was king, and I was not."),
                ("paragraph", "A footnote that is still text."),
                ("heading2", "Chapter II"),
                ("paragraph", "First paragraph of two."),
                ("paragraph", "Second paragraph of two."),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vuelca estadísticas de EPUB reales:
    /// `YAPPY_EPUBS="/a.epub:/b.epub" cargo test -p yappy-app vuelca_epubs -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn vuelca_epubs() {
        let lista = std::env::var("YAPPY_EPUBS").expect("YAPPY_EPUBS sin definir");
        for r in lista.split(':').filter(|s| !s.is_empty()) {
            let ruta = Path::new(r);
            let t0 = std::time::Instant::now();
            let ps = match load_rich_from_file(ruta) {
                Ok(p) => p,
                Err(e) => {
                    println!("ERROR {r}: {e}");
                    continue;
                }
            };
            let ms = t0.elapsed().as_millis();
            let max = ps.iter().map(|p| p.text.chars().count()).max().unwrap_or(0);
            let titulos = ps.iter().filter(|p| p.kind.starts_with("heading")).count();
            let cuerpo = alfanumericos_del_cuerpo(ruta);
            let sacado = alfanumericos(&ps);
            let nombre = ruta.file_name().unwrap().to_string_lossy();
            println!(
                "{nombre}\n   párrafos={} títulos={titulos} máx={max} car. texto={sacado}/{cuerpo} ({:.2}%) {ms} ms",
                ps.len(),
                100.0 * sacado as f64 / cuerpo.max(1) as f64
            );
            for p in ps.iter().filter(|p| !p.text.is_empty()).take(4) {
                println!(
                    "     [{}] {}",
                    p.kind,
                    p.text.chars().take(90).collect::<String>()
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "doc_loader_corpus.rs"]
mod corpus;
