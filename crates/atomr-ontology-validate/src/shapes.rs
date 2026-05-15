//! SHACL-style shape validation: cardinality, datatype, domain, range.

use atomr_ontology_core::{schema::Datatype, Ontology, PropertyValue};

use crate::report::{ValidationFinding, ValidationReport};

/// Check shape constraints against the schema.
pub fn check_shapes(ontology: &Ontology) -> ValidationReport {
    let mut report = ValidationReport::default();

    // Node-level cardinality + datatype.
    for node in ontology.nodes.values() {
        for type_name in &node.types {
            let Some(nt) = ontology.schema.node_type(type_name) else { continue };
            for prop in &nt.properties {
                let value = node.properties.get(&prop.name);
                let count = u32::from(value.is_some());
                if !prop.cardinality.contains(count) {
                    report.push(
                        ValidationFinding::error(
                            "cardinality.violation",
                            format!(
                                "node {} has {} value(s) for property `{}` (cardinality {}..{})",
                                node.id,
                                count,
                                prop.name,
                                prop.cardinality.min,
                                prop.cardinality
                                    .max
                                    .map(|n| n.to_string())
                                    .unwrap_or_else(|| "*".to_string()),
                            ),
                        )
                        .focus(node.id.to_string()),
                    );
                }
                if let Some(value) = value {
                    if !value_matches_datatype(value, prop.datatype) {
                        report.push(
                            ValidationFinding::warning(
                                "datatype.mismatch",
                                format!(
                                    "node {} property `{}` has value not matching declared datatype {:?}",
                                    node.id, prop.name, prop.datatype
                                ),
                            )
                            .focus(node.id.to_string()),
                        );
                    }
                }
            }
        }
    }

    // Edge-level domain / range.
    for edge in ontology.edges.values() {
        let Some(et) = ontology.schema.edge_type(&edge.label) else { continue };
        if !et.domain.is_empty() {
            if let Some(src) = ontology.node(&edge.source) {
                if !type_in_chain(ontology, src, &et.domain) {
                    report.push(
                        ValidationFinding::error(
                            "edge.domain",
                            format!(
                                "edge {} source {} does not satisfy domain {:?}",
                                edge.id, edge.source, et.domain
                            ),
                        )
                        .focus(edge.id.to_string()),
                    );
                }
            }
        }
        if !et.range.is_empty() {
            if let Some(tgt) = ontology.node(&edge.target) {
                if !type_in_chain(ontology, tgt, &et.range) {
                    report.push(
                        ValidationFinding::error(
                            "edge.range",
                            format!(
                                "edge {} target {} does not satisfy range {:?}",
                                edge.id, edge.target, et.range
                            ),
                        )
                        .focus(edge.id.to_string()),
                    );
                }
            }
        }
    }

    report
}

fn type_in_chain(ontology: &Ontology, node: &atomr_ontology_core::Node, allowed: &[String]) -> bool {
    for ty in &node.types {
        for n in ontology.schema.supertypes_of(ty) {
            if allowed.iter().any(|a| a == n) {
                return true;
            }
        }
    }
    false
}

fn value_matches_datatype(value: &PropertyValue, datatype: Datatype) -> bool {
    matches!(
        (value, datatype),
        (PropertyValue::String(_), Datatype::String)
            | (PropertyValue::Integer(_), Datatype::Integer)
            | (PropertyValue::Float(_), Datatype::Float)
            | (PropertyValue::Bool(_), Datatype::Bool)
            | (PropertyValue::DateTime(_), Datatype::DateTime)
            | (PropertyValue::Iri(_), Datatype::Iri)
            | (PropertyValue::Bytes(_), Datatype::Bytes)
            | (PropertyValue::Json(_), Datatype::Json)
            | (PropertyValue::Null, _)
    )
}
