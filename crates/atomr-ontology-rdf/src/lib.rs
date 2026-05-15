//! RDF / OWL adapter for the canonical labeled property graph.
//!
//! The LPG model lives in [`atomr-ontology-core`](https://docs.rs/atomr-ontology-core).
//! This crate ships:
//!
//! - The RDF/OWL vocabulary types: [`Class`], [`Individual`],
//!   [`ObjectProperty`], [`DataProperty`], [`Triple`], [`Quad`].
//! - An [`adapter`] module that projects an
//!   [`Ontology`](atomr_ontology_core::Ontology) into a stream of
//!   triples and back.
//! - Optional [`turtle`], [`ntriples`], and [`jsonld`] writers gated
//!   behind cargo features.
//!
//! The RDF projection is **lossy by design**: PropertyValue::Json is
//! stringified, content-addressed [`NodeId`](atomr_ontology_core::NodeId)s become blank nodes when
//! no IRI is set, and inverse edges are not duplicated.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod owl;
pub mod triple;

#[cfg(feature = "jsonld")]
pub mod jsonld;
#[cfg(feature = "ntriples")]
pub mod ntriples;
#[cfg(feature = "turtle")]
pub mod turtle;

pub use adapter::{from_rdf, to_rdf, AdapterError};
pub use owl::{Class, DataProperty, Individual, ObjectProperty};
pub use triple::{Object, Quad, Subject, Triple};
