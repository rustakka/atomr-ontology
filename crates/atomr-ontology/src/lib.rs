//! Umbrella facade for the `atomr-ontology` workspace.
//!
//! Re-exports the Tier 1–3 crates behind cargo features so downstream
//! consumers only pull what they use. The [`prelude`] module
//! re-exports the most common types for ergonomic imports.

#![forbid(unsafe_code)]

pub use atomr_ontology_core as core;

#[cfg(feature = "rdf")]
pub use atomr_ontology_rdf as rdf;

#[cfg(feature = "provenance")]
pub use atomr_ontology_provenance as provenance;

#[cfg(feature = "store")]
pub use atomr_ontology_store as store;

#[cfg(feature = "extract")]
pub use atomr_ontology_extract as extract;

#[cfg(feature = "induce")]
pub use atomr_ontology_induce as induce;

#[cfg(feature = "validate")]
pub use atomr_ontology_validate as validate;

#[cfg(feature = "org")]
pub use atomr_ontology_org as org;

#[cfg(feature = "testkit")]
pub use atomr_ontology_testkit as testkit;

#[cfg(feature = "infer")]
pub use atomr_infer as infer;

#[cfg(feature = "agents")]
pub use atomr_agents as agents;

/// Ergonomic re-exports for everyday use.
pub mod prelude {
    pub use atomr_ontology_core::{
        Axiom, AxiomKind, Cardinality, Datatype, Edge, EdgeId, EdgeType, Iri, Namespace, Node, NodeId,
        NodeType, Ontology, OntologyError, Property, PropertyType, PropertyValue, Record, Schema, Vocabulary,
    };

    #[cfg(feature = "provenance")]
    pub use atomr_ontology_provenance::{
        Activity, AgentKind, AgentRef, ProvAgent, ProvEntity, ProvenanceId, ProvenanceLog,
    };

    #[cfg(feature = "store")]
    pub use atomr_ontology_store::{
        EdgePattern, MatchRow, MemStore, NodePattern, OntologyStore, TraversalPlan, TraversalStep,
    };

    #[cfg(feature = "extract")]
    pub use atomr_ontology_extract::{
        Backend, BackendError, EntityCandidate, EntityResolver, Prompt, RecordExtractor, RelationCandidate,
        RelationExtractor, TermCandidate, TermExtractor,
    };

    #[cfg(feature = "induce")]
    pub use atomr_ontology_induce::{
        AxiomMiner, ConceptCluster, ConceptFormer, SubclassProposal, TaxonomyInducer,
    };

    #[cfg(feature = "validate")]
    pub use atomr_ontology_validate::{validate, Severity, ValidationFinding, ValidationReport};
}

#[cfg(feature = "agents")]
pub mod agents_integration;

#[cfg(feature = "infer")]
pub mod infer_integration;
