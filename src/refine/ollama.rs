// Phase 6: Ollama local API  POST /api/chat  (non-streaming)
use anyhow::Result;
use async_trait::async_trait;
use crate::config::RefineConfig;
use super::Refiner;

pub struct OllamaRefiner {
    config: RefineConfig,
}

impl OllamaRefiner {
    pub fn new(config: &RefineConfig) -> Self {
        Self { config: config.clone() }
    }
}

#[async_trait]
impl Refiner for OllamaRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        let _ = (&self.config, raw_text);
        // Phase 6: POST to base_url/api/chat, parse NDJSON response
        todo!("Phase 6: Ollama refiner")
    }
}
