use crate::error::JustTalkError;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    Recording,
    Transcribing,
    Refining,
    Injecting,
}

#[derive(Debug)]
pub enum AppEvent {
    HotkeyPressed,
    TranscribeDone(String),
    RefineDone(String),
    OutputDone,
    Error(JustTalkError),
    Quit,
    ReloadConfig,
}
