// Phase 6: Anthropic Messages API  POST /v1/messages
use anyhow::Result;
use async_trait::async_trait;
use crate::config::RefineConfig;
use super::Refiner;

pub struct ClaudeRefiner {
    config: RefineConfig,
}

impl ClaudeRefiner {
    pub fn new(config: &RefineConfig) -> Self {
        Self { config: config.clone() }
    }
}

#[async_trait]
impl Refiner for ClaudeRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        let _ = (&self.config, raw_text);
        // Phase 6: build Messages payload, POST, extract content[0].text
        // On error: log + return raw_text (never discard user input)
        todo!("Phase 6: Claude API refiner")
    }
}
