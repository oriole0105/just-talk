use anyhow::Result;
use async_trait::async_trait;
use crate::config::{TranscribeBackend, TranscribeConfig};

pub mod local;
pub mod remote;

#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcribe 16 kHz mono PCM samples to text.
    async fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<String>;
}

/// Factory: create the configured transcriber backend.
pub fn create_transcriber(config: &TranscribeConfig) -> Box<dyn Transcriber> {
    match config.backend {
        TranscribeBackend::Local  => Box::new(local::LocalTranscriber::new(config)),
        TranscribeBackend::OpenAi => Box::new(remote::RemoteTranscriber::new(config)),
    }
}
