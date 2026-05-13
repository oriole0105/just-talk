//! Keyboard injection via enigo (cross-platform).
//!
//! `type_text` feeds the string to the OS input pipeline as if typed by the
//! user.  enigo uses `Key::Unicode` under the hood which supports CJK and all
//! other Unicode code points.
//!
//! When `delay_ms > 0` each character is sent separately with a sleep between
//! them.  This slows injection but improves compatibility with applications
//! that throttle or buffer rapid key events (common with Chinese IME host apps).

use enigo::{Enigo, Keyboard, Settings};

/// Type `text` into the currently focused field.
///
/// `delay_ms` — milliseconds between each character event.
/// Pass `0` to send the entire string in a single call (fastest but may drop
/// characters in some apps).
pub fn type_text(text: &str, delay_ms: u64) -> anyhow::Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Enigo init failed: {:?}", e))?;

    if delay_ms == 0 {
        enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Text injection failed: {:?}", e))?;
    } else {
        for ch in text.chars() {
            let s = ch.to_string();
            enigo
                .text(&s)
                .map_err(|e| anyhow::anyhow!("Char injection failed for '{}': {:?}", ch, e))?;
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
    }

    tracing::debug!(chars = text.chars().count(), delay_ms, "Text injected");
    Ok(())
}
