mod app;
mod audio;
mod config;
mod error;
mod hotkey;
mod notification;
mod output;
mod refine;
mod transcribe;

#[cfg(feature = "tray")]
mod tray;

fn main() {
    println!("just-talk v{}", env!("CARGO_PKG_VERSION"));
}
