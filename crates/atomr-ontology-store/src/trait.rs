//! `OntologyStore` async trait — the repository surface for ontologies.

use async_trait::async_trait;
use thiserror::Error;

use atomr_ontology_core::{Axiom, Edge, EdgeId, Node, NodeId, Ontology, OntologyError};
use atomr_ontology_provenance::{Activity, ProvenanceId, ProvenanceLog};

use crate::pattern::{MatchRow, NodePattern, TraversalPlan};

/// Errors raised by store implementations.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Wraps an `OntologyError` from the core crate.
    #[error(transparent)]
    Ontology(#[from] OntologyError),
    /// Wrapped I/O error.
    #[error("io error: {0}")]
    Io(String),
    /// A reference was missing.
    #[error("not found: {0}")]
    NotFound(String),
}

/// Repository surface for ontologies.
///
/// The trait is async-safe (`Send + Sync`) so it can be wrapped in
/// `Arc<dyn OntologyStore>` and shared between agents.
#[async_trait]
pub trait OntologyStore: Send + Sync {
    /// Insert or replace a node.
    async fn upsert_node(&self, node: Node) -> Result<NodeId, StoreError>;

    /// Insert or replace an edge.
    async fn upsert_edge(&self, edge: Edge) -> Result<EdgeId, StoreError>;

    /// Insert or replace an axiom.
    async fn upsert_axiom(&self, axiom: Axiom) -> Result<(), StoreError>;

    /// Fetch a node by id.
    async fn node(&self, id: &NodeId) -> Result<Option<Node>, StoreError>;

    /// Fetch an edge by id.
    async fn edge(&self, id: &EdgeId) -> Result<Option<Edge>, StoreError>;

    /// Apply a node pattern, returning bindings.
    async fn match_pattern(&self, pattern: &NodePattern) -> Result<Vec<MatchRow>, StoreError>;

    /// Execute a multi-step traversal plan and return bindings.
    async fn traverse(&self, plan: &TraversalPlan) -> Result<Vec<MatchRow>, StoreError>;

    /// Snapshot the current state as an [`Ontology`].
    async fn snapshot(&self) -> Result<Ontology, StoreError>;

    /// Compute a coarse diff between this store and another `Ontology`.
    async fn diff(&self, other: &Ontology) -> Result<StoreDiff, StoreError>;

    /// Commit a delta with provenance attached.
    async fn commit_with_provenance(
        &self,
        delta: OntologyDelta,
        activity: Activity,
    ) -> Result<ProvenanceId, StoreError>;

    /// Read the provenance log.
    async fn provenance(&self) -> Result<ProvenanceLog, StoreError>;
}

/// A delta to be applied atomically with provenance.
#[derive(Clone, Debug, Default)]
pub struct OntologyDelta {
    /// Nodes to upsert.
    pub nodes: Vec<Node>,
    /// Edges to upsert.
    pub edges: Vec<Edge>,
    /// Axioms to upsert.
    pub axioms: Vec<Axiom>,
}

impl OntologyDelta {
    /// Empty delta.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a node.
    pub fn with_node(mut self, node: Node) -> Self {
        self.nodes.push(node);
        self
    }

    /// Append an edge.
    pub fn with_edge(mut self, edge: Edge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Append an axiom.
    pub fn with_axiom(mut self, axiom: Axiom) -> Self {
        self.axioms.push(axiom);
        self
    }

    /// `true` when the delta has no contents.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty() && self.axioms.is_empty()
    }
}

/// Coarse diff of two ontology snapshots.
#[derive(Clone, Debug, Default)]
pub struct StoreDiff {
    /// Nodes present in self but not in other.
    pub added_nodes: Vec<NodeId>,
    /// Nodes present in other but not in self.
    pub removed_nodes: Vec<NodeId>,
    /// Edges present in self but not in other.
    pub added_edges: Vec<EdgeId>,
    /// Edges present in other but not in self.
    pub removed_edges: Vec<EdgeId>,
}
