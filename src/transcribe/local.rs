// Phase 5: whisper-rs (whisper.cpp FFI) local transcription
use anyhow::Result;
use async_trait::async_trait;
use crate::config::TranscribeConfig;
use super::Transcriber;

pub struct LocalTranscriber {
    config: TranscribeConfig,
}

impl LocalTranscriber {
    pub fn new(config: &TranscribeConfig) -> Self {
        Self { config: config.clone() }
    }
}

#[async_trait]
impl Transcriber for LocalTranscriber {
    async fn transcribe(&self, _pcm: &[f32], _sample_rate: u32) -> Result<String> {
        let _ = &self.config;
        todo!("Phase 5: load ggml model via whisper-rs and run inference")
    }
}
