use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TranscribeBackend {
    Local,
    OpenAi,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RefineBackend {
    Claude,
    OpenAi,
    Ollama,
    None,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HotkeyConfig {
    pub key: String,
    #[serde(default)]
    pub modifiers: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TranscribeConfig {
    pub backend: TranscribeBackend,
    pub model_path: Option<PathBuf>,
    pub language: Option<String>,
    pub openai_api_key: Option<String>,
    #[serde(default = "default_whisper_model")]
    pub openai_model: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RefineConfig {
    pub backend: RefineBackend,
    pub model: String,
    pub system_prompt: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OutputConfig {
    #[serde(default = "default_true")]
    pub prefer_inject: bool,
    #[serde(default = "default_inject_delay")]
    pub inject_delay_ms: u64,
    #[serde(default = "default_true")]
    pub clipboard_fallback: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub transcribe: TranscribeConfig,
    pub refine: RefineConfig,
    pub output: OutputConfig,
}

impl Config {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

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

fn default_whisper_model() -> String {
    "whisper-1".to_string()
}

fn default_true() -> bool {
    true
}

fn default_inject_delay() -> u64 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_path_env() {
        std::env::set_var("JUST_TALK_CONFIG", "/tmp/test_config.toml");
        let path = Config::find_path(None);
        assert_eq!(path, Some(PathBuf::from("/tmp/test_config.toml")));
        std::env::remove_var("JUST_TALK_CONFIG");
    }

    #[test]
    fn test_find_path_cli_takes_priority() {
        std::env::set_var("JUST_TALK_CONFIG", "/tmp/env_config.toml");
        let cli = PathBuf::from("/tmp/cli_config.toml");
        let path = Config::find_path(Some(cli.clone()));
        assert_eq!(path, Some(cli));
        std::env::remove_var("JUST_TALK_CONFIG");
    }
}
