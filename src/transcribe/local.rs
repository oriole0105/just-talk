//! Local Whisper transcription via whisper-rs (whisper.cpp FFI).
//!
//! Gated behind the `local-whisper` Cargo feature because it requires cmake.
//! Without the feature the struct still compiles but `transcribe()` returns an
//! actionable error rather than a link-time failure.

use anyhow::Result;
use async_trait::async_trait;
use crate::config::TranscribeConfig;
use super::Transcriber;

// ---------------------------------------------------------------------------
// Struct definition — ctx field is conditional on the feature
// ---------------------------------------------------------------------------

pub struct LocalTranscriber {
    config: TranscribeConfig,
    #[cfg(feature = "local-whisper")]
    ctx: std::sync::Arc<whisper_rs::WhisperContext>,
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl LocalTranscriber {
    pub fn new(config: &TranscribeConfig) -> anyhow::Result<Self> {
        // Without the feature, compile a no-op stub that errors at runtime.
        #[cfg(not(feature = "local-whisper"))]
        return Ok(Self { config: config.clone() });

        // With the feature: eagerly load the ggml model so transcribe() is fast.
        #[cfg(feature = "local-whisper")]
        {
            let model_path = config
                .model_path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "transcribe.model_path not configured; \
                     set it in config.toml or download a ggml model"
                ))?;

            tracing::info!(path = %model_path.display(), "Loading whisper model");

            let ctx = whisper_rs::WhisperContext::new_with_params(
                model_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("model_path is not valid UTF-8"))?,
                whisper_rs::WhisperContextParameters::default(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to load whisper model: {:?}", e))?;

            tracing::info!("Whisper model loaded");

            Ok(Self {
                config: config.clone(),
                ctx: std::sync::Arc::new(ctx),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Transcriber impl — real (local-whisper enabled)
// ---------------------------------------------------------------------------

#[cfg(feature = "local-whisper")]
#[async_trait]
impl Transcriber for LocalTranscriber {
    async fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<String> {
        use whisper_rs::{FullParams, SamplingStrategy};

        if sample_rate != 16_000 {
            anyhow::bail!(
                "LocalTranscriber expects 16 kHz mono PCM, got {} Hz",
                sample_rate
            );
        }

        let ctx = std::sync::Arc::clone(&self.ctx);
        let pcm_owned = pcm.to_vec();
        // Normalise language: "auto" → None (whisper auto-detects), else pass through
        let lang: Option<String> = match self.config.language.as_deref() {
            None | Some("auto") => None,
            Some(l) => Some(l.to_string()),
        };

        let text = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let mut state = ctx
                .create_state()
                .map_err(|e| anyhow::anyhow!("create_state failed: {:?}", e))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(lang.as_deref());
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            state
                .full(params, &pcm_owned)
                .map_err(|e| anyhow::anyhow!("Whisper inference failed: {:?}", e))?;

            let n = state
                .full_n_segments()
                .map_err(|e| anyhow::anyhow!("full_n_segments failed: {:?}", e))?;

            let mut result = String::new();
            for i in 0..n {
                let seg = state
                    .full_get_segment_text(i)
                    .map_err(|e| anyhow::anyhow!("segment {} text failed: {:?}", i, e))?;
                result.push_str(seg.trim());
                result.push(' ');
            }
            Ok(result.trim().to_string())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking join error: {}", e))??;

        tracing::debug!(chars = text.len(), "Local transcription complete");
        Ok(text)
    }
}

// ---------------------------------------------------------------------------
// Transcriber impl — stub (local-whisper disabled)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "local-whisper"))]
#[async_trait]
impl Transcriber for LocalTranscriber {
    async fn transcribe(&self, _pcm: &[f32], _sample_rate: u32) -> Result<String> {
        let _ = &self.config;
        anyhow::bail!(
            "Local Whisper is not compiled in. \
             Rebuild with `--features local-whisper` (requires cmake and a ggml model)."
        )
    }
}
