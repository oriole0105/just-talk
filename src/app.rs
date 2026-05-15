//! App state machine — wires together all subsystems.
//!
//! State transitions (§2.3):
//!
//! ```text
//! Idle  ──[HotkeyPressed]──► Recording
//! Recording  ──[HotkeyPressed]──► Transcribing  (audio.stop() → spawn transcribe task)
//! Transcribing  ──[TranscribeDone]──► Refining  (spawn refine task)
//! Refining  ──[RefineDone]──► Injecting  (spawn output task / dry-run println)
//! Injecting  ──[OutputDone]──► Idle
//! * ──[Error]──► Idle
//! * ──[ReloadConfig]──► (rebuild transcriber + refiner, stay in state)
//! * ──[Quit]──► exit
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::audio::AudioCapture;
use crate::config::{self, Config};
use crate::error::JustTalkError;
use crate::hotkey::HotkeyManager;
use crate::notification;
use crate::output::OutputManager;
use crate::overlay::{self, OverlayApp, OverlayState, SharedOverlay};
use crate::refine::{self, Refiner};
use crate::transcribe::{self, Transcriber};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

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

// Type aliases to reduce verbosity when storing trait objects in Arc.
type DynTranscriber = Arc<dyn Transcriber + Send + Sync>;
type DynRefiner = Arc<dyn Refiner + Send + Sync>;

// ---------------------------------------------------------------------------
// Pure state transition (no side effects — testable without async)
// ---------------------------------------------------------------------------

/// Return the next `AppState` for `(state, event)`, or `None` if the event
/// is silently ignored in the current state.
pub fn apply_transition(state: &AppState, event: &AppEvent) -> Option<AppState> {
    match (state, event) {
        (AppState::Idle, AppEvent::HotkeyPressed) => Some(AppState::Recording),
        (AppState::Recording, AppEvent::HotkeyPressed) => Some(AppState::Transcribing),
        (AppState::Transcribing, AppEvent::TranscribeDone(_)) => Some(AppState::Refining),
        (AppState::Refining, AppEvent::RefineDone(_)) => Some(AppState::Injecting),
        (AppState::Injecting, AppEvent::OutputDone) => Some(AppState::Idle),
        (_, AppEvent::Error(_)) => Some(AppState::Idle),
        // ReloadConfig and Quit are handled as side-effects; no state change.
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    config: Config,
    config_path: Option<PathBuf>,
    dry_run: bool,
}

impl App {
    pub fn new(config: Config, config_path: Option<PathBuf>, dry_run: bool) -> Self {
        Self {
            config,
            config_path,
            dry_run,
        }
    }

    /// Block the calling thread until the app exits (Ctrl+C or `Quit` event).
    pub fn run(self) -> anyhow::Result<()> {
        let overlay: SharedOverlay = Arc::new(Mutex::new(OverlayState::default()));

        let (tx, rx) = mpsc::channel::<AppEvent>(64);
        let hotkey_cfg = self.config.hotkey.clone();

        // On macOS: polls CGEventSourceKeyState (no TSM calls — avoids macOS 26 crash).
        // On Linux/Windows: rdev::listen on a background thread.
        HotkeyManager::spawn(tx.clone(), &hotkey_cfg)?;

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        // Tokio event loop on a background thread — eframe must own the main thread.
        let tx_for_overlay = tx.clone();
        let config_path_for_overlay = self.config_path.clone();
        let overlay2 = Arc::clone(&overlay);
        std::thread::Builder::new()
            .name("event-loop".to_string())
            .spawn(move || {
                let result = rt.block_on(self.event_loop(tx, rx, overlay2));
                if let Err(ref e) = result {
                    tracing::error!("Event loop error: {}", e);
                }
                // event_loop already sets overlay.quit = true before returning,
                // which causes eframe to close on the next repaint (~100 ms).
            })?;

        // eframe must run on the main thread (macOS NSApplication requirement).
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_transparent(true)
                .with_always_on_top()
                .with_resizable(false)
                .with_active(false) // never steal keyboard focus
                .with_mouse_passthrough(true)
                .with_inner_size([overlay::WINDOW_W, overlay::WINDOW_H]),
            ..Default::default()
        };

        eframe::run_native(
            "just-talk",
            options,
            Box::new(move |_cc| {
                Ok(Box::new(OverlayApp::new(
                    overlay,
                    tx_for_overlay,
                    config_path_for_overlay,
                )))
            }),
        )
        .map_err(|e| anyhow::anyhow!("eframe error: {e}"))
    }

    async fn event_loop(
        self,
        tx: mpsc::Sender<AppEvent>,
        mut rx: mpsc::Receiver<AppEvent>,
        overlay: SharedOverlay,
    ) -> anyhow::Result<()> {
        let App {
            config,
            config_path,
            dry_run,
        } = self;

        // Initialise subsystems.
        let mut transcriber: DynTranscriber =
            Arc::from(transcribe::create_transcriber(&config.transcribe)?);
        let mut refiner: DynRefiner = Arc::from(refine::create_refiner(&config.refine));
        let mut current_cfg = config;

        // Config hot-reload watcher (dropped when event_loop exits).
        let _watcher = config_path.as_ref().and_then(|p| {
            config::watch_config(p, tx.clone())
                .map_err(|e| tracing::warn!("Config watch init failed: {}", e))
                .ok()
        });

        // Ctrl+C → Quit.
        {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    tracing::info!("Ctrl+C — shutting down");
                    let _ = tx2.send(AppEvent::Quit).await;
                }
            });
        }

        let mut state = AppState::Idle;
        let mut audio: Option<AudioCapture> = None;

        tracing::info!("just-talk ready (hotkey={})", current_cfg.hotkey.key);
        notification::show(
            "just-talk",
            &format!("Ready — press {} to record", current_cfg.hotkey.key),
        );

        while let Some(event) = rx.recv().await {
            tracing::debug!(?state, ?event, "event");

            // Use a cloned state in the match to avoid borrow-checker issues
            // when reassigning `state` inside arms.
            let s = state.clone();
            match (s, event) {
                // ── Idle → Recording ─────────────────────────────────────
                (AppState::Idle, AppEvent::HotkeyPressed) => {
                    match start_recording(Arc::clone(&overlay)) {
                        Ok(cap) => {
                            audio = Some(cap);
                            state = AppState::Recording;
                            overlay.lock().unwrap().set_recording();
                            notification::show("just-talk", "Recording…");
                            tracing::info!("→ Recording");
                        }
                        Err(e) => {
                            tracing::error!("Audio start failed: {}", e);
                            notification::show_error("just-talk", &format!("Mic error: {e}"));
                        }
                    }
                }

                // ── Recording → Transcribing ──────────────────────────────
                (AppState::Recording, AppEvent::HotkeyPressed) => {
                    state = AppState::Transcribing;
                    overlay.lock().unwrap().set_processing("Transcribing…");
                    notification::show("just-talk", "Transcribing…");
                    tracing::info!("→ Transcribing");

                    if let Some(cap) = audio.take() {
                        // Stop recording inline — fast (drops stream, copies PCM).
                        // AudioCapture is not guaranteed Send, so we don't pass it
                        // across the thread boundary; only Vec<f32> (Send) is.
                        match cap.stop() {
                            Ok(pcm) => {
                                let tx2 = tx.clone();
                                let t = Arc::clone(&transcriber);
                                tokio::spawn(async move {
                                    let evt = run_transcribe(pcm, t).await;
                                    let _ = tx2.send(evt).await;
                                });
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(AppEvent::Error(JustTalkError::audio(e.to_string())))
                                    .await;
                            }
                        }
                    }
                }

                // ── Transcribing → Refining ───────────────────────────────
                (AppState::Transcribing, AppEvent::TranscribeDone(raw)) => {
                    state = AppState::Refining;
                    overlay.lock().unwrap().set_processing("Refining…");
                    tracing::info!(chars = raw.len(), "→ Refining");

                    let tx2 = tx.clone();
                    let r = Arc::clone(&refiner);
                    tokio::spawn(async move {
                        let evt = run_refine(raw, r).await;
                        let _ = tx2.send(evt).await;
                    });
                }

                // ── Refining → Injecting ──────────────────────────────────
                (AppState::Refining, AppEvent::RefineDone(text)) => {
                    state = AppState::Injecting;
                    tracing::info!(chars = text.len(), "→ Injecting");

                    if dry_run {
                        println!("[just-talk] {text}");
                        let _ = tx.send(AppEvent::OutputDone).await;
                    } else {
                        let tx2 = tx.clone();
                        let out = OutputManager::new(&current_cfg.output);
                        tokio::spawn(async move {
                            let evt = run_output(out, text).await;
                            let _ = tx2.send(evt).await;
                        });
                    }
                }

                // ── Injecting → Idle ──────────────────────────────────────
                (AppState::Injecting, AppEvent::OutputDone) => {
                    state = AppState::Idle;
                    overlay.lock().unwrap().start_fade_out();
                    tracing::info!("→ Idle");
                }

                // ── Error (any state) → Idle ──────────────────────────────
                (_, AppEvent::Error(e)) => {
                    tracing::error!("Error in {:?}: {}", state, e);
                    notification::show_error("just-talk", &e.to_string());
                    overlay.lock().unwrap().start_fade_out();
                    audio = None;
                    state = AppState::Idle;
                }

                // ── ReloadConfig ──────────────────────────────────────────
                (_, AppEvent::ReloadConfig) => {
                    if let Some(ref path) = config_path {
                        match Config::load(path) {
                            Ok(new_cfg) => {
                                match transcribe::create_transcriber(&new_cfg.transcribe) {
                                    Ok(t) => transcriber = Arc::from(t),
                                    Err(e) => tracing::warn!("Transcriber reload failed: {}", e),
                                }
                                refiner = Arc::from(refine::create_refiner(&new_cfg.refine));
                                current_cfg = new_cfg;
                                tracing::info!("Config reloaded");
                                notification::show("just-talk", "Config reloaded");
                            }
                            Err(e) => tracing::warn!("Config reload failed: {}", e),
                        }
                    }
                }

                // ── Quit ──────────────────────────────────────────────────
                (_, AppEvent::Quit) => {
                    tracing::info!("Quit — exiting event loop");
                    break;
                }

                // ── Ignore unexpected combinations ────────────────────────
                (s, e) => {
                    tracing::debug!("Ignoring {:?} in state {:?}", e, s);
                }
            }
        }

        // Signal eframe to close on the next repaint (~100 ms).
        overlay.lock().unwrap().quit = true;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper: open device + start stream
// ---------------------------------------------------------------------------

fn start_recording(overlay: SharedOverlay) -> anyhow::Result<AudioCapture> {
    let mut cap = AudioCapture::new()?;
    cap.set_overlay(overlay);
    cap.start()?;
    Ok(cap)
}

// ---------------------------------------------------------------------------
// Async task helpers (run inside tokio::spawn)
// ---------------------------------------------------------------------------

async fn run_transcribe(pcm: Vec<f32>, t: DynTranscriber) -> AppEvent {
    match t.transcribe(&pcm, 16_000).await {
        Ok(text) => {
            tracing::info!(chars = text.len(), "Transcription done");
            AppEvent::TranscribeDone(text)
        }
        Err(e) => AppEvent::Error(JustTalkError::transcription(e.to_string())),
    }
}

async fn run_refine(raw: String, r: DynRefiner) -> AppEvent {
    match r.refine(&raw).await {
        Ok(text) => {
            tracing::info!(chars = text.len(), "Refinement done");
            AppEvent::RefineDone(text)
        }
        Err(e) => AppEvent::Error(JustTalkError::refinement(e.to_string())),
    }
}

async fn run_output(out: OutputManager, text: String) -> AppEvent {
    match out.send(&text).await {
        Ok(()) => AppEvent::OutputDone,
        Err(e) => AppEvent::Error(JustTalkError::output(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey() -> AppEvent {
        AppEvent::HotkeyPressed
    }
    fn tdone(s: &str) -> AppEvent {
        AppEvent::TranscribeDone(s.into())
    }
    fn rdone(s: &str) -> AppEvent {
        AppEvent::RefineDone(s.into())
    }
    fn err() -> AppEvent {
        AppEvent::Error(JustTalkError::audio("test"))
    }

    #[test]
    fn happy_path_full_cycle() {
        let seq = [
            (AppState::Idle, hotkey(), AppState::Recording),
            (AppState::Recording, hotkey(), AppState::Transcribing),
            (AppState::Transcribing, tdone("hello"), AppState::Refining),
            (AppState::Refining, rdone("Hello."), AppState::Injecting),
            (AppState::Injecting, AppEvent::OutputDone, AppState::Idle),
        ];
        for (from, event, expected) in seq {
            let got = apply_transition(&from, &event);
            assert_eq!(got, Some(expected.clone()), "from={from:?}");
        }
    }

    #[test]
    fn error_returns_to_idle_from_any_state() {
        let states = [
            AppState::Idle,
            AppState::Recording,
            AppState::Transcribing,
            AppState::Refining,
            AppState::Injecting,
        ];
        for s in states {
            assert_eq!(
                apply_transition(&s, &err()),
                Some(AppState::Idle),
                "from={s:?}"
            );
        }
    }

    #[test]
    fn quit_and_reload_return_none() {
        for s in [AppState::Idle, AppState::Recording] {
            assert_eq!(apply_transition(&s, &AppEvent::Quit), None);
            assert_eq!(apply_transition(&s, &AppEvent::ReloadConfig), None);
        }
    }

    #[test]
    fn wrong_event_in_state_is_ignored() {
        // HotkeyPressed is only meaningful in Idle and Recording.
        assert_eq!(apply_transition(&AppState::Transcribing, &hotkey()), None);
        assert_eq!(apply_transition(&AppState::Injecting, &hotkey()), None);
        // OutputDone only matters in Injecting.
        assert_eq!(
            apply_transition(&AppState::Idle, &AppEvent::OutputDone),
            None
        );
        assert_eq!(
            apply_transition(&AppState::Refining, &AppEvent::OutputDone),
            None
        );
    }

    #[test]
    fn transcribe_done_only_in_transcribing() {
        assert_eq!(apply_transition(&AppState::Idle, &tdone("x")), None);
        assert_eq!(apply_transition(&AppState::Refining, &tdone("x")), None);
    }

    #[test]
    fn refine_done_only_in_refining() {
        assert_eq!(apply_transition(&AppState::Idle, &rdone("x")), None);
        assert_eq!(apply_transition(&AppState::Injecting, &rdone("x")), None);
    }
}
