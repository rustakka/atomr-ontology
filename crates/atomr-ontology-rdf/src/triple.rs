//! RDF triple and quad types.

use serde::{Deserialize, Serialize};

use atomr_ontology_core::Iri;

/// The subject of a triple — either an IRI-named resource or a
/// blank node (anonymous resource with a local label).
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Subject {
    /// Named resource.
    Iri(Iri),
    /// Blank node with a local label (e.g. `_:n0`).
    Blank(String),
}

impl Subject {
    /// Construct a blank-node subject from a numeric counter.
    pub fn blank_n(n: u64) -> Self {
        Subject::Blank(format!("n{n}"))
    }
}

/// The object of a triple — IRI, blank node, or a typed literal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Object {
    /// Named resource.
    Iri(Iri),
    /// Blank node.
    Blank(String),
    /// Typed literal: lexical form plus an XSD datatype IRI.
    Literal {
        /// Lexical form.
        lexical: String,
        /// XSD datatype IRI (`xsd:string` if untyped text).
        datatype: Iri,
        /// Optional BCP 47 language tag (only meaningful for `rdf:langString`).
        language: Option<String>,
    },
}

impl Object {
    /// Plain `xsd:string` literal.
    pub fn xsd_string(value: impl Into<String>) -> Self {
        Object::Literal {
            lexical: value.into(),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#string"),
            language: None,
        }
    }

    /// `xsd:integer` literal.
    pub fn xsd_integer(value: i64) -> Self {
        Object::Literal {
            lexical: value.to_string(),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
            language: None,
        }
    }

    /// `xsd:double` literal.
    pub fn xsd_double(value: f64) -> Self {
        Object::Literal {
            lexical: value.to_string(),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#double"),
            language: None,
        }
    }

    /// `xsd:boolean` literal.
    pub fn xsd_boolean(value: bool) -> Self {
        Object::Literal {
            lexical: value.to_string(),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#boolean"),
            language: None,
        }
    }

    /// `xsd:dateTime` literal from a chrono UTC timestamp.
    pub fn xsd_date_time(value: chrono::DateTime<chrono::Utc>) -> Self {
        Object::Literal {
            lexical: value.to_rfc3339(),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            language: None,
        }
    }
}

/// An RDF triple `<subject> <predicate> <object>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Triple {
    /// Subject.
    pub subject: Subject,
    /// Predicate IRI.
    pub predicate: Iri,
    /// Object.
    pub object: Object,
}

impl Triple {
    /// Build a triple.
    pub fn new(subject: Subject, predicate: Iri, object: Object) -> Self {
        Self { subject, predicate, object }
    }
}

/// An RDF quad — triple plus a named graph IRI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quad {
    /// Subject.
    pub subject: Subject,
    /// Predicate IRI.
    pub predicate: Iri,
    /// Object.
    pub object: Object,
    /// Named graph IRI.
    pub graph: Iri,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xsd_literals_round_trip() {
        let lit = Object::xsd_integer(42);
        match lit {
            Object::Literal { lexical, datatype, .. } => {
                assert_eq!(lexical, "42");
                assert!(datatype.as_str().ends_with("#integer"));
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn blank_node_helper() {
        match Subject::blank_n(7) {
            Subject::Blank(s) => assert_eq!(s, "n7"),
            _ => panic!("expected blank"),
        }
    }
}
