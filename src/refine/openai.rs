// Phase 6: OpenAI Chat Completions API  POST /v1/chat/completions
use anyhow::Result;
use async_trait::async_trait;
use crate::config::RefineConfig;
use super::Refiner;

pub struct OpenAiRefiner {
    config: RefineConfig,
}

impl OpenAiRefiner {
    pub fn new(config: &RefineConfig) -> Self {
        Self { config: config.clone() }
    }
}

#[async_trait]
impl Refiner for OpenAiRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        let _ = (&self.config, raw_text);
        // Phase 6: build chat payload, POST, extract choices[0].message.content
        todo!("Phase 6: OpenAI chat refiner")
    }
}
