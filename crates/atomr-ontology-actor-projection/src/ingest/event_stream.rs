//! Event-stream ingest — subscribe to a `broadcast::Receiver<JournalEvent>`
//! the caller supplies.

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::batch::ActorBatch;
use crate::source::{Cursor, JournalEvent};
use crate::IngestError;

use super::{IngestCtx, IngestKind, IngestMode};

/// Ingest mode that subscribes to an externally-provided
/// [`broadcast::Receiver<JournalEvent>`].
///
/// Each event becomes a single-event batch. The source is consulted
/// only when a state pull is required (the latest blob for the actor
/// that emitted the event).
///
/// Construct with a freshly-subscribed receiver:
///
/// ```ignore
/// use tokio::sync::broadcast;
/// use atomr_ontology_actor_projection::ingest::EventStreamIngest;
/// use atomr_ontology_actor_projection::source::JournalEvent;
///
/// let (tx, _rx) = broadcast::channel::<JournalEvent>(64);
/// let ingest = EventStreamIngest::subscribe(&tx);
/// ```
pub struct EventStreamIngest {
    label: String,
    rx: Mutex<Option<broadcast::Receiver<JournalEvent>>>,
}

impl EventStreamIngest {
    /// Wrap a freshly-created receiver.
    pub fn from_receiver(rx: broadcast::Receiver<JournalEvent>) -> Self {
        Self { label: "event-stream".into(), rx: Mutex::new(Some(rx)) }
    }

    /// Convenience: subscribe to a sender now.
    pub fn subscribe(sender: &broadcast::Sender<JournalEvent>) -> Self {
        Self::from_receiver(sender.subscribe())
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl IngestMode for EventStreamIngest {
    fn label(&self) -> &str {
        &self.label
    }

    fn kind(&self) -> IngestKind {
        IngestKind::EventStream
    }

    async fn run(&self, mut ctx: IngestCtx) -> Result<(), IngestError> {
        let mut rx = self
            .rx
            .lock()
            .take()
            .ok_or_else(|| IngestError::Configuration("event-stream receiver already taken".into()))?;

        loop {
            tokio::select! {
                _ = ctx.shutdown.changed() => {
                    if ctx.is_shutdown() {
                        return Ok(());
                    }
                }
                msg = rx.recv() => {
                    match msg {
                        Ok(event) => {
                            let mut states = Vec::new();
                            if let Some(state) = ctx.source.state(&event.actor).await? {
                                states.push(state);
                            }
                            let cursor = event.cursor.clone();
                            let batch = ActorBatch {
                                paths: event.path.iter().cloned().collect(),
                                events: vec![event],
                                states,
                                cursor,
                                origin: Some("event-stream".into()),
                            };
                            ctx.send(batch).await?;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Skip lagged messages; durable source can replay.
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Best-effort: flush a synthetic empty batch carrying the
                            // latest cursor so downstream knows where we stopped.
                            let cursor = ctx.source.cursor().await.unwrap_or(Cursor::beginning());
                            let batch = ActorBatch { cursor, ..ActorBatch::empty() }
                                .with_origin("event-stream");
                            if !batch.is_empty() {
                                ctx.send(batch).await?;
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}
