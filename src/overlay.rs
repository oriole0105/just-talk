//! Floating overlay window — waveform during recording, status text during
//! transcription / refinement, fade-out on completion.
//!
//! The window starts hidden and is shown / hidden via `ViewportCommand::Visible`.
//! It always stays on top and has no decorations (frameless, transparent bg).

use egui::{Color32, Pos2, Rect, RichText, Vec2};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const BAR_COUNT: usize = 40;
pub const WINDOW_W: f32 = 340.0;
pub const WINDOW_H: f32 = 90.0;
const FADE_SECS: f32 = 0.45;
/// Rolling buffer: ~0.5 s of 16 kHz mono — enough for smooth waveform display.
const SAMPLE_CAP: usize = 8_000;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum OverlayPhase {
    Hidden,
    Recording,
    Processing { label: String },
    FadingOut { started: Instant },
}

pub struct OverlayState {
    pub phase: OverlayPhase,
    /// Recent raw audio samples fed from the cpal callback for waveform drawing.
    pub samples: VecDeque<f32>,
    /// Set to `true` when the event loop is done so eframe can exit cleanly.
    pub quit: bool,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            phase: OverlayPhase::Hidden,
            samples: VecDeque::with_capacity(SAMPLE_CAP),
            quit: false,
        }
    }
}

impl OverlayState {
    /// Push raw (pre-resampled) audio samples into the rolling visualisation buffer.
    /// Called from the cpal callback via `try_lock` — never blocks.
    pub fn push_samples(&mut self, data: &[f32]) {
        for &s in data {
            if self.samples.len() >= SAMPLE_CAP {
                self.samples.pop_front();
            }
            self.samples.push_back(s);
        }
    }

    pub fn set_recording(&mut self) {
        self.samples.clear();
        self.phase = OverlayPhase::Recording;
    }

    pub fn set_processing(&mut self, label: impl Into<String>) {
        self.samples.clear();
        self.phase = OverlayPhase::Processing {
            label: label.into(),
        };
    }

    pub fn start_fade_out(&mut self) {
        if !matches!(
            self.phase,
            OverlayPhase::Hidden | OverlayPhase::FadingOut { .. }
        ) {
            self.phase = OverlayPhase::FadingOut {
                started: Instant::now(),
            };
        }
    }

    fn hide(&mut self) {
        self.phase = OverlayPhase::Hidden;
        self.samples.clear();
    }
}

/// Thread-safe handle passed to audio capture, app logic, and the UI.
pub type SharedOverlay = Arc<Mutex<OverlayState>>;

// ---------------------------------------------------------------------------
// eframe App
// ---------------------------------------------------------------------------

pub struct OverlayApp {
    overlay: SharedOverlay,
    positioned: bool,
}

impl OverlayApp {
    pub fn new(overlay: SharedOverlay) -> Self {
        Self {
            overlay,
            positioned: false,
        }
    }
}

impl eframe::App for OverlayApp {
    /// Return fully transparent so the OS composites only what egui draws.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // On first frame, move the window to screen bottom-centre.
        if !self.positioned {
            if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                let x = (monitor.x - WINDOW_W) / 2.0;
                let y = monitor.y - WINDOW_H - 60.0;
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(x, y)));
            }
            self.positioned = true;
        }

        let mut state = self.overlay.lock().unwrap();

        // Event-loop finished — close the window so eframe::run_native returns.
        if state.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        match state.phase.clone() {
            OverlayPhase::Hidden => {
                // Window is transparent — no content drawn, mouse events pass through.
                drop(state);
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }

            OverlayPhase::FadingOut { started } => {
                let progress = started.elapsed().as_secs_f32() / FADE_SECS;
                let alpha = ((1.0 - progress) * 230.0).clamp(0.0, 230.0) as u8;
                if alpha == 0 {
                    state.hide();
                    drop(state);
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    return;
                }
                render_panel(ctx, &state, alpha);
            }

            _ => {
                render_panel(ctx, &state, 220);
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(33)); // ~30 fps
    }
}

// ---------------------------------------------------------------------------
// Panel rendering
// ---------------------------------------------------------------------------

fn render_panel(ctx: &egui::Context, state: &OverlayState, alpha: u8) {
    let bg = Color32::from_rgba_unmultiplied(18, 18, 22, alpha);
    let frame = egui::Frame::none()
        .fill(bg)
        .rounding(egui::Rounding::same(14.0))
        .inner_margin(egui::Margin::symmetric(16.0, 10.0));

    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        ui.vertical_centered(|ui| match &state.phase {
            OverlayPhase::Recording => {
                ui.label(
                    RichText::new("  Recording")
                        .color(Color32::from_rgba_unmultiplied(240, 240, 240, alpha))
                        .size(13.0),
                );
                ui.add_space(4.0);
                draw_waveform(ui, &state.samples, alpha);
            }

            OverlayPhase::Processing { label } => {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(label.as_str())
                        .color(Color32::from_rgba_unmultiplied(200, 200, 200, alpha))
                        .size(13.0),
                );
            }

            OverlayPhase::FadingOut { .. } => {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Done")
                        .color(Color32::from_rgba_unmultiplied(120, 220, 120, alpha))
                        .size(13.0),
                );
            }

            OverlayPhase::Hidden => {}
        });
    });
}

fn draw_waveform(ui: &mut egui::Ui, samples: &VecDeque<f32>, alpha: u8) {
    let width = ui.available_width().min(WINDOW_W - 32.0);
    let height = 36.0_f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    let step = (width / BAR_COUNT as f32).max(1.0);
    let bar_w = (step * 0.65).max(1.5);

    for i in 0..BAR_COUNT {
        let seg_start = i * samples.len() / BAR_COUNT;
        let seg_end = ((i + 1) * samples.len() / BAR_COUNT).min(samples.len());

        let amplitude = if seg_end > seg_start {
            let rms = samples
                .range(seg_start..seg_end)
                .map(|s| s * s)
                .sum::<f32>()
                / (seg_end - seg_start) as f32;
            (rms.sqrt() * 5.0).clamp(0.04, 1.0)
        } else {
            0.04
        };

        let bar_h = (amplitude * height).max(4.0);
        let x = rect.left() + step * i as f32 + step / 2.0;

        // Colour shifts green → yellow as amplitude rises.
        let r = (amplitude * 200.0).min(210.0) as u8;
        let g = (140.0 + amplitude * 80.0).min(230.0) as u8;
        let color = Color32::from_rgba_unmultiplied(r, g, 55, alpha);

        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(bar_w, bar_h)),
            2.5,
            color,
        );
    }
}
