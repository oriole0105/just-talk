//! AI text refinement via the OpenAI Chat Completions API (POST /v1/chat/completions).
//!
//! On any API error the raw transcript is returned unchanged (P6-11).

use super::Refiner;
use crate::config::RefineConfig;
use anyhow::Result;
use async_trait::async_trait;

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

pub struct OpenAiRefiner {
    config: RefineConfig,
    client: reqwest::Client,
}

impl OpenAiRefiner {
    pub fn new(config: &RefineConfig) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Refiner for OpenAiRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        match call_openai(&self.client, &self.config, raw_text).await {
            Ok(text) => Ok(text),
            Err(e) => {
                tracing::warn!(error = %e, "OpenAI refiner failed — returning raw transcript");
                Ok(raw_text.to_string())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP helper
// ---------------------------------------------------------------------------

pub async fn call_openai(
    client: &reqwest::Client,
    config: &RefineConfig,
    raw_text: &str,
) -> anyhow::Result<String> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("refine.api_key not configured for OpenAI backend"))?;

    let base_url = config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

    let payload = serde_json::json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": config.system_prompt},
            {"role": "user",   "content": raw_text}
        ]
    });

    tracing::debug!(model = %config.model, url = %url, "Calling OpenAI Chat API");

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("OpenAI HTTP send failed: {}", e))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("OpenAI response JSON parse failed: {}", e))?;

    if !status.is_success() {
        anyhow::bail!("OpenAI API returned {}: {:?}", status, body);
    }

    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No choices[0].message.content in OpenAI response: {:?}",
                body
            )
        })
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
        let _ = OpenAiRefiner::new(&cfg);
    }
}
