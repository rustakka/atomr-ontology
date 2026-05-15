//! Reusable fixtures: toy corpora, golden ontologies.

use atomr_ontology_core::{
    axiom::{Axiom, AxiomKind},
    schema::{Cardinality, EdgeType, NodeType, PropertyType},
    Datatype, Edge, Iri, Node, Ontology,
};

/// A small, deterministic seed corpus describing two organizations
/// and their members. Used by examples and tests.
pub fn toy_corpus() -> Vec<&'static str> {
    vec![
        "Acme Inc. is a corporation founded in 1995. Globex is its subsidiary.",
        "Bob Smith works at Acme Inc.; he is a member of the Engineering department.",
        "Globex Inc. employs Carol Davis as Chief Technology Officer.",
    ]
}

/// A golden W3C-Org-style ontology used as a baseline for round-trip
/// tests. Two organizations, one person, one membership edge.
pub fn toy_org_ontology() -> Ontology {
    let mut o = Ontology::with_iri("https://example.org/ontology/toy-org/v1").expect("static IRI is valid");
    o.vocabulary = atomr_ontology_core::namespace::Vocabulary::with_standard_bindings();

    let org_iri = Iri::from_unchecked("http://www.w3.org/ns/org#Organization");
    let formal_iri = Iri::from_unchecked("http://www.w3.org/ns/org#FormalOrganization");
    let person_iri = Iri::from_unchecked("http://xmlns.com/foaf/0.1/Person");
    let member_of_iri = Iri::from_unchecked("http://www.w3.org/ns/org#memberOf");

    let org_class = NodeType::new("Organization")
        .with_iri(org_iri.clone())
        .with_description("A collection of people organized for some purpose (W3C Org).");
    let formal_class =
        NodeType::new("FormalOrganization").with_iri(formal_iri).with_supertype("Organization");
    let name_prop = PropertyType {
        name: "name".into(),
        datatype: Datatype::String,
        cardinality: Cardinality::ONE,
        iri: Some(Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#label")),
        description: Some("Display name".into()),
    };
    let person_class = NodeType::new("Person").with_iri(person_iri).with_property(name_prop.clone());
    let org_class = org_class.with_property(name_prop);

    o.schema.declare_node_type(org_class);
    o.schema.declare_node_type(formal_class);
    o.schema.declare_node_type(person_class);
    o.schema.declare_edge_type(
        EdgeType::new("memberOf")
            .with_iri(member_of_iri)
            .with_domain("Person")
            .with_range("Organization")
            .with_description("The agent is a member of the organization."),
    );

    // Subclass axiom is implied by FormalOrganization's supertype, but we
    // also assert it explicitly so the validator round-trips it.
    o.upsert_axiom(Axiom::new(AxiomKind::SubClassOf {
        sub: "FormalOrganization".into(),
        sup: "Organization".into(),
    }));

    let acme_iri = Iri::new("https://example.org/Acme").unwrap();
    let globex_iri = Iri::new("https://example.org/Globex").unwrap();
    let bob_iri = Iri::new("https://example.org/Bob").unwrap();

    let acme = o.upsert_node(
        Node::from_iri(acme_iri, "Organization")
            .with_label("FormalOrganization")
            .with_property("name", "Acme Inc."),
    );
    let globex = o.upsert_node(
        Node::from_iri(globex_iri, "Organization")
            .with_label("FormalOrganization")
            .with_property("name", "Globex Inc."),
    );
    let bob = o.upsert_node(Node::from_iri(bob_iri, "Person").with_property("name", "Bob Smith"));

    let _ = o.upsert_edge(Edge::between(bob, "memberOf", acme));
    let _ = o.upsert_edge(Edge::between(globex, "memberOf", acme));

    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_well_formed() {
        let o = toy_org_ontology();
        assert!(o.iri.is_some());
        assert!(o.schema.node_type("Organization").is_some());
        assert_eq!(o.nodes.len(), 3);
        assert_eq!(o.edges.len(), 2);
    }
}
