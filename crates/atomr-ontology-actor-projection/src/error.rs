//! Crate-wide error types.

use thiserror::Error;

use atomr_ontology_core::IriError;
use atomr_ontology_store::r#trait::StoreError;

/// Errors raised by [`ActorPersistenceSource`](crate::source::ActorPersistenceSource).
#[derive(Debug, Error)]
pub enum SourceError {
    /// Underlying I/O or checkpointer failure.
    #[error("source io: {0}")]
    Io(String),
    /// A record was malformed (missing required fields, invalid path, etc.).
    #[error("malformed source record: {0}")]
    Malformed(String),
    /// Wrapped IRI validation failure.
    #[error(transparent)]
    Iri(#[from] IriError),
    /// Anything else a custom source wants to report.
    #[error("source error: {0}")]
    Other(String),
}

/// Errors raised by [`IngestMode`](crate::ingest::IngestMode) drivers.
#[derive(Debug, Error)]
pub enum IngestError {
    /// Underlying source failed.
    #[error(transparent)]
    Source(#[from] SourceError),
    /// The projector's batch channel closed before ingest finished.
    #[error("projector channel closed")]
    ChannelClosed,
    /// The ingest mode encountered a configuration problem at runtime.
    #[error("ingest configuration: {0}")]
    Configuration(String),
    /// Anything else a custom mode wants to report.
    #[error("ingest error: {0}")]
    Other(String),
}

/// Errors raised by [`ProjectionStrategy`](crate::projection::ProjectionStrategy).
#[derive(Debug, Error)]
pub enum ProjectionError {
    /// Wrapped IRI validation failure.
    #[error(transparent)]
    Iri(#[from] IriError),
    /// The batch violated the active fixed schema.
    #[error("schema violation: {0}")]
    SchemaViolation(String),
    /// The batch could not be projected.
    #[error("projection error: {0}")]
    Other(String),
}

/// Errors raised by [`Projector`](crate::Projector).
#[derive(Debug, Error)]
pub enum ProjectorError {
    /// A source returned an error during the run.
    #[error(transparent)]
    Source(#[from] SourceError),
    /// An ingest mode failed.
    #[error(transparent)]
    Ingest(#[from] IngestError),
    /// A projection step failed.
    #[error(transparent)]
    Projection(#[from] ProjectionError),
    /// The destination store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Builder validation failure.
    #[error("invalid projector configuration: {0}")]
    Configuration(String),
}
