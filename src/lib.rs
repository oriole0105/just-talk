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

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod tray;
