//! Polling ingest — periodically asks the source for new journal
//! events and emits the diff.

use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;

use crate::batch::ActorBatch;
use crate::source::Cursor;
use crate::IngestError;

use super::{IngestCtx, IngestKind, IngestMode};

/// Polls the source on a fixed interval. Each tick fetches journal
/// events strictly after the last seen cursor; if any arrive, they
/// are emitted as a batch along with the latest known paths and the
/// states of any newly-seen actors.
pub struct PollingIngest {
    label: String,
    interval: Duration,
    cursor: Mutex<Cursor>,
    include_paths: bool,
}

impl PollingIngest {
    /// Poll every `interval`, starting from the beginning of the
    /// journal. Path discovery is included on every tick.
    pub fn every(interval: Duration) -> Self {
        Self {
            label: "polling".into(),
            interval,
            cursor: Mutex::new(Cursor::beginning()),
            include_paths: true,
        }
    }

    /// Resume polling from a known cursor.
    pub fn resume(interval: Duration, cursor: Cursor) -> Self {
        Self {
            label: "polling".into(),
            interval,
            cursor: Mutex::new(cursor),
            include_paths: true,
        }
    }

    /// Skip path discovery (events-only).
    pub fn events_only(mut self) -> Self {
        self.include_paths = false;
        self
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl IngestMode for PollingIngest {
    fn label(&self) -> &str {
        &self.label
    }

    fn kind(&self) -> IngestKind {
        IngestKind::Polling
    }

    async fn run(&self, mut ctx: IngestCtx) -> Result<(), IngestError> {
        let mut ticker = tokio::time::interval(self.interval);
        // First tick is immediate; align to the interval.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ctx.shutdown.changed() => {
                    if ctx.is_shutdown() {
                        return Ok(());
                    }
                }
                _ = ticker.tick() => {
                    let since = self.cursor.lock().clone();
                    let events = ctx.source.journal(&since).await?;
                    let paths = if self.include_paths {
                        ctx.source.paths().await?
                    } else {
                        Vec::new()
                    };
                    if events.is_empty() && paths.is_empty() {
                        continue;
                    }

                    let mut actor_ids = std::collections::BTreeSet::new();
                    for e in &events {
                        actor_ids.insert(e.actor.clone());
                    }
                    let mut states = Vec::new();
                    for id in actor_ids {
                        if let Some(s) = ctx.source.state(&id).await? {
                            states.push(s);
                        }
                    }
                    let cursor = ctx.source.cursor().await?;
                    *self.cursor.lock() = cursor.clone();

                    let batch = ActorBatch {
                        paths,
                        events,
                        states,
                        cursor,
                        origin: Some("polling".into()),
                    };
                    ctx.send(batch).await?;
                }
            }
        }
    }
}
