# Contributing to Yappy

Thanks for considering a contribution. Yappy is small and pragmatic — keep PRs focused and we'll review quickly.

## Quick start

```bash
git clone https://github.com/joseluissaorin/yappy.git
cd yappy/yappy-app
npm install
npm run tauri dev
```

First run downloads the Supertonic 3 model (~380 MB). Audio playback, OCR, browser bridge all activate on first use.

## Project structure

- `crates/yappy-core/` — pure Rust synthesis (no Tauri / UI deps). Lives outside `yappy-app/` so it could become a stand-alone library.
- `yappy-app/src/routes/` — SvelteKit pages: `/` (home), `/player` (mini-player), `/document` (audiobook editor).
- `yappy-app/src-tauri/src/` — Tauri 2 backend.
- `extension/chromium/` — MV3 extension. The build copies it into `yappy-app/resources/extension/` automatically.

## Tests

```bash
cargo test --workspace          # core + app rust tests
cd yappy-app && npm run check   # svelte-check + tsc
```

## Code style

- Rust: `cargo fmt`, `cargo clippy -- -D warnings` before opening a PR.
- TypeScript/Svelte: stick to the existing style; no formatter is enforced yet.
- Comments: explain **why** something non-obvious is the way it is — not what each line does. Naming and types should carry the "what".

## Reporting bugs

When something stalls or behaves oddly, share:
1. The build ID shown bottom-right of the document window (e.g. `v0.1.0-doc-rev14-...`).
2. The last ~64 KB of `~/Library/Application Support/com.yappy.app/yappy.log` (Preferences → Diagnostics → "show last 64 kb").
3. The file you opened (if it's not private).

The log captures both backend tracing and frontend events.

## Areas where help is especially welcome

- **Normalization** for non-English/Spanish languages (German, French, Italian, Japanese, Korean, …) — see `crates/yappy-core/src/normalize.rs`.
- **Windows / Linux capture features.** Most macOS-only capture code (selection, active-document AppleScript, Apple Vision OCR) is `cfg(target_os = "macos")` gated; the Windows/Linux equivalents are open.
- **m4b audiobook export** with chapter markers. Render-to-WAV exists; AAC + chapter metadata is the next step.
- **Pronunciation dictionary** UI + apply-at-synth pipeline.
- **Drag-to-reorder** paragraphs in the document editor.

## License

By contributing you agree your code is MIT-licensed under the project's [LICENSE](./LICENSE).
