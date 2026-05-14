//! Keyboard injection via enigo (cross-platform).
//!
//! `type_text` feeds the string to the OS input pipeline as if typed by the
//! user.  enigo uses `Key::Unicode` under the hood which supports CJK and all
//! other Unicode code points.
//!
//! When `delay_ms > 0` each character is sent separately with a sleep between
//! them.  This slows injection but improves compatibility with applications
//! that throttle or buffer rapid key events (common with Chinese IME host apps).

#[cfg(not(target_os = "macos"))]
use enigo::{Enigo, Keyboard, Settings};

/// Paste clipboard contents into the focused field by simulating Cmd+V (macOS).
///
/// Uses raw CoreGraphics FFI — no TSM calls — safe to call from any thread on
/// macOS 26+ which added `dispatch_assert_queue` inside TSM entry points.
/// Assumes the caller has already written the text to the clipboard.
#[cfg(target_os = "macos")]
pub fn paste_macos(pre_delay_ms: u64) -> anyhow::Result<()> {
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(source: *mut c_void, vk: u16, keydown: bool) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut c_void);
        fn CFRelease(cf: *const c_void);
    }

    // kCGAnnotatedSessionEventTap = 2 (injects into the current login session).
    const TAP: u32 = 2;
    // kCGEventFlagMaskCommand = 1 << 20 = 0x00100000.
    const CMD_FLAG: u64 = 0x0010_0000;
    // kVK_ANSI_V = 0x09  (layout-independent ANSI key code, no TSM lookup needed).
    const VK_V: u16 = 0x09;

    if pre_delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(pre_delay_ms));
    }

    unsafe {
        let null: *mut c_void = std::ptr::null_mut();

        let down = CGEventCreateKeyboardEvent(null, VK_V, true);
        CGEventSetFlags(down, CMD_FLAG);
        CGEventPost(TAP, down);
        CFRelease(down);

        std::thread::sleep(std::time::Duration::from_millis(20));

        let up = CGEventCreateKeyboardEvent(null, VK_V, false);
        CGEventSetFlags(up, 0);
        CGEventPost(TAP, up);
        CFRelease(up);
    }

    tracing::debug!(pre_delay_ms, "Cmd+V posted via CoreGraphics");
    Ok(())
}

/// Type `text` into the currently focused field.
///
/// `delay_ms` — milliseconds between each character event.
/// Pass `0` to send the entire string in a single call (fastest but may drop
/// characters in some apps).
#[cfg(not(target_os = "macos"))]
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
