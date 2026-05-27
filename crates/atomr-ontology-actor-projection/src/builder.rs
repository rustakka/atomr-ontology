//! Fluent [`ProjectorBuilder`].

use std::sync::Arc;

use atomr_ontology_provenance::AgentRef;
use atomr_ontology_store::r#trait::OntologyStore;

use crate::ingest::IngestMode;
use crate::projection::ProjectionStrategy;
use crate::projector::{default_channel_capacity, default_projector_agent, Projector};
use crate::source::ActorPersistenceSource;
use crate::strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy};
use crate::ProjectorError;

/// Fluent constructor for [`Projector`].
///
/// Required fields: source, at least one ingest mode, projection,
/// store. Other fields default sensibly.
#[derive(Default)]
pub struct ProjectorBuilder {
    source: Option<Arc<dyn ActorPersistenceSource>>,
    ingest: Vec<Arc<dyn IngestMode>>,
    projection: Option<Arc<dyn ProjectionStrategy>>,
    iri: Option<IriMintingStrategy>,
    conflict: Option<ConflictResolution>,
    schema: Option<SchemaStrategy>,
    store: Option<Arc<dyn OntologyStore>>,
    channel_capacity: Option<usize>,
    agent: Option<AgentRef>,
}

impl ProjectorBuilder {
    /// Empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source.
    pub fn source(mut self, source: Arc<dyn ActorPersistenceSource>) -> Self {
        self.source = Some(source);
        self
    }

    /// Append an ingest mode. Modes execute concurrently.
    pub fn with_ingest(mut self, ingest: Arc<dyn IngestMode>) -> Self {
        self.ingest.push(ingest);
        self
    }

    /// Set the projection strategy.
    pub fn projection(mut self, projection: Arc<dyn ProjectionStrategy>) -> Self {
        self.projection = Some(projection);
        self
    }

    /// Set the IRI minting strategy. Defaults to [`IriMintingStrategy::ContentAddressed`].
    pub fn iri(mut self, iri: IriMintingStrategy) -> Self {
        self.iri = Some(iri);
        self
    }

    /// Set the conflict resolution policy. Defaults to [`ConflictResolution::LastWriteWins`].
    pub fn conflict(mut self, conflict: ConflictResolution) -> Self {
        self.conflict = Some(conflict);
        self
    }

    /// Set the schema strategy. Defaults to [`SchemaStrategy::InducedSchema`].
    pub fn schema(mut self, schema: SchemaStrategy) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Set the destination store.
    pub fn store(mut self, store: Arc<dyn OntologyStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Override the batch channel capacity.
    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = Some(capacity);
        self
    }

    /// Override the provenance agent used for activities.
    pub fn agent(mut self, agent: AgentRef) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Validate and finalize.
    pub fn build(self) -> Result<Projector, ProjectorError> {
        let source = self.source.ok_or_else(|| ProjectorError::Configuration("source required".into()))?;
        if self.ingest.is_empty() {
            return Err(ProjectorError::Configuration(
                "at least one ingest mode required".into(),
            ));
        }
        let projection = self
            .projection
            .ok_or_else(|| ProjectorError::Configuration("projection required".into()))?;
        let store = self.store.ok_or_else(|| ProjectorError::Configuration("store required".into()))?;
        Ok(Projector {
            source,
            ingest: self.ingest,
            projection,
            iri: self.iri.unwrap_or(IriMintingStrategy::ContentAddressed),
            conflict: self.conflict.unwrap_or_default(),
            schema: self.schema.unwrap_or_default(),
            store,
            channel_capacity: self.channel_capacity.unwrap_or_else(default_channel_capacity),
            agent: self.agent.unwrap_or_else(default_projector_agent),
        })
    }
}
