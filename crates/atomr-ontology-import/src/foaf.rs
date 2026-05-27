//! FOAF (Friend of a Friend) importer.
//!
//! Recognizes the FOAF vocabulary at `http://xmlns.com/foaf/0.1/`:
//!
//! - `foaf:Person`, `foaf:Organization` become `NodeType`s.
//! - `foaf:knows`, `foaf:member` become `EdgeType`s.
//! - `foaf:name`, `foaf:mbox`, `foaf:homepage` attach as
//!   `PropertyType`s on the appropriate classes.

use atomr_ontology_core::{Datatype, Ontology};
use atomr_ontology_provenance::Activity;
use atomr_ontology_rdf::turtle;

use crate::error::ImportError;
use crate::mapping::{ClassSpec, DataPropertySpec, Mapping, ObjectPropertySpec};

/// Canonical FOAF namespace.
pub const FOAF_NS: &str = "http://xmlns.com/foaf/0.1/";

const CLASSES: &[ClassSpec] = &[
    ClassSpec { iri: "http://xmlns.com/foaf/0.1/Person", local: "Person" },
    ClassSpec { iri: "http://xmlns.com/foaf/0.1/Organization", local: "Organization" },
];

const DATA_PROPERTIES: &[(DataPropertySpec, &[&str])] = &[
    (
        DataPropertySpec {
            iri: "http://xmlns.com/foaf/0.1/name",
            local: "name",
            datatype: Datatype::String,
        },
        &["Person", "Organization"],
    ),
    (
        DataPropertySpec {
            iri: "http://xmlns.com/foaf/0.1/mbox",
            local: "mbox",
            datatype: Datatype::Iri,
        },
        &["Person", "Organization"],
    ),
    (
        DataPropertySpec {
            iri: "http://xmlns.com/foaf/0.1/homepage",
            local: "homepage",
            datatype: Datatype::Iri,
        },
        &["Person", "Organization"],
    ),
];

const OBJECT_PROPERTIES: &[(ObjectPropertySpec, &[&str], &[&str])] = &[
    (
        ObjectPropertySpec { iri: "http://xmlns.com/foaf/0.1/knows", local: "knows" },
        &["Person"],
        &["Person"],
    ),
    (
        ObjectPropertySpec { iri: "http://xmlns.com/foaf/0.1/member", local: "member" },
        &["Organization"],
        &["Person"],
    ),
];

const MAPPING: Mapping = Mapping {
    classes: CLASSES,
    data_properties: DATA_PROPERTIES,
    object_properties: OBJECT_PROPERTIES,
    // FOAF documents often reference people / orgs via `foaf:knows`
    // without restating `rdf:type` — default to `Person` so those
    // edges still connect.
    default_object_class: Some("Person"),
};

/// Import a FOAF Turtle document.
///
/// Returns the populated [`Ontology`] paired with a finished
/// [`Activity`] record (label `"foaf-import"`).
pub fn import_foaf(turtle_input: &str) -> Result<(Ontology, Activity), ImportError> {
    let activity = Activity::started("foaf-import")
        .with_attribute("vocabulary", serde_json::json!(FOAF_NS))
        .with_attribute("format", serde_json::json!("turtle"));

    let triples = turtle::parse(turtle_input)?;

    let mut ontology = crate::mapping::declare(&MAPPING);
    crate::mapping::project(&mut ontology, &MAPPING, &triples);

    Ok((ontology, activity.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::PropertyValue;

    const SAMPLE: &str = r#"
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .
        @prefix ex: <https://example.org/> .

        ex:Acme a foaf:Organization ;
            foaf:name "ACME Corp" ;
            foaf:homepage <https://acme.example.org/> ;
            foaf:member ex:Alice .

        ex:Alice a foaf:Person ;
            foaf:name "Alice" ;
            foaf:mbox <mailto:alice@example.org> ;
            foaf:knows ex:Bob .

        ex:Bob a foaf:Person ;
            foaf:name "Bob" .
    "#;

    #[test]
    fn schema_declares_person_and_organization() {
        let (o, _) = import_foaf(SAMPLE).unwrap();
        let person = o.schema.node_type("Person").expect("Person declared");
        assert_eq!(person.iri.as_ref().unwrap().as_str(), "http://xmlns.com/foaf/0.1/Person");
        assert!(person.properties.iter().any(|p| p.name == "name"));
        assert!(person.properties.iter().any(|p| p.name == "mbox"));

        let org = o.schema.node_type("Organization").expect("Organization declared");
        assert!(org.properties.iter().any(|p| p.name == "homepage"));

        for label in &["knows", "member"] {
            assert!(o.schema.edge_type(label).is_some(), "edge type {label} declared");
        }
    }

    #[test]
    fn projects_individuals_and_edges() {
        let (o, _) = import_foaf(SAMPLE).unwrap();
        assert_eq!(o.node_count(), 3, "Acme, Alice, Bob");
        // 1 member (Acme -> Alice), 1 knows (Alice -> Bob).
        assert_eq!(o.edge_count(), 2);

        let alice = o
            .nodes
            .values()
            .find(|n| n.iri.as_ref().map(|i| i.as_str()) == Some("https://example.org/Alice"))
            .expect("Alice present");
        assert_eq!(alice.property("name"), Some(&PropertyValue::String("Alice".into())));
        match alice.property("mbox") {
            Some(PropertyValue::Iri(iri)) => assert_eq!(iri.as_str(), "mailto:alice@example.org"),
            other => panic!("expected mbox iri, got {other:?}"),
        }
    }

    #[test]
    fn emits_finished_activity() {
        let (_o, act) = import_foaf(SAMPLE).unwrap();
        assert_eq!(act.label, "foaf-import");
        assert!(act.ended_at.is_some());
    }
}
