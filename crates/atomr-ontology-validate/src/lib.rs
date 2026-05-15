//! Validation — shape (SHACL-style) and axiom-consistency checks.
//!
//! The [`validate`] entry point runs every check and returns a
//! [`ValidationReport`]. A consumer (typically the commit stage of
//! the auto-extract pipeline) can choose to:
//!
//! 1. Reject the commit if there are errors.
//! 2. Filter out individual offending nodes/edges/axioms.
//! 3. Surface warnings to a human reviewer.

#![forbid(unsafe_code)]

pub mod consistency;
pub mod report;
pub mod shapes;

pub use consistency::check_consistency;
pub use report::{Severity, ValidationFinding, ValidationReport};
pub use shapes::check_shapes;

use atomr_ontology_core::Ontology;

/// Run all available checks and aggregate them into a single report.
pub fn validate(ontology: &Ontology) -> ValidationReport {
    let mut report = ValidationReport::default();
    report.extend(check_shapes(ontology));
    report.extend(check_consistency(ontology));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{
        axiom::{Axiom, AxiomKind},
        schema::{Cardinality, EdgeType, NodeType, PropertyType},
        Datatype, Edge, Iri, Node,
    };

    #[test]
    fn empty_ontology_is_clean() {
        let o = Ontology::new();
        assert!(validate(&o).is_clean());
    }

    #[test]
    fn detects_required_property_violation() {
        let mut o = Ontology::new();
        let nt = NodeType::new("Organization").with_property(PropertyType {
            name: "name".into(),
            datatype: Datatype::String,
            cardinality: Cardinality::ONE,
            iri: None,
            description: None,
        });
        o.schema.declare_node_type(nt);
        o.upsert_node(Node::new("Organization"));
        let r = validate(&o);
        assert!(!r.is_clean());
    }

    #[test]
    fn detects_disjoint_violation() {
        let mut o = Ontology::new();
        o.schema.declare_node_type(NodeType::new("A"));
        o.schema.declare_node_type(NodeType::new("B"));
        o.upsert_axiom(Axiom::new(AxiomKind::DisjointWith { left: "A".into(), right: "B".into() }));
        let _id = o.upsert_node(Node::new("A").with_label("B"));
        let r = validate(&o);
        assert!(!r.is_clean());
    }

    #[test]
    fn detects_domain_violation() {
        let mut o = Ontology::new();
        o.schema.declare_node_type(NodeType::new("Organization"));
        o.schema.declare_node_type(NodeType::new("Person"));
        o.schema.declare_edge_type(
            EdgeType::new("memberOf").with_domain("Organization").with_range("Organization"),
        );
        let p = o.upsert_node(Node::from_iri(Iri::new("https://example.org/Bob").unwrap(), "Person"));
        let org = o.upsert_node(Node::new("Organization"));
        o.upsert_edge(Edge::between(p, "memberOf", org));
        let r = validate(&o);
        assert!(!r.is_clean());
    }
}
