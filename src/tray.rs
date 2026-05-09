// Phase 2: system tray icon (tray-icon crate)
// Only compiled when feature = "tray" is active.

pub struct TrayManager;

impl TrayManager {
    pub fn new() -> anyhow::Result<Self> {
        todo!("Phase 2: tray-icon setup")
    }

    pub fn set_recording(&mut self, _recording: bool) {
        // TODO Phase 2: swap idle/recording icon
    }
}
