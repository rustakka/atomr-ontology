//! Reference vocabulary for organizational ontologies.
//!
//! This crate is a **worked example**, not a privileged core. It
//! defines the W3C Org Ontology core types (`Organization`,
//! `FormalOrganization`, `OrganizationalUnit`, `Membership`, `Role`,
//! `Post`, `Site`) and the schema.org `Organization` projection,
//! ready to be merged into a user's [`Ontology`].
//!
//! Downstream users are free to ignore this crate entirely and bring
//! their own vocabulary; the rest of the workspace makes no
//! assumption that it is loaded.

#![forbid(unsafe_code)]

use atomr_ontology_core::{
    axiom::{Axiom, AxiomKind},
    namespace::Vocabulary,
    schema::{Cardinality, EdgeType, NodeType, PropertyType},
    Datatype, Iri, Ontology,
};

/// W3C Org Ontology namespace base.
pub const ORG_NS: &str = "http://www.w3.org/ns/org#";
/// FOAF namespace base.
pub const FOAF_NS: &str = "http://xmlns.com/foaf/0.1/";
/// schema.org namespace base.
pub const SCHEMA_NS: &str = "http://schema.org/";

fn iri(base: &str, name: &str) -> Iri {
    Iri::from_unchecked(format!("{base}{name}"))
}

/// Build an [`Ontology`] containing the reference vocabulary.
///
/// Calling this on an existing ontology is idempotent — repeated
/// declarations of the same node/edge type simply overwrite the
/// entry with the same value.
pub fn build_reference_vocabulary(out: &mut Ontology) {
    out.vocabulary = if out.vocabulary.iter().next().is_some() {
        out.vocabulary.clone()
    } else {
        Vocabulary::with_standard_bindings()
    };

    out.schema.declare_node_type(
        NodeType::new("Organization")
            .with_iri(iri(ORG_NS, "Organization"))
            .with_description("A collection of people organized for some purpose (W3C Org).")
            .with_property(label_property()),
    );
    out.schema.declare_node_type(
        NodeType::new("FormalOrganization")
            .with_iri(iri(ORG_NS, "FormalOrganization"))
            .with_supertype("Organization")
            .with_description(
                "An organization recognized in the world at large, e.g., a corporation, government.",
            ),
    );
    out.schema.declare_node_type(
        NodeType::new("OrganizationalUnit")
            .with_iri(iri(ORG_NS, "OrganizationalUnit"))
            .with_supertype("Organization")
            .with_description("A unit of an organization, e.g., a department or division."),
    );
    out.schema.declare_node_type(
        NodeType::new("Person")
            .with_iri(iri(FOAF_NS, "Person"))
            .with_description("A natural person.")
            .with_property(label_property()),
    );
    out.schema.declare_node_type(
        NodeType::new("Role")
            .with_iri(iri(ORG_NS, "Role"))
            .with_description("A role that an agent may play."),
    );
    out.schema.declare_node_type(
        NodeType::new("Post")
            .with_iri(iri(ORG_NS, "Post"))
            .with_description("A position that may be held by a single agent."),
    );
    out.schema.declare_node_type(
        NodeType::new("Site")
            .with_iri(iri(ORG_NS, "Site"))
            .with_description("A site that an organization occupies."),
    );
    out.schema.declare_node_type(
        NodeType::new("Membership")
            .with_iri(iri(ORG_NS, "Membership"))
            .with_description("Indirect relation between an agent and an organization."),
    );

    // Edges.
    out.schema.declare_edge_type(
        EdgeType::new("memberOf")
            .with_iri(iri(ORG_NS, "memberOf"))
            .with_domain("Person")
            .with_domain("Organization")
            .with_range("Organization")
            .with_inverse("hasMember")
            .with_description("Indicates that the agent is a member of the organization."),
    );
    out.schema.declare_edge_type(
        EdgeType::new("hasMember")
            .with_iri(iri(ORG_NS, "hasMember"))
            .with_domain("Organization")
            .with_range("Person")
            .with_range("Organization")
            .with_inverse("memberOf"),
    );
    out.schema.declare_edge_type(
        EdgeType::new("subOrganizationOf")
            .with_iri(iri(ORG_NS, "subOrganizationOf"))
            .with_domain("Organization")
            .with_range("Organization")
            .with_inverse("hasSubOrganization"),
    );
    out.schema.declare_edge_type(
        EdgeType::new("hasSubOrganization")
            .with_iri(iri(ORG_NS, "hasSubOrganization"))
            .with_domain("Organization")
            .with_range("Organization")
            .with_inverse("subOrganizationOf"),
    );
    out.schema.declare_edge_type(
        EdgeType::new("hasMembership")
            .with_iri(iri(ORG_NS, "hasMembership"))
            .with_domain("Person")
            .with_range("Membership"),
    );
    out.schema.declare_edge_type(
        EdgeType::new("organization")
            .with_iri(iri(ORG_NS, "organization"))
            .with_domain("Membership")
            .with_range("Organization"),
    );
    out.schema.declare_edge_type(
        EdgeType::new("role").with_iri(iri(ORG_NS, "role")).with_domain("Membership").with_range("Role"),
    );
    out.schema.declare_edge_type(
        EdgeType::new("hasSite")
            .with_iri(iri(ORG_NS, "hasSite"))
            .with_domain("Organization")
            .with_range("Site"),
    );

    // schema.org projection: schema:Organization is treated as an
    // equivalent class to org:Organization.
    out.upsert_axiom(Axiom::new(AxiomKind::EquivalentClass {
        left: "Organization".into(),
        right: "schema:Organization".into(),
    }));
    out.upsert_axiom(Axiom::new(AxiomKind::SubClassOf {
        sub: "FormalOrganization".into(),
        sup: "Organization".into(),
    }));
    out.upsert_axiom(Axiom::new(AxiomKind::SubClassOf {
        sub: "OrganizationalUnit".into(),
        sup: "Organization".into(),
    }));
    out.upsert_axiom(Axiom::new(AxiomKind::InverseOf { left: "memberOf".into(), right: "hasMember".into() }));
    out.upsert_axiom(Axiom::new(AxiomKind::InverseOf {
        left: "subOrganizationOf".into(),
        right: "hasSubOrganization".into(),
    }));
    out.upsert_axiom(Axiom::new(AxiomKind::Transitive { property: "subOrganizationOf".into() }));
}

fn label_property() -> PropertyType {
    PropertyType {
        name: "name".into(),
        datatype: Datatype::String,
        cardinality: Cardinality::ONE,
        iri: Some(Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#label")),
        description: Some("Display name".into()),
    }
}

/// Build a fresh reference [`Ontology`] (no instances).
pub fn reference_ontology() -> Ontology {
    let mut o = Ontology::with_iri("https://atomr.dev/ontology/org/v1").expect("static IRI is valid");
    build_reference_vocabulary(&mut o);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_ontology_has_declared_types() {
        let o = reference_ontology();
        assert!(o.schema.node_type("Organization").is_some());
        assert!(o.schema.node_type("FormalOrganization").is_some());
        assert!(o.schema.edge_type("memberOf").is_some());
        assert!(o.axioms.values().any(|a| matches!(a.kind, AxiomKind::Transitive { .. })));
    }
}
