//! Phase 7 integration tests for the output module.
//!
//! Clipboard tests require a running display server — mark with #[ignore] so
//! they are skipped in headless CI and run manually:
//!   cargo test -- --ignored

/// Compile-time guard: ensures output module is reachable.
#[test]
fn placeholder_output() {}

/// Clipboard write → read round-trip including CJK characters.
#[test]
#[ignore = "requires display server (run manually)"]
fn clipboard_round_trip_ascii() {
    just_talk::output::clipboard::write("hello world").expect("write");
    let got = just_talk::output::clipboard::read().expect("read");
    assert_eq!(got, "hello world");
}

#[test]
#[ignore = "requires display server (run manually)"]
fn clipboard_round_trip_chinese() {
    let text = "語音輸入測試：你好世界！";
    just_talk::output::clipboard::write(text).expect("write");
    let got = just_talk::output::clipboard::read().expect("read");
    assert_eq!(got, text);
}

/// OutputManager with prefer_inject=false should route to clipboard.
/// Uses a real clipboard so also ignored in headless CI.
#[tokio::test]
#[ignore = "requires display server (run manually)"]
async fn output_manager_no_inject_writes_clipboard() {
    use just_talk::config::OutputConfig;
    use just_talk::output::OutputManager;

    let cfg = OutputConfig {
        prefer_inject: false,
        inject_delay_ms: 0,
        clipboard_fallback: true,
    };
    let manager = OutputManager::new(&cfg);
    manager.send("integration test text").await.expect("send");

    let got = just_talk::output::clipboard::read().expect("read");
    assert_eq!(got, "integration test text");
}
