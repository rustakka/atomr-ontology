//! Parse SHACL Turtle back into an [`atomr_ontology_core::Schema`].
//!
//! The parser is intentionally focused: it consumes the subset of
//! SHACL that [`crate::compile::to_shacl_turtle`] emits — namely
//! `sh:NodeShape` declarations with `sh:targetClass`, `sh:property`
//! pointers, and per-property blocks carrying `sh:path`,
//! `sh:minCount`, `sh:maxCount`, `sh:datatype`. Anything beyond this
//! is silently ignored so externally-authored shapes can still drive
//! the validate crate.

use std::collections::{BTreeMap, BTreeSet};

use atomr_ontology_core::{Cardinality, Datatype, Iri, NodeType, PropertyType, Schema};
use atomr_ontology_rdf::{turtle, AdapterError, Object, Subject, Triple};
use thiserror::Error;

use crate::ns::{shacl_iri, XSD_URI};

/// Errors raised while parsing a SHACL Turtle document.
#[derive(Debug, Error)]
pub enum ShaclParseError {
    /// The underlying Turtle adapter rejected the input.
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    /// A required SHACL term was missing from a shape.
    #[error("missing required SHACL term: {0}")]
    MissingRequired(String),
    /// A catch-all for unexpected structural failures.
    #[error("shacl parse error: {0}")]
    Other(String),
}

/// Parse a SHACL Turtle document and project it into a [`Schema`].
pub fn from_shacl_turtle(input: &str) -> Result<Schema, ShaclParseError> {
    let triples = turtle::parse(input)?;
    build_schema(&triples)
}

fn build_schema(triples: &[Triple]) -> Result<Schema, ShaclParseError> {
    let rdf_type = Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let node_shape = shacl_iri("NodeShape");
    let target_class = shacl_iri("targetClass");
    let property = shacl_iri("property");
    let path = shacl_iri("path");
    let min_count = shacl_iri("minCount");
    let max_count = shacl_iri("maxCount");
    let datatype = shacl_iri("datatype");

    // Index every triple by subject so we can walk property blocks
    // without quadratic scans.
    let mut by_subject: BTreeMap<SubjectKey, Vec<&Triple>> = BTreeMap::new();
    for t in triples {
        by_subject.entry(SubjectKey::from(&t.subject)).or_default().push(t);
    }

    // Identify NodeShape subjects.
    let mut shape_subjects: BTreeSet<SubjectKey> = BTreeSet::new();
    for t in triples {
        if iri_eq(&t.predicate, &rdf_type) {
            if let Object::Iri(obj) = &t.object {
                if iri_eq(obj, &node_shape) {
                    shape_subjects.insert(SubjectKey::from(&t.subject));
                }
            }
        }
    }

    let mut schema = Schema::new();

    for shape in &shape_subjects {
        let Some(triples) = by_subject.get(shape) else {
            continue;
        };

        // Resolve the target class IRI.
        let target_iri = triples
            .iter()
            .find_map(|t| {
                if iri_eq(&t.predicate, &target_class) {
                    if let Object::Iri(iri) = &t.object {
                        return Some(iri.clone());
                    }
                }
                None
            })
            .ok_or_else(|| ShaclParseError::MissingRequired("sh:targetClass".into()))?;

        let type_name = local_name(&target_iri).to_string();
        let mut node_ty = NodeType::new(type_name).with_iri(target_iri);

        // Walk each property reference and parse its block.
        for t in triples {
            if !iri_eq(&t.predicate, &property) {
                continue;
            }
            let block_key = match &t.object {
                Object::Iri(iri) => SubjectKey::Iri(iri.as_str().to_string()),
                Object::Blank(label) => SubjectKey::Blank(label.clone()),
                Object::Literal { .. } => continue,
            };
            let Some(block_triples) = by_subject.get(&block_key) else {
                continue;
            };
            if let Some(prop_ty) = parse_property_block(
                block_triples,
                &path,
                &min_count,
                &max_count,
                &datatype,
            )? {
                node_ty = node_ty.with_property(prop_ty);
            }
        }

        schema.declare_node_type(node_ty);
    }

    Ok(schema)
}

fn parse_property_block(
    triples: &[&Triple],
    path_iri: &Iri,
    min_count_iri: &Iri,
    max_count_iri: &Iri,
    datatype_iri: &Iri,
) -> Result<Option<PropertyType>, ShaclParseError> {
    let mut path: Option<Iri> = None;
    let mut min: u32 = 0;
    let mut max: Option<u32> = None;
    let mut datatype: Option<Datatype> = None;

    for t in triples {
        if iri_eq(&t.predicate, path_iri) {
            if let Object::Iri(iri) = &t.object {
                path = Some(iri.clone());
            }
        } else if iri_eq(&t.predicate, min_count_iri) {
            if let Some(n) = literal_as_u32(&t.object) {
                min = n;
            }
        } else if iri_eq(&t.predicate, max_count_iri) {
            if let Some(n) = literal_as_u32(&t.object) {
                max = Some(n);
            }
        } else if iri_eq(&t.predicate, datatype_iri) {
            if let Object::Iri(iri) = &t.object {
                datatype = Some(datatype_from_xsd(iri));
            }
        }
    }

    let Some(path) = path else {
        // No sh:path: not a datatype property block we model. Skip
        // gracefully so edge-style blocks (sh:nodeKind / sh:class)
        // don't fail the parse.
        return Ok(None);
    };

    // Only emit a PropertyType when we can identify a scalar datatype.
    // Blocks lacking sh:datatype are likely edge constraints; skip them.
    let Some(datatype) = datatype else {
        return Ok(None);
    };

    let name = local_name(&path).to_string();
    Ok(Some(PropertyType {
        name,
        datatype,
        cardinality: Cardinality { min, max },
        iri: Some(path),
        description: None,
    }))
}

fn literal_as_u32(obj: &Object) -> Option<u32> {
    match obj {
        Object::Literal { lexical, .. } => lexical.parse::<u32>().ok(),
        _ => None,
    }
}

fn datatype_from_xsd(iri: &Iri) -> Datatype {
    let local = iri.as_str().strip_prefix(XSD_URI).unwrap_or(iri.as_str());
    match local {
        "string" => Datatype::String,
        "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
        | "positiveInteger" | "negativeInteger" | "nonPositiveInteger" => Datatype::Integer,
        "double" | "float" | "decimal" => Datatype::Float,
        "boolean" => Datatype::Bool,
        "dateTime" | "date" => Datatype::DateTime,
        "anyURI" => Datatype::Iri,
        "base64Binary" | "hexBinary" => Datatype::Bytes,
        _ => Datatype::String,
    }
}

fn local_name(iri: &Iri) -> &str {
    let s = iri.as_str();
    if let Some(idx) = s.rfind(|c: char| c == '#' || c == '/' || c == ':') {
        &s[idx + 1..]
    } else {
        s
    }
}

fn iri_eq(a: &Iri, b: &Iri) -> bool {
    a.as_str() == b.as_str()
}

/// A subject key usable in `BTreeMap` / `BTreeSet` (the wire `Subject`
/// type intentionally lacks `Ord`).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SubjectKey {
    Iri(String),
    Blank(String),
}

impl From<&Subject> for SubjectKey {
    fn from(s: &Subject) -> Self {
        match s {
            Subject::Iri(iri) => SubjectKey::Iri(iri.as_str().to_string()),
            Subject::Blank(label) => SubjectKey::Blank(label.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_node_shape() {
        let ttl = r#"
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix ex: <http://example.org/> .

ex:OrganizationShape a sh:NodeShape ;
  sh:targetClass ex:Organization ;
  sh:property _:b0 .

_:b0 sh:path ex:name ;
  sh:minCount "1"^^xsd:integer ;
  sh:maxCount "1"^^xsd:integer ;
  sh:datatype xsd:string .
"#;
        let schema = from_shacl_turtle(ttl).unwrap();
        let ty = schema.node_type("Organization").expect("Organization type missing");
        assert_eq!(ty.properties.len(), 1, "expected 1 property, got {:?}", ty.properties);
        let prop = &ty.properties[0];
        assert_eq!(prop.name, "name");
        assert_eq!(prop.datatype, Datatype::String);
        assert_eq!(prop.cardinality.min, 1);
        assert_eq!(prop.cardinality.max, Some(1));
    }
}
