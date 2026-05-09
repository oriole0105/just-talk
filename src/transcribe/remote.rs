// Phase 5: OpenAI Whisper API  POST /v1/audio/transcriptions
use anyhow::Result;
use async_trait::async_trait;
use crate::config::TranscribeConfig;
use super::Transcriber;

pub struct RemoteTranscriber {
    config: TranscribeConfig,
}

impl RemoteTranscriber {
    pub fn new(config: &TranscribeConfig) -> Self {
        Self { config: config.clone() }
    }
}

#[async_trait]
impl Transcriber for RemoteTranscriber {
    async fn transcribe(&self, _pcm: &[f32], _sample_rate: u32) -> Result<String> {
        let _ = &self.config;
        // Phase 5: hound → WAV bytes → reqwest multipart POST → parse JSON text field
        todo!("Phase 5: OpenAI Whisper API with retry")
    }
}
