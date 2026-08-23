//! Local WAV archive writer. Writes 48kHz stereo PCM to a `.wav.part` file, then
//! fsyncs, hashes, and atomically renames to `.wav` on finalization.

use crate::error::{AppError, AppResult};
use hound::{WavSpec, WavWriter};
use sha2::Digest;
use std::fs;
use std::io::BufWriter;
use std::path::PathBuf;

/// Writes stereo 48kHz PCM to a WAV file with atomic finalization.
pub struct ArchiveWriter {
    writer: Option<WavWriter<BufWriter<fs::File>>>,
    part_path: PathBuf,
    final_path: PathBuf,
    frames_written: u64,
}

impl ArchiveWriter {
    /// Create a new archive writer. Writes to `{path}.wav.part` until `finalize()`.
    pub fn create(path: PathBuf) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let part_path = path.with_extension("wav.part");
        let spec = WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = WavWriter::create(&part_path, spec)
            .map_err(|e| AppError::Audio(format!("failed to create archive wav: {e}")))?;

        Ok(Self {
            writer: Some(writer),
            part_path,
            final_path: path,
            frames_written: 0,
        })
    }

    /// Write a single stereo frame (two i16 samples).
    pub fn write_frame(&mut self, left: i16, right: i16) -> AppResult<()> {
        if let Some(ref mut w) = self.writer {
            w.write_sample(left)
                .map_err(|e| AppError::Audio(format!("archive write error: {e}")))?;
            w.write_sample(right)
                .map_err(|e| AppError::Audio(format!("archive write error: {e}")))?;
            self.frames_written += 1;
        }
        Ok(())
    }

    /// Write interleaved f32 stereo frames, converting to i16 PCM.
    pub fn write_interleaved_f32(&mut self, data: &[f32]) -> AppResult<()> {
        for chunk in data.chunks(2) {
            if chunk.len() < 2 {
                break;
            }
            let left = (chunk[0].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            let right = (chunk[1].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            self.write_frame(left, right)?;
        }
        Ok(())
    }

    /// Finalize the WAV file: fsync, SHA-256 hash, atomically rename .part → .wav,
    /// and return the SHA-256 hex digest.
    pub fn finalize(mut self) -> AppResult<(PathBuf, String)> {
        if let Some(w) = self.writer.take() {
            w.finalize()
                .map_err(|e| AppError::Audio(format!("archive finalize error: {e}")))?;
        }

        // Read back the file for hashing
        let data = fs::read(&self.part_path)?;
        let hash = hex::encode(sha2::Sha256::digest(&data));

        // fsync the directory
        if let Some(parent) = self.part_path.parent() {
            let fd = fs::File::open(parent)?;
            fd.sync_all()?;
        }

        // Atomic rename
        fs::rename(&self.part_path, &self.final_path)?;

        Ok((self.final_path, hash))
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_archive_write_and_finalize() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wav");
        let mut writer = ArchiveWriter::create(path.clone()).unwrap();

        // Write 1 second of silence (48k frames at 48kHz)
        for _ in 0..48_000 {
            writer.write_frame(0, 0).unwrap();
        }

        let (final_path, hash) = writer.finalize().unwrap();
        assert!(final_path.exists());
        assert!(!hash.is_empty());
        assert_eq!(final_path.extension().unwrap(), "wav");
        // Verify the part file is gone
        assert!(!final_path.with_extension("wav.part").exists());
    }
}