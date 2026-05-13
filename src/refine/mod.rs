use anyhow::Result;
use async_trait::async_trait;
use crate::config::{RefineBackend, RefineConfig};

pub mod claude;
pub mod ollama;
pub mod openai;
pub mod passthrough;

#[async_trait]
pub trait Refiner: Send + Sync {
    /// Polish raw Whisper transcript and return the refined text.
    async fn refine(&self, raw_text: &str) -> Result<String>;
}

/// Factory: create the configured refiner backend.
pub fn create_refiner(config: &RefineConfig) -> Box<dyn Refiner + Send + Sync> {
    match config.backend {
        RefineBackend::Claude  => Box::new(claude::ClaudeRefiner::new(config)),
        RefineBackend::OpenAi  => Box::new(openai::OpenAiRefiner::new(config)),
        RefineBackend::Ollama  => Box::new(ollama::OllamaRefiner::new(config)),
        RefineBackend::None    => Box::new(passthrough::PassthroughRefiner),
    }
}
