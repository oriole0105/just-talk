//! Global hotkey listener.
//!
//! # macOS note
//! macOS 26+ enforces `dispatch_assert_queue` inside TSM (Text Services Manager).
//! `rdev::listen` calls `TSMGetInputSourceProperty` from its event-tap callback
//! thread, which trips that assertion on any keypress → SIGTRAP.
//!
//! On macOS we bypass rdev entirely and poll `CGEventSourceKeyState` (a plain
//! thread-safe C function with no TSM calls) at 5 ms intervals instead.
//!
//! On Linux/Windows rdev works fine on a background thread.

#[cfg(not(target_os = "macos"))]
use rdev::listen;
use rdev::{EventType, Key};
use std::collections::HashSet;
#[cfg(not(target_os = "macos"))]
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
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(crate) fn is_active(&self, pressed: &HashSet<Key>) -> bool {
        match self {
            Self::Ctrl => {
                pressed.contains(&Key::ControlLeft) || pressed.contains(&Key::ControlRight)
            }
            Self::Shift => pressed.contains(&Key::ShiftLeft) || pressed.contains(&Key::ShiftRight),
            Self::Alt => pressed.contains(&Key::Alt) || pressed.contains(&Key::AltGr),
            Self::Meta => pressed.contains(&Key::MetaLeft) || pressed.contains(&Key::MetaRight),
            Self::MetaRight => pressed.contains(&Key::MetaRight),
        }
    }

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "ctrl" | "control" => Ok(Self::Ctrl),
            "shift" => Ok(Self::Shift),
            "alt" | "option" | "opt" => Ok(Self::Alt),
            "meta" | "cmd" | "command" | "win" | "super" => Ok(Self::Meta),
            "metacright" | "rightcmd" | "rcmd" | "rightcommand" | "cmdright" => Ok(Self::MetaRight),
            other => anyhow::bail!("Unknown modifier: '{}'", other),
        }
    }

    /// macOS virtual key codes for this modifier group (for polling).
    #[cfg(target_os = "macos")]
    fn vkcodes(&self) -> &'static [u16] {
        match self {
            Self::Ctrl => &[0x3B, 0x3E],  // Left/Right Control
            Self::Shift => &[0x38, 0x3C], // Left/Right Shift
            Self::Alt => &[0x3A, 0x3D],   // Left/Right Option
            Self::Meta => &[0x37, 0x36],  // Left/Right Command
            Self::MetaRight => &[0x36],   // Right Command only
        }
    }
}

// ---------------------------------------------------------------------------
// Key string → rdev::Key
// ---------------------------------------------------------------------------

pub(crate) fn parse_key(s: &str) -> anyhow::Result<Key> {
    let key = match s {
        // Function keys
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        // Special keys
        "CapsLock" => Key::CapsLock,
        "Space" => Key::Space,
        "Tab" => Key::Tab,
        "Return" | "Enter" => Key::Return,
        "Escape" | "Esc" => Key::Escape,
        "Backspace" => Key::Backspace,
        "Delete" | "Del" => Key::Delete,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "UpArrow" | "Up" => Key::UpArrow,
        "DownArrow" | "Down" => Key::DownArrow,
        "LeftArrow" | "Left" => Key::LeftArrow,
        "RightArrow" | "Right" => Key::RightArrow,
        // Meta / Command / Win / Super keys (standalone, not as modifier)
        "RightCmd" | "RightCommand" | "RightMeta" | "RightWin" | "RightSuper" => Key::MetaRight,
        "LeftCmd" | "LeftCommand" | "LeftMeta" | "LeftWin" | "LeftSuper" => Key::MetaLeft,
        // Single letter or digit (case-insensitive)
        s if s.len() == 1 => match s.to_uppercase().as_str() {
            "A" => Key::KeyA,
            "B" => Key::KeyB,
            "C" => Key::KeyC,
            "D" => Key::KeyD,
            "E" => Key::KeyE,
            "F" => Key::KeyF,
            "G" => Key::KeyG,
            "H" => Key::KeyH,
            "I" => Key::KeyI,
            "J" => Key::KeyJ,
            "K" => Key::KeyK,
            "L" => Key::KeyL,
            "M" => Key::KeyM,
            "N" => Key::KeyN,
            "O" => Key::KeyO,
            "P" => Key::KeyP,
            "Q" => Key::KeyQ,
            "R" => Key::KeyR,
            "S" => Key::KeyS,
            "T" => Key::KeyT,
            "U" => Key::KeyU,
            "V" => Key::KeyV,
            "W" => Key::KeyW,
            "X" => Key::KeyX,
            "Y" => Key::KeyY,
            "Z" => Key::KeyZ,
            "0" => Key::Num0,
            "1" => Key::Num1,
            "2" => Key::Num2,
            "3" => Key::Num3,
            "4" => Key::Num4,
            "5" => Key::Num5,
            "6" => Key::Num6,
            "7" => Key::Num7,
            "8" => Key::Num8,
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
// macOS virtual key code mapping (for CGEventSourceKeyState polling)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn key_to_vkcode(key: Key) -> Option<u16> {
    Some(match key {
        Key::F1 => 0x7A,
        Key::F2 => 0x78,
        Key::F3 => 0x63,
        Key::F4 => 0x76,
        Key::F5 => 0x60,
        Key::F6 => 0x61,
        Key::F7 => 0x62,
        Key::F8 => 0x64,
        Key::F9 => 0x65,
        Key::F10 => 0x6D,
        Key::F11 => 0x67,
        Key::F12 => 0x6F,
        Key::CapsLock => 0x39,
        Key::Space => 0x31,
        Key::Tab => 0x30,
        Key::Return => 0x24,
        Key::Escape => 0x35,
        Key::Backspace => 0x33,
        Key::Delete => 0x75,
        Key::Home => 0x73,
        Key::End => 0x77,
        Key::PageUp => 0x74,
        Key::PageDown => 0x79,
        Key::UpArrow => 0x7E,
        Key::DownArrow => 0x7D,
        Key::LeftArrow => 0x7B,
        Key::RightArrow => 0x7C,
        Key::MetaLeft => 0x37,
        Key::MetaRight => 0x36,
        Key::ControlLeft => 0x3B,
        Key::ControlRight => 0x3E,
        Key::ShiftLeft => 0x38,
        Key::ShiftRight => 0x3C,
        Key::Alt => 0x3A,   // Left Option
        Key::AltGr => 0x3D, // Right Option
        Key::KeyA => 0x00,
        Key::KeyB => 0x0B,
        Key::KeyC => 0x08,
        Key::KeyD => 0x02,
        Key::KeyE => 0x0E,
        Key::KeyF => 0x03,
        Key::KeyG => 0x05,
        Key::KeyH => 0x04,
        Key::KeyI => 0x22,
        Key::KeyJ => 0x26,
        Key::KeyK => 0x28,
        Key::KeyL => 0x25,
        Key::KeyM => 0x2E,
        Key::KeyN => 0x2D,
        Key::KeyO => 0x1F,
        Key::KeyP => 0x23,
        Key::KeyQ => 0x0C,
        Key::KeyR => 0x0F,
        Key::KeyS => 0x01,
        Key::KeyT => 0x11,
        Key::KeyU => 0x20,
        Key::KeyV => 0x09,
        Key::KeyW => 0x0D,
        Key::KeyX => 0x07,
        Key::KeyY => 0x10,
        Key::KeyZ => 0x06,
        Key::Num0 => 0x1D,
        Key::Num1 => 0x12,
        Key::Num2 => 0x13,
        Key::Num3 => 0x14,
        Key::Num4 => 0x15,
        Key::Num5 => 0x17,
        Key::Num6 => 0x16,
        Key::Num7 => 0x1A,
        Key::Num8 => 0x1C,
        Key::Num9 => 0x19,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Core event logic — extracted for unit testability (used on all platforms)
// ---------------------------------------------------------------------------

/// Update `pressed` and return `true` if this event triggers the hotkey.
#[cfg_attr(target_os = "macos", allow(dead_code))]
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
// Config parsing (shared)
// ---------------------------------------------------------------------------

pub(crate) fn parse_config(
    config: &HotkeyConfig,
) -> anyhow::Result<(Key, Vec<ModifierGroup>, Option<u64>)> {
    let target_key = parse_key(&config.key)
        .map_err(|e| anyhow::anyhow!("Invalid hotkey key '{}': {}", config.key, e))?;
    let required_modifiers: Vec<ModifierGroup> = config
        .modifiers
        .iter()
        .map(|m| ModifierGroup::from_str(m))
        .collect::<anyhow::Result<_>>()?;
    let double_tap_ms =
        (config.trigger == HotkeyTrigger::DoubleTap).then_some(config.double_tap_ms);
    Ok((target_key, required_modifiers, double_tap_ms))
}

// ---------------------------------------------------------------------------
// rdev callback (Linux / Windows only)
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
fn make_rdev_callback(
    sender: mpsc::Sender<AppEvent>,
    target_key: Key,
    required_modifiers: Vec<ModifierGroup>,
    double_tap_ms: Option<u64>,
) -> impl FnMut(rdev::Event) {
    let pressed = Mutex::new(HashSet::<Key>::new());
    let last_release: Mutex<Option<std::time::Instant>> = Mutex::new(None);

    move |event: rdev::Event| {
        let Ok(mut p) = pressed.lock() else { return };

        let single_fired =
            process_event(&event.event_type, target_key, &required_modifiers, &mut p);

        let fire = if let Some(threshold) = double_tap_ms {
            match &event.event_type {
                rdev::EventType::KeyRelease(key) if *key == target_key => {
                    if let Ok(mut lr) = last_release.lock() {
                        *lr = Some(std::time::Instant::now());
                    }
                    false
                }
                rdev::EventType::KeyPress(_) if single_fired => {
                    let Ok(mut lr) = last_release.lock() else {
                        return;
                    };
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub struct HotkeyManager;

impl HotkeyManager {
    /// Parse config and start the hotkey listener in a background thread.
    ///
    /// On macOS: polls `CGEventSourceKeyState` every 5 ms — avoids the
    /// TSM `dispatch_assert_queue` crash introduced in macOS 26.
    /// On Linux/Windows: uses `rdev::listen` in a background thread.
    pub fn spawn(sender: mpsc::Sender<AppEvent>, config: &HotkeyConfig) -> anyhow::Result<()> {
        #[cfg(target_os = "macos")]
        return Self::spawn_poll_macos(sender, config);

        #[cfg(not(target_os = "macos"))]
        {
            let (target_key, required_modifiers, double_tap_ms) = parse_config(config)?;
            tracing::info!(key = %config.key, modifiers = ?config.modifiers,
                trigger = ?config.trigger, "Registering hotkey");

            std::thread::Builder::new()
                .name("hotkey-listener".to_string())
                .spawn(move || {
                    let result = listen(make_rdev_callback(
                        sender,
                        target_key,
                        required_modifiers,
                        double_tap_ms,
                    ));
                    if let Err(e) = result {
                        tracing::error!("rdev listener exited: {:?}", e);
                    }
                })?;

            Ok(())
        }
    }

    /// macOS: poll key state every 5 ms using CoreGraphics — avoids rdev's TSM crash.
    ///
    /// Uses three methods in parallel (some work on some macOS versions, some don't):
    /// - `CGEventSourceKeyState` with CombinedSessionState (stateID=0)
    /// - `CGEventSourceKeyState` with HIDSystemState (stateID=1)
    /// - `CGEventSourceFlagsState` NX device bits (reliable for modifier keys)
    #[cfg(target_os = "macos")]
    fn spawn_poll_macos(
        sender: mpsc::Sender<AppEvent>,
        config: &HotkeyConfig,
    ) -> anyhow::Result<()> {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceKeyState(stateID: i32, virtualKey: u16) -> bool;
            // Returns 64-bit CGEventFlags (includes NX device modifier bits in low word)
            fn CGEventSourceFlagsState(stateID: i32) -> u64;
        }

        // NX device-specific modifier bits (in low word of CGEventFlags).
        // These distinguish Left/Right Command, Shift, Option, Control.
        const NX_DEVICELCMDKEYMASK: u64 = 0x0000_0008;
        const NX_DEVICERCMDKEYMASK: u64 = 0x0000_0010;

        let (target_key, required_modifiers, double_tap_ms) = parse_config(config)?;

        let target_vkcode = key_to_vkcode(target_key)
            .ok_or_else(|| anyhow::anyhow!("Key {:?} is not supported on macOS", target_key))?;

        // Device-flag bit to check in CGEventSourceFlagsState (for Command keys)
        let target_nx_flag: Option<u64> = match target_vkcode {
            0x36 => Some(NX_DEVICERCMDKEYMASK),
            0x37 => Some(NX_DEVICELCMDKEYMASK),
            _ => None,
        };

        let mod_codes: Vec<&'static [u16]> =
            required_modifiers.iter().map(|mg| mg.vkcodes()).collect();

        tracing::info!(
            key = %config.key,
            vkcode = format!("0x{:02X}", target_vkcode),
            trigger = ?config.trigger,
            "Registering hotkey (macOS poll — run with --verbose to see key-state events)"
        );

        std::thread::Builder::new()
            .name("hotkey-poll".to_string())
            .spawn(move || {
                let mut was_pressed = false;
                let mut last_release: Option<std::time::Instant> = None;

                loop {
                    // Check required modifiers
                    let mods_ok = mod_codes.iter().all(|codes| {
                        codes.iter().any(|&vk| unsafe {
                            CGEventSourceKeyState(0, vk) || CGEventSourceKeyState(1, vk)
                        })
                    });

                    // Detect target key via all available methods
                    let now_pressed = mods_ok && {
                        let ks0 = unsafe { CGEventSourceKeyState(0, target_vkcode) };
                        let ks1 = unsafe { CGEventSourceKeyState(1, target_vkcode) };
                        let flag = target_nx_flag
                            .map(|mask| {
                                let f0 = unsafe { CGEventSourceFlagsState(0) };
                                let f1 = unsafe { CGEventSourceFlagsState(1) };
                                (f0 & mask) != 0 || (f1 & mask) != 0
                            })
                            .unwrap_or(false);
                        ks0 || ks1 || flag
                    };

                    if now_pressed != was_pressed {
                        tracing::debug!(
                            "Key 0x{:02X} state → {}",
                            target_vkcode,
                            if now_pressed { "DOWN" } else { "UP" }
                        );
                    }

                    if now_pressed && !was_pressed {
                        // Rising edge — check double-tap window.
                        let should_fire = match double_tap_ms {
                            Some(threshold) => last_release
                                .map(|t| t.elapsed().as_millis() < threshold as u128)
                                .unwrap_or(false),
                            None => true,
                        };
                        if should_fire {
                            tracing::info!("Hotkey fired → Recording");
                            let _ = sender.blocking_send(AppEvent::HotkeyPressed);
                            last_release = None;
                        } else {
                            tracing::debug!(
                                "First tap — waiting for double-tap within {}ms",
                                double_tap_ms.unwrap_or(0)
                            );
                        }
                    } else if !now_pressed && was_pressed {
                        // Falling edge — record for double-tap detection.
                        if double_tap_ms.is_some() {
                            last_release = Some(std::time::Instant::now());
                        }
                    }

                    was_pressed = now_pressed;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })?;

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
        assert_eq!(
            ModifierGroup::from_str("Ctrl").unwrap(),
            ModifierGroup::Ctrl
        );
        assert_eq!(
            ModifierGroup::from_str("control").unwrap(),
            ModifierGroup::Ctrl
        );
        assert_eq!(
            ModifierGroup::from_str("SHIFT").unwrap(),
            ModifierGroup::Shift
        );
        assert_eq!(ModifierGroup::from_str("alt").unwrap(), ModifierGroup::Alt);
        assert_eq!(
            ModifierGroup::from_str("option").unwrap(),
            ModifierGroup::Alt
        );
        assert_eq!(ModifierGroup::from_str("cmd").unwrap(), ModifierGroup::Meta);
        assert_eq!(
            ModifierGroup::from_str("Meta").unwrap(),
            ModifierGroup::Meta
        );
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

    fn press(key: Key) -> EventType {
        EventType::KeyPress(key)
    }
    fn release(key: Key) -> EventType {
        EventType::KeyRelease(key)
    }

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

        assert!(!process_event(&press(Key::F4), Key::F4, &mods, &mut p));

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

        process_event(&press(Key::ControlLeft), Key::F4, &mods, &mut p);
        assert!(!process_event(&press(Key::F4), Key::F4, &mods, &mut p));

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
