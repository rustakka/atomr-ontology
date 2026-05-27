//! Projection strategies — turn an
//! [`ActorBatch`](crate::batch::ActorBatch) into an
//! [`OntologyDelta`](atomr_ontology_store::r#trait::OntologyDelta).

use async_trait::async_trait;

use atomr_ontology_store::r#trait::OntologyDelta;

use crate::batch::ActorBatch;
use crate::strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy};
use crate::ProjectionError;

mod event_stream;
mod flat;
mod hierarchical;
mod snapshot_diff;

pub use event_stream::EventStreamProjection;
pub use flat::FlatProjection;
pub use hierarchical::HierarchicalProjection;
pub use snapshot_diff::SnapshotDiffProjection;

/// Built-in projection-shape tags.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectionKind {
    /// Workflow → Run → Step → Actor hierarchy.
    Hierarchical,
    /// One node per journal event with a chronological successor edge.
    EventStream,
    /// Only the diff from the previously-projected batch.
    SnapshotDiff,
    /// One denormalized node per (workflow, run) carrying all step state.
    Flat,
    /// Anything user-defined.
    Custom(String),
}

/// Inputs to a projection step that are global to the projector.
#[derive(Clone, Debug)]
pub struct ProjectionCtx {
    /// IRI minting policy.
    pub iri: IriMintingStrategy,
    /// Conflict resolution policy.
    pub conflict: ConflictResolution,
    /// Schema strategy.
    pub schema: SchemaStrategy,
    /// Label of the source the batch came from.
    pub source_label: String,
}

/// SPI for a projection strategy.
#[async_trait]
pub trait ProjectionStrategy: Send + Sync {
    /// Stable, human-readable label.
    fn label(&self) -> &str;
    /// Built-in tag.
    fn kind(&self) -> ProjectionKind;
    /// Turn `batch` into a delta. Implementations should NOT consult
    /// the destination store; conflict resolution against existing
    /// nodes is the projector's job.
    async fn project(
        &self,
        batch: &ActorBatch,
        ctx: &ProjectionCtx,
    ) -> Result<OntologyDelta, ProjectionError>;
}
