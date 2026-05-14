//! System clipboard access via arboard (cross-platform).

use arboard::Clipboard;

/// Write text to the system clipboard.
pub fn write(text: &str) -> anyhow::Result<()> {
    let mut cb = Clipboard::new().map_err(|e| anyhow::anyhow!("Clipboard init failed: {}", e))?;
    cb.set_text(text)
        .map_err(|e| anyhow::anyhow!("Clipboard write failed: {}", e))?;
    tracing::debug!(chars = text.len(), "Clipboard written");
    Ok(())
}

/// Read text from the system clipboard (used for testing and dry-run).
pub fn read() -> anyhow::Result<String> {
    let mut cb = Clipboard::new().map_err(|e| anyhow::anyhow!("Clipboard init failed: {}", e))?;
    cb.get_text()
        .map_err(|e| anyhow::anyhow!("Clipboard read failed: {}", e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Requires a running display server (not available in headless CI).
    /// Run manually: cargo test -- --ignored clipboard_round_trip
    #[test]
    #[ignore = "requires display server"]
    fn clipboard_round_trip() {
        let text = "just-talk 語音輸入測試 hello world 🎤";
        write(text).expect("write");
        let got = read().expect("read");
        assert_eq!(got, text);
    }

    #[test]
    #[ignore = "requires display server"]
    fn clipboard_overwrites_previous() {
        write("first").expect("write first");
        write("second").expect("write second");
        assert_eq!(read().expect("read"), "second");
    }
}
