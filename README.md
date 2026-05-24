<div align="center">

# Yappy

**Read anything aloud.**

A local-first desktop app that turns *anything you read* into clean, expressive speech — selections, documents, web pages, screenshots — without ever leaving your computer. No API keys, no cloud, no telemetry.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release builds](https://github.com/joseluissaorin/yappy/actions/workflows/release.yml/badge.svg)](https://github.com/joseluissaorin/yappy/actions/workflows/release.yml)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20·%20Windows%20·%20Linux-cream)](https://github.com/joseluissaorin/yappy/releases)

[Download](https://github.com/joseluissaorin/yappy/releases) · [Build from source](#build-from-source) · [Architecture](#architecture)

</div>

---

## What is Yappy?

Yappy started from a frustration with **two** things at once. The first is the obvious one: every decent text-to-speech tool either sounds robotic, requires an internet connection, charges per character, or all three. The second is more practical — even when you've paid for a good one, **the UX is wrong**. I want to listen to an article while I'm cooking. While I'm at the gym. While I'm walking somewhere. I don't want to copy-paste text into a web form, click a button, wait for a download, open a separate audio player, and then realize I missed a sentence and have to start over. I want one keystroke, the audio starts, and I can put the phone in my pocket and go.

Yappy is what that should feel like. You press `⌥⌘R` from anywhere on your Mac. It figures out what you're trying to read — text you've selected, a PDF you have open, a Wikipedia page in the browser tab you're looking at, even just whatever's on screen as a screenshot — captures it, normalizes it (numbers and Roman numerals get spelled out, abbreviations expanded), routes it through [Supertonic 3](https://github.com/Supertone-inc/supertonic) running locally on your machine, and the audio starts playing within a second. Nothing leaves your computer.

For longer-form reading there's a **full audiobook editor**: open a `.pdf` or `.docx` or `.md` or `.epub`, and Yappy parses the structure, extracts the text (PDF text or OCR for scanned pages), splits it into paragraphs with chapter-aware navigation, and lets you edit each paragraph inline before reading. You can override the voice, speed, or pre-pause for individual paragraphs; render the whole thing or just selected chapters to a single `.wav` file. Headings get auto-applied pauses and slightly slower speech for that natural narrator cadence.

### Why now: the Spanish-quality wall finally cracked

I'm a Spanish speaker. Until very recently, decent-quality TTS in Spanish was something only the big paid APIs could deliver. Open models existed, but they were either huge (5B+ parameters — unrunnable on consumer hardware), English-only, or sounded like a navigation system from 2007. **Supertonic 3 is the first open model under 100M parameters that produces genuinely decent Spanish** — and the same is true for the 30 other languages it ships with. "Under 100M" matters: it means the model runs comfortably on any laptop made in the last five years, fits in 380 MB on disk, and starts streaming audio in under a second on Apple Silicon. That's the threshold where local-first TTS stops being a research curiosity and becomes a usable everyday tool.

### Accessibility tool, not gadget

Being able to *hear* what you read is foundational for a lot of people — dyslexia, low vision, fatigue after a long day of screen reading, language learners practicing pronunciation, anyone with attention issues that make eyes-on-text harder than ears-on-audio. Tools that solve this should be **free**, work **without an internet connection**, and not require **a credit card**. Yappy is [MIT-licensed](LICENSE) and ships every piece bundled — the voice model, the OCR engine, the PDF parser, the browser extension — so you can clone it once, build it, and never need this repo again.

## Inspired by Handy

The aesthetic, the hotkey-first interaction model, and the "one focused job, done well, fully local" philosophy all draw directly from [Handy](https://github.com/cjpais/Handy) — the Whisper-based dictation tool by [Christopher James Pais](https://github.com/cjpais). Where Handy makes your microphone instantly become text, Yappy makes any text instantly become voice. Same axis, opposite direction. If you like one you'll probably like the other.

The Handy-style cream + hot-pink + hand-drawn-mascot look isn't a copy — it's an explicit homage. The Patrick Hand display font, the Quicksand body font, the soft-shadowed buttons, the speech-bubble-dog mascot — it's the same design language pointed at a different problem.

## Features

### Read anywhere

- **One hotkey, four sources.** `⌥⌘R` reads — in order of priority — the current text selection, the cleaned-up content of the page in your focused browser (via the bundled Chromium extension), the active document of whatever app you're in (Word, Pages, Preview, Notes, …), or a screen-OCR fallback. Whichever is available.
- **Clipboard reader.** `⌥⌘V` reads whatever's on your clipboard, no app context needed.
- **Drop-to-read.** Drag any document file onto the main window and it opens in the document editor.

### The document editor

This is the part that has surprised people the most. Open a file and you get a full audiobook-grade editor:

- **Inline paragraph editing** — fix a typo or a weird OCR character, then play the updated text.
- **Per-paragraph overrides**: voice, speed, pre-pause silence. Each paragraph gets its own settings drawer.
- **Markdown-aware rhythm**: `#` headings (h1-h6), bullet lists, blockquotes, and horizontal rules get appropriate pauses and slight speed adjustments automatically. You feel the structure when listening.
- **Chapter sidebar** with click-to-jump navigation. Indents by heading level. Highlights the chapter currently playing.
- **Render selected chapters** to `.wav`. Tick the chapters you want; the rest is skipped.
- **Real karaoke sync** — both the *playing paragraph* AND the *playing chunk inside that paragraph* are highlighted live, derived from audio sample position rather than synth completion (which races way ahead of playback). The pink-marked phrase is what you're hearing *right now*.
- **Find & replace · undo/redo · project autosave** — edits survive quitting the app.
- **Rhythm slider** — multiplies the whole markdown rhythm so you can speed-read or slow-read the same doc without disturbing your global voice settings.

### Voices

Ten named voices distilled from Supertonic 3's style embeddings — Alex, James, Robert, Sam, Daniel (men); Sarah, Lily, Jessica, Olivia, Emily (women). Each one is fluent in **31 languages**, with automatic per-paragraph language detection so you can read mixed-language documents without ceremony.

### Format support

| Format | Engine | Notes |
|--------|--------|-------|
| Plain text · markdown · CSV | direct read | Markdown structure → reading rhythm |
| `.pdf` | [pdf_oxide](https://crates.io/crates/pdf_oxide) | 5× faster than pdf-extract, 100% pass rate on 3,830-PDF corpus |
| Scanned PDF | [PDFium](https://pdfium.googlesource.com/pdfium/) rasterize → [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) | All bundled — no `brew install` step needed |
| `.docx` · `.odt` · `.rtf` · `.html` · `.pptx` · `.xlsx` | [anytomd](https://crates.io/crates/anytomd) | Pure Rust |
| `.epub` | [epub crate](https://crates.io/crates/epub) | Chapters joined |
| Browser pages | [Defuddle](https://github.com/kepano/defuddle) (in the bundled Chromium extension) | Strips ads, sidebars, footers; removes Wikipedia's link duplication |
| Screen OCR | [Apple Vision](https://developer.apple.com/documentation/vision) (macOS) or PaddleOCR (cross-platform) | |

### Multi-window

Each opened file is its own document window with its own editor state. Keep one paper open while editing another.

### Spanish-aware normalization

Because the author's daily-driver test cases are Spanish lit papers: numbers ("1492" → "mil cuatrocientos noventa y dos"), Roman numerals in century context ("siglo XX" → "siglo veinte"), abbreviations (`pág.`, `art.`, `cap.`, `a.C.`, `d.C.`, `op. cit.`, `Av.`, `Avda.`, `S.A.`, `vol.`, ordinals like `1º 2ª 3er`), and dates are all expanded before synthesis.

## Install

### Pre-built (recommended)

Grab the latest [release](https://github.com/joseluissaorin/yappy/releases) for your platform:

- **macOS Apple Silicon** — `Yappy_X.Y.Z_macos_arm64_notarized.dmg` — fully notarized, opens cleanly.
- **macOS Intel** — `Yappy_X.Y.Z_x64.dmg`
- **Windows x64** — `Yappy_X.Y.Z_x64-setup.exe` or `.msi`
- **Linux x64** — `yappy_X.Y.Z_amd64.AppImage` or `.deb`

First launch downloads the Supertonic 3 voice model (~380 MB) into your platform's standard app-data directory. After that Yappy runs offline forever.

### Build from source

You'll need [Rust](https://rustup.rs/) (stable), [Node](https://nodejs.org/) 20+, and `npm`. On Linux you also need `webkit2gtk-4.1-dev`, `libssl-dev`, `libayatana-appindicator3-dev` (the [Tauri prereqs](https://v2.tauri.app/start/prerequisites/#linux)).

```bash
git clone https://github.com/joseluissaorin/yappy.git
cd yappy/yappy-app
npm install
npm run tauri dev      # dev mode with hot-reload
```

For a production build:

```bash
./scripts/build-mac.sh                # macOS — auto-picks the strongest signing identity
./scripts/notarize.sh                 # Apple notarization (requires Developer ID)
# Or, for any platform:
cd yappy-app && npm run tauri build
```

CI cuts releases for all four platforms via [`.github/workflows/release.yml`](.github/workflows/release.yml) — push a `v*.*.*` tag and Tauri's official action handles the rest.

## Architecture

```
yappy/
├── crates/
│   └── yappy-core/        Pure-Rust synthesis pipeline. No Tauri / UI deps.
│                          Text normalize → language-detect → paragraph chunk
│                          → Supertonic 3 ONNX inference → AudioChunk stream.
│                          Could become a standalone library.
├── yappy-app/
│   ├── src/               SvelteKit frontend, Svelte 5 runes mode.
│   │   ├── routes/
│   │   │   ├── +page          Home dashboard
│   │   │   ├── player/+page   Floating mini-player
│   │   │   └── document/+page Audiobook editor
│   │   └── lib/               Shared components + IPC bindings
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── commands.rs    Tauri command handlers (~80 of them)
│   │   │   ├── playback.rs    Audio thread, chunk-queue, karaoke tracking
│   │   │   ├── capture/       Selection, OCR, browser, doc loader
│   │   │   ├── bridge.rs      WebSocket bridge to chromium extension
│   │   │   ├── settings.rs    Persisted user prefs (atomic writes)
│   │   │   ├── state.rs       Shared AppState (HashMap<window, doc>)
│   │   │   └── windows.rs     Multi-window lifecycle
│   │   └── icons/             App + tray icons
│   ├── resources/
│   │   ├── paddleocr/         PP-OCRv4 ONNX (det · cls · rec) ~15 MB
│   │   ├── pdfium/            libpdfium for macOS · Windows · Linux ~22 MB
│   │   ├── extension/         Chromium MV3 extension shipped bundled
│   │   ├── defuddle.js        Clean-page extractor
│   │   └── licenses/          Third-party license texts
│   └── macos/yappy-ocr.swift  Apple Vision OCR helper (macOS only)
├── extension/                 Source of the bundled Chromium extension
├── scripts/                   Build + notarize helpers
└── .github/workflows/         CI: native build matrix for all 4 platforms
```

## Performance

- **PDF parse**: 0.8 ms mean per page via `pdf_oxide`. An 8-page digital PDF reads in ~30 ms.
- **Synthesis**: Supertonic 3 produces audio chunks ~5–10× faster than playback on Apple Silicon, so the audio buffer stays well ahead of the listener. Streaming starts within ~400 ms of pressing the hotkey.
- **Memory**: ~80 MB resident idle, ~250 MB while actively reading a long document.

## Privacy

- **No telemetry. No analytics. No phone-home.** Yappy never makes outbound HTTP calls for analytics purposes.
- **Models download once** from Hugging Face on first launch, then never again.
- **The Chromium extension** talks only to `127.0.0.1:47898` (your local Yappy app). It never sends page content to a remote server.
- **Apple's notarization service** sees a hash of the binary (one-time, at build time, by the author) — that's the only Apple involvement.

## Credits

- **[Supertonic 3](https://github.com/Supertone-inc/supertonic)** — the 99M-parameter ONNX TTS that does the heavy lifting. OpenRAIL-M licensed.
- **[Handy](https://github.com/cjpais/Handy)** — for proving the "local · keyboard-driven · friendly mascot" desktop-utility template works, and for the design language Yappy openly borrows.
- **[Defuddle](https://github.com/kepano/defuddle)** — Kepano's clean-page extractor.
- **[PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)** — the PP-OCRv4 mobile ONNX models.
- **[PDFium](https://pdfium.googlesource.com/pdfium/)** — Google's PDF engine via the [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) distribution and the [pdfium-render](https://crates.io/crates/pdfium-render) crate.
- **[pdf_oxide](https://crates.io/crates/pdf_oxide)** — the fast pure-Rust PDF text extractor.
- **[Tauri](https://tauri.app/)** + **[SvelteKit](https://kit.svelte.dev/)** — the desktop shell.
- **[ONNX Runtime](https://onnxruntime.ai/)** via the [ort](https://crates.io/crates/ort) crate.
- Fonts: [Quicksand](https://fonts.google.com/specimen/Quicksand) and [Patrick Hand](https://fonts.google.com/specimen/Patrick+Hand), both SIL OFL.

## Contributing

Bug reports, PRs, voice quality improvements, normalization rules for other languages, and translations are all welcome. See [CONTRIBUTING.md](./CONTRIBUTING.md) for the quick-start. Areas where help is especially needed:

- **Non-English/Spanish normalization** — German, French, Italian, Japanese, Korean, …
- **Windows / Linux capture features** — the macOS-only `cfg`-gated capture code (selection, active-document AppleScript, Apple Vision OCR) needs platform-specific implementations.
- **`.m4b` audiobook export** — `.wav` render works; AAC + chapter markers is the natural next step.
- **Pronunciation dictionary** UI + apply-at-synth pipeline.

## License

[MIT](./LICENSE). The bundled components keep their own licenses; the full texts live in `yappy-app/resources/licenses/`.

---

<div align="center">
<sub>Built because reading aloud should be a feature of the world, not a subscription.</sub>
</div>
