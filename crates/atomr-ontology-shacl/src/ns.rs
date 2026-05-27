//! Shared SHACL / XSD namespace constants and helpers.

use atomr_ontology_core::Iri;

/// SHACL namespace IRI.
pub const SH_URI: &str = "http://www.w3.org/ns/shacl#";
/// Conventional prefix for the SHACL namespace.
pub const SH_PREFIX: &str = "sh";

/// XSD namespace IRI.
pub const XSD_URI: &str = "http://www.w3.org/2001/XMLSchema#";
/// Conventional prefix for the XSD namespace.
pub const XSD_PREFIX: &str = "xsd";

/// Build an absolute SHACL IRI from a local name (e.g. `"NodeShape"`).
pub fn shacl_iri(local: &str) -> Iri {
    Iri::from_unchecked(format!("{SH_URI}{local}"))
}

/// Build an absolute XSD IRI from a local name (e.g. `"string"`).
pub fn xsd_iri(local: &str) -> Iri {
    Iri::from_unchecked(format!("{XSD_URI}{local}"))
}
