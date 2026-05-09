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

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "just-talk",
    version,
    about = "Cross-platform voice IME — speak to type, powered by Whisper + AI"
)]
struct Cli {
    /// Path to config file (default: ~/.config/just-talk/config.toml)
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable verbose (DEBUG-level) logging
    #[arg(short, long)]
    verbose: bool,

    /// Dry-run: transcribe and refine but print to stdout instead of injecting
    #[arg(long)]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    init_tracing(cli.verbose);

    let config_path = config::Config::find_path(cli.config);
    let cfg = config::Config::load_or_default(config_path.as_deref());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        config = ?config_path,
        hotkey = %cfg.hotkey.key,
        transcribe = ?cfg.transcribe.backend,
        refine = ?cfg.refine.backend,
        dry_run = cli.dry_run,
        "just-talk starting"
    );

    if cli.dry_run {
        tracing::info!("Dry-run mode: output will be printed to stdout");
    }

    // Phase 8: replace with App::new(cfg).run()
    tracing::info!("App state machine not yet implemented (Phase 8). Exiting.");
    Ok(())
}

fn init_tracing(verbose: bool) {
    use tracing_subscriber::{fmt, EnvFilter};

    let level = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("just_talk={level},warn")));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();
}
