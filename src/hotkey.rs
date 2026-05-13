//! Global hotkey listener via rdev.
//!
//! # macOS note
//! `rdev::listen` must run on the **main thread** on macOS (CoreGraphics
//! event tap constraint). `HotkeyManager::spawn` creates a background thread,
//! which is correct for Linux / Windows. On macOS the App state machine
//! (Phase 8) will keep `rdev::listen` on `main()` and pass events back via
//! the same channel.

use rdev::{listen, EventType, Key};
use std::collections::HashSet;
use std::sync::Mutex;
use tokio::sync::mpsc;

use crate::app::AppEvent;
use crate::config::{HotkeyConfig, HotkeyTrigger};

// ---------------------------------------------------------------------------
// Modifier abstraction
// ---------------------------------------------------------------------------

/// Logical modifier group — either key of a left/right pair counts as active.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModifierGroup {
    Ctrl,
    Shift,
    Alt,
    Meta,
    MetaRight,
}

impl ModifierGroup {
    pub(crate) fn is_active(&self, pressed: &HashSet<Key>) -> bool {
        match self {
            Self::Ctrl      => pressed.contains(&Key::ControlLeft) || pressed.contains(&Key::ControlRight),
            Self::Shift     => pressed.contains(&Key::ShiftLeft)   || pressed.contains(&Key::ShiftRight),
            Self::Alt       => pressed.contains(&Key::Alt)         || pressed.contains(&Key::AltGr),
            Self::Meta      => pressed.contains(&Key::MetaLeft)    || pressed.contains(&Key::MetaRight),
            Self::MetaRight => pressed.contains(&Key::MetaRight),
        }
    }

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "ctrl" | "control"                                          => Ok(Self::Ctrl),
            "shift"                                                     => Ok(Self::Shift),
            "alt" | "option" | "opt"                                    => Ok(Self::Alt),
            "meta" | "cmd" | "command" | "win" | "super"                => Ok(Self::Meta),
            "metacright" | "rightcmd" | "rcmd" | "rightcommand" | "cmdright" => Ok(Self::MetaRight),
            other => anyhow::bail!("Unknown modifier: '{}'", other),
        }
    }
}

// ---------------------------------------------------------------------------
// Key string → rdev::Key
// ---------------------------------------------------------------------------

pub(crate) fn parse_key(s: &str) -> anyhow::Result<Key> {
    let key = match s {
        // Function keys
        "F1"  => Key::F1,  "F2"  => Key::F2,  "F3"  => Key::F3,
        "F4"  => Key::F4,  "F5"  => Key::F5,  "F6"  => Key::F6,
        "F7"  => Key::F7,  "F8"  => Key::F8,  "F9"  => Key::F9,
        "F10" => Key::F10, "F11" => Key::F11, "F12" => Key::F12,
        // Special keys
        "CapsLock"                   => Key::CapsLock,
        "Space"                      => Key::Space,
        "Tab"                        => Key::Tab,
        "Return" | "Enter"           => Key::Return,
        "Escape" | "Esc"             => Key::Escape,
        "Backspace"                  => Key::Backspace,
        "Delete"  | "Del"            => Key::Delete,
        "Home"                       => Key::Home,
        "End"                        => Key::End,
        "PageUp"                     => Key::PageUp,
        "PageDown"                   => Key::PageDown,
        "UpArrow"    | "Up"          => Key::UpArrow,
        "DownArrow"  | "Down"        => Key::DownArrow,
        "LeftArrow"  | "Left"        => Key::LeftArrow,
        "RightArrow" | "Right"       => Key::RightArrow,
        // Meta / Command / Win / Super keys (standalone, not as modifier)
        "RightCmd" | "RightCommand" | "RightMeta" | "RightWin" | "RightSuper" => Key::MetaRight,
        "LeftCmd"  | "LeftCommand"  | "LeftMeta"  | "LeftWin"  | "LeftSuper"  => Key::MetaLeft,
        // Single letter or digit (case-insensitive)
        s if s.len() == 1 => match s.to_uppercase().as_str() {
            "A" => Key::KeyA, "B" => Key::KeyB, "C" => Key::KeyC,
            "D" => Key::KeyD, "E" => Key::KeyE, "F" => Key::KeyF,
            "G" => Key::KeyG, "H" => Key::KeyH, "I" => Key::KeyI,
            "J" => Key::KeyJ, "K" => Key::KeyK, "L" => Key::KeyL,
            "M" => Key::KeyM, "N" => Key::KeyN, "O" => Key::KeyO,
            "P" => Key::KeyP, "Q" => Key::KeyQ, "R" => Key::KeyR,
            "S" => Key::KeyS, "T" => Key::KeyT, "U" => Key::KeyU,
            "V" => Key::KeyV, "W" => Key::KeyW, "X" => Key::KeyX,
            "Y" => Key::KeyY, "Z" => Key::KeyZ,
            "0" => Key::Num0, "1" => Key::Num1, "2" => Key::Num2,
            "3" => Key::Num3, "4" => Key::Num4, "5" => Key::Num5,
            "6" => Key::Num6, "7" => Key::Num7, "8" => Key::Num8,
            "9" => Key::Num9,
            other => anyhow::bail!("Unknown key character: '{}'", other),
        },
        other => anyhow::bail!(
            "Unknown key: '{}'. Supported: F1-F12, CapsLock, Space, Tab, Return, \
             Escape, Backspace, Delete, Home, End, PageUp, PageDown, arrow keys, A-Z, 0-9",
            other
        ),
    };
    Ok(key)
}

// ---------------------------------------------------------------------------
// Core event logic — extracted for unit testability
// ---------------------------------------------------------------------------

/// Update `pressed` and return `true` if this event triggers the hotkey.
pub(crate) fn process_event(
    event_type: &EventType,
    target_key: Key,
    required_modifiers: &[ModifierGroup],
    pressed: &mut HashSet<Key>,
) -> bool {
    match event_type {
        EventType::KeyPress(key) => {
            pressed.insert(*key);
            *key == target_key && required_modifiers.iter().all(|mg| mg.is_active(pressed))
        }
        EventType::KeyRelease(key) => {
            pressed.remove(key);
            false
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub struct HotkeyManager;

fn parse_config(config: &HotkeyConfig) -> anyhow::Result<(Key, Vec<ModifierGroup>, Option<u64>)> {
    let target_key = parse_key(&config.key)
        .map_err(|e| anyhow::anyhow!("Invalid hotkey key '{}': {}", config.key, e))?;
    let required_modifiers: Vec<ModifierGroup> = config
        .modifiers
        .iter()
        .map(|m| ModifierGroup::from_str(m))
        .collect::<anyhow::Result<_>>()?;
    let double_tap_ms = (config.trigger == HotkeyTrigger::DoubleTap).then_some(config.double_tap_ms);
    Ok((target_key, required_modifiers, double_tap_ms))
}

fn make_rdev_callback(
    sender: mpsc::Sender<AppEvent>,
    target_key: Key,
    required_modifiers: Vec<ModifierGroup>,
    double_tap_ms: Option<u64>,
) -> impl FnMut(rdev::Event) {
    let pressed = Mutex::new(HashSet::<Key>::new());
    // Last time the target key was released; used only for double-tap mode.
    let last_release: Mutex<Option<std::time::Instant>> = Mutex::new(None);

    move |event: rdev::Event| {
        let Ok(mut p) = pressed.lock() else { return };

        // `process_event` updates `pressed` and returns true when the key combo fires.
        let single_fired = process_event(&event.event_type, target_key, &required_modifiers, &mut p);

        // For double-tap: track releases and require two presses within the window.
        let fire = if let Some(threshold) = double_tap_ms {
            match &event.event_type {
                rdev::EventType::KeyRelease(key) if *key == target_key => {
                    if let Ok(mut lr) = last_release.lock() {
                        *lr = Some(std::time::Instant::now());
                    }
                    false
                }
                rdev::EventType::KeyPress(_) if single_fired => {
                    let Ok(mut lr) = last_release.lock() else { return };
                    let is_double = lr
                        .map(|t| t.elapsed().as_millis() < threshold as u128)
                        .unwrap_or(false);
                    *lr = None;
                    is_double
                }
                _ => false,
            }
        } else {
            single_fired
        };

        if fire {
            tracing::debug!("Hotkey fired — sending HotkeyPressed");
            let _ = sender.blocking_send(AppEvent::HotkeyPressed);
        }
    }
}

impl HotkeyManager {
    /// Parse config and start the hotkey listener in a background thread.
    /// On Linux / Windows only — do NOT call on macOS (rdev requires main thread).
    pub fn spawn(sender: mpsc::Sender<AppEvent>, config: &HotkeyConfig) -> anyhow::Result<()> {
        let (target_key, required_modifiers, double_tap_ms) = parse_config(config)?;
        tracing::info!(key = %config.key, modifiers = ?config.modifiers,
            trigger = ?config.trigger, "Registering hotkey");

        std::thread::Builder::new()
            .name("hotkey-listener".to_string())
            .spawn(move || {
                let result = listen(make_rdev_callback(sender, target_key, required_modifiers, double_tap_ms));
                if let Err(e) = result {
                    tracing::error!("rdev listener exited: {:?}", e);
                }
            })?;

        Ok(())
    }

    /// Run the hotkey listener on the **current** thread (blocking).
    ///
    /// Required on macOS: `rdev::listen` uses `CGEventTap` which only delivers
    /// events when called from the main thread's run loop.
    pub fn run_on_current_thread(
        sender: mpsc::Sender<AppEvent>,
        config: &HotkeyConfig,
    ) -> anyhow::Result<()> {
        let (target_key, required_modifiers, double_tap_ms) = parse_config(config)?;
        tracing::info!(key = %config.key, modifiers = ?config.modifiers,
            trigger = ?config.trigger, "Registering hotkey (main thread)");

        if let Err(e) = listen(make_rdev_callback(sender, target_key, required_modifiers, double_tap_ms)) {
            tracing::error!("rdev listener exited: {:?}", e);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ----- parse_key -----

    #[test]
    fn parse_function_keys() {
        assert_eq!(parse_key("F4").unwrap(), Key::F4);
        assert_eq!(parse_key("F1").unwrap(), Key::F1);
        assert_eq!(parse_key("F12").unwrap(), Key::F12);
    }

    #[test]
    fn parse_special_keys() {
        assert_eq!(parse_key("CapsLock").unwrap(), Key::CapsLock);
        assert_eq!(parse_key("Space").unwrap(), Key::Space);
        assert_eq!(parse_key("Return").unwrap(), Key::Return);
        assert_eq!(parse_key("Enter").unwrap(), Key::Return);
        assert_eq!(parse_key("Esc").unwrap(), Key::Escape);
        assert_eq!(parse_key("Left").unwrap(), Key::LeftArrow);
        assert_eq!(parse_key("PageUp").unwrap(), Key::PageUp);
    }

    #[test]
    fn parse_letter_keys_case_insensitive() {
        assert_eq!(parse_key("A").unwrap(), Key::KeyA);
        assert_eq!(parse_key("a").unwrap(), Key::KeyA);
        assert_eq!(parse_key("Z").unwrap(), Key::KeyZ);
    }

    #[test]
    fn parse_digit_keys() {
        assert_eq!(parse_key("0").unwrap(), Key::Num0);
        assert_eq!(parse_key("9").unwrap(), Key::Num9);
    }

    #[test]
    fn parse_key_unknown_is_err() {
        assert!(parse_key("XYZ").is_err());
        assert!(parse_key("").is_err());
        assert!(parse_key("@").is_err());
    }

    // ----- ModifierGroup -----

    #[test]
    fn modifier_from_str_all_aliases() {
        assert_eq!(ModifierGroup::from_str("Ctrl").unwrap(), ModifierGroup::Ctrl);
        assert_eq!(ModifierGroup::from_str("control").unwrap(), ModifierGroup::Ctrl);
        assert_eq!(ModifierGroup::from_str("SHIFT").unwrap(), ModifierGroup::Shift);
        assert_eq!(ModifierGroup::from_str("alt").unwrap(), ModifierGroup::Alt);
        assert_eq!(ModifierGroup::from_str("option").unwrap(), ModifierGroup::Alt);
        assert_eq!(ModifierGroup::from_str("cmd").unwrap(), ModifierGroup::Meta);
        assert_eq!(ModifierGroup::from_str("Meta").unwrap(), ModifierGroup::Meta);
        assert_eq!(ModifierGroup::from_str("win").unwrap(), ModifierGroup::Meta);
    }

    #[test]
    fn modifier_from_str_unknown_is_err() {
        assert!(ModifierGroup::from_str("HyperKey").is_err());
    }

    #[test]
    fn ctrl_accepts_left_and_right() {
        let mg = ModifierGroup::Ctrl;
        let mut p = HashSet::new();
        assert!(!mg.is_active(&p));
        p.insert(Key::ControlLeft);
        assert!(mg.is_active(&p));
        p.clear();
        p.insert(Key::ControlRight);
        assert!(mg.is_active(&p));
    }

    #[test]
    fn shift_accepts_left_and_right() {
        let mg = ModifierGroup::Shift;
        let mut p = HashSet::new();
        p.insert(Key::ShiftRight);
        assert!(mg.is_active(&p));
    }

    // ----- process_event -----

    fn press(key: Key) -> EventType { EventType::KeyPress(key) }
    fn release(key: Key) -> EventType { EventType::KeyRelease(key) }

    #[test]
    fn fires_on_target_key_alone() {
        let mut p = HashSet::new();
        assert!(process_event(&press(Key::F4), Key::F4, &[], &mut p));
    }

    #[test]
    fn does_not_fire_on_wrong_key() {
        let mut p = HashSet::new();
        assert!(!process_event(&press(Key::F5), Key::F4, &[], &mut p));
    }

    #[test]
    fn does_not_fire_on_key_release() {
        let mut p = HashSet::new();
        process_event(&press(Key::F4), Key::F4, &[], &mut p);
        assert!(!process_event(&release(Key::F4), Key::F4, &[], &mut p));
    }

    #[test]
    fn requires_modifier_held_before_target() {
        let mods = [ModifierGroup::Ctrl];
        let mut p = HashSet::new();

        // Target alone — no fire
        assert!(!process_event(&press(Key::F4), Key::F4, &mods, &mut p));

        // Hold Ctrl, then target — fire
        let mut p = HashSet::new();
        process_event(&press(Key::ControlLeft), Key::F4, &mods, &mut p);
        assert!(process_event(&press(Key::F4), Key::F4, &mods, &mut p));
    }

    #[test]
    fn releases_remove_key_from_pressed_set() {
        let mut p = HashSet::new();
        process_event(&press(Key::ControlLeft), Key::F4, &[], &mut p);
        assert!(p.contains(&Key::ControlLeft));
        process_event(&release(Key::ControlLeft), Key::F4, &[], &mut p);
        assert!(!p.contains(&Key::ControlLeft));
    }

    #[test]
    fn all_modifiers_must_be_held() {
        let mods = [ModifierGroup::Ctrl, ModifierGroup::Shift];
        let mut p = HashSet::new();

        // Only Ctrl — no fire
        process_event(&press(Key::ControlLeft), Key::F4, &mods, &mut p);
        assert!(!process_event(&press(Key::F4), Key::F4, &mods, &mut p));

        // Both Ctrl + Shift — fire
        let mut p = HashSet::new();
        process_event(&press(Key::ControlLeft), Key::F4, &mods, &mut p);
        process_event(&press(Key::ShiftLeft), Key::F4, &mods, &mut p);
        assert!(process_event(&press(Key::F4), Key::F4, &mods, &mut p));
    }

    #[test]
    fn non_key_events_are_ignored() {
        let mut p = HashSet::new();
        let mouse_move = EventType::MouseMove { x: 0.0, y: 0.0 };
        assert!(!process_event(&mouse_move, Key::F4, &[], &mut p));
    }
}
