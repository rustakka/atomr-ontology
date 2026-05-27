//! One-shot historical replay ingest.

use async_trait::async_trait;

use crate::batch::ActorBatch;
use crate::source::Cursor;
use crate::IngestError;

use super::{IngestCtx, IngestKind, IngestMode};

/// Reads the source once: every known path, every journal event since
/// `since`, and the state of every actor referenced by either. Emits a
/// single batch and exits.
#[derive(Clone, Debug, Default)]
pub struct ReplayIngest {
    label: String,
    since: Cursor,
}

impl ReplayIngest {
    /// Replay the entire history (`Cursor::beginning()`).
    pub fn once() -> Self {
        Self { label: "replay".into(), since: Cursor::beginning() }
    }

    /// Replay only events after the given cursor.
    pub fn since(cursor: Cursor) -> Self {
        Self { label: "replay".into(), since: cursor }
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl IngestMode for ReplayIngest {
    fn label(&self) -> &str {
        &self.label
    }

    fn kind(&self) -> IngestKind {
        IngestKind::Replay
    }

    async fn run(&self, ctx: IngestCtx) -> Result<(), IngestError> {
        let paths = ctx.source.paths().await?;
        let events = ctx.source.journal(&self.since).await?;

        // Pull state for every actor we touched, deduplicated.
        let mut actor_ids = std::collections::BTreeSet::new();
        for p in &paths {
            actor_ids.insert(p.actor.clone());
        }
        for e in &events {
            actor_ids.insert(e.actor.clone());
        }
        let mut states = Vec::with_capacity(actor_ids.len());
        for id in actor_ids {
            if let Some(s) = ctx.source.state(&id).await? {
                states.push(s);
            }
        }

        let cursor = ctx.source.cursor().await?;
        let batch = ActorBatch {
            paths,
            events,
            states,
            cursor,
            origin: Some("replay".into()),
        };
        if !batch.is_empty() {
            ctx.send(batch).await?;
        }
        Ok(())
    }
}
