//! Output module: inject text into focused field, or write to clipboard.
//!
//! Decision flow for `send()`:
//!   prefer_inject=true →
//!     focused element is TextInput → `inject::type_text` (spawn_blocking)
//!       → if inject fails AND clipboard_fallback=true → `clipboard::write`
//!     Other/Unknown → if clipboard_fallback=true → `clipboard::write`
//!   prefer_inject=false → `clipboard::write`

pub mod clipboard;
pub mod focus;
pub mod inject;

use anyhow::Result;
use crate::config::OutputConfig;
use focus::FocusedElement;

pub struct OutputManager {
    config: OutputConfig,
}

impl OutputManager {
    pub fn new(config: &OutputConfig) -> Self {
        Self { config: config.clone() }
    }

    /// Deliver `text` to the user according to the output config.
    pub async fn send(&self, text: &str) -> Result<()> {
        if !self.config.prefer_inject {
            return clipboard::write(text);
        }

        let focused = focus::get_focused_element_type();
        tracing::debug!(?focused, "Focus detection result");

        match focused {
            // TextInput or Unknown (focus detection not implemented) → try injection.
            FocusedElement::TextInput | FocusedElement::Unknown => {
                let text_owned = text.to_string();
                let delay = self.config.inject_delay_ms;

                // inject::type_text is blocking (enigo FFI + optional sleep).
                let result = tokio::task::spawn_blocking(move || {
                    inject::type_text(&text_owned, delay)
                })
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking join error: {}", e))?;

                if let Err(e) = result {
                    tracing::warn!(error = %e, "Key injection failed");
                    if self.config.clipboard_fallback {
                        tracing::info!("Falling back to clipboard");
                        clipboard::write(text)?;
                    }
                }
            }
            // Explicitly not a text field → skip injection.
            FocusedElement::Other => {
                if self.config.clipboard_fallback {
                    clipboard::write(text)?;
                } else {
                    tracing::warn!("Focus is Other and clipboard_fallback=false — output dropped");
                }
            }
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
    use crate::config::OutputConfig;

    fn default_manager() -> OutputManager {
        OutputManager::new(&OutputConfig::default())
    }

    #[test]
    fn output_manager_new_does_not_panic() {
        let _ = default_manager();
    }

    #[test]
    fn prefer_inject_false_config() {
        let cfg = OutputConfig { prefer_inject: false, ..OutputConfig::default() };
        let m = OutputManager::new(&cfg);
        assert!(!m.config.prefer_inject);
    }
}
