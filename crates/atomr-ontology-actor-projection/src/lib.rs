//! Project actor-system persistence data into an
//! [`OntologyStore`](atomr_ontology_store::r#trait::OntologyStore).
//!
//! This crate sits in the workspace's tier-2 services layer and turns
//! **actor-system persistence data** — supervision-tree paths, journal
//! events, and serialized state blobs — into ontology graphs.
//!
//! # Concepts
//!
//! - [`ActorPersistenceSource`] (in [`source`]) is the SPI for reading
//!   actor data. The crate ships with [`source::InMemoryActorPersistenceSource`]
//!   for tests and [`source::SnapshotActorPersistenceSource`] (feature
//!   `snapshot-source`) that wraps any [`Checkpointer`](atomr_ontology_persist::Checkpointer)
//!   whose snapshots already carry actor records.
//! - [`IngestMode`](ingest::IngestMode) drives the source. Built-ins:
//!   [`ReplayIngest`](ingest::ReplayIngest), [`PollingIngest`](ingest::PollingIngest),
//!   [`PushHookIngest`](ingest::PushHookIngest),
//!   [`EventStreamIngest`](ingest::EventStreamIngest). Multiple modes
//!   may run together against the same source.
//! - [`ProjectionStrategy`](projection::ProjectionStrategy) shapes the
//!   resulting ontology. Built-ins: [`HierarchicalProjection`](projection::HierarchicalProjection),
//!   [`EventStreamProjection`](projection::EventStreamProjection),
//!   [`SnapshotDiffProjection`](projection::SnapshotDiffProjection),
//!   [`FlatProjection`](projection::FlatProjection).
//! - [`IriMintingStrategy`](strategy::IriMintingStrategy),
//!   [`ConflictResolution`](strategy::ConflictResolution), and
//!   [`SchemaStrategy`](strategy::SchemaStrategy) are pure-functional
//!   policy enums applied during projection.
//! - [`Projector`] composes everything; [`ProjectorBuilder`] is the
//!   fluent constructor.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use atomr_ontology_actor_projection::{
//!     ingest::ReplayIngest,
//!     projection::HierarchicalProjection,
//!     source::InMemoryActorPersistenceSource,
//!     strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy},
//!     vocab, ProjectorBuilder,
//! };
//! use atomr_ontology_core::Iri;
//! use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let store = Arc::new(PersistentStore::new(MemCheckpointer::new()).await?);
//! let source = Arc::new(InMemoryActorPersistenceSource::new("demo"));
//! let projector = ProjectorBuilder::new()
//!     .source(source)
//!     .with_ingest(Arc::new(ReplayIngest::once()))
//!     .projection(Arc::new(HierarchicalProjection::default()))
//!     .iri(IriMintingStrategy::PathBased { base: Iri::new("https://atomr.dev/actor/")? })
//!     .conflict(ConflictResolution::Merge)
//!     .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
//!     .store(store)
//!     .build()?;
//! projector.run().await?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod batch;
pub mod ingest;
pub mod projection;
pub mod projector;
pub mod source;
pub mod strategy;
pub mod vocab;

mod builder;
mod error;

pub use builder::ProjectorBuilder;
pub use error::{IngestError, ProjectionError, ProjectorError, SourceError};
pub use projector::{Projector, ProjectorReport};
