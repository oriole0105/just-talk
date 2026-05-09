use crate::app::AppEvent;

// Phase 3: rdev global hotkey listener
pub struct HotkeyManager;

impl HotkeyManager {
    /// Spawn a dedicated OS thread that listens for the configured hotkey
    /// and sends AppEvent::HotkeyPressed on each trigger.
    pub fn spawn(_sender: tokio::sync::mpsc::Sender<AppEvent>, _key: &str, _modifiers: &[String]) {
        // TODO Phase 3: parse key string → rdev::Key, call rdev::listen
        todo!("Phase 3: rdev hotkey listener")
    }
}
