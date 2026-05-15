//! Error type shared by the core crate.

use thiserror::Error;

use crate::id::IdError;
use crate::iri::IriError;

/// Errors that arise when manipulating the canonical ontology types.
#[derive(Debug, Error)]
pub enum OntologyError {
    /// The supplied IRI was malformed.
    #[error(transparent)]
    Iri(#[from] IriError),
    /// The supplied id was malformed.
    #[error(transparent)]
    Id(#[from] IdError),
    /// Referenced an undeclared type.
    #[error("unknown type: {0}")]
    UnknownType(String),
    /// A schema constraint was violated.
    #[error("schema violation: {0}")]
    SchemaViolation(String),
    /// A duplicate id was inserted where uniqueness is required.
    #[error("duplicate id: {0}")]
    DuplicateId(String),
    /// A reference target was missing.
    #[error("dangling reference: {0}")]
    Dangling(String),
}
