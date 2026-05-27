//! Ingest modes — drivers that pump data from an
//! [`ActorPersistenceSource`](crate::source::ActorPersistenceSource)
//! into the projector.
//!
//! All four built-in modes implement the [`IngestMode`] trait and can
//! be combined in a single [`Projector`](crate::Projector). The
//! projector multiplexes their outputs into one
//! [`ActorBatch`](crate::batch::ActorBatch) stream.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, watch};

use crate::batch::ActorBatch;
use crate::source::ActorPersistenceSource;
use crate::IngestError;

mod event_stream;
mod polling;
mod push_hook;
mod replay;

pub use event_stream::EventStreamIngest;
pub use polling::PollingIngest;
pub use push_hook::{push_hook_pair, PushHookCheckpointer, PushHookIngest};
pub use replay::ReplayIngest;

/// Built-in ingest mode tags. Custom modes simply implement
/// [`IngestMode`] and may return [`IngestKind::Custom`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IngestKind {
    /// One-shot historical replay.
    Replay,
    /// Periodic polling of the source.
    Polling,
    /// Wraps an upstream [`Checkpointer`](atomr_ontology_persist::Checkpointer)
    /// to fire on each save.
    PushHook,
    /// Subscribes to an event stream supplied externally.
    EventStream,
    /// Anything user-defined.
    Custom(String),
}

/// Runtime context handed to an ingest mode.
///
/// The mode owns the cursor and emits one
/// [`ActorBatch`](crate::batch::ActorBatch) per logical unit of work.
/// It must exit promptly when `shutdown` flips to `true`.
pub struct IngestCtx {
    /// The source the mode reads from.
    pub source: Arc<dyn ActorPersistenceSource>,
    /// Channel into the projector.
    pub sender: mpsc::Sender<ActorBatch>,
    /// Shutdown signal — set to `true` by the projector when stopping.
    pub shutdown: watch::Receiver<bool>,
}

impl IngestCtx {
    /// Send a batch, mapping a closed channel into [`IngestError::ChannelClosed`].
    pub async fn send(&self, batch: ActorBatch) -> Result<(), IngestError> {
        self.sender.send(batch).await.map_err(|_| IngestError::ChannelClosed)
    }

    /// `true` when shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.borrow()
    }
}

/// SPI for an ingest driver.
#[async_trait]
pub trait IngestMode: Send + Sync {
    /// Stable, human-readable label (used in tracing spans).
    fn label(&self) -> &str;
    /// Diagnostic tag for the built-in mode kind.
    fn kind(&self) -> IngestKind;
    /// Drive the source until shutdown or natural exit.
    async fn run(&self, ctx: IngestCtx) -> Result<(), IngestError>;
}
