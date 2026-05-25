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
