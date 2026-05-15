//! Storage layer for an [`Ontology`](atomr_ontology_core::Ontology).
//!
//! The trait is [`OntologyStore`]; the bundled implementation is
//! [`MemStore`] (in-memory, parking-lot guarded, async surface for
//! interface uniformity).
//!
//! Query primitives are expressed as Rust builder types
//! ([`pattern::NodePattern`], [`pattern::TraversalPlan`]) rather than
//! a string DSL. This keeps the API typed at the call site, lets
//! downstream callers compose patterns programmatically, and side-
//! steps the openCypher / SPARQL parsing burden in v0.1. The shape
//! of the patterns is deliberately close to openCypher and SPARQL
//! BGPs so a string adapter can be added later without redesigning
//! the store contract.

#![forbid(unsafe_code)]

pub mod mem;
pub mod pattern;
pub mod r#trait;

pub use mem::MemStore;
pub use pattern::{EdgePattern, MatchRow, NodePattern, TraversalPlan, TraversalStep};
pub use r#trait::{OntologyDelta, OntologyStore, StoreDiff, StoreError};
