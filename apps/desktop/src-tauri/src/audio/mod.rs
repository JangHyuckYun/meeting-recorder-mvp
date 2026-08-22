//! Microphone capture via `cpal`. Writes 16-bit PCM WAV to a local file so the same pipeline
//! (STT client reading a WAV/PCM file) works for both live captures and ingested test files.

use crate::error::{AppError, AppResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A running capture session. Dropping the `cpal::Stream` stops capture; call `stop()` to
/// finalize the WAV file and get back the path + duration.
pub struct CaptureSession {
    stream: cpal::Stream,
    writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    path: PathBuf,
    sample_rate: u32,
    frames_written: Arc<std::sync::atomic::AtomicU64>,
    stopped: Arc<AtomicBool>,
}

// cpal::Stream is not Send on some platforms' backends, but on macOS CoreAudio it is safe to
// hold across the await points we use it for (we never call stream methods from another thread
// concurrently). We only move it into the Tauri-managed state, never share it across threads.
unsafe impl Send for CaptureSession {}

impl CaptureSession {
    pub fn start(out_path: PathBuf) -> AppResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::Audio("no default input device".to_string()))?;
        let config = device
            .default_input_config()
            .map_err(|e| AppError::Audio(format!("no supported input config: {e}")))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = WavWriter::create(&out_path, spec)
            .map_err(|e| AppError::Audio(format!("failed to create wav writer: {e}")))?;
        let writer = Arc::new(Mutex::new(Some(writer)));
        let writer_cb = writer.clone();
        let frames_written = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let frames_cb = frames_written.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_cb = stopped.clone();

        let err_fn = |err| eprintln!("audio stream error: {err}");

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    if stopped_cb.load(Ordering::Relaxed) {
                        return;
                    }
                    let mut guard = writer_cb.lock().unwrap();
                    if let Some(w) = guard.as_mut() {
                        // Downmix to mono by averaging channels, convert f32 [-1,1] -> i16 PCM.
                        for frame in data.chunks(channels as usize) {
                            let avg: f32 = frame.iter().sum::<f32>() / frame.len().max(1) as f32;
                            let sample = (avg.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            let _ = w.write_sample(sample);
                        }
                        frames_cb.fetch_add((data.len() / channels.max(1) as usize) as u64, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AppError::Audio(format!("failed to build input stream: {e}")))?;

        stream
            .play()
            .map_err(|e| AppError::Audio(format!("failed to start stream: {e}")))?;

        Ok(Self {
            stream,
            writer,
            path: out_path,
            sample_rate,
            frames_written,
            stopped,
        })
    }

    /// Stops the stream, finalizes the WAV file, and returns (path, duration_ms).
    pub fn stop(self) -> AppResult<(PathBuf, i64)> {
        self.stopped.store(true, Ordering::Relaxed);
        drop(self.stream);
        let mut guard = self.writer.lock().unwrap();
        if let Some(w) = guard.take() {
            w.finalize()
                .map_err(|e| AppError::Audio(format!("failed to finalize wav: {e}")))?;
        }
        let frames = self.frames_written.load(Ordering::Relaxed);
        let duration_ms = (frames as f64 / self.sample_rate as f64 * 1000.0) as i64;
        Ok((self.path, duration_ms))
    }
}
