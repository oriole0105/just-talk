//! AI text refinement via the Anthropic Messages API (POST /v1/messages).
//!
//! On any API error the raw transcript is returned unchanged so the user
//! never loses their dictation (P6-11).

use anyhow::Result;
use async_trait::async_trait;
use crate::config::RefineConfig;
use super::Refiner;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 1024;

pub struct ClaudeRefiner {
    config: RefineConfig,
    client: reqwest::Client,
}

impl ClaudeRefiner {
    pub fn new(config: &RefineConfig) -> Self {
        Self { config: config.clone(), client: reqwest::Client::new() }
    }
}

#[async_trait]
impl Refiner for ClaudeRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        match call_claude(&self.client, &self.config, raw_text).await {
            Ok(text) => Ok(text),
            Err(e) => {
                tracing::warn!(error = %e, "Claude refiner failed — returning raw transcript");
                Ok(raw_text.to_string())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP helper (pub(crate) so the integration test can exercise it directly)
// ---------------------------------------------------------------------------

pub async fn call_claude(
    client: &reqwest::Client,
    config: &RefineConfig,
    raw_text: &str,
) -> anyhow::Result<String> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("refine.api_key not configured for Claude backend"))?;

    let base_url = config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    let payload = serde_json::json!({
        "model": config.model,
        "max_tokens": MAX_TOKENS,
        "system": config.system_prompt,
        "messages": [{"role": "user", "content": raw_text}]
    });

    tracing::debug!(model = %config.model, url = %url, "Calling Claude API");

    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Claude HTTP send failed: {}", e))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Claude response JSON parse failed: {}", e))?;

    if !status.is_success() {
        anyhow::bail!("Claude API returned {}: {:?}", status, body);
    }

    body["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No content[0].text in Claude response: {:?}", body))
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
        let _ = ClaudeRefiner::new(&cfg);
    }
}
