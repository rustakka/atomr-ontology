//! Canonical labeled property graph types for the atomr-ontology workspace.
//!
//! This crate is the Tier 1 data layer: pure data, no I/O, no actors,
//! no LLM dependencies. Every other crate in the workspace consumes
//! these types as its lingua franca.
//!
//! The canonical model is a [labeled property graph][lpg] (LPG). A
//! non-canonical RDF/OWL projection lives in
//! [`atomr-ontology-rdf`](https://docs.rs/atomr-ontology-rdf).
//!
//! [lpg]: https://neo4j.com/developer/graph-database/

#![forbid(unsafe_code)]

pub mod axiom;
pub mod edge;
pub mod error;
pub mod id;
pub mod iri;
pub mod namespace;
pub mod node;
pub mod ontology;
pub mod record;
pub mod schema;

pub use axiom::{Axiom, AxiomId, AxiomKind};
pub use edge::Edge;
pub use error::OntologyError;
pub use id::{EdgeId, IdError, NodeId, ProvenanceId, RecordId};
pub use iri::{Iri, IriError};
pub use namespace::{Namespace, Vocabulary};
pub use node::{Node, Property, PropertyValue};
pub use ontology::Ontology;
pub use record::Record;
pub use schema::{Cardinality, Datatype, EdgeType, NodeType, PropertyType, Schema};
