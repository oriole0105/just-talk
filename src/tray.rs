//! macOS menu bar icon via tray-icon crate.
//!
//! Lives on the main thread (NSStatusItem requirement).  Created inside
//! OverlayApp::new() which is called from eframe's creation callback — at that
//! point NSApplication is already set up.

use std::path::PathBuf;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

use crate::overlay::OverlayPhase;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

pub enum TrayPoll {
    None,
    Quit,
    OpenConfig,
}

pub struct TrayManager {
    _icon: TrayIcon,
    quit_id: tray_icon::menu::MenuId,
    config_id: tray_icon::menu::MenuId,
    config_path: Option<PathBuf>,
    last_phase: PhaseTag,
}

/// Cheap discriminant to avoid redundant icon redraws.
#[derive(PartialEq)]
enum PhaseTag {
    Hidden,
    Recording,
    Processing,
    FadingOut,
}

impl TrayManager {
    pub fn new(config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let menu = Menu::new();

        let config_item = MenuItem::new("Open Config…", true, None);
        let quit_item = MenuItem::new("Quit just-talk", true, None);

        let quit_id = quit_item.id().clone();
        let config_id = config_item.id().clone();

        menu.append_items(&[&config_item, &PredefinedMenuItem::separator(), &quit_item])?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("just-talk — Idle")
            .with_icon(circle_icon(22, [130, 130, 130, 255]))
            .build()?;

        Ok(Self {
            _icon: tray,
            quit_id,
            config_id,
            config_path,
            last_phase: PhaseTag::Hidden,
        })
    }

    /// Update icon + tooltip to match the current overlay phase.
    /// Call every frame from OverlayApp::update().
    pub fn sync_phase(&mut self, phase: &OverlayPhase) {
        let tag = match phase {
            OverlayPhase::Hidden => PhaseTag::Hidden,
            OverlayPhase::Recording => PhaseTag::Recording,
            OverlayPhase::Processing { .. } => PhaseTag::Processing,
            OverlayPhase::FadingOut { .. } => PhaseTag::FadingOut,
        };

        if tag == self.last_phase {
            return;
        }
        self.last_phase = tag;

        match phase {
            OverlayPhase::Hidden | OverlayPhase::FadingOut { .. } => {
                let _ = self._icon.set_tooltip(Some("just-talk — Idle"));
                let _ = self
                    ._icon
                    .set_icon(Some(circle_icon(22, [130, 130, 130, 255])));
            }
            OverlayPhase::Recording => {
                let _ = self._icon.set_tooltip(Some("just-talk — Recording…"));
                let _ = self
                    ._icon
                    .set_icon(Some(circle_icon(22, [210, 50, 50, 255])));
            }
            OverlayPhase::Processing { label } => {
                let _ = self._icon.set_tooltip(Some(format!("just-talk — {label}")));
                let _ = self
                    ._icon
                    .set_icon(Some(circle_icon(22, [200, 155, 40, 255])));
            }
        }
    }

    /// Non-blocking poll for menu events. Returns the first pending action.
    pub fn poll(&self) -> TrayPoll {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                return TrayPoll::Quit;
            }
            if event.id == self.config_id {
                return TrayPoll::OpenConfig;
            }
        }
        TrayPoll::None
    }

    /// Open the config file in the system default editor.
    pub fn open_config(&self) {
        if let Some(ref path) = self.config_path {
            let _ = std::process::Command::new("open").arg(path).spawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Icon helpers
// ---------------------------------------------------------------------------

/// Generate a filled anti-aliased circle as an RGBA bitmap.
fn circle_icon(size: u32, color: [u8; 4]) -> Icon {
    let n = (size * size * 4) as usize;
    let mut rgba = vec![0u8; n];
    let center = size as f32 / 2.0;
    let radius = center - 1.5;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            if (dx * dx + dy * dy).sqrt() <= radius {
                let i = ((y * size + x) * 4) as usize;
                rgba[i] = color[0];
                rgba[i + 1] = color[1];
                rgba[i + 2] = color[2];
                rgba[i + 3] = color[3];
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("valid circle icon")
}
