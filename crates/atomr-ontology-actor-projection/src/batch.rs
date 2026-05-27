//! [`ActorBatch`] — a single unit of work flowing from ingest to
//! projection.

use crate::source::{Cursor, JournalEvent, SerializedState, SupervisionPath};

/// One unit of actor data ready for projection.
///
/// A batch may carry any combination of paths, events, and states.
/// Empty batches are valid (the projector skips them).
#[derive(Clone, Debug, Default)]
pub struct ActorBatch {
    /// Supervision paths discovered in this batch.
    pub paths: Vec<SupervisionPath>,
    /// Journal events in this batch (chronological order).
    pub events: Vec<JournalEvent>,
    /// Latest state blobs for actors in this batch.
    pub states: Vec<SerializedState>,
    /// Cursor at the end of the batch — the position to resume from.
    pub cursor: Cursor,
    /// Optional label, useful for tracing which ingest mode produced this.
    pub origin: Option<String>,
}

impl ActorBatch {
    /// Empty batch at the beginning of time.
    pub fn empty() -> Self {
        Self::default()
    }

    /// `true` when the batch carries no work.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.events.is_empty() && self.states.is_empty()
    }

    /// Attach an origin label.
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// Replace the cursor.
    pub fn with_cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = cursor;
        self
    }
}
