//! [`Projector`] — composes a source, one or more ingest modes, and a
//! projection strategy; commits deltas to an
//! [`OntologyStore`](atomr_ontology_store::r#trait::OntologyStore).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use atomr_ontology_core::Node;
use atomr_ontology_provenance::{Activity, AgentRef};
use atomr_ontology_store::r#trait::{OntologyDelta, OntologyStore};

use crate::batch::ActorBatch;
use crate::ingest::{IngestCtx, IngestMode};
use crate::projection::{ProjectionCtx, ProjectionStrategy};
use crate::source::ActorPersistenceSource;
use crate::strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy};
use crate::{IngestError, ProjectorError};

const DEFAULT_BATCH_CHANNEL_CAPACITY: usize = 64;

/// Summary returned by [`Projector::run`].
#[derive(Clone, Debug, Default)]
pub struct ProjectorReport {
    /// Total number of batches projected and committed.
    pub batches: u64,
    /// Total number of nodes written (post-conflict-resolution).
    pub nodes_written: u64,
    /// Total number of edges written.
    pub edges_written: u64,
    /// Number of provenance activities recorded.
    pub activities_recorded: u64,
}

/// Composes a source, ingest modes, projection strategy, and a destination
/// store. Spawned tasks own the source for their lifetime.
pub struct Projector {
    pub(crate) source: Arc<dyn ActorPersistenceSource>,
    pub(crate) ingest: Vec<Arc<dyn IngestMode>>,
    pub(crate) projection: Arc<dyn ProjectionStrategy>,
    pub(crate) iri: IriMintingStrategy,
    pub(crate) conflict: ConflictResolution,
    pub(crate) schema: SchemaStrategy,
    pub(crate) store: Arc<dyn OntologyStore>,
    pub(crate) channel_capacity: usize,
    pub(crate) agent: AgentRef,
}

impl Projector {
    /// Run the projector until all ingest modes finish naturally.
    ///
    /// Returns a [`ProjectorReport`] summarizing the work. The default
    /// suits one-shot replay; long-running modes (Polling, EventStream)
    /// should use [`Projector::run_until_shutdown`].
    pub async fn run(self) -> Result<ProjectorReport, ProjectorError> {
        self.run_until(None).await
    }

    /// Run for at most `timeout`. Useful in tests and demos.
    pub async fn run_with_timeout(self, timeout: Duration) -> Result<ProjectorReport, ProjectorError> {
        self.run_until(Some(timeout)).await
    }

    /// Run with an external shutdown channel. The projector returns
    /// when *either* every ingest mode has exited or the shutdown
    /// receiver flips to `true`.
    pub async fn run_until_shutdown(
        self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<ProjectorReport, ProjectorError> {
        let (tx, rx) = watch::channel(false);
        let projector_handle = tokio::spawn(self.run_inner(rx));
        let _ = shutdown.changed().await;
        let _ = tx.send(true);
        match projector_handle.await {
            Ok(res) => res,
            Err(e) => Err(ProjectorError::Configuration(format!("projector task panicked: {e}"))),
        }
    }

    async fn run_until(self, timeout: Option<Duration>) -> Result<ProjectorReport, ProjectorError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let projector_handle = tokio::spawn(self.run_inner(shutdown_rx));
        let result = match timeout {
            Some(dur) => match tokio::time::timeout(dur, projector_handle).await {
                Ok(join_result) => join_result,
                Err(_) => {
                    let _ = shutdown_tx.send(true);
                    // Best-effort: wait briefly for the projector to
                    // wind down so its report is returned.
                    return Err(ProjectorError::Configuration(
                        "projector timed out before all ingest modes exited".into(),
                    ));
                }
            },
            None => projector_handle.await,
        };
        match result {
            Ok(res) => res,
            Err(e) => Err(ProjectorError::Configuration(format!("projector task panicked: {e}"))),
        }
    }

    async fn run_inner(
        self,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<ProjectorReport, ProjectorError> {
        let (batch_tx, mut batch_rx) = mpsc::channel::<ActorBatch>(self.channel_capacity);
        let projection = self.projection.clone();
        let source_label = self.source.label().to_owned();
        let store = self.store.clone();
        let iri = self.iri.clone();
        let conflict = self.conflict;
        let schema = self.schema.clone();
        let agent = self.agent.clone();

        // Launch ingest tasks.
        let mut tasks: JoinSet<Result<(), IngestError>> = JoinSet::new();
        for mode in self.ingest {
            let source = self.source.clone();
            let sender = batch_tx.clone();
            let shutdown = shutdown_rx.clone();
            tasks.spawn(async move {
                let ctx = IngestCtx { source, sender, shutdown };
                mode.run(ctx).await
            });
        }
        drop(batch_tx); // The projector itself doesn't produce batches.

        let mut report = ProjectorReport::default();
        // Drain batches; commit each into the store.
        while let Some(batch) = batch_rx.recv().await {
            if batch.is_empty() {
                continue;
            }
            let ctx = ProjectionCtx {
                iri: iri.clone(),
                conflict,
                schema: schema.clone(),
                source_label: source_label.clone(),
            };
            let raw_delta = projection.project(&batch, &ctx).await?;
            if schema.is_strict() {
                enforce_strict_schema(&raw_delta, schema.baseline())?;
            }
            let delta = reconcile_delta(raw_delta, &*store, conflict).await?;
            if delta.is_empty() {
                continue;
            }
            let activity = Activity::started(format!(
                "actor-projection:{}:{}",
                projection.label(),
                batch.origin.as_deref().unwrap_or("anon")
            ))
            .by(agent.clone())
            .with_attribute(
                "projection",
                serde_json::Value::String(projection.label().to_owned()),
            )
            .with_attribute(
                "source",
                serde_json::Value::String(source_label.clone()),
            )
            .with_attribute(
                "cursor_version",
                serde_json::Value::from(batch.cursor.version),
            )
            .finish();

            let nodes = delta.nodes.len() as u64;
            let edges = delta.edges.len() as u64;
            store.commit_with_provenance(delta, activity).await?;
            report.batches += 1;
            report.nodes_written += nodes;
            report.edges_written += edges;
            report.activities_recorded += 1;
        }

        // Drain ingest task results — propagate the first failure.
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(ProjectorError::Ingest(e)),
                Err(e) => {
                    return Err(ProjectorError::Configuration(format!(
                        "ingest task panicked: {e}"
                    )));
                }
            }
        }

        Ok(report)
    }
}

fn enforce_strict_schema(
    delta: &OntologyDelta,
    schema: Option<&atomr_ontology_core::Schema>,
) -> Result<(), ProjectorError> {
    let Some(schema) = schema else { return Ok(()) };
    for node in &delta.nodes {
        for ty in &node.types {
            if schema.node_type(ty).is_none() {
                return Err(ProjectorError::Projection(
                    crate::ProjectionError::SchemaViolation(format!(
                        "node type {ty:?} not declared in fixed schema"
                    )),
                ));
            }
        }
    }
    for edge in &delta.edges {
        if schema.edge_type(&edge.label).is_none() {
            return Err(ProjectorError::Projection(
                crate::ProjectionError::SchemaViolation(format!(
                    "edge label {:?} not declared in fixed schema",
                    edge.label
                )),
            ));
        }
    }
    Ok(())
}

async fn reconcile_delta(
    raw: OntologyDelta,
    store: &dyn OntologyStore,
    conflict: ConflictResolution,
) -> Result<OntologyDelta, ProjectorError> {
    // For LastWriteWins we can pass through unchanged.
    if matches!(conflict, ConflictResolution::LastWriteWins) {
        return Ok(raw);
    }
    let mut out = OntologyDelta::new();
    for node in raw.nodes {
        let existing = store.node(&node.id).await?;
        if let Some(reconciled) = conflict.reconcile(existing.as_ref(), node) {
            out.nodes.push(reconciled);
        }
    }
    // Edges are deduplicated by id at the store layer; pass through.
    out.edges = raw.edges;
    out.axioms = raw.axioms;
    Ok(out)
}

/// Convenience builder for the projector's [`AgentRef`].
pub(crate) fn default_projector_agent() -> AgentRef {
    AgentRef::software("agent://atomr-ontology-actor-projection", "ActorProjector")
}

/// Default channel capacity used when the builder does not override.
pub const fn default_channel_capacity() -> usize {
    DEFAULT_BATCH_CHANNEL_CAPACITY
}

// Unused private helper signature to keep the export stable.
#[allow(dead_code)]
fn _node_keepalive(_: &Node) {}
