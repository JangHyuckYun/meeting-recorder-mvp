#![allow(dead_code)]

//! Minutes caching with transcript-generation invalidation. When an ASR revision (REVISED)
//! supersedes a committed caption, the minutes grounded in that transcript become stale and
//! must be regenerated. The generation counter is the source of truth for staleness.

use crate::models::CaptionStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Staleness state for a recording's minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MinutesState {
    Fresh,
    Stale,
}

/// Tracks the transcript generation a recording's minutes were built from.
#[derive(Debug, Clone)]
pub struct MinutesCache {
    /// recording_id -> (last built transcript_generation, staleness)
    records: HashMap<Uuid, CacheEntry>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    built_generation: u64,
    current_generation: u64,
    state: MinutesState,
}

impl Default for MinutesCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MinutesCache {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    /// Register a transcript generation for a recording without minutes yet.
    pub fn track_transcript(&mut self, recording_id: Uuid, generation: u64) {
        let entry = self
            .records
            .entry(recording_id)
            .or_insert(CacheEntry {
                built_generation: 0,
                current_generation: generation,
                state: MinutesState::Stale,
            });
        entry.current_generation = entry.current_generation.max(generation);
        // A new transcript with no minutes built yet is necessarily stale.
        if entry.built_generation < entry.current_generation {
            entry.state = MinutesState::Stale;
        }
    }

    /// Record that minutes were freshly generated from a given transcript generation.
    pub fn mark_built(&mut self, recording_id: Uuid, generation: u64) -> MinutesState {
        let entry = self.records.entry(recording_id).or_insert(CacheEntry {
            built_generation: 0,
            current_generation: generation,
            state: MinutesState::Fresh,
        });
        entry.built_generation = generation;
        entry.state = if generation >= entry.current_generation {
            MinutesState::Fresh
        } else {
            MinutesState::Stale
        };
        entry.state
    }

    /// A REVISED caption superseded a committed one → bump generation and mark stale.
    pub fn on_caption_applied(
        &mut self,
        recording_id: Uuid,
        status: CaptionStatus,
    ) -> Option<u64> {
        if status != CaptionStatus::Revised {
            return None;
        }
        let entry = self
            .records
            .entry(recording_id)
            .or_insert(CacheEntry {
                built_generation: 0,
                current_generation: 0,
                state: MinutesState::Stale,
            });
        entry.current_generation += 1;
        entry.state = if entry.built_generation >= entry.current_generation {
            MinutesState::Fresh
        } else {
            MinutesState::Stale
        };
        Some(entry.current_generation)
    }

    /// Current staleness of a recording's minutes (defaults to stale if unknown).
    pub fn state(&self, recording_id: &Uuid) -> MinutesState {
        self.records
            .get(recording_id)
            .map(|e| e.state)
            .unwrap_or(MinutesState::Stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_built_from_needs_generation() {
        let rid = Uuid::new_v4();
        let mut cache = MinutesCache::new();
        cache.track_transcript(rid, 1);
        assert_eq!(cache.state(&rid), MinutesState::Stale);
    }

    #[test]
    fn test_mark_built_makes_fresh() {
        let rid = Uuid::new_v4();
        let mut cache = MinutesCache::new();
        cache.track_transcript(rid, 1);
        cache.mark_built(rid, 1);
        assert_eq!(cache.state(&rid), MinutesState::Fresh);
    }

    #[test]
    fn test_revision_invalidates_stale() {
        let rid = Uuid::new_v4();
        let mut cache = MinutesCache::new();
        cache.track_transcript(rid, 1);
        cache.mark_built(rid, 1);
        assert_eq!(cache.state(&rid), MinutesState::Fresh);

        // A REVISED caption bumps the generation and marks stale.
        cache.on_caption_applied(rid, CaptionStatus::Revised);
        assert_eq!(cache.state(&rid), MinutesState::Stale);
    }

    #[test]
    fn test_non_revised_does_not_invalidate() {
        let rid = Uuid::new_v4();
        let mut cache = MinutesCache::new();
        cache.track_transcript(rid, 1);
        cache.mark_built(rid, 1);
        cache.on_caption_applied(rid, CaptionStatus::Committed);
        cache.on_caption_applied(rid, CaptionStatus::Stable);
        assert_eq!(cache.state(&rid), MinutesState::Fresh);
    }
}