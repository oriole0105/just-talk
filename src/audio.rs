//! Microphone capture via cpal + rubato resampler.
//!
//! # Thread model
//! `AudioCapture::start()` opens a cpal stream on an audio thread chosen by
//! the platform driver.  Samples are appended (under `Mutex`) to a shared
//! `Vec<f32>`.  `stop()` drops the stream, takes the buffer, downmixes to mono,
//! and resamples to 16 kHz — all on the caller's thread.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use crate::overlay::SharedOverlay;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TARGET_SAMPLE_RATE: u32 = 16_000;
const DEFAULT_MAX_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Internal recording state
// ---------------------------------------------------------------------------

struct RecordingState {
    _stream: Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub struct AudioCapture {
    device: cpal::Device,
    stream_config: SupportedStreamConfig,
    max_secs: u64,
    state: Option<RecordingState>,
    overlay: Option<SharedOverlay>,
}

impl AudioCapture {
    /// Open the default input device and choose a supported stream config.
    pub fn new() -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default audio input device found"))?;

        let stream_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("Cannot get input config: {}", e))?;

        tracing::info!(
            device = %device.name().unwrap_or_default(),
            sample_rate = stream_config.sample_rate().0,
            channels = stream_config.channels(),
            format = ?stream_config.sample_format(),
            "Audio device opened"
        );

        Ok(Self {
            device,
            stream_config,
            max_secs: DEFAULT_MAX_SECS,
            state: None,
            overlay: None,
        })
    }

    /// Attach an overlay for live waveform visualisation.
    pub fn set_overlay(&mut self, overlay: SharedOverlay) {
        self.overlay = Some(overlay);
    }

    /// Begin recording. Calling `start()` while already recording is a no-op.
    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.state.is_some() {
            return Ok(());
        }

        let sample_rate = self.stream_config.sample_rate().0;
        let channels = self.stream_config.channels();
        let max_samples = (sample_rate as u64 * channels as u64 * self.max_secs) as usize;

        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let stream = build_stream(
            &self.device,
            &self.stream_config,
            Arc::clone(&buffer),
            max_samples,
            self.overlay.clone(),
        )?;
        stream
            .play()
            .map_err(|e| anyhow::anyhow!("Stream play error: {}", e))?;

        tracing::info!("Recording started (max {}s)", self.max_secs);

        self.state = Some(RecordingState {
            _stream: stream,
            buffer,
            sample_rate,
            channels,
        });
        Ok(())
    }

    /// Stop recording and return 16 kHz mono PCM, or `Ok(vec![])` if never started.
    pub fn stop(mut self) -> anyhow::Result<Vec<f32>> {
        let state = match self.state.take() {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        // Drop the stream first so the callback stops writing.
        drop(state._stream);

        let raw = state
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("Audio buffer mutex poisoned"))?
            .clone();

        tracing::info!(
            samples = raw.len(),
            duration_secs = raw.len() as f64 / (state.sample_rate as f64 * state.channels as f64),
            "Recording stopped"
        );

        let mono = downmix_to_mono(&raw, state.channels);
        let pcm16k = resample_to_16k(&mono, state.sample_rate)?;
        Ok(pcm16k)
    }
}

// ---------------------------------------------------------------------------
// Pure audio helpers (pub(crate) for unit-testability)
// ---------------------------------------------------------------------------

/// Downmix interleaved multi-channel PCM to mono by averaging channels.
pub(crate) fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Resample mono PCM from `source_rate` to 16 000 Hz.
///
/// Uses `rubato::SincFixedIn` with a high-quality sinc kernel.
/// Returns an error if `source_rate == 0` or rubato construction fails.
pub(crate) fn resample_to_16k(mono: &[f32], source_rate: u32) -> anyhow::Result<Vec<f32>> {
    if source_rate == TARGET_SAMPLE_RATE {
        return Ok(mono.to_vec());
    }
    if source_rate == 0 {
        anyhow::bail!("source_rate must be > 0");
    }
    if mono.is_empty() {
        return Ok(vec![]);
    }

    let ratio = TARGET_SAMPLE_RATE as f64 / source_rate as f64;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let chunk_size = 1024_usize;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .map_err(|e| anyhow::anyhow!("Resampler init failed: {:?}", e))?;

    let expected_out = (mono.len() as f64 * ratio).ceil() as usize + chunk_size;
    let mut output: Vec<f32> = Vec::with_capacity(expected_out);

    let mut pos = 0;
    while pos < mono.len() {
        let end = (pos + chunk_size).min(mono.len());
        let mut chunk = mono[pos..end].to_vec();
        // Zero-pad last chunk to full chunk_size.
        if chunk.len() < chunk_size {
            chunk.resize(chunk_size, 0.0);
        }
        let out_frames = resampler
            .process(&[chunk], None)
            .map_err(|e| anyhow::anyhow!("Resampler process failed: {:?}", e))?;
        output.extend_from_slice(&out_frames[0]);
        pos += chunk_size;
    }

    // Trim to the exact expected number of output samples.
    let exact = (mono.len() as f64 * ratio).round() as usize;
    output.truncate(exact);

    Ok(output)
}

// ---------------------------------------------------------------------------
// Stream construction
// ---------------------------------------------------------------------------

fn append_samples(data: &[f32], buffer: &Arc<Mutex<Vec<f32>>>, max_samples: usize) {
    if let Ok(mut buf) = buffer.lock() {
        let remaining = max_samples.saturating_sub(buf.len());
        if remaining == 0 {
            return;
        }
        let to_append = data.len().min(remaining);
        buf.extend_from_slice(&data[..to_append]);
    }
}

fn push_to_overlay(data: &[f32], overlay: &Option<SharedOverlay>) {
    if let Some(ov) = overlay {
        // try_lock: skip if contended — never stall the realtime audio thread.
        if let Ok(mut state) = ov.try_lock() {
            state.push_samples(data);
        }
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    max_samples: usize,
    overlay: Option<SharedOverlay>,
) -> anyhow::Result<Stream> {
    let err_fn = |e| tracing::error!("Audio stream error: {}", e);
    let cfg = config.config();

    let stream = match config.sample_format() {
        SampleFormat::F32 => {
            let buf = Arc::clone(&buffer);
            let ov = overlay.clone();
            device.build_input_stream(
                &cfg,
                move |data: &[f32], _| {
                    push_to_overlay(data, &ov);
                    append_samples(data, &buf, max_samples);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let buf = Arc::clone(&buffer);
            let ov = overlay.clone();
            device.build_input_stream(
                &cfg,
                move |data: &[i16], _| {
                    let floats: Vec<f32> = data.iter().map(|&s| s as f32 / 32_768.0).collect();
                    push_to_overlay(&floats, &ov);
                    append_samples(&floats, &buf, max_samples);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let buf = Arc::clone(&buffer);
            let ov = overlay.clone();
            device.build_input_stream(
                &cfg,
                move |data: &[u16], _| {
                    let floats: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - 32_768.0) / 32_768.0)
                        .collect();
                    push_to_overlay(&floats, &ov);
                    append_samples(&floats, &buf, max_samples);
                },
                err_fn,
                None,
            )
        }
        fmt => anyhow::bail!("Unsupported sample format: {:?}", fmt),
    }
    .map_err(|e| anyhow::anyhow!("Failed to build input stream: {}", e))?;

    Ok(stream)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ----- downmix_to_mono -----

    #[test]
    fn mono_passthrough() {
        let samples = vec![0.1, 0.2, 0.3];
        let out = downmix_to_mono(&samples, 1);
        assert_eq!(out, samples);
    }

    #[test]
    fn stereo_downmix_averages_channels() {
        // L=1.0 R=-1.0 → mono=0.0; L=0.5 R=0.5 → mono=0.5
        let samples = vec![1.0_f32, -1.0, 0.5, 0.5];
        let out = downmix_to_mono(&samples, 2);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn quad_downmix() {
        // Four channels all 0.4 → average 0.4
        let samples = vec![0.4_f32; 4];
        let out = downmix_to_mono(&samples, 4);
        assert_eq!(out.len(), 1);
        assert!((out[0] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn downmix_empty_input() {
        let out = downmix_to_mono(&[], 2);
        assert!(out.is_empty());
    }

    // ----- resample_to_16k -----

    #[test]
    fn resample_passthrough_when_already_16k() {
        let signal: Vec<f32> = (0..1600).map(|i| i as f32 / 1600.0).collect();
        let out = resample_to_16k(&signal, 16_000).unwrap();
        assert_eq!(out.len(), signal.len());
    }

    #[test]
    fn resample_empty_returns_empty() {
        let out = resample_to_16k(&[], 44_100).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn resample_zero_rate_is_err() {
        let signal = vec![0.0_f32; 100];
        assert!(resample_to_16k(&signal, 0).is_err());
    }

    #[test]
    fn resample_44100_to_16k_output_length() {
        // 44100 Hz, 1 second of audio → ~16000 samples at 16k
        let signal: Vec<f32> = vec![0.0_f32; 44_100];
        let out = resample_to_16k(&signal, 44_100).unwrap();
        let expected = 16_000_usize;
        // Allow ±2% tolerance for rubato's chunk-boundary rounding
        let tolerance = (expected as f64 * 0.02) as usize;
        assert!(
            out.len().abs_diff(expected) <= tolerance,
            "Expected ~{expected} samples, got {}",
            out.len()
        );
    }

    #[test]
    fn resample_48000_to_16k_output_length() {
        let signal: Vec<f32> = vec![0.0_f32; 48_000];
        let out = resample_to_16k(&signal, 48_000).unwrap();
        let expected = 16_000_usize;
        let tolerance = (expected as f64 * 0.02) as usize;
        assert!(
            out.len().abs_diff(expected) <= tolerance,
            "Expected ~{expected} samples, got {}",
            out.len()
        );
    }

    // ----- append_samples -----

    #[test]
    fn append_samples_respects_max() {
        let buf = Arc::new(Mutex::new(Vec::new()));
        append_samples(&[1.0, 2.0, 3.0, 4.0, 5.0], &buf, 3);
        let locked = buf.lock().unwrap();
        assert_eq!(*locked, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn append_samples_no_op_when_full() {
        let buf = Arc::new(Mutex::new(vec![1.0_f32, 2.0, 3.0]));
        append_samples(&[4.0], &buf, 3);
        let locked = buf.lock().unwrap();
        assert_eq!(locked.len(), 3);
    }
}
