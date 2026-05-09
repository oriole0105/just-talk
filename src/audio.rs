// Phase 4: cpal microphone capture + rubato resampler

pub struct AudioCapture;

impl AudioCapture {
    /// Open the default input device.
    pub fn new() -> anyhow::Result<Self> {
        todo!("Phase 4: cpal device enumeration")
    }

    /// Begin recording into an internal buffer.
    pub fn start(&mut self) -> anyhow::Result<()> {
        todo!("Phase 4: cpal stream start")
    }

    /// Stop recording and return the resampled 16 kHz mono PCM buffer.
    pub fn stop(self) -> anyhow::Result<Vec<f32>> {
        todo!("Phase 4: stop stream, resample to 16 kHz mono")
    }
}
