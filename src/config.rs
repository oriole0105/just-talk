use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Backend enums
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TranscribeBackend {
    Local,
    #[serde(rename = "openai")]
    OpenAi,
}

impl Default for TranscribeBackend {
    fn default() -> Self { Self::Local }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefineBackend {
    Claude,
    #[serde(rename = "openai")]
    OpenAi,
    Ollama,
    None,
}

impl Default for RefineBackend {
    fn default() -> Self { Self::None }
}

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct HotkeyConfig {
    pub key: String,
    pub modifiers: Vec<String>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self { key: "F4".to_string(), modifiers: vec![] }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct TranscribeConfig {
    pub backend: TranscribeBackend,
    pub model_path: Option<PathBuf>,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
    /// Override the API base URL (e.g. "https://api.groq.com/openai" for Groq).
    /// Defaults to the official OpenAI endpoint.
    pub base_url: Option<String>,
}

impl Default for TranscribeConfig {
    fn default() -> Self {
        Self {
            backend: TranscribeBackend::Local,
            model_path: dirs::data_local_dir()
                .map(|d| d.join("just-talk").join("models").join("ggml-base.bin")),
            language: Some("auto".to_string()),
            prompt: None,
            openai_api_key: None,
            openai_model: "whisper-1".to_string(),
            base_url: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct RefineConfig {
    pub backend: RefineBackend,
    pub model: String,
    pub system_prompt: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self {
            backend: RefineBackend::None,
            model: "claude-haiku-4-5-20251001".to_string(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            api_key: None,
            base_url: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct OutputConfig {
    pub prefer_inject: bool,
    pub inject_delay_ms: u64,
    pub clipboard_fallback: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self { prefer_inject: true, inject_delay_ms: 20, clipboard_fallback: true }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub transcribe: TranscribeConfig,
    pub refine: RefineConfig,
    pub output: OutputConfig,
}

const DEFAULT_SYSTEM_PROMPT: &str = "\
你是一個語音輸入修飾助手。請將以下語音辨識結果修正錯別字、標點，使其通順自然。\
只輸出修正後的文字，不要加任何解釋。";

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

impl Config {
    /// Load config from a TOML file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load config, falling back to defaults if no file is found or parse fails.
    pub fn load_or_default(path: Option<&Path>) -> Self {
        match path {
            Some(p) => Self::load(p).unwrap_or_else(|e| {
                tracing::warn!("Failed to load config from {}: {}. Using defaults.", p.display(), e);
                Self::default()
            }),
            None => {
                tracing::info!("No config file found. Using built-in defaults.");
                Self::default()
            }
        }
    }

    /// Resolve config file path (CLI > env > XDG > same dir as binary).
    pub fn find_path(cli_path: Option<PathBuf>) -> Option<PathBuf> {
        if let Some(p) = cli_path {
            return Some(p);
        }
        if let Ok(p) = std::env::var("JUST_TALK_CONFIG") {
            return Some(PathBuf::from(p));
        }
        if let Some(config_dir) = dirs::config_dir() {
            let p = config_dir.join("just-talk").join("config.toml");
            if p.exists() {
                return Some(p);
            }
        }
        let local = PathBuf::from("config.toml");
        if local.exists() {
            return Some(local);
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Hot-reload (P1-04)
// ---------------------------------------------------------------------------

/// Spawn a filesystem watcher on `path`.
/// Sends `AppEvent::ReloadConfig` whenever the file is modified or replaced.
/// The returned `RecommendedWatcher` must be kept alive for watching to continue.
pub fn watch_config(
    path: &Path,
    sender: tokio::sync::mpsc::Sender<crate::app::AppEvent>,
) -> anyhow::Result<RecommendedWatcher> {
    let mut watcher =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(ev) if ev.kind.is_modify() || ev.kind.is_create() => {
                tracing::debug!("Config file changed, reloading...");
                let _ = sender.blocking_send(crate::app::AppEvent::ReloadConfig);
            }
            Err(e) => tracing::warn!("Config watcher error: {}", e),
            _ => {}
        })?;
    watcher.watch(path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

// ---------------------------------------------------------------------------
// Tests (P1-07)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, content: &str) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn test_load_full_toml() {
        let content = r#"
[hotkey]
key = "CapsLock"
modifiers = ["Ctrl"]

[transcribe]
backend = "local"
language = "zh"
openai_model = "whisper-1"

[refine]
backend = "none"
model = "test-model"
system_prompt = "Fix it."

[output]
prefer_inject = false
inject_delay_ms = 50
clipboard_fallback = false
"#;
        let p = write_temp("jt_test_full.toml", content);
        let cfg = Config::load(&p).unwrap();
        assert_eq!(cfg.hotkey.key, "CapsLock");
        assert_eq!(cfg.hotkey.modifiers, vec!["Ctrl"]);
        assert_eq!(cfg.transcribe.language, Some("zh".to_string()));
        assert_eq!(cfg.output.prefer_inject, false);
        assert_eq!(cfg.output.inject_delay_ms, 50);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn test_load_minimal_toml_uses_defaults() {
        // Only [hotkey] key is specified; everything else falls back to defaults.
        let content = r#"
[hotkey]
key = "F5"
"#;
        let p = write_temp("jt_test_minimal.toml", content);
        let cfg = Config::load(&p).unwrap();
        assert_eq!(cfg.hotkey.key, "F5");
        assert_eq!(cfg.hotkey.modifiers, Vec::<String>::new());
        assert_eq!(cfg.transcribe.backend, TranscribeBackend::Local);
        assert_eq!(cfg.transcribe.openai_model, "whisper-1");
        assert_eq!(cfg.refine.backend, RefineBackend::None);
        assert_eq!(cfg.output.prefer_inject, true);
        assert_eq!(cfg.output.inject_delay_ms, 20);
        assert_eq!(cfg.output.clipboard_fallback, true);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn test_load_empty_toml_all_defaults() {
        let p = write_temp("jt_test_empty.toml", "");
        let cfg = Config::load(&p).unwrap();
        assert_eq!(cfg.hotkey.key, "F4");
        assert_eq!(cfg.output.inject_delay_ms, 20);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn test_find_path_cli_overrides_env() {
        std::env::set_var("JUST_TALK_CONFIG", "/tmp/env_config.toml");
        let cli = PathBuf::from("/tmp/cli_config.toml");
        let result = Config::find_path(Some(cli.clone()));
        assert_eq!(result, Some(cli));
        std::env::remove_var("JUST_TALK_CONFIG");
    }

    #[test]
    fn test_find_path_env_fallback() {
        std::env::remove_var("JUST_TALK_CONFIG");
        let p = write_temp("jt_env_config.toml", "");
        let path_str = p.to_str().unwrap().to_string();
        std::env::set_var("JUST_TALK_CONFIG", &path_str);
        let result = Config::find_path(None);
        assert_eq!(result, Some(PathBuf::from(&path_str)));
        std::env::remove_var("JUST_TALK_CONFIG");
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn test_load_or_default_no_file() {
        let cfg = Config::load_or_default(None);
        assert_eq!(cfg.hotkey.key, "F4");
    }

    #[test]
    fn test_openai_backend_deserializes() {
        let content = r#"
[transcribe]
backend = "openai"
openai_api_key = "sk-test"

[refine]
backend = "openai"
model = "gpt-4o"
system_prompt = "Fix it."
api_key = "sk-test"
"#;
        let p = write_temp("jt_test_openai.toml", content);
        let cfg = Config::load(&p).unwrap();
        assert_eq!(cfg.transcribe.backend, TranscribeBackend::OpenAi);
        assert_eq!(cfg.refine.backend, RefineBackend::OpenAi);
        std::fs::remove_file(p).ok();
    }
}
