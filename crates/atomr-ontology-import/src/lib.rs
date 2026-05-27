//! Bulk importers for external ontology standards (SKOS, FOAF,
//! schema.org JSON-LD).
//!
//! Each importer parses a serialized document, projects its triples
//! into the canonical [`Ontology`](atomr_ontology_core::Ontology)
//! model, and returns the resulting graph alongside an
//! [`Activity`](atomr_ontology_provenance::Activity) record so the
//! import is traceable as PROV-O provenance.
//!
//! - [`import_skos`] consumes SKOS Turtle.
//! - [`import_foaf`] consumes FOAF Turtle.
//! - [`import_schema_org`] consumes schema.org JSON-LD.
//!
//! All three share an [`ImportError`] surface and the same vocabulary
//! mapping strategy: recognized classes become
//! [`NodeType`](atomr_ontology_core::schema::NodeType)s, recognized
//! object properties become
//! [`EdgeType`](atomr_ontology_core::schema::EdgeType)s, and
//! recognized data properties attach as
//! [`PropertyType`](atomr_ontology_core::schema::PropertyType)s on
//! the relevant node types.

#![forbid(unsafe_code)]

pub mod error;
pub mod foaf;
pub mod schema_org;
pub mod skos;

mod mapping;

pub use error::ImportError;
pub use foaf::import_foaf;
pub use schema_org::import_schema_org;
pub use skos::import_skos;
