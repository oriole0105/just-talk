//! just-talk library crate — exposes all modules for integration tests.

pub mod app;
pub mod audio;
pub mod config;
pub mod error;
pub mod hotkey;
pub mod notification;
pub mod output;
pub mod overlay;
pub mod refine;
pub mod transcribe;

#[cfg(target_os = "macos")]
pub mod tray;
