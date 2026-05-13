//! Remote Whisper transcription via the OpenAI `/v1/audio/transcriptions` API.
//!
//! Flow: f32 PCM → in-memory WAV (hound) → multipart POST (reqwest) → parse JSON.
//! Retries up to 2 additional times on network/5xx errors with exponential backoff.

use anyhow::Result;
use async_trait::async_trait;
use crate::config::TranscribeConfig;
use super::Transcriber;

const OPENAI_BASE_URL: &str = "https://api.openai.com";
const MAX_RETRIES: u32 = 2;

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

pub struct RemoteTranscriber {
    config: TranscribeConfig,
    client: reqwest::Client,
}

impl RemoteTranscriber {
    pub fn new(config: &TranscribeConfig) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::Client::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Transcriber impl
// ---------------------------------------------------------------------------

#[async_trait]
impl Transcriber for RemoteTranscriber {
    async fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<String> {
        let api_key = self
            .config
            .openai_api_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("transcribe.openai_api_key not configured"))?;

        let wav_bytes = pcm_to_wav(pcm, sample_rate)?;
        let model = self.config.openai_model.clone();
        let language = self.config.language.clone();
        let prompt = self.config.prompt.clone();
        let base_url = self.config.base_url.as_deref().unwrap_or(OPENAI_BASE_URL);

        let mut last_err: anyhow::Error = anyhow::anyhow!("no attempts made");

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = 500u64 * (1 << (attempt - 1)); // 500ms, 1000ms
                tracing::warn!(attempt, delay_ms, "Retrying Whisper API call");
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            match post_transcription(
                &self.client,
                &wav_bytes,
                api_key,
                &model,
                language.as_deref(),
                prompt.as_deref(),
                base_url,
            )
            .await
            {
                Ok(text) => {
                    tracing::debug!(chars = text.len(), "Remote transcription complete");
                    return Ok(text);
                }
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "Whisper API attempt failed");
                    last_err = e;
                }
            }
        }

        Err(last_err.context("All Whisper API attempts exhausted"))
    }
}

// ---------------------------------------------------------------------------
// Helpers (pub(crate) for unit tests)
// ---------------------------------------------------------------------------

/// Encode 16 kHz mono f32 PCM as an in-memory WAV file.
pub fn pcm_to_wav(pcm: &[f32], sample_rate: u32) -> anyhow::Result<Vec<u8>> {
    use std::io::Cursor;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| anyhow::anyhow!("WavWriter::new failed: {}", e))?;
        for &s in pcm {
            writer
                .write_sample(s)
                .map_err(|e| anyhow::anyhow!("write_sample failed: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| anyhow::anyhow!("WavWriter::finalize failed: {}", e))?;
    }

    Ok(cursor.into_inner())
}

async fn post_transcription(
    client: &reqwest::Client,
    wav_bytes: &[u8],
    api_key: &str,
    model: &str,
    language: Option<&str>,
    prompt: Option<&str>,
    base_url: &str,
) -> anyhow::Result<String> {
    use reqwest::multipart;

    let file_part = multipart::Part::bytes(wav_bytes.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| anyhow::anyhow!("MIME type error: {}", e))?;

    let mut form = multipart::Form::new()
        .text("model", model.to_string())
        .part("file", file_part);

    if let Some(lang) = language {
        if lang != "auto" {
            form = form.text("language", lang.to_string());
        }
    }

    if let Some(p) = prompt {
        form = form.text("prompt", p.to_string());
    }

    let url = format!("{}/v1/audio/transcriptions", base_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP send failed: {}", e))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("JSON parse failed: {}", e))?;

    if !status.is_success() {
        anyhow::bail!("OpenAI API returned {}: {:?}", status, body);
    }

    body["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No 'text' field in OpenAI response: {:?}", body))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_to_wav_round_trip() {
        let pcm: Vec<f32> = (0..160).map(|i| (i as f32) / 160.0 - 0.5).collect();
        let wav = pcm_to_wav(&pcm, 16_000).expect("encode");

        // Decode with hound and verify samples match
        let mut reader = hound::WavReader::new(std::io::Cursor::new(&wav)).expect("decode");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);

        let decoded: Vec<f32> = reader
            .samples::<f32>()
            .map(|s| s.expect("sample"))
            .collect();
        assert_eq!(decoded.len(), pcm.len());
        for (orig, dec) in pcm.iter().zip(decoded.iter()) {
            assert!(
                (orig - dec).abs() < 1e-6,
                "mismatch: orig={orig} dec={dec}"
            );
        }
    }

    #[test]
    fn pcm_to_wav_empty_is_valid() {
        let wav = pcm_to_wav(&[], 16_000).expect("empty encode");
        let reader = hound::WavReader::new(std::io::Cursor::new(&wav)).expect("decode");
        assert_eq!(reader.len(), 0);
    }

    #[test]
    fn pcm_to_wav_produces_nonempty_bytes() {
        let wav = pcm_to_wav(&[0.0_f32; 100], 44_100).expect("encode");
        assert!(wav.len() > 44, "WAV header alone is 44 bytes");
    }
}
