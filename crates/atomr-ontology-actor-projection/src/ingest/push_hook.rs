//! Push-hook ingest — wraps an upstream
//! [`Checkpointer`](atomr_ontology_persist::Checkpointer) so that each
//! successful `save` triggers a projection.
//!
//! Composition: pair [`PushHookCheckpointer`] with [`PushHookIngest`]
//! using [`push_hook_pair`]. The checkpointer publishes a signal on
//! every save; the ingest mode subscribes and re-reads the source to
//! emit a batch.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use atomr_ontology_persist::{Checkpointer, CheckpointerError, Snapshot};

use crate::batch::ActorBatch;
use crate::source::Cursor;
use crate::IngestError;

use super::{IngestCtx, IngestKind, IngestMode};

const SIGNAL_CHANNEL_CAPACITY: usize = 64;

/// Create a paired ([`PushHookCheckpointer`], [`PushHookIngest`]).
///
/// The checkpointer delegates `save`/`load` to `inner` and publishes a
/// signal on every successful `save`. The ingest mode listens for the
/// signal and re-reads its source.
pub fn push_hook_pair(
    inner: Arc<dyn Checkpointer>,
) -> (PushHookCheckpointer, PushHookIngest) {
    let (tx, _) = broadcast::channel(SIGNAL_CHANNEL_CAPACITY);
    let cp = PushHookCheckpointer { inner, signal: tx.clone() };
    let ingest = PushHookIngest { label: "push-hook".into(), signal: Mutex::new(Some(tx)) };
    (cp, ingest)
}

/// [`Checkpointer`] wrapper that broadcasts a signal on every save.
pub struct PushHookCheckpointer {
    inner: Arc<dyn Checkpointer>,
    signal: broadcast::Sender<()>,
}

impl PushHookCheckpointer {
    /// Borrow the inner checkpointer.
    pub fn inner(&self) -> &Arc<dyn Checkpointer> {
        &self.inner
    }
}

#[async_trait]
impl Checkpointer for PushHookCheckpointer {
    async fn save(&self, snapshot: Snapshot) -> Result<(), CheckpointerError> {
        self.inner.save(snapshot).await?;
        // Ignore the error; an empty receiver list is harmless.
        let _ = self.signal.send(());
        Ok(())
    }

    async fn load(&self) -> Result<Option<Snapshot>, CheckpointerError> {
        self.inner.load().await
    }

    fn label(&self) -> &str {
        self.inner.label()
    }
}

/// Ingest mode that wakes up on every upstream save.
pub struct PushHookIngest {
    label: String,
    // `Mutex<Option<...>>` lets us take ownership of the sender at
    // run-time so we can subscribe; the sender itself is `Clone`.
    signal: Mutex<Option<broadcast::Sender<()>>>,
}

impl PushHookIngest {
    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl IngestMode for PushHookIngest {
    fn label(&self) -> &str {
        &self.label
    }

    fn kind(&self) -> IngestKind {
        IngestKind::PushHook
    }

    async fn run(&self, mut ctx: IngestCtx) -> Result<(), IngestError> {
        let sender = self
            .signal
            .lock()
            .clone()
            .ok_or_else(|| IngestError::Configuration("push-hook signal unavailable".into()))?;
        let mut rx = sender.subscribe();
        let mut cursor = Cursor::beginning();
        loop {
            tokio::select! {
                _ = ctx.shutdown.changed() => {
                    if ctx.is_shutdown() {
                        return Ok(());
                    }
                }
                msg = rx.recv() => {
                    match msg {
                        Ok(()) => {
                            let events = ctx.source.journal(&cursor).await?;
                            let paths = ctx.source.paths().await?;
                            let mut actor_ids = std::collections::BTreeSet::new();
                            for e in &events { actor_ids.insert(e.actor.clone()); }
                            let mut states = Vec::new();
                            for id in actor_ids {
                                if let Some(s) = ctx.source.state(&id).await? {
                                    states.push(s);
                                }
                            }
                            let new_cursor = ctx.source.cursor().await?;
                            cursor = new_cursor.clone();
                            let batch = ActorBatch {
                                paths,
                                events,
                                states,
                                cursor: new_cursor,
                                origin: Some("push-hook".into()),
                            };
                            if !batch.is_empty() {
                                ctx.send(batch).await?;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Resync from the latest cursor; data is durable in the source.
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}
