//! schema.org JSON-LD importer.
//!
//! Recognizes the schema.org vocabulary at `https://schema.org/`.
//!
//! - `schema:Organization`, `schema:Person`, `schema:WebSite`,
//!   `schema:Place` become `NodeType`s.
//! - `schema:memberOf`, `schema:worksFor` become `EdgeType`s.
//! - `schema:name`, `schema:url`, `schema:address` attach as
//!   `PropertyType`s on the recognized classes.
//!
//! The JSON-LD parser used (from `atomr_ontology_rdf::jsonld`)
//! supports the compact-IRI form (e.g. `"schema:name"`) when the
//! document's `@context` binds the `schema` prefix, as well as the
//! full IRI form (`"https://schema.org/name"`). Both forms collapse
//! to the same triple after parsing, so we match exclusively on the
//! full IRI.

use atomr_ontology_core::{Datatype, Ontology};
use atomr_ontology_provenance::Activity;
use atomr_ontology_rdf::jsonld;

use crate::error::ImportError;
use crate::mapping::{ClassSpec, DataPropertySpec, Mapping, ObjectPropertySpec};

/// Canonical schema.org namespace.
pub const SCHEMA_NS: &str = "https://schema.org/";

const CLASSES: &[ClassSpec] = &[
    ClassSpec { iri: "https://schema.org/Organization", local: "Organization" },
    ClassSpec { iri: "https://schema.org/Person", local: "Person" },
    ClassSpec { iri: "https://schema.org/WebSite", local: "WebSite" },
    ClassSpec { iri: "https://schema.org/Place", local: "Place" },
];

const DATA_PROPERTIES: &[(DataPropertySpec, &[&str])] = &[
    (
        DataPropertySpec {
            iri: "https://schema.org/name",
            local: "name",
            datatype: Datatype::String,
        },
        &["Organization", "Person", "WebSite", "Place"],
    ),
    (
        DataPropertySpec {
            iri: "https://schema.org/url",
            local: "url",
            datatype: Datatype::Iri,
        },
        &["Organization", "Person", "WebSite", "Place"],
    ),
    (
        DataPropertySpec {
            iri: "https://schema.org/address",
            local: "address",
            datatype: Datatype::String,
        },
        &["Organization", "Person", "Place"],
    ),
];

const OBJECT_PROPERTIES: &[(ObjectPropertySpec, &[&str], &[&str])] = &[
    (
        ObjectPropertySpec { iri: "https://schema.org/memberOf", local: "memberOf" },
        &["Person", "Organization"],
        &["Organization"],
    ),
    (
        ObjectPropertySpec { iri: "https://schema.org/worksFor", local: "worksFor" },
        &["Person"],
        &["Organization"],
    ),
];

const MAPPING: Mapping = Mapping {
    classes: CLASSES,
    data_properties: DATA_PROPERTIES,
    object_properties: OBJECT_PROPERTIES,
    // schema.org documents often reference an Organization as the
    // target of `memberOf` / `worksFor` without restating its type
    // inline — fall back to `Organization` for unyielded targets.
    default_object_class: Some("Organization"),
};

/// Import a schema.org JSON-LD document.
///
/// Returns the populated [`Ontology`] paired with a finished
/// [`Activity`] record (label `"schema-org-import"`).
pub fn import_schema_org(jsonld_input: &str) -> Result<(Ontology, Activity), ImportError> {
    let activity = Activity::started("schema-org-import")
        .with_attribute("vocabulary", serde_json::json!(SCHEMA_NS))
        .with_attribute("format", serde_json::json!("json-ld"));

    let triples = jsonld::parse(jsonld_input)?;

    let mut ontology = crate::mapping::declare(&MAPPING);
    crate::mapping::project(&mut ontology, &MAPPING, &triples);

    Ok((ontology, activity.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::PropertyValue;

    const SAMPLE: &str = r#"{
        "@context": {
            "schema": "https://schema.org/"
        },
        "@graph": [
            {
                "@id": "https://example.org/Acme",
                "@type": "https://schema.org/Organization",
                "https://schema.org/name": [{"@value": "Acme Inc."}],
                "https://schema.org/url": [{"@id": "https://acme.example.org/"}],
                "https://schema.org/address": [{"@value": "123 Way"}]
            },
            {
                "@id": "https://example.org/Alice",
                "@type": "https://schema.org/Person",
                "https://schema.org/name": [{"@value": "Alice"}],
                "https://schema.org/worksFor": [{"@id": "https://example.org/Acme"}],
                "https://schema.org/memberOf": [{"@id": "https://example.org/Acme"}]
            },
            {
                "@id": "https://example.org/site",
                "@type": "https://schema.org/WebSite",
                "https://schema.org/name": [{"@value": "Acme Web"}],
                "https://schema.org/url": [{"@id": "https://acme.example.org/"}]
            }
        ]
    }"#;

    #[test]
    fn schema_declares_recognized_classes_and_edges() {
        let (o, _) = import_schema_org(SAMPLE).unwrap();
        for class in &["Organization", "Person", "WebSite", "Place"] {
            let nt = o.schema.node_type(class).unwrap_or_else(|| panic!("{class} declared"));
            assert!(nt.iri.is_some());
        }
        for edge in &["memberOf", "worksFor"] {
            assert!(o.schema.edge_type(edge).is_some(), "edge type {edge} declared");
        }
    }

    #[test]
    fn projects_individuals_and_edges() {
        let (o, _) = import_schema_org(SAMPLE).unwrap();
        assert_eq!(o.node_count(), 3, "Acme, Alice, site");
        // 1 worksFor + 1 memberOf, both Alice -> Acme.
        assert_eq!(o.edge_count(), 2);

        let alice = o
            .nodes
            .values()
            .find(|n| n.iri.as_ref().map(|i| i.as_str()) == Some("https://example.org/Alice"))
            .expect("Alice present");
        assert_eq!(alice.property("name"), Some(&PropertyValue::String("Alice".into())));

        let acme = o
            .nodes
            .values()
            .find(|n| n.iri.as_ref().map(|i| i.as_str()) == Some("https://example.org/Acme"))
            .expect("Acme present");
        assert_eq!(acme.property("name"), Some(&PropertyValue::String("Acme Inc.".into())));
        assert_eq!(acme.property("address"), Some(&PropertyValue::String("123 Way".into())));
        match acme.property("url") {
            Some(PropertyValue::Iri(iri)) => assert_eq!(iri.as_str(), "https://acme.example.org/"),
            other => panic!("expected url iri, got {other:?}"),
        }
    }

    #[test]
    fn emits_finished_activity() {
        let (_o, act) = import_schema_org(SAMPLE).unwrap();
        assert_eq!(act.label, "schema-org-import");
        assert!(act.ended_at.is_some());
    }

    #[test]
    fn rejects_malformed_json() {
        let err = import_schema_org("not json").unwrap_err();
        matches!(err, ImportError::Adapter(_));
    }
}
