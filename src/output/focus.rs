//! Detect whether the currently focused UI element is an editable text field.
//!
//! Platform implementations use the OS accessibility APIs.  All platforms
//! currently return `Unknown` (safe fallback → clipboard path) until the
//! platform-specific crates are wired in (requires user granting accessibility
//! permissions on macOS/Windows).

/// Result of querying the currently focused UI element.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusedElement {
    /// An editable text field (input, textarea, contenteditable, …)
    TextInput,
    /// Focused element exists but is not a text input
    Other,
    /// Accessibility query failed, unsupported platform, or permissions denied
    Unknown,
}

/// Return the type of the currently focused element.
///
/// Returns `Unknown` on all platforms until platform-specific probes are
/// implemented.  The `Unknown` path routes to the clipboard, which is always
/// safe.
pub fn get_focused_element_type() -> FocusedElement {
    #[cfg(target_os = "macos")]
    return probe_macos();

    #[cfg(target_os = "windows")]
    return probe_windows();

    #[cfg(target_os = "linux")]
    return probe_linux();

    // Fallback for platforms not matched above.
    #[allow(unreachable_code)]
    FocusedElement::Unknown
}

// ---------------------------------------------------------------------------
// macOS — Accessibility API (AXUIElement)
//
// Requires:
//   1. `accessibility` crate (or raw AX FFI via `core-foundation`).
//   2. User has granted Accessibility permissions in System Settings.
//
// Probe logic:
//   AXUIElementCreateSystemWide() → copyAttributeValue(kAXFocusedUIElement)
//   → copyAttributeValue(kAXRole) → compare to "AXTextField"/"AXTextArea"
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn probe_macos() -> FocusedElement {
    // Full implementation requires the `accessibility` crate and AX permissions.
    // Returning Unknown routes output to clipboard, which is always correct.
    tracing::trace!("macOS focus probe not yet implemented — using clipboard path");
    FocusedElement::Unknown
}

// ---------------------------------------------------------------------------
// Windows — UI Automation (IUIAutomation)
//
// Requires: `windows` crate with Win32_UI_Accessibility feature.
// Probe: GetFocusedElement → GetCurrentControlType →
//        compare to UIA_EditControlTypeId / UIA_DocumentControlTypeId
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn probe_windows() -> FocusedElement {
    tracing::trace!("Windows focus probe not yet implemented — using clipboard path");
    FocusedElement::Unknown
}

// ---------------------------------------------------------------------------
// Linux — AT-SPI2 (atspi crate)
//
// Probe: atspi::accessible::AccessibleProxy → get role →
//        compare to atspi::Role::Entry / atspi::Role::Text
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn probe_linux() -> FocusedElement {
    tracing::trace!("Linux focus probe not yet implemented — using clipboard path");
    FocusedElement::Unknown
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_focused_element_type_does_not_panic() {
        // On all platforms this should return without panicking.
        let _ = get_focused_element_type();
    }

    #[test]
    fn focused_element_debug() {
        assert_eq!(format!("{:?}", FocusedElement::TextInput), "TextInput");
        assert_eq!(format!("{:?}", FocusedElement::Unknown), "Unknown");
    }
}
