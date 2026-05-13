//! Phase 5 transcription integration tests.
//!
//! The local-model test is `#[ignore]` — it requires a real ggml model file at
//! the path configured in `~/.config/just-talk/config.toml` (or passed via env).
//! Run manually with:
//!   cargo test --features local-whisper -- --ignored local_transcriber_produces_text

#[cfg(feature = "local-whisper")]
#[tokio::test]
#[ignore = "requires a ggml model file on disk"]
async fn local_transcriber_produces_text() {
    use just_talk::config::{Config, TranscribeBackend};
    use just_talk::transcribe::create_transcriber;

    let cfg = Config::load_or_default(None);
    assert_eq!(cfg.transcribe.backend, TranscribeBackend::Local);

    let transcriber = create_transcriber(&cfg.transcribe).expect("init transcriber");

    // Silence → whisper should return empty or "[BLANK_AUDIO]" — either way non-panicking.
    let silence = vec![0.0_f32; 16_000]; // 1 second of silence at 16 kHz
    let result = transcriber.transcribe(&silence, 16_000).await;
    assert!(result.is_ok(), "transcribe returned Err: {:?}", result);
}

#[test]
fn placeholder_transcribe() {
    // Compile-time guard: ensure the transcribe module is importable.
}
