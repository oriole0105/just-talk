pub mod clipboard;
pub mod focus;
pub mod inject;

use anyhow::Result;
use focus::FocusedElement;

pub struct OutputManager;

impl OutputManager {
    pub fn new() -> Self {
        Self
    }

    /// Send text to the focused input field, or fall back to clipboard.
    pub async fn send(&self, text: &str) -> Result<()> {
        match focus::get_focused_element_type() {
            FocusedElement::TextInput => {
                inject::type_text(text).or_else(|_| clipboard::write(text))
            }
            FocusedElement::Other | FocusedElement::Unknown => clipboard::write(text),
        }
    }
}
