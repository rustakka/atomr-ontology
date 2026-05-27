//! Ontology learning over extractor outputs.
//!
//! - [`TaxonomyInducer`] proposes `rdfs:subClassOf` axioms.
//! - [`ConceptFormer`] clusters synonymous terms into candidate
//!   `NodeType`s.
//! - [`AxiomMiner`] proposes `Functional`, `InverseOf`, `Domain`,
//!   `Range`, … axioms.
//!
//! All three are LLM-driven in the default configuration, but each
//! exposes a `propose_from_pairs` / `propose_from_groups` static
//! helper that lets callers feed in deterministic hints (e.g. from
//! string distance, vector clustering, schema heuristics) without
//! going through a backend.

#![forbid(unsafe_code)]

pub mod axioms;
pub mod axioms_agentic;
pub mod concepts;
pub mod taxonomy;
pub mod taxonomy_agentic;

pub use axioms::AxiomMiner;
pub use axioms_agentic::AgenticAxiomMiner;
pub use concepts::{ConceptCluster, ConceptFormer};
pub use taxonomy::{SubclassProposal, TaxonomyInducer};
pub use taxonomy_agentic::AgenticTaxonomyInducer;
