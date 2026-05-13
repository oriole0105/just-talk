//! Phase 6 integration tests for AI refinement backends.
//! Uses wiremock to intercept HTTP calls and verify request shape.

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Helper: build RefineConfig pointing at the mock server
// ---------------------------------------------------------------------------

fn claude_config(base_url: &str) -> just_talk::config::RefineConfig {
    just_talk::config::RefineConfig {
        backend: just_talk::config::RefineBackend::Claude,
        model: "claude-haiku-4-5-20251001".into(),
        system_prompt: "Fix punctuation.".into(),
        api_key: Some("sk-test-key".into()),
        base_url: Some(base_url.to_string()),
    }
}

fn openai_config(base_url: &str) -> just_talk::config::RefineConfig {
    just_talk::config::RefineConfig {
        backend: just_talk::config::RefineBackend::OpenAi,
        model: "gpt-4o-mini".into(),
        system_prompt: "Fix punctuation.".into(),
        api_key: Some("sk-test-openai".into()),
        base_url: Some(base_url.to_string()),
    }
}

fn ollama_config(base_url: &str) -> just_talk::config::RefineConfig {
    just_talk::config::RefineConfig {
        backend: just_talk::config::RefineBackend::Ollama,
        model: "gemma2:2b".into(),
        system_prompt: "Fix punctuation.".into(),
        api_key: None,
        base_url: Some(base_url.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Claude: correct headers + response parsing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn claude_refiner_sends_correct_request() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "sk-test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Refined output."}],
            "model": "claude-haiku-4-5-20251001",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cfg = claude_config(&server.uri());
    let client = reqwest::Client::new();
    let result = just_talk::refine::claude::call_claude(&client, &cfg, "raw text").await;

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), "Refined output.");
    server.verify().await;
}

// ---------------------------------------------------------------------------
// Claude: fallback to raw text on 500
// ---------------------------------------------------------------------------

#[tokio::test]
async fn claude_refiner_falls_back_on_api_error() {
    use just_talk::refine::Refiner;

    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "error": {"type": "api_error", "message": "overloaded"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cfg = claude_config(&server.uri());
    let refiner = just_talk::refine::claude::ClaudeRefiner::new(&cfg);
    let result = refiner.refine("my raw text").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my raw text");
    server.verify().await;
}

// ---------------------------------------------------------------------------
// OpenAI: choices[0].message.content parsing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn openai_refiner_sends_correct_request() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-01",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "OpenAI refined."},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cfg = openai_config(&server.uri());
    let client = reqwest::Client::new();
    let result = just_talk::refine::openai::call_openai(&client, &cfg, "raw text").await;

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), "OpenAI refined.");
    server.verify().await;
}

// ---------------------------------------------------------------------------
// Ollama: message.content parsing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ollama_refiner_sends_correct_request() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "gemma2:2b",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {"role": "assistant", "content": "Ollama refined."},
            "done": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cfg = ollama_config(&server.uri());
    let client = reqwest::Client::new();
    let result = just_talk::refine::ollama::call_ollama(&client, &cfg, "raw text").await;

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), "Ollama refined.");
    server.verify().await;
}

#[test]
fn placeholder_refine() {}
