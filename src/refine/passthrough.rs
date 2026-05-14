use super::Refiner;
use anyhow::Result;
use async_trait::async_trait;

/// No-op refiner: returns the raw transcript unchanged.
pub struct PassthroughRefiner;

#[async_trait]
impl Refiner for PassthroughRefiner {
    async fn refine(&self, raw_text: &str) -> Result<String> {
        Ok(raw_text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn passthrough_returns_input_unchanged() {
        let r = PassthroughRefiner;
        let result = r.refine("hello world").await.unwrap();
        assert_eq!(result, "hello world");
    }
}
