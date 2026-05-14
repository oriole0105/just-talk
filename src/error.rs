use thiserror::Error;

#[derive(Error, Debug)]
pub enum JustTalkError {
    #[error("Audio capture failed: {0}")]
    AudioCapture(String),

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("AI refinement failed: {0}")]
    Refinement(String),

    #[error("Output failed: {0}")]
    Output(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Hotkey error: {0}")]
    Hotkey(String),
}

impl JustTalkError {
    pub fn audio(msg: impl Into<String>) -> Self {
        Self::AudioCapture(msg.into())
    }
    pub fn transcription(msg: impl Into<String>) -> Self {
        Self::Transcription(msg.into())
    }
    pub fn refinement(msg: impl Into<String>) -> Self {
        Self::Refinement(msg.into())
    }
    pub fn output(msg: impl Into<String>) -> Self {
        Self::Output(msg.into())
    }
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    pub fn hotkey(msg: impl Into<String>) -> Self {
        Self::Hotkey(msg.into())
    }
}
