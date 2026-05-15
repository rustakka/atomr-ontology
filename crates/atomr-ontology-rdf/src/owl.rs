//! OWL vocabulary surfaces — `Class`, `Individual`, `ObjectProperty`,
//! `DataProperty`. These are convenience views over `NodeType` /
//! `EdgeType` / `PropertyType` from `atomr-ontology-core`.

use serde::{Deserialize, Serialize};

use atomr_ontology_core::Iri;

/// An OWL class (LPG analogue: `NodeType`).
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Class {
    /// Class name in the LPG schema.
    pub name: String,
    /// Canonical IRI.
    pub iri: Iri,
    /// Direct superclasses.
    pub super_classes: Vec<Iri>,
}

/// An OWL individual (LPG analogue: `Node`).
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Individual {
    /// Canonical IRI of the individual.
    pub iri: Iri,
    /// Class memberships (IRIs).
    pub types: Vec<Iri>,
}

/// An OWL object property — relation between individuals (LPG
/// analogue: an `EdgeType` whose range is an IRI-bearing node type).
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ObjectProperty {
    /// Property name.
    pub name: String,
    /// Canonical IRI.
    pub iri: Iri,
    /// Domain class IRIs.
    pub domain: Vec<Iri>,
    /// Range class IRIs.
    pub range: Vec<Iri>,
    /// `true` when the property is functional.
    pub functional: bool,
    /// Optional inverse property IRI.
    pub inverse_of: Option<Iri>,
}

/// An OWL datatype property — relation between an individual and a
/// literal (LPG analogue: a `PropertyType`).
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct DataProperty {
    /// Property name.
    pub name: String,
    /// Canonical IRI.
    pub iri: Iri,
    /// Domain class IRIs.
    pub domain: Vec<Iri>,
    /// XSD datatype IRI for the range.
    pub range_xsd: Iri,
}
