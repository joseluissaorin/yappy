# Yappy Desktop — Linux + Windows verification session brief

You're picking up a port from a macOS session that touched a lot of
platform-specific code it couldn't actually run. Your job: build Yappy on
the host platform (Linux or Windows), run the test plan in §4, and report
back what works / breaks. If something obviously needs fixing, fix it.

## 1. What changed in this session

Recent commits (all on `main`, see `git log --oneline`):

  - **ONNX Runtime hardware acceleration EPs** — `lib.rs` now registers a
    priority list of execution providers per platform. The list is logged
    to `yappy.log` at startup; first line you should check is which EPs got
    picked up.
      - Windows default: **DirectML → XNNPACK → CPU**. DirectML routes ORT
        ops to any DX12 GPU (NVIDIA, AMD, Intel, Qualcomm). No extra
        install — DirectML.dll ships with Windows 10 1903+.
      - Linux default: **XNNPACK → CPU**.
      - Opt-in via Cargo features (rebuild required):
          - `--features cuda` — NVIDIA GPUs on Linux + Windows. Needs CUDA
            Toolkit + cuDNN at build time. Huge speedup.
          - `--features tensorrt` — NVIDIA + TensorRT for ops that map.
          - `--features rocm` — AMD GPUs on Linux. Needs ROCm install.
          - `--features openvino` — Intel CPU/iGPU/dGPU acceleration.

  - **Windows-native depth** (`os_win.rs` module, gated to Windows):
      - **System Media Transport Controls (SMTC)** — wires the Yappy main
        window's HWND into Windows' media-transport system. Media keys on
        keyboards (Play/Pause, Next/Prev), Bluetooth headphone buttons,
        and the volume-flyout playback widget all drive Yappy playback.
        Mirrors what `NowPlaying.swift` does on iOS / macOS.
      - **Taskbar progress indicator** — during multi-hour audiobook
        renders, the Yappy taskbar icon paints a progress bar. Uses
        `ITaskbarList3::SetProgressValue`.
      - **Mica window backdrop** (Win 11+) — set via
        `tauri.windows.conf.json` window effects.
      - **Single-instance** — second launch (e.g. double-clicking a .epub
        with Yappy registered as default) focuses the existing window and
        forwards the file path via a `file_open_request` event.
      - **File associations** — Yappy registers as a handler for `.epub`,
        `.pdf`, `.docx`, `.txt`, `.md` at install time (NSIS + WiX).
      - **WebView2 bootstrapper embedded** — `embedBootstrapper` mode +
        `silent: true` so users without WebView2 Runtime preinstalled get
        it during Yappy installation, no manual step.

  - **Linux GTK / WebKit fixes** in `lib.rs::run()` startup:
      - Sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` (silences the "black
        rectangle webview" bug on Intel UHD / Nvidia GBM / Mesa < 23).
      - Sets `YAPPY_HOTKEYS_UNSUPPORTED=wayland` if `WAYLAND_DISPLAY` is
        present so the settings UI can surface a helpful explanation.

  - **`tauri.windows.conf.json`** — embeds the WebView2 bootstrapper at
    install time and configures NSIS + WiX with sensible defaults.

  - **`tauri.linux.conf.json`** — declares apt/rpm runtime deps explicitly
    (webkit2gtk-4.1, ayatana-appindicator, alsa, xdo, openssl).

  - **`pdfium-render` path resolution** in `capture/doc_loader.rs` —
    walks more candidate paths on Linux: `$APPDIR/usr/lib/yappy/`,
    `/usr/lib/yappy/`, etc. for `.deb`/`.rpm`/AppImage installs.

  - The iOS-only paths (`mobile.rs`, `gen/apple/`, Live Activity, Share
    Sheet, library tab, etc.) are all behind `#[cfg(mobile)]` — they
    won't compile into your Linux/Windows binary. Don't worry about them.

## 2. Build setup

### Linux (Ubuntu 22.04+ / Fedora 39+ / Arch / Debian 12+)

System deps (Ubuntu/Debian):

```bash
sudo apt update
sudo apt install -y \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev libssl-dev libxdo-dev libasound2-dev \
    pkg-config build-essential curl
```

Fedora:

```bash
sudo dnf install -y \
    webkit2gtk4.1-devel gtk3-devel libayatana-appindicator-gtk3-devel \
    librsvg2-devel openssl-devel libxdo-devel alsa-lib-devel \
    pkg-config gcc-c++ make curl
```

Rust + Tauri CLI:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install tauri-cli --version "^2" --locked
```

Node 20 + npm deps:

```bash
nvm install 20 || curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
cd yappy-app && npm ci && cd ..
```

Build:

```bash
cd yappy-app
cargo tauri build              # debug + release artifacts at target/release/bundle/
# For CUDA on NVIDIA:
cargo tauri build -- --features cuda
```

### Windows 11 / Windows 10 1903+

Prereqs (winget):

```powershell
winget install -e --id Microsoft.VisualStudio.2022.BuildTools --silent --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install -e --id Rustlang.Rustup
winget install -e --id OpenJS.NodeJS.LTS
winget install -e --id Microsoft.EdgeWebView2Runtime  # usually preinstalled
```

After install: open a fresh PowerShell so PATH picks up Rust + Node.

```powershell
cargo install tauri-cli --version "^2" --locked
cd yappy-app
npm ci
cargo tauri build
# Artifacts at: src-tauri\target\release\bundle\msi\ and \nsis\
# For CUDA on NVIDIA:
cargo tauri build -- --features cuda
```

For CUDA: install CUDA Toolkit 12.x and set `CUDA_PATH` env var BEFORE
`cargo tauri build`. ORT's build script reads it.

## 3. ONNX EP verification

After first launch, open the log file. The first line of interest:

```
ort: registered execution providers (priority order): DirectML → XNNPACK
```

Log location:
- Linux: `~/.local/share/com.yappy.app/yappy.log`
- Windows: `%LOCALAPPDATA%\com.yappy.app\yappy.log`

If the line says only `XNNPACK` on a machine that has a GPU, the GPU EP
failed to initialize at runtime — likely missing driver or runtime lib.
ORT prints diagnostics on the next few lines of the log.

To force-disable hardware acceleration (debugging):

```bash
ORT_DISABLE_ALL_HW=1 ./yappy
```

(Not yet wired — file an issue.)

## 4. Test plan (check each item)

### 4.1 First-launch smoke

  - [ ] App icon appears in the application launcher / Start menu
  - [ ] Double-clicking it opens the main window within 5s
  - [ ] Onboarding modal appears with the macOS-style copy ("for your mac"
        — iOS-only branch should NOT show on desktop)
  - [ ] "set up browsers" CTA is visible (desktop feature, hidden on iOS)
  - [ ] Tray icon appears (Linux: only on tray-supporting DEs — Cinnamon,
        KDE, XFCE, recent GNOME with extension)

### 4.2 Voice model download

  - [ ] Click "download voices" — 380 MB download starts with progress bar
  - [ ] Progress updates smoothly (no UI freeze)
  - [ ] Completion lands at the main play screen

### 4.3 Read-something flow

  - [ ] Open a Wikipedia article in your browser
  - [ ] Select a paragraph
  - [ ] Hit ⌥⌘R (macOS) / Ctrl+Alt+R (Linux/Windows) — Yappy reads it
  - [ ] Player window appears bottom-right (or wherever you positioned it)
  - [ ] Pause/resume/stop in the floating player work
  - [ ] Lock the screen on Linux: playback continues (audio background)
  - [ ] Wayland sessions: the hotkey is expected to NOT work — verify the
        Settings page surfaces the "Wayland: hotkeys unsupported" warning

### 4.4 Document import

  - [ ] Drag-and-drop a PDF onto the main window: opens in editor
  - [ ] Drag a .epub: opens
  - [ ] Drag a scanned PDF: pdfium loads, OCR runs (watch CPU spike), text appears
  - [ ] Linux: check log for `pdfium dylib: …` line — should resolve to the
        bundled libpdfium.so under `/usr/lib/yappy/` or AppImage equivalent
  - [ ] Windows: same for `pdfium.dll` under the install dir

### 4.5 ONNX hardware acceleration

  - [ ] Launch with a representative document (10+ paragraphs)
  - [ ] Note synthesis latency per paragraph (logged at INFO level)
  - [ ] On a machine with a GPU, latency should be noticeably better than
        CPU-only — typical M-series gets ~0.5s/paragraph, an RTX 3060 with
        CUDA should hit ~0.2s/paragraph, an Intel UHD with DirectML around
        ~0.4s/paragraph
  - [ ] Check the log: `ort: registered execution providers (priority order):`
        line lists the EPs you expect

### 4.6 Audiobook render

  - [ ] Open a longer document (book chapter, 50+ paragraphs)
  - [ ] Hit "Render audiobook" — choose .m4b
  - [ ] Render progress bar updates
  - [ ] When done, the .m4b plays in VLC / mpv / Apple Books (test outside Yappy)
  - [ ] Chapter markers appear in the player (one per heading in the source doc)
  - [ ] On Linux: AppImage rendering works (some Linux distros have weird
        /tmp permissions — verify the temp .m4b path is writable)

### 4.7 Browser extension bridge

  - [ ] Settings → "set up browsers" — extension folder reveals in file manager
  - [ ] Load unpacked in Chrome/Edge/Vivaldi → see paired chip
  - [ ] Click Yappy's read-now button from the extension toolbar: extracted
        clean article text gets read

### 4.8 Settings persistence

  - [ ] Change voice, speed, volume
  - [ ] Quit and relaunch — settings stick
  - [ ] Linux: `~/.config/com.yappy.app/settings.json` exists
  - [ ] Windows: `%APPDATA%\com.yappy.app\settings.json` exists

### 4.9 Tray menu / system integration

  - [ ] Tray menu items: "Read clipboard", "Pause/Resume", "Stop", "Quit"
  - [ ] All work when invoked from the tray
  - [ ] App can be quit cleanly via tray "Quit" (no zombie process)
  - [ ] Linux: tray icon survives DE restarts (test by killing gnome-shell)

### 4.10 Windows-specific (skip on Linux)

  - [ ] **UI Automation active-window text extraction**: open a Word doc,
        select a paragraph, hit Ctrl+Alt+R. Yappy should read that exact
        selection — verify in `yappy.log`: `uia: extracted N chars from
        selection`. **Not OCR** (which would take seconds and log a
        screencapture step). Try in Edge, Chrome, Notepad, Notion, VS
        Code, Adobe Reader — they all expose UIA trees. (Some games and
        broken Electron apps don't, and fall through to OCR — that's OK.)
  - [ ] **Front-app detection**: with Word foregrounded, `yappy.log`
        capture-source line should mention `app_name: "WINWORD"` or
        similar (process exe filename without extension). Without the
        UIA work this used to be None on Windows.
  - [ ] **Capture chain order**: selection → browser-extension paired tab
        → UIA (Windows) / AppleScript (macOS) → OCR. Verify each step
        kicks in only when prior fails, by closing the browser then
        repeating the hotkey on Word.
  - [ ] **SMTC**: start a TTS read. The Windows volume flyout (taskbar
        speaker icon → upper area) now shows "Yappy" with play/pause.
        Press Play/Pause on a keyboard's media key → playback pauses.
  - [ ] Pair Bluetooth headphones, tap the play button on them → toggles
        playback. The button-press handler in `os_win.rs` is mapped to
        `pb.pause() / pb.resume()`.
  - [ ] **Taskbar progress**: start an audiobook render. The Yappy taskbar
        icon should paint a green progress bar that ticks up. Clears when
        render completes.
  - [ ] **Mica backdrop** (Windows 11): the main window's background has
        the translucent Mica effect with the desktop wallpaper subtly
        showing through. Win 10 falls back to a solid background.
  - [ ] **File association**: right-click a `.epub` file → "Open with" →
        "Yappy" appears. Choose it. Yappy opens with that file loaded.
        Second double-click of a .epub while Yappy's already running
        should NOT spawn a duplicate process — it should focus the
        existing window and load the new file.
  - [ ] **WebView2 bootstrap**: install Yappy on a fresh Windows 10 LTSC
        VM (no WebView2 runtime). The installer should pull WebView2 down
        silently and Yappy should launch first time.

### 4.11 Edge cases

  - [ ] Plug in headphones during playback — audio switches device cleanly
        (cpal handles this on Linux/Windows; if it doesn't, log the issue)
  - [ ] Disconnect network during voice model download — see helpful error,
        offer retry
  - [ ] Open a 200+ page PDF — doesn't OOM
  - [ ] Render audiobook for a 50,000-word document — completes within
        reasonable time (~10-30 min depending on hardware)

## 5. Known issues (don't waste time on these)

  - **Wayland global hotkeys**: tauri-plugin-global-shortcut silently
    no-ops. The `YAPPY_HOTKEYS_UNSUPPORTED=wayland` env var is set; the
    Settings UI is supposed to surface this. If it doesn't, that's worth
    fixing.
  - **Old GNOME tray**: GNOME 3.26+ removed system-tray support. Users
    need the [TopIcons Plus](https://extensions.gnome.org/extension/1031/topicons/)
    extension or Yappy's tray won't appear. Document — don't fix.
  - **WebView2 on Windows 10 LTSC 1809**: must install the WebView2
    Runtime separately. The embed-bootstrapper config in
    `tauri.windows.conf.json` should make the installer do this
    automatically; verify by running the installer on a fresh Win10 VM.
  - **PulseAudio on WSLg**: cpal-on-Linux-on-Windows is finicky. If you're
    testing in WSLg, expect glitchy audio — that's a known WSL limitation
    not a Yappy bug.

## 6. Where to file findings

  - PRs to https://github.com/joseluissaorin/yappy
  - Or update this file with `## 7. <YourPlatform> session findings` and
    push back to main

## 7. macOS findings (reference)

  - Apple Silicon + iOS Simulator: ✅ all green (this is where v0.1.0
    shipped from). The CoreML EP via Neural Engine knocks ~30% off
    synthesis latency vs CPU-only.
  - macOS Intel: untested on this session (no Intel Mac available).
    Should work — CoreML EP requires no GPU on Intel, falls back to CPU
    automatically. Worth verifying on macos-13 if you ever resurrect that
    runner.
