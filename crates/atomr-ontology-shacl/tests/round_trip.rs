//! Integration test: schema → SHACL Turtle → schema must preserve
//! type names, property names, datatypes, and cardinality bounds.

use atomr_ontology_core::{
    schema::{Cardinality, NodeType, PropertyType},
    Datatype, Iri, Schema,
};
use atomr_ontology_shacl::{from_shacl_turtle, to_shacl_turtle};

fn build_org_schema() -> Schema {
    let mut s = Schema::new();
    s.declare_node_type(
        NodeType::new("Organization")
            .with_iri(Iri::from_unchecked("http://example.org/Organization"))
            .with_property(PropertyType {
                name: "name".into(),
                datatype: Datatype::String,
                cardinality: Cardinality::ONE,
                iri: Some(Iri::from_unchecked("http://example.org/name")),
                description: None,
            })
            .with_property(PropertyType {
                name: "founded".into(),
                datatype: Datatype::DateTime,
                cardinality: Cardinality::OPTIONAL,
                iri: Some(Iri::from_unchecked("http://example.org/founded")),
                description: None,
            }),
    );
    s
}

#[test]
fn schema_round_trips_through_shacl_turtle() {
    let original = build_org_schema();
    let ttl = to_shacl_turtle(&original).expect("compile");
    let parsed = from_shacl_turtle(&ttl).expect("parse");

    assert_eq!(
        parsed.node_types.len(),
        original.node_types.len(),
        "type count mismatch\n{ttl}"
    );

    let orig_ty = original.node_type("Organization").unwrap();
    let parsed_ty = parsed.node_type("Organization").expect("Organization missing after round-trip");

    assert_eq!(parsed_ty.iri, orig_ty.iri, "iri mismatch");
    assert_eq!(
        parsed_ty.properties.len(),
        orig_ty.properties.len(),
        "property count mismatch\n{ttl}"
    );

    // Properties may come back in any order; index by name.
    for orig_prop in &orig_ty.properties {
        let parsed_prop = parsed_ty
            .properties
            .iter()
            .find(|p| p.name == orig_prop.name)
            .unwrap_or_else(|| panic!("property {:?} missing", orig_prop.name));
        assert_eq!(parsed_prop.datatype, orig_prop.datatype, "datatype mismatch for {}", orig_prop.name);
        assert_eq!(
            parsed_prop.cardinality, orig_prop.cardinality,
            "cardinality mismatch for {}",
            orig_prop.name
        );
    }
}
