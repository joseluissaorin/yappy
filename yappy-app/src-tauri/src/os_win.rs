//! Windows-native helpers. Mirrors the iOS `mobile.rs` pattern: each
//! function is callable from cross-platform code; the implementations live
//! behind `cfg(target_os = "windows")` and stubbed elsewhere.
//!
//! What's here:
//!   - **SMTC (System Media Transport Controls)** — the Windows equivalent
//!     of macOS Now Playing / iOS Lock Screen controls. Surfaces Yappy's
//!     audio in the volume flyout, lets media keys + Bluetooth headphone
//!     buttons drive playback.
//!   - **Taskbar progress** — ITaskbarList3::SetProgressValue paints a
//!     progress bar over the Yappy icon in the taskbar during long
//!     audiobook renders.
//!
//! Things deliberately NOT here (require more involved patterns):
//!   - Jump List (ICustomDestinationList COM dance, future work)
//!   - Mica/Acrylic backdrop (configured via tauri.windows.conf.json)
//!   - File associations (declared in tauri.windows.conf.json)
//!   - Toast notifications (already covered by tauri-plugin-notification
//!     which uses WinRT toasts on Windows)

#![allow(unused_variables)]

use std::sync::Arc;

use crate::state::AppState;

// ─── Cross-platform stubs (compiled on every target) ───────────────────

#[cfg(not(target_os = "windows"))]
pub fn smtc_set_metadata(_title: &str, _artist: &str, _is_playing: bool) {}
#[cfg(not(target_os = "windows"))]
pub fn smtc_set_playback_status(_is_playing: bool) {}
#[cfg(not(target_os = "windows"))]
pub fn smtc_clear() {}
#[cfg(not(target_os = "windows"))]
pub fn install_smtc_handlers(_playback: Arc<crate::playback::PlaybackController>, _hwnd: isize) {}
#[cfg(not(target_os = "windows"))]
pub fn taskbar_progress_set(_value: u64, _total: u64) {}
#[cfg(not(target_os = "windows"))]
pub fn taskbar_progress_clear() {}
#[cfg(not(target_os = "windows"))]
pub fn front_app_name() -> Option<String> { None }
#[cfg(not(target_os = "windows"))]
pub fn active_window_text() -> Option<String> { None }
#[cfg(not(target_os = "windows"))]
pub fn clipboard_read_text() -> Option<String> { None }
#[cfg(not(target_os = "windows"))]
pub fn clipboard_write_text(_text: &str) -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn clipboard_sequence_number() -> i64 { 0 }
#[cfg(not(target_os = "windows"))]
pub fn send_ctrl_c() -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn capture_foreground_window_png(_out: &std::path::Path) -> anyhow::Result<()> {
    anyhow::bail!("capture_foreground_window_png: not available on this OS")
}

// ─── Windows implementations ───────────────────────────────────────────

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::{Mutex, OnceLock};

    use windows::Foundation::TypedEventHandler;
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoInitializeEx, CoCreateInstance, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
    use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList, TBPF_NORMAL, TBPF_NOPROGRESS};
    use windows::core::Interface;

    /// Stored SMTC instance — created on first metadata push, kept alive for
    /// the lifetime of the process so handler subscriptions stay valid.
    static SMTC: OnceLock<Mutex<Option<SystemMediaTransportControls>>> = OnceLock::new();
    static TASKBAR: OnceLock<Mutex<Option<ITaskbarList3>>> = OnceLock::new();
    static MAIN_HWND: OnceLock<Mutex<Option<HWND>>> = OnceLock::new();

    fn ensure_com_init() {
        // Idempotent — Tauri also calls CoInitializeEx; calling it again on
        // an already-initialized thread is documented to return RPC_E_CHANGED_MODE
        // which we just swallow.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    /// Get-or-create the singleton SMTC instance for the main window.
    fn smtc_get_or_create() -> Option<SystemMediaTransportControls> {
        let cell = SMTC.get_or_init(|| Mutex::new(None));
        let mut guard = cell.lock().ok()?;
        if let Some(s) = guard.as_ref() {
            return Some(s.clone());
        }
        ensure_com_init();
        let hwnd_guard = MAIN_HWND.get()?.lock().ok()?;
        let hwnd = (*hwnd_guard)?;
        unsafe {
            let interop: ISystemMediaTransportControlsInterop = windows::core::factory::<
                SystemMediaTransportControls,
                ISystemMediaTransportControlsInterop,
            >()
            .ok()?;
            let smtc: SystemMediaTransportControls = interop.GetForWindow(hwnd).ok()?;
            smtc.SetIsEnabled(true).ok()?;
            smtc.SetIsPlayEnabled(true).ok()?;
            smtc.SetIsPauseEnabled(true).ok()?;
            smtc.SetIsNextEnabled(false).ok()?;
            smtc.SetIsPreviousEnabled(false).ok()?;
            tracing::info!("smtc: created for HWND {hwnd:?}");
            *guard = Some(smtc.clone());
            Some(smtc)
        }
    }

    pub fn set_metadata(title: &str, artist: &str, is_playing: bool) {
        let Some(smtc) = smtc_get_or_create() else { return };
        let title_h = windows::core::HSTRING::from(title);
        let artist_h = windows::core::HSTRING::from(artist);
        unsafe {
            if let Ok(updater) = smtc.DisplayUpdater() {
                let _ = updater.SetType(MediaPlaybackType::Music);
                if let Ok(props) = updater.MusicProperties() {
                    let _ = props.SetTitle(&title_h);
                    let _ = props.SetArtist(&artist_h);
                }
                let _ = updater.Update();
            }
            let _ = smtc.SetPlaybackStatus(if is_playing {
                MediaPlaybackStatus::Playing
            } else {
                MediaPlaybackStatus::Paused
            });
        }
    }

    pub fn set_playback_status(is_playing: bool) {
        let Some(smtc) = smtc_get_or_create() else { return };
        unsafe {
            let _ = smtc.SetPlaybackStatus(if is_playing {
                MediaPlaybackStatus::Playing
            } else {
                MediaPlaybackStatus::Paused
            });
        }
    }

    pub fn clear() {
        let Some(smtc) = smtc_get_or_create() else { return };
        unsafe {
            let _ = smtc.SetPlaybackStatus(MediaPlaybackStatus::Stopped);
            let _ = smtc.SetIsEnabled(false);
        }
    }

    pub fn install_handlers(
        playback: super::Arc<crate::playback::PlaybackController>,
        hwnd_raw: isize,
    ) {
        // Stash the HWND so smtc_get_or_create can wire it on first call.
        let cell = MAIN_HWND.get_or_init(|| Mutex::new(None));
        if let Ok(mut g) = cell.lock() {
            *g = Some(HWND(hwnd_raw as _));
        }
        let Some(smtc) = smtc_get_or_create() else { return };

        // Move a clone of the playback Arc into the event handler.
        let pb_for_buttons = playback.clone();
        let handler = TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_sender, args| {
            let pb = pb_for_buttons.clone();
            if let Some(args) = args.as_ref() {
                if let Ok(btn) = args.Button() {
                    match btn {
                        SystemMediaTransportControlsButton::Play => pb.resume(),
                        SystemMediaTransportControlsButton::Pause => pb.pause(),
                        SystemMediaTransportControlsButton::Stop => pb.stop(),
                        SystemMediaTransportControlsButton::FastForward => pb.seek(15.0),
                        SystemMediaTransportControlsButton::Rewind => pb.seek(-15.0),
                        _ => {}
                    }
                }
            }
            Ok(())
        });
        if let Err(e) = unsafe { smtc.ButtonPressed(&handler) } {
            tracing::warn!("smtc: ButtonPressed subscribe failed: {e:?}");
        } else {
            tracing::info!("smtc: button handlers wired");
        }
    }

    fn taskbar_get_or_create() -> Option<ITaskbarList3> {
        let cell = TASKBAR.get_or_init(|| Mutex::new(None));
        let mut guard = cell.lock().ok()?;
        if let Some(t) = guard.as_ref() {
            return Some(t.clone());
        }
        ensure_com_init();
        unsafe {
            let tl: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
            tl.HrInit().ok()?;
            *guard = Some(tl.clone());
            Some(tl)
        }
    }

    pub fn taskbar_progress_set(value: u64, total: u64) {
        let Some(tb) = taskbar_get_or_create() else { return };
        let hwnd_guard = MAIN_HWND.get().and_then(|c| c.lock().ok());
        let Some(g) = hwnd_guard else { return };
        let Some(hwnd) = *g else { return };
        unsafe {
            let _ = tb.SetProgressState(hwnd, TBPF_NORMAL);
            let _ = tb.SetProgressValue(hwnd, value, total);
        }
    }

    pub fn taskbar_progress_clear() {
        let Some(tb) = taskbar_get_or_create() else { return };
        let hwnd_guard = MAIN_HWND.get().and_then(|c| c.lock().ok());
        let Some(g) = hwnd_guard else { return };
        let Some(hwnd) = *g else { return };
        unsafe {
            let _ = tb.SetProgressState(hwnd, TBPF_NOPROGRESS);
        }
    }

    // ─── Front-app detection (Win32, no UIA needed) ─────────────────────
    //
    // Mirrors macOS's `osascript`-based NSWorkspace front-app query. Fast
    // path: GetForegroundWindow → process ID → exe path → file_name.

    use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    pub fn front_app_name() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; MAX_PATH as usize];
            let len = GetModuleFileNameExW(Some(handle), None, &mut buf);
            let _ = CloseHandle(handle);
            if len == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let name = std::path::Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())?;
            Some(name)
        }
    }

    // ─── UI Automation text extraction ───────────────────────────────────
    //
    // The macOS equivalent here is `active_doc::active_document_text` which
    // uses app-specific AppleScript to ask Safari / Pages / Notes for the
    // visible text. On Windows, the universal pattern is UI Automation
    // (UIA) — a COM accessibility API that exposes a tree of UI elements
    // with text patterns. We:
    //   1. Get the foreground HWND.
    //   2. Resolve it to an IUIAutomationElement via ElementFromHandle.
    //   3. Try to pull a SELECTED text range first (matches macOS's "give
    //      me what the user is looking at" preference).
    //   4. Fall back to "visible ranges" — the on-screen viewport text.
    //   5. Final fall-back: the window title (better than nothing).
    //
    // Works in: Word, Notion, Edge, Chrome, Firefox, Notepad, VS Code,
    // Adobe Reader, anything that opts into UI Automation properly. Some
    // games / electron apps with no accessibility tree return nothing,
    // and we fall through to OCR in the capture chain.

    // ─── Native clipboard + SendInput (no PowerShell flash) ─────────────
    //
    // Yappy's "smart capture" pipeline previously spawned powershell.exe
    // four times per Ctrl+Alt+R press: clipboard snapshot, SendKeys ⌃C,
    // clipboard read, clipboard restore. Each spawn briefly flashes a
    // console window AND adds ~100-300ms latency. Native Win32 = zero
    // flash, sub-ms call cost.

    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };

    const VK_CONTROL: VIRTUAL_KEY = VIRTUAL_KEY(0x11);
    const VK_C: VIRTUAL_KEY = VIRTUAL_KEY(0x43);

    pub fn clipboard_sequence_number() -> i64 {
        unsafe { GetClipboardSequenceNumber() as i64 }
    }

    pub fn clipboard_read_text() -> Option<String> {
        unsafe {
            OpenClipboard(None).ok()?;
            // RAII-style: ensure we always close. Using a closure + immediate
            // call means errors short-circuit but we still hit the close.
            let out = (|| -> Option<String> {
                let h = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
                let hglobal = windows::Win32::Foundation::HGLOBAL(h.0);
                let ptr = GlobalLock(hglobal);
                if ptr.is_null() {
                    return None;
                }
                // Find UTF-16 null terminator.
                let p = ptr as *const u16;
                let mut len = 0usize;
                while *p.add(len) != 0 {
                    len += 1;
                    if len > 50_000_000 {
                        break; // sanity cap
                    }
                }
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                let _ = GlobalUnlock(hglobal);
                if s.is_empty() { None } else { Some(s) }
            })();
            let _ = CloseClipboard();
            out
        }
    }

    pub fn clipboard_write_text(text: &str) -> bool {
        unsafe {
            // UTF-16 with explicit null terminator.
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let size_bytes = wide.len() * std::mem::size_of::<u16>();
            let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, size_bytes) else {
                return false;
            };
            let dst = GlobalLock(hmem);
            if dst.is_null() {
                return false;
            }
            std::ptr::copy_nonoverlapping(
                wide.as_ptr() as *const u8,
                dst as *mut u8,
                size_bytes,
            );
            let _ = GlobalUnlock(hmem);
            if OpenClipboard(None).is_err() {
                return false;
            }
            let _ = EmptyClipboard();
            // SetClipboardData takes ownership of hmem on success.
            let ok = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0 as _))).is_ok();
            let _ = CloseClipboard();
            ok
        }
    }

    /// Synthesise Ctrl+C via SendInput — the Windows equivalent of macOS's
    /// CGEvent ⌘C in `capture/selection.rs`. No PowerShell flash, sub-ms.
    pub fn send_ctrl_c() -> bool {
        let make = |vk: VIRTUAL_KEY, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        // Down: Ctrl, C. Up: C, Ctrl. Matches the Windows-OS-internal order
        // that apps actually expect to see (some apps misbehave with the
        // wrong release order).
        let inputs = [
            make(VK_CONTROL, false),
            make(VK_C, false),
            make(VK_C, true),
            make(VK_CONTROL, true),
        ];
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == inputs.len() as u32
        }
    }

    // ─── Native screen capture (GDI BitBlt → PNG) ───────────────────────
    //
    // Replaces the last remaining PowerShell call in the capture pipeline
    // (was `powershell.exe -Command Add-Type System.Drawing; BitBlt...`).
    // Faster (no .NET startup), no console window even with
    // CREATE_NO_WINDOW, and we control the exact pixel format.
    //
    // Captures the FOREGROUND window's bounds, matching macOS's behavior
    // (osascript / NSWorkspace get the front-app's frame). Falls back to
    // the full virtual screen if GetWindowRect returns garbage (e.g. when
    // the foreground window is the system shell with no rect).

    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    };
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowRect, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    pub fn capture_foreground_window_png(out: &std::path::Path) -> anyhow::Result<()> {
        unsafe {
            // 1) Pick a capture rect. Prefer the foreground window's frame
            //    (focused-window OCR, matches macOS). Fall back to the full
            //    virtual screen if GetWindowRect doesn't return something
            //    reasonable.
            let fg = GetForegroundWindow();
            let (x, y, w, h) = {
                let mut r = RECT::default();
                if !fg.0.is_null() && GetWindowRect(fg, &mut r).is_ok()
                    && r.right > r.left && r.bottom > r.top
                {
                    (r.left, r.top, r.right - r.left, r.bottom - r.top)
                } else {
                    (
                        GetSystemMetrics(SM_XVIRTUALSCREEN),
                        GetSystemMetrics(SM_YVIRTUALSCREEN),
                        GetSystemMetrics(SM_CXVIRTUALSCREEN),
                        GetSystemMetrics(SM_CYVIRTUALSCREEN),
                    )
                }
            };
            if w <= 0 || h <= 0 {
                anyhow::bail!("capture: nonsense capture dimensions {w}x{h}");
            }

            // 2) Allocate compatible DC + bitmap, BitBlt the screen into it.
            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                anyhow::bail!("capture: GetDC(NULL) failed");
            }
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            if mem_dc.is_invalid() {
                ReleaseDC(None, screen_dc);
                anyhow::bail!("capture: CreateCompatibleDC failed");
            }
            let hbm = CreateCompatibleBitmap(screen_dc, w, h);
            if hbm.is_invalid() {
                let _ = DeleteDC(mem_dc);
                ReleaseDC(None, screen_dc);
                anyhow::bail!("capture: CreateCompatibleBitmap failed");
            }
            let old = SelectObject(mem_dc, hbm.into());
            let blt_ok = BitBlt(mem_dc, 0, 0, w, h, Some(screen_dc), x, y, SRCCOPY).is_ok();
            if !blt_ok {
                let _ = SelectObject(mem_dc, old);
                let _ = DeleteObject(hbm.into());
                let _ = DeleteDC(mem_dc);
                ReleaseDC(None, screen_dc);
                anyhow::bail!("capture: BitBlt failed");
            }

            // 3) GetDIBits into a 32-bit BGRA buffer. biHeight negative ⇒
            //    top-down rows so we don't need to flip later.
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;
            let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
            let read = GetDIBits(
                mem_dc,
                hbm,
                0,
                h as u32,
                Some(buf.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // 4) GDI cleanup before we get to the PNG encode (let go of all
            //    the kernel handles ASAP).
            let _ = SelectObject(mem_dc, old);
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);

            if read == 0 {
                anyhow::bail!("capture: GetDIBits read 0 rows");
            }

            // 5) BGRA → RGBA in place. Then encode PNG via the `image`
            //    crate (already a workspace dep).
            for chunk in buf.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            let img = image::RgbaImage::from_raw(w as u32, h as u32, buf)
                .ok_or_else(|| anyhow::anyhow!("capture: pixel buffer size mismatch"))?;
            img.save(out)?;
            Ok(())
        }
    }

    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement,
        IUIAutomationTextPattern, UIA_TextPatternId, TreeScope_Subtree,
    };

    pub fn active_window_text() -> Option<String> {
        unsafe {
            ensure_com_init();
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let uia: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let elem: IUIAutomationElement = uia.ElementFromHandle(hwnd).ok()?;

            // Walk the subtree looking for an element that supports the
            // TextPattern. ElementFromHandle returns the top-level frame;
            // the document/editor that actually has text is usually a few
            // levels down.
            let condition = uia.CreateTrueCondition().ok()?;
            let descendants = elem.FindAll(TreeScope_Subtree, &condition).ok()?;
            let count = descendants.Length().ok()?;

            // Prefer SELECTED text from the first element that has any.
            for i in 0..count {
                let Ok(d) = descendants.GetElement(i) else { continue };
                let Ok(pat_unknown) = d.GetCurrentPattern(UIA_TextPatternId) else { continue };
                let Ok(text_pattern) = pat_unknown.cast::<IUIAutomationTextPattern>() else { continue };
                if let Ok(selection) = text_pattern.GetSelection() {
                    if let Ok(sel_len) = selection.Length() {
                        if sel_len > 0 {
                            let mut combined = String::new();
                            for s in 0..sel_len {
                                if let Ok(r) = selection.GetElement(s) {
                                    if let Ok(t) = r.GetText(-1) {
                                        let txt = t.to_string();
                                        if !txt.trim().is_empty() {
                                            if !combined.is_empty() { combined.push('\n'); }
                                            combined.push_str(&txt);
                                        }
                                    }
                                }
                            }
                            if !combined.trim().is_empty() {
                                tracing::info!("uia: extracted {} chars from selection", combined.len());
                                return Some(combined);
                            }
                        }
                    }
                }
            }

            // No selection — try the FIRST element's "visible ranges" which
            // gives us roughly the on-screen viewport. Stops at the first
            // element with non-trivial text content.
            for i in 0..count {
                let Ok(d) = descendants.GetElement(i) else { continue };
                let Ok(pat_unknown) = d.GetCurrentPattern(UIA_TextPatternId) else { continue };
                let Ok(text_pattern) = pat_unknown.cast::<IUIAutomationTextPattern>() else { continue };
                if let Ok(ranges) = text_pattern.GetVisibleRanges() {
                    if let Ok(r_len) = ranges.Length() {
                        let mut combined = String::new();
                        for r in 0..r_len {
                            if let Ok(range) = ranges.GetElement(r) {
                                if let Ok(t) = range.GetText(-1) {
                                    let txt = t.to_string();
                                    if !txt.trim().is_empty() {
                                        if !combined.is_empty() { combined.push('\n'); }
                                        combined.push_str(&txt);
                                    }
                                }
                            }
                        }
                        if combined.trim().chars().count() > 50 {
                            // 50-char threshold avoids picking up tiny
                            // toolbar labels and finding the actual content.
                            tracing::info!("uia: extracted {} chars from visible viewport", combined.len());
                            return Some(combined);
                        }
                    }
                }
            }

            None
        }
    }
}

#[cfg(target_os = "windows")]
pub fn smtc_set_metadata(title: &str, artist: &str, is_playing: bool) {
    imp::set_metadata(title, artist, is_playing);
}
#[cfg(target_os = "windows")]
pub fn smtc_set_playback_status(is_playing: bool) {
    imp::set_playback_status(is_playing);
}
#[cfg(target_os = "windows")]
pub fn smtc_clear() {
    imp::clear();
}
#[cfg(target_os = "windows")]
pub fn install_smtc_handlers(playback: Arc<crate::playback::PlaybackController>, hwnd: isize) {
    imp::install_handlers(playback, hwnd);
}
#[cfg(target_os = "windows")]
pub fn taskbar_progress_set(value: u64, total: u64) {
    imp::taskbar_progress_set(value, total);
}
#[cfg(target_os = "windows")]
pub fn taskbar_progress_clear() {
    imp::taskbar_progress_clear();
}
#[cfg(target_os = "windows")]
pub fn front_app_name() -> Option<String> {
    imp::front_app_name()
}
#[cfg(target_os = "windows")]
pub fn active_window_text() -> Option<String> {
    imp::active_window_text()
}
#[cfg(target_os = "windows")]
pub fn clipboard_read_text() -> Option<String> { imp::clipboard_read_text() }
#[cfg(target_os = "windows")]
pub fn clipboard_write_text(text: &str) -> bool { imp::clipboard_write_text(text) }
#[cfg(target_os = "windows")]
pub fn clipboard_sequence_number() -> i64 { imp::clipboard_sequence_number() }
#[cfg(target_os = "windows")]
pub fn send_ctrl_c() -> bool { imp::send_ctrl_c() }
#[cfg(target_os = "windows")]
pub fn capture_foreground_window_png(out: &std::path::Path) -> anyhow::Result<()> {
    imp::capture_foreground_window_png(out)
}
