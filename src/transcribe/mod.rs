use crate::config::{TranscribeBackend, TranscribeConfig};
use anyhow::Result;
use async_trait::async_trait;

pub mod local;
pub mod remote;

#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcribe 16 kHz mono PCM samples to text.
    async fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<String>;
}

/// Factory: create the configured transcriber backend.
///
/// Returns `Err` if the backend cannot be initialised (e.g. model file missing).
pub fn create_transcriber(config: &TranscribeConfig) -> Result<Box<dyn Transcriber + Send + Sync>> {
    match config.backend {
        TranscribeBackend::Local => Ok(Box::new(local::LocalTranscriber::new(config)?)),
        TranscribeBackend::OpenAi => Ok(Box::new(remote::RemoteTranscriber::new(config))),
    }
}
