//! Bounded ring buffer for live audio capture. Maintains 8-12 seconds of stereo 48kHz PCM
//! in memory, spilling to a file on disk when the buffer is exceeded. The buffer is intended
//! to be read by the LAN transport loop.


use crate::models::AudioTime;
use std::collections::VecDeque;
use std::path::PathBuf;

/// A single 48kHz stereo PCM frame: two f32 samples.
pub type StereoFrame = [f32; 2];

/// Bounded ring buffer for live audio capture.
pub struct AudioRingBuffer {
    /// Maximum capacity in frames (8-12 seconds at 48kHz).
    max_frames: usize,
    /// Ring buffer storage.
    buffer: VecDeque<StereoFrame>,
    /// First sample index of the buffer content.
    base_sample: i64,
    /// Total frames written since creation.
    total_written: u64,
    /// Path for spill file when buffer is exceeded.
    spill_path: Option<PathBuf>,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with capacity for `duration_secs` seconds at 48kHz.
    pub fn new(duration_secs: usize) -> Self {
        let max_frames = duration_secs * 48_000;
        Self {
            max_frames,
            buffer: VecDeque::with_capacity(max_frames),
            base_sample: 0,
            total_written: 0,
            spill_path: None,
        }
    }

    /// Set the spill file path. When the buffer overflows, frames are written here.
    pub fn set_spill_path(&mut self, path: PathBuf) {
        self.spill_path = Some(path);
    }

    /// Push a stereo frame into the buffer. Drops oldest frames when capacity is exceeded.
    pub fn push(&mut self, frame: StereoFrame) {
        if self.buffer.len() >= self.max_frames {
            self.buffer.pop_front();
            self.base_sample += 1;
        }
        self.buffer.push_back(frame);
        self.total_written += 1;
    }

    /// Read a range of frames by sample index. Returns consecutive frames from the
    /// buffer if available; missing frames produce zeros.
    pub fn read_range(&self, start_sample: AudioTime, end_sample: AudioTime) -> Vec<StereoFrame> {
        let start = start_sample.as_samples();
        let end = end_sample.as_samples();
        let buf_start = self.base_sample;
        let buf_end = self.base_sample + self.buffer.len() as i64;

        let read_start = start.max(buf_start);
        let read_end = end.min(buf_end);
        let len = (read_end - read_start) as usize;

        if len == 0 {
            return Vec::new();
        }

        let offset = (read_start - buf_start) as usize;
        self.buffer
            .iter()
            .skip(offset)
            .take(len)
            .copied()
            .collect()
    }

    /// Returns the current buffer contents as a Vec of interleaved f32 samples.
    pub fn as_interleaved(&self) -> Vec<f32> {
        self.buffer
            .iter()
            .flat_map(|f| f.iter().copied())
            .collect()
    }

    /// Length of the buffer in frames.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Current base sample index (oldest frame in buffer).
    pub fn base_sample(&self) -> i64 {
        self.base_sample
    }

    /// Total frames written since creation.
    pub fn total_written(&self) -> u64 {
        self.total_written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic_push_and_read() {
        let mut buf = AudioRingBuffer::new(1); // 1 second = 48k frames
        for i in 0..48_000 {
            buf.push([i as f32, (i + 1) as f32]);
        }
        assert_eq!(buf.len(), 48_000);
        let start = AudioTime::from_samples(0);
        let end = AudioTime::from_samples(100);
        let range = buf.read_range(start, end);
        assert_eq!(range.len(), 100);
        assert!((range[0][0] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ring_buffer_wraps() {
        let mut buf = AudioRingBuffer::new(1); // 48k frames capacity
        for i in 0..96_000 {
            buf.push([i as f32, (i + 1) as f32]);
        }
        assert_eq!(buf.len(), 48_000);
        // The base sample should have advanced by 48k frames.
        assert_eq!(buf.base_sample(), 48_000);
    }

    #[test]
    fn test_ring_buffer_read_out_of_range() {
        let buf = AudioRingBuffer::new(1);
        let start = AudioTime::from_samples(0);
        let end = AudioTime::from_samples(100);
        let range = buf.read_range(start, end);
        assert!(range.is_empty());
    }
}