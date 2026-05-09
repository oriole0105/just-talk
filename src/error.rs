#[derive(thiserror::Error, Debug)]
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
