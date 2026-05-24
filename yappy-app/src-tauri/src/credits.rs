//! Bundled license & credit texts. Returned as a single payload to the UI.

use serde::Serialize;

#[derive(Serialize)]
pub struct Credit {
    pub name: &'static str,
    pub kind: &'static str, // model / library / font / asset
    pub version: &'static str,
    pub license: &'static str,
    pub url: &'static str,
    pub note: &'static str,
}

#[derive(Serialize)]
pub struct LicenseDoc {
    pub name: &'static str,
    pub url: &'static str,
    pub text: &'static str,
}

pub fn credits() -> &'static [Credit] {
    const C: &[Credit] = &[
        // Voice & inference
        Credit { name: "Supertonic 3", kind: "model", version: "v1.7.3 (open weights)", license: "OpenRAIL-M",
            url: "https://huggingface.co/Supertone/supertonic-3",
            note: "31-language, 99 M-param ONNX TTS model. Trained and released by Supertone Inc." },
        Credit { name: "supertonic (SDK)", kind: "code", version: "main", license: "MIT",
            url: "https://github.com/supertone-inc/supertonic",
            note: "Reference Rust inference code vendored into yappy-core." },
        Credit { name: "ONNX Runtime", kind: "library", version: "2.0.0-rc.10", license: "MIT",
            url: "https://onnxruntime.ai/",
            note: "Powers Supertonic 3 + PaddleOCR inference." },

        // OCR + readability
        Credit { name: "PaddleOCR (PP-OCRv4)", kind: "model", version: "v4 mobile det+cls+rec", license: "Apache-2.0",
            url: "https://github.com/PaddlePaddle/PaddleOCR",
            note: "Cross-platform text detection + recognition. Models bundled (~16 MB)." },
        Credit { name: "paddle-ocr-rs", kind: "library", version: "0.6", license: "Apache-2.0",
            url: "https://crates.io/crates/paddle-ocr-rs",
            note: "Rust glue to run PaddleOCR via ONNX Runtime." },
        Credit { name: "defuddle", kind: "library", version: "0.18+", license: "MIT",
            url: "https://github.com/kepano/defuddle",
            note: "Extracts clean article content from web pages. Embedded as a JS payload." },

        // Tauri + plugins
        Credit { name: "Tauri", kind: "library", version: "2.x", license: "MIT / Apache-2.0",
            url: "https://tauri.app/",
            note: "Desktop app framework." },
        Credit { name: "SvelteKit", kind: "library", version: "2.x", license: "MIT",
            url: "https://svelte.dev/",
            note: "Front-end framework." },
        Credit { name: "cpal", kind: "library", version: "0.15", license: "Apache-2.0",
            url: "https://crates.io/crates/cpal",
            note: "Cross-platform audio output." },
        Credit { name: "rubato", kind: "library", version: "0.16", license: "MIT",
            url: "https://crates.io/crates/rubato",
            note: "High-quality sample-rate conversion." },
        Credit { name: "hound", kind: "library", version: "3.5", license: "Apache-2.0",
            url: "https://crates.io/crates/hound",
            note: "WAV file writing." },
        Credit { name: "whatlang", kind: "library", version: "0.16", license: "MIT",
            url: "https://crates.io/crates/whatlang",
            note: "Per-paragraph language detection." },
        Credit { name: "image", kind: "library", version: "0.25", license: "MIT",
            url: "https://crates.io/crates/image",
            note: "Image decoding for OCR." },

        // Fonts
        Credit { name: "Quicksand", kind: "font", version: "1.0", license: "SIL OFL 1.1",
            url: "https://fonts.google.com/specimen/Quicksand",
            note: "Rounded sans for body & UI." },
        Credit { name: "Patrick Hand", kind: "font", version: "1.0", license: "SIL OFL 1.1",
            url: "https://fonts.google.com/specimen/Patrick+Hand",
            note: "Hand-drawn display font for headings." },
        Credit { name: "JetBrains Mono", kind: "font", version: "—", license: "SIL OFL 1.1",
            url: "https://www.jetbrains.com/lp/mono/",
            note: "Monospaced fallback for kbd / code." },

        // Inspirations
        Credit { name: "Handy", kind: "inspiration", version: "—", license: "—",
            url: "https://handy.computer",
            note: "Design + interaction inspiration for the cozy aesthetic." },
        Credit { name: "Mozilla Readability", kind: "inspiration", version: "—", license: "Apache-2.0",
            url: "https://github.com/mozilla/readability",
            note: "Prior art for article extraction; defuddle is in the same lineage." },
    ];
    C
}

pub fn license_docs() -> &'static [LicenseDoc] {
    const DOCS: &[LicenseDoc] = &[
        LicenseDoc { name: "Yappy (MIT)", url: "https://opensource.org/licenses/MIT",
            text: include_str!("../../../LICENSE") },
        LicenseDoc { name: "Supertonic 3 (OpenRAIL-M)", url: "https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE",
            text: include_str!("../../resources/licenses/LICENSE-OpenRAIL-M.txt") },
        LicenseDoc { name: "PaddleOCR (Apache-2.0)", url: "https://github.com/PaddlePaddle/PaddleOCR/blob/main/LICENSE",
            text: include_str!("../../resources/licenses/LICENSE-Apache-2.0.txt") },
        LicenseDoc { name: "defuddle (MIT)", url: "https://github.com/kepano/defuddle/blob/main/LICENSE",
            text: include_str!("../../resources/licenses/LICENSE-defuddle-MIT.txt") },
        LicenseDoc { name: "Quicksand & Patrick Hand (SIL OFL 1.1)", url: "https://scripts.sil.org/cms/scripts/page.php?item_id=OFL_web",
            text: include_str!("../../resources/licenses/LICENSE-OFL-1.1.txt") },
    ];
    DOCS
}
