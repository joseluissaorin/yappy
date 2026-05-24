# Yappy

**Local text-to-speech for everything you read.**

Yappy is a desktop app that turns whatever's on your screen — selected text, a PDF, a Word doc, a web page in your browser — into clean, expressive audio. Everything runs **on-device**: no API keys, no telemetry, no internet round-trip for inference. Powered by [Supertonic&nbsp;3](https://github.com/Supertone-inc/supertonic) (99M-param ONNX, MIT-friendly), [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR), [Defuddle](https://github.com/kepano/defuddle) for clean-page extraction, and [PDFium](https://pdfium.googlesource.com/pdfium/) for PDF text + rasterization fallback.

10 named voices · 31 languages · macOS / Windows / Linux · MIT licensed.

> Status: early. Works well on macOS today, Windows/Linux are wired up in CI but receive less testing. PRs welcome.

## Features

- **Read anywhere with one hotkey.** `⌥⌘R` reads the current selection, the active document, a screenshot of what's on screen, or the page in your browser (via the bundled extension).
- **Document reader / audiobook editor.** Open a `.pdf` / `.docx` / `.md` / `.epub` / `.html` / `.rtf` / `.txt`. Edit text inline, override voice / speed / pause per paragraph, scrub paragraphs with chapter-aware navigation, render the whole thing (or a selection of chapters) to a single `.wav`.
- **Markdown-aware rhythm.** Headings, lists, blockquotes, and horizontal rules get sensible pauses and speed adjustments automatically; override with the doc-window rhythm slider.
- **Karaoke sync.** Real chunk-level highlighting — the word/sentence being heard is marked inside the active paragraph, the rest is dimmed as it advances.
- **Multi-window.** Open multiple documents side-by-side, each with its own editor state.
- **Chromium extension.** Click the Yappy icon in your toolbar to read the cleaned (defuddled) content of any page. The extension ships bundled inside the app — install via Preferences → Browser Extension.
- **Mini-player.** Stays out of the way while audio plays. Pin / pause / restart / scrub ±15s / save as `.wav`.
- **Spanish normalization.** Numbers, Roman numerals (siglo XX → veinte), abbreviations (pág. → página, a.C. → antes de Cristo), and dates are spelled out before synthesis.
- **Project autosave.** Edits, voice/speed/pause overrides, rhythm slider are saved per file — reopen the document and your work comes back.

## Install (pre-built)

Once the GitHub Actions release workflow has built a tagged version, grab the artifact for your platform:
- macOS: `.dmg` (Apple Silicon + Intel)
- Windows: `.msi` (installer) or portable `.exe`
- Linux: `.AppImage` or `.deb`

## Build from source

You'll need [Rust](https://rustup.rs/) (stable), [Node](https://nodejs.org/) 20+, and `npm`.

```bash
git clone https://github.com/joseluissaorin/yappy.git
cd yappy/yappy-app
npm install
npm run tauri dev
```

The first launch downloads the Supertonic 3 model (~380 MB) into the platform's standard data directory:
- macOS: `~/Library/Application Support/com.yappy.app/models/`
- Windows: `%APPDATA%\com.yappy.app\models\`
- Linux: `~/.local/share/com.yappy.app/models/`

### Production build

```bash
# macOS — uses the strongest signing identity in your keychain.
./scripts/build-mac.sh

# After a successful build with a Developer ID Application cert:
./scripts/notarize.sh
```

For Windows/Linux see `.github/workflows/release.yml` — push a `v*.*.*` tag or trigger the workflow manually.

## Architecture

```
yappy/
├── crates/
│   └── yappy-core/        Pure-Rust synthesis pipeline (text normalize → chunk →
│                          Supertonic 3 ONNX inference). No Tauri / UI bindings.
├── yappy-app/
│   ├── src/               SvelteKit frontend (home, /document, /player routes).
│   ├── src-tauri/         Tauri 2 backend — commands, settings, playback, bridge,
│   │                      capture (selection / OCR / browser), document loader.
│   └── resources/         Bundled artifacts:
│       ├── paddleocr/     PP-OCRv4 ONNX (det, cls, rec) ~15 MB
│       ├── pdfium/        PDFium shared libs for macOS / Windows / Linux ~22 MB
│       ├── extension/     Chromium extension (MV3) shipped inside the .app
│       └── licenses/      Third-party license texts
├── extension/             Source of the Chromium extension (mirrored into
│                          yappy-app/resources/ during build).
├── scripts/               Build + notarize helpers.
└── .github/workflows/     CI: release builds for macOS / Windows / Linux.
```

## License

Yappy is [MIT](./LICENSE) licensed. The bundled components keep their own licenses:
- **Supertonic 3** model weights — OpenRAIL-M (see `yappy-app/resources/licenses/`)
- **PaddleOCR** ONNX — Apache 2.0
- **PDFium** — BSD-3-Clause
- **Defuddle** — MIT
- Fonts (Quicksand, Patrick Hand) — SIL OFL 1.1

## Contributing

Bug reports, PRs, voice samples, normalization improvements (especially for non-English/Spanish), and translations all welcome. See [CONTRIBUTING.md](./CONTRIBUTING.md).

## Credits

Built with [Tauri 2](https://tauri.app/), [SvelteKit](https://kit.svelte.dev/), and [ONNX Runtime](https://onnxruntime.ai/). Named voices use Supertonic 3's style embeddings.
