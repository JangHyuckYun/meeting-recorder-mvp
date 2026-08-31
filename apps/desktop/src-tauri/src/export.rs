//! Renders a stored transcript to the subtitle/text formats the export menu offers.
//! Pure string building — the command layer owns the filesystem.
//
// ponytail: DOCX and PDF are deferred. Both need a real document library (docx-rs /
// printpdf) plus layout decisions; add them when a user actually asks, and build them on
// top of the markdown rendering below rather than from the segments again.

use crate::error::{AppError, AppResult};
use crate::models::TranscriptSegment;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Srt,
    Vtt,
    Md,
    Txt,
}

impl ExportFormat {
    pub fn from_str(value: &str) -> AppResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "srt" => Ok(ExportFormat::Srt),
            "vtt" => Ok(ExportFormat::Vtt),
            "md" | "markdown" => Ok(ExportFormat::Md),
            "txt" | "text" => Ok(ExportFormat::Txt),
            other => Err(AppError::InvalidState(format!(
                "unsupported export format `{other}`; expected srt, vtt, md, or txt"
            ))),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Srt => "srt",
            ExportFormat::Vtt => "vtt",
            ExportFormat::Md => "md",
            ExportFormat::Txt => "txt",
        }
    }
}

/// `HH:MM:SS,mmm` (SRT) or `HH:MM:SS.mmm` (VTT) depending on the separator.
fn timestamp(ms: i64, millis_separator: char) -> String {
    let ms = ms.max(0);
    let (hours, minutes, seconds, millis) = (
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1_000) % 60,
        ms % 1_000,
    );
    format!("{hours:02}:{minutes:02}:{seconds:02}{millis_separator}{millis:03}")
}

/// `HH:MM:SS`, for the human-readable formats.
fn clock(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:02}",
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1_000) % 60
    )
}

/// Applies the user's per-recording speaker renames, falling back to the diarization label.
fn speaker(segment: &TranscriptSegment, names: &BTreeMap<String, String>) -> String {
    names
        .get(&segment.speaker_label)
        .cloned()
        .unwrap_or_else(|| segment.speaker_label.clone())
}

pub fn render(
    title: &str,
    segments: &[TranscriptSegment],
    names: &BTreeMap<String, String>,
    format: ExportFormat,
) -> String {
    match format {
        ExportFormat::Srt => segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                format!(
                    "{}\n{} --> {}\n{}: {}\n",
                    index + 1,
                    timestamp(segment.start_ms, ','),
                    timestamp(segment.end_ms, ','),
                    speaker(segment, names),
                    segment.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        ExportFormat::Vtt => {
            let cues = segments
                .iter()
                .map(|segment| {
                    format!(
                        "{} --> {}\n{}: {}\n",
                        timestamp(segment.start_ms, '.'),
                        timestamp(segment.end_ms, '.'),
                        speaker(segment, names),
                        segment.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("WEBVTT\n\n{cues}")
        }
        ExportFormat::Md => {
            let body = segments
                .iter()
                .map(|segment| {
                    format!(
                        "**{}** `{}`\n\n{}\n",
                        speaker(segment, names),
                        clock(segment.start_ms),
                        segment.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("# {title}\n\n{body}")
        }
        ExportFormat::Txt => segments
            .iter()
            .map(|segment| {
                format!(
                    "[{}] {}: {}",
                    clock(segment.start_ms),
                    speaker(segment, names),
                    segment.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Turns a recording title into something safe to place in a filename on any platform.
pub fn safe_filename(title: &str, suffix: &str, format: ExportFormat) -> String {
    let stem: String = title
        .chars()
        .map(|c| {
            if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let stem = stem.trim().trim_matches('.');
    let stem = if stem.is_empty() { "transcript" } else { stem };
    format!("{stem}-{suffix}.{}", format.extension())
}
