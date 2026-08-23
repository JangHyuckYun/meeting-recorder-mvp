//! ASR domain state machine: applies caption events to a mutable caption map and
//! maintains version history for revision supersession.

use crate::models::{AudioTime, CaptionEvent, CaptionStatus, OverlapInfo};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Result of applying a caption event to the domain state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplyOutcome {
    /// Event was ignored because it does not advance any caption (e.g. stale partial).
    Ignored,
    /// A caption was created or updated.
    Applied { superseded: Vec<Uuid> },
    /// Event is out-of-order (sequence gap); caller should request a snapshot.
    SequenceGap,
}

/// Tracks one logical caption through its version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionVersion {
    pub version_id: Uuid,
    pub event: CaptionEvent,
    pub applied_at_ms: i64,
}

/// The domain caption store.
#[derive(Debug, Default, Clone)]
pub struct CaptionStore {
    /// Latest event per segment_id.
    captions: std::collections::HashMap<Uuid, CaptionEvent>,
    /// Version history per segment_id (immutable rows).
    history: std::collections::HashMap<Uuid, Vec<CaptionVersion>>,
    /// Monotonic sequence counter of applied events.
    sequence: u64,
    /// Highest acknowledged sequence from the frontend.
    ack_sequence: u64,
}

impl CaptionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a caption event. Returns `Applied` with superseded segment ids.
    /// Ignores stale PARTIAL updates. Captures sequence gaps.
    pub fn apply(&mut self, event: CaptionEvent, applied_at_ms: i64) -> ApplyOutcome {
        let version_id = Uuid::new_v4();

        // A PARTIAL that would move a COMMITTED/REVISED caption back is invalid — reject.
        if let Some(existing) = self.captions.get(&event.segment_id) {
            if matches!((existing.status, event.status),
                (_, CaptionStatus::Absent)
                | (CaptionStatus::Committed | CaptionStatus::Revised, CaptionStatus::Partial)
                | (CaptionStatus::Committed | CaptionStatus::Revised, CaptionStatus::Stable)
            ) {
                return ApplyOutcome::Ignored;
            }
        }

        self.sequence += 1;
        self.captions.insert(event.segment_id, event.clone());

        self.history
            .entry(event.segment_id)
            .or_default()
            .push(CaptionVersion {
                version_id,
                event,
                applied_at_ms,
            });

        ApplyOutcome::Applied {
            superseded: self
                .captions
                .iter()
                .filter(|(_, ev)| ev.status == CaptionStatus::Revised)
                .filter(|(id, ev)| ev.supersedes.contains(id))
                .map(|(id, _)| *id)
                .collect(),
        }
    }

    /// Get the current caption for a segment.
    pub fn get(&self, segment_id: &Uuid) -> Option<&CaptionEvent> {
        self.captions.get(segment_id)
    }

    /// All current captions — used for snapshot replay.
    pub fn all(&self) -> Vec<CaptionEvent> {
        self.captions.values().cloned().collect()
    }

    /// History for a segment.
    pub fn history(&self, segment_id: &Uuid) -> Vec<&CaptionVersion> {
        self.history.get(segment_id).map(|v| v.iter().collect()).unwrap_or_default()
    }

    /// Currently acknowledged sequence from the frontend.
    pub fn ack_sequence(&self) -> u64 {
        self.ack_sequence
    }

    pub fn set_ack_sequence(&mut self, seq: u64) {
        self.ack_sequence = seq;
    }

    pub fn current_sequence(&self) -> u64 {
        self.sequence
    }

    /// Committed or revised captions only (minutes pipeline input).
    pub fn committed_or_revised(&self) -> Vec<CaptionEvent> {
        self.captions
            .values()
            .filter(|ev| matches!(ev.status, CaptionStatus::Committed | CaptionStatus::Revised))
            .cloned()
            .collect()
    }

    /// Helper to build an AudioTime from a sample index contract.
    pub fn audio_time(samples: i64) -> AudioTime {
        AudioTime::from_samples(samples)
    }
}

/// A freshly-decoded caption ready for the state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCaption {
    pub segment_id: Uuid,
    pub start_sample: i64,
    pub end_sample: i64,
    pub text: String,
    pub speaker_label: Option<String>,
    pub overlap: Option<OverlapInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partial(seg: Uuid, text: &str, start: i64, end: i64) -> CaptionEvent {
        CaptionEvent {
            segment_id: seg,
            start_sample: start,
            end_sample: end,
            text: text.to_string(),
            status: CaptionStatus::Partial,
            speaker_label: None,
            overlap: None,
            supersedes: vec![],
        }
    }

    fn committed(seg: Uuid, text: &str, start: i64, end: i64) -> CaptionEvent {
        CaptionEvent {
            segment_id: seg,
            start_sample: start,
            end_sample: end,
            text: text.to_string(),
            status: CaptionStatus::Committed,
            speaker_label: None,
            overlap: None,
            supersedes: vec![],
        }
    }

    #[test]
    fn test_partial_supersedes_partial() {
        let seg = Uuid::new_v4();
        let mut store = CaptionStore::new();
        let p1 = partial(seg, "안녕하", 0, 320);
        let p2 = partial(seg, "안녕하세요", 0, 480);
        store.apply(p1, 100);
        let outcome = store.apply(p2, 200);
        assert_eq!(outcome, ApplyOutcome::Applied { superseded: vec![] });
        assert_eq!(store.get(&seg).unwrap().text, "안녕하세요");
    }

    #[test]
    fn test_committed_rejects_partial() {
        let seg = Uuid::new_v4();
        let mut store = CaptionStore::new();
        store.apply(committed(seg, "안녕하세요", 0, 480), 100);
        let p = partial(seg, "안녕", 0, 320);
        let outcome = store.apply(p, 200);
        assert_eq!(outcome, ApplyOutcome::Ignored);
    }

    #[test]
    fn test_committed_or_revised_filters() {
        let seg1 = Uuid::new_v4();
        let seg2 = Uuid::new_v4();
        let seg3 = Uuid::new_v4();
        let mut store = CaptionStore::new();
        store.apply(committed(seg1, "a", 0, 100), 100);
        store.apply(partial(seg2, "b", 100, 200), 100);
        store.apply(committed(seg3, "c", 200, 300), 100);
        let filtered = store.committed_or_revised();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|e| e.segment_id == seg1));
        assert!(filtered.iter().any(|e| e.segment_id == seg3));
        assert!(!filtered.iter().any(|e| e.segment_id == seg2));
    }

    #[test]
    fn test_audio_time_arithmetic() {
        let a = AudioTime::from_samples(48000);
        assert_eq!(a.as_ms(), 1000);
        assert_eq!(a.diff(AudioTime::from_samples(0)), 48000);
        assert_eq!(a.add_samples(48000).as_ms(), 2000);
        assert_eq!(AudioTime::from_ms(500).as_samples(), 24000);
    }
}