//! Error surface for the bulk importers.

use thiserror::Error;

use atomr_ontology_rdf::AdapterError;

/// Errors raised by SKOS / FOAF / schema.org importers.
#[derive(Debug, Error)]
pub enum ImportError {
    /// The serialized input could not be parsed by the underlying
    /// RDF adapter.
    #[error("parse error: {0}")]
    Parse(String),

    /// Triples were parsed but could not be projected into the
    /// canonical LPG model — e.g. a property targeted a class we do
    /// not recognize, or a referenced subject is malformed.
    #[error("mapping error: {0}")]
    Mapping(String),

    /// Propagated [`AdapterError`] from the RDF parsers.
    #[error(transparent)]
    Adapter(#[from] AdapterError),
}
