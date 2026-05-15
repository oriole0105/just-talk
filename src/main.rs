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

    /// Dry-run: transcribe and refine, but print output to stdout instead of injecting
    #[arg(long)]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Keep the guard alive for the entire process — dropping it flushes and
    // closes the file appender worker thread.
    let _log_guard = init_tracing(cli.verbose);

    let config_path = just_talk::config::Config::find_path(cli.config);
    let cfg = just_talk::config::Config::load_or_default(config_path.as_deref());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        config = ?config_path,
        hotkey = %cfg.hotkey.key,
        transcribe = ?cfg.transcribe.backend,
        refine = ?cfg.refine.backend,
        dry_run = cli.dry_run,
        "just-talk starting"
    );

    just_talk::app::App::new(cfg, config_path, cli.dry_run).run()
}

fn init_tracing(verbose: bool) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let level = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("just_talk={level},warn")));

    // Log file: ~/Library/Logs/just-talk/just-talk.log (macOS standard location).
    // Falls back to ~/.local/share/just-talk/ on Linux/Windows.
    let log_dir = log_directory();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "just-talk.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(filter)
        // stdout: coloured, compact — visible when running from terminal
        .with(fmt::layer().with_target(false).compact())
        // file: plain text, no ANSI colour codes
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(false)
                .compact(),
        )
        .init();

    tracing::info!(log_dir = %log_dir.display(), "Logging to file");
    guard
}

fn log_directory() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_default()
            .join("Library/Logs/just-talk")
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("just-talk")
            .join("logs")
    }
}
