//! Dynamic hotkey registration with parsing of human-readable combos like `⌥⌘R`,
//! `cmd+alt+space`, `ctrl+shift+f1`.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::commands;
use crate::settings::SettingsStore;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    ReadNow,
    PauseResume,
    ReadClipboard,
}

pub fn parse_combo(combo: &str) -> Result<Shortcut> {
    let mut mods = Modifiers::empty();
    let mut key: Option<Code> = None;
    let normalized = combo.to_lowercase();
    let tokens: Vec<&str> = if normalized.contains('+') {
        normalized.split('+').map(str::trim).collect()
    } else {
        // chord notation like "⌥⌘R" or "cmd alt r"
        let mut tmp: Vec<String> = Vec::new();
        let mut cur = String::new();
        for ch in combo.chars() {
            // glyph-based separators
            if matches!(ch, '⌘' | '⌃' | '⌥' | '⇧' | '⎈' | '⇧') {
                if !cur.is_empty() {
                    tmp.push(cur.clone());
                    cur.clear();
                }
                tmp.push(ch.to_string());
            } else if ch.is_whitespace() {
                if !cur.is_empty() {
                    tmp.push(cur.clone());
                    cur.clear();
                }
            } else {
                cur.push(ch);
            }
        }
        if !cur.is_empty() {
            tmp.push(cur);
        }
        return parse_tokens(tmp.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice());
    };
    for t in tokens {
        match t {
            "cmd" | "command" | "meta" | "super" | "win" | "⌘" => mods |= Modifiers::SUPER,
            "ctrl" | "control" | "⌃" => mods |= Modifiers::CONTROL,
            "alt" | "option" | "opt" | "⌥" => mods |= Modifiers::ALT,
            "shift" | "⇧" => mods |= Modifiers::SHIFT,
            "" => {}
            other => {
                key = Some(key_code_from(other)?);
            }
        }
    }
    let Some(k) = key else {
        return Err(anyhow!("no key in shortcut '{combo}'"));
    };
    Ok(Shortcut::new(Some(mods), k))
}

fn parse_tokens(tokens: &[&str]) -> Result<Shortcut> {
    let mut mods = Modifiers::empty();
    let mut key: Option<Code> = None;
    for t in tokens {
        let lower = t.to_lowercase();
        match lower.as_str() {
            "cmd" | "command" | "meta" | "super" | "win" | "⌘" => mods |= Modifiers::SUPER,
            "ctrl" | "control" | "⌃" => mods |= Modifiers::CONTROL,
            "alt" | "option" | "opt" | "⌥" => mods |= Modifiers::ALT,
            "shift" | "⇧" => mods |= Modifiers::SHIFT,
            _ => key = Some(key_code_from(&lower)?),
        }
    }
    let Some(k) = key else {
        return Err(anyhow!("no key in shortcut"));
    };
    Ok(Shortcut::new(Some(mods), k))
}

fn key_code_from(s: &str) -> Result<Code> {
    Ok(match s {
        "a" => Code::KeyA, "b" => Code::KeyB, "c" => Code::KeyC, "d" => Code::KeyD,
        "e" => Code::KeyE, "f" => Code::KeyF, "g" => Code::KeyG, "h" => Code::KeyH,
        "i" => Code::KeyI, "j" => Code::KeyJ, "k" => Code::KeyK, "l" => Code::KeyL,
        "m" => Code::KeyM, "n" => Code::KeyN, "o" => Code::KeyO, "p" => Code::KeyP,
        "q" => Code::KeyQ, "r" => Code::KeyR, "s" => Code::KeyS, "t" => Code::KeyT,
        "u" => Code::KeyU, "v" => Code::KeyV, "w" => Code::KeyW, "x" => Code::KeyX,
        "y" => Code::KeyY, "z" => Code::KeyZ,
        "0" => Code::Digit0, "1" => Code::Digit1, "2" => Code::Digit2, "3" => Code::Digit3,
        "4" => Code::Digit4, "5" => Code::Digit5, "6" => Code::Digit6, "7" => Code::Digit7,
        "8" => Code::Digit8, "9" => Code::Digit9,
        "f1" => Code::F1, "f2" => Code::F2, "f3" => Code::F3, "f4" => Code::F4,
        "f5" => Code::F5, "f6" => Code::F6, "f7" => Code::F7, "f8" => Code::F8,
        "f9" => Code::F9, "f10" => Code::F10, "f11" => Code::F11, "f12" => Code::F12,
        "space" | "spacebar" | "spc" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "backspace" => Code::Backspace,
        "minus" | "-" => Code::Minus,
        "equal" | "equals" | "=" => Code::Equal,
        "comma" | "," => Code::Comma,
        "period" | "." => Code::Period,
        "slash" | "/" => Code::Slash,
        "backslash" | "\\" => Code::Backslash,
        "semicolon" | ";" => Code::Semicolon,
        "quote" | "'" => Code::Quote,
        "left" | "arrow_left" => Code::ArrowLeft,
        "right" | "arrow_right" => Code::ArrowRight,
        "up" | "arrow_up" => Code::ArrowUp,
        "down" | "arrow_down" => Code::ArrowDown,
        other => return Err(anyhow!("unknown key '{other}'")),
    })
}

pub fn unregister_all<R: tauri::Runtime>(handle: &tauri::AppHandle<R>) {
    let _ = handle.global_shortcut().unregister_all();
}

pub fn register_from_settings<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    state: &Arc<AppState>,
) -> Result<()> {
    unregister_all(handle);

    let (read_combo, pause_combo) = {
        let s = state.settings.lock().unwrap();
        (s.hotkey_read_now.clone(), s.hotkey_pause_resume.clone())
    };

    let read_shortcut = parse_combo(&read_combo).unwrap_or_else(|_| {
        Shortcut::new(
            Some(Modifiers::ALT | Modifiers::SUPER),
            Code::KeyR,
        )
    });
    let pause_shortcut = parse_combo(&pause_combo).unwrap_or_else(|_| {
        Shortcut::new(
            Some(Modifiers::ALT | Modifiers::SUPER),
            Code::Space,
        )
    });

    let app1 = handle.clone();
    let state1 = state.clone();
    let read_sc = read_shortcut.clone();
    handle.global_shortcut().on_shortcut(
        read_shortcut,
        move |_app, sc, ev| {
            if ev.state == ShortcutState::Pressed && *sc == read_sc {
                let h = app1.clone();
                let s = state1.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = commands::trigger_read_now(h, s).await {
                        tracing::error!("read_now: {e:?}");
                    }
                });
            }
        },
    )?;

    let app2 = handle.clone();
    let state2 = state.clone();
    let pause_sc = pause_shortcut.clone();
    handle.global_shortcut().on_shortcut(
        pause_shortcut,
        move |_app, sc, ev| {
            if ev.state == ShortcutState::Pressed && *sc == pause_sc {
                let h = app2.clone();
                let s = state2.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = commands::toggle_pause(h, s).await;
                });
            }
        },
    )?;

    Ok(())
}

pub fn set_hotkey<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    state: &Arc<AppState>,
    action: Action,
    combo: String,
) -> Result<()> {
    // Validate first.
    let _ = parse_combo(&combo).map_err(|e| anyhow!("invalid combo '{combo}': {e}"))?;
    {
        let mut s = state.settings.lock().unwrap();
        match action {
            Action::ReadNow => s.hotkey_read_now = combo,
            Action::PauseResume => s.hotkey_pause_resume = combo,
            Action::ReadClipboard => {} // future
        }
        let snap = s.clone();
        drop(s);
        SettingsStore::save(handle, &snap)?;
    }
    register_from_settings(handle, state)?;
    Ok(())
}
