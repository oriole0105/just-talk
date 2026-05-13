//! AI text refinement via the Ollama local API (POST /api/chat, non-streaming).
//!
//! Default base URL: http://localhost:11434.
//! On any API error the raw transcript is returned unchanged (P6-11).

use anyhow::Result;
use async_trait::async_trait;
use crate::config::RefineConfig;
use super::Refiner;

const DEFAULT_BASE_URL: &str = "http://localhost:11434";

pub struct OllamaRefiner {
    config: RefineConfig,
    client: reqwest::Client,
}

impl OllamaRefiner {
    pub fn new(config: &RefineConfig) -> Self {
        Self { config: config.clone(), client: reqwest::Client::new() }
    }
}

#[async_trait]
impl Refiner for OllamaRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        match call_ollama(&self.client, &self.config, raw_text).await {
            Ok(text) => Ok(text),
            Err(e) => {
                tracing::warn!(error = %e, "Ollama refiner failed — returning raw transcript");
                Ok(raw_text.to_string())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP helper
// ---------------------------------------------------------------------------

pub async fn call_ollama(
    client: &reqwest::Client,
    config: &RefineConfig,
    raw_text: &str,
) -> anyhow::Result<String> {
    let base_url = config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let payload = serde_json::json!({
        "model":  config.model,
        "stream": false,
        "messages": [
            {"role": "system", "content": config.system_prompt},
            {"role": "user",   "content": raw_text}
        ]
    });

    tracing::debug!(model = %config.model, url = %url, "Calling Ollama API");

    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Ollama HTTP send failed: {}", e))?;

    let status = resp.status();

    // With stream:false Ollama returns a single JSON object.
    // Guard against accidental NDJSON by reading the raw text first.
    let text = resp
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("Ollama response read failed: {}", e))?;

    // Parse only the first non-empty line in case Ollama sends NDJSON anyway.
    let first_line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(&text);

    let body: serde_json::Value = serde_json::from_str(first_line)
        .map_err(|e| anyhow::anyhow!("Ollama JSON parse failed: {} — body: {}", e, first_line))?;

    if !status.is_success() {
        anyhow::bail!("Ollama API returned {}: {:?}", status, body);
    }

    body["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No message.content in Ollama response: {:?}", body))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RefineConfig;

    #[test]
    fn new_does_not_panic() {
        let cfg = RefineConfig::default();
        let _ = OllamaRefiner::new(&cfg);
    }
}
