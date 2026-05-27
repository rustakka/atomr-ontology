//! SKOS (Simple Knowledge Organization System) importer.
//!
//! Recognizes the SKOS core vocabulary at
//! `http://www.w3.org/2004/02/skos/core#` and projects it into the
//! LPG model:
//!
//! - `skos:Concept` becomes a `NodeType` named `Concept`.
//! - `skos:broader`, `skos:narrower`, `skos:related` become
//!   `EdgeType`s.
//! - `skos:prefLabel`, `skos:altLabel`, `skos:definition` attach as
//!   `PropertyType`s on `Concept`.
//! - Every subject typed `skos:Concept` becomes a `Node`; its
//!   `skos:prefLabel` value (if any) is mirrored into a `name`
//!   property so downstream consumers can read it through the same
//!   key used by other importers.

use atomr_ontology_core::{Datatype, Node, Ontology, PropertyValue};
use atomr_ontology_provenance::Activity;
use atomr_ontology_rdf::turtle;

use crate::error::ImportError;
use crate::mapping::{ClassSpec, DataPropertySpec, Mapping, ObjectPropertySpec};

/// Canonical SKOS namespace.
pub const SKOS_NS: &str = "http://www.w3.org/2004/02/skos/core#";

const CLASSES: &[ClassSpec] = &[ClassSpec {
    iri: "http://www.w3.org/2004/02/skos/core#Concept",
    local: "Concept",
}];

const DATA_PROPERTIES: &[(DataPropertySpec, &[&str])] = &[
    (
        DataPropertySpec {
            iri: "http://www.w3.org/2004/02/skos/core#prefLabel",
            local: "prefLabel",
            datatype: Datatype::String,
        },
        &["Concept"],
    ),
    (
        DataPropertySpec {
            iri: "http://www.w3.org/2004/02/skos/core#altLabel",
            local: "altLabel",
            datatype: Datatype::String,
        },
        &["Concept"],
    ),
    (
        DataPropertySpec {
            iri: "http://www.w3.org/2004/02/skos/core#definition",
            local: "definition",
            datatype: Datatype::String,
        },
        &["Concept"],
    ),
];

const OBJECT_PROPERTIES: &[(ObjectPropertySpec, &[&str], &[&str])] = &[
    (
        ObjectPropertySpec { iri: "http://www.w3.org/2004/02/skos/core#broader", local: "broader" },
        &["Concept"],
        &["Concept"],
    ),
    (
        ObjectPropertySpec { iri: "http://www.w3.org/2004/02/skos/core#narrower", local: "narrower" },
        &["Concept"],
        &["Concept"],
    ),
    (
        ObjectPropertySpec { iri: "http://www.w3.org/2004/02/skos/core#related", local: "related" },
        &["Concept"],
        &["Concept"],
    ),
];

const MAPPING: Mapping = Mapping {
    classes: CLASSES,
    data_properties: DATA_PROPERTIES,
    object_properties: OBJECT_PROPERTIES,
    default_object_class: Some("Concept"),
};

/// Import a SKOS Turtle document.
///
/// Returns the populated [`Ontology`] paired with a finished
/// [`Activity`] record (label `"skos-import"`) so the import can be
/// committed to a provenance log.
pub fn import_skos(turtle_input: &str) -> Result<(Ontology, Activity), ImportError> {
    let activity = Activity::started("skos-import")
        .with_attribute("vocabulary", serde_json::json!(SKOS_NS))
        .with_attribute("format", serde_json::json!("turtle"));

    let triples = turtle::parse(turtle_input)?;

    let mut ontology = crate::mapping::declare(&MAPPING);
    crate::mapping::project(&mut ontology, &MAPPING, &triples);

    // Mirror prefLabel into a `name` property on every Concept so
    // callers have a consistent surface across importers.
    mirror_pref_label_as_name(&mut ontology);

    Ok((ontology, activity.finish()))
}

fn mirror_pref_label_as_name(ontology: &mut Ontology) {
    let node_ids: Vec<_> = ontology.nodes.keys().copied().collect();
    for id in node_ids {
        let Some(node) = ontology.nodes.get(&id) else { continue };
        if !node.has_type("Concept") {
            continue;
        }
        if node.property("name").is_some() {
            continue;
        }
        let Some(PropertyValue::String(label)) = node.property("prefLabel").cloned() else { continue };
        let updated: Node = node.clone().with_property("name", label);
        ontology.upsert_node(updated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <https://example.org/concept/> .

        ex:Animal a skos:Concept ;
            skos:prefLabel "Animal"@en ;
            skos:definition "A living organism." .

        ex:Cat a skos:Concept ;
            skos:prefLabel "Cat"@en ;
            skos:altLabel "Feline"@en ;
            skos:broader ex:Animal .

        ex:Dog a skos:Concept ;
            skos:prefLabel "Dog"@en ;
            skos:broader ex:Animal ;
            skos:related ex:Cat .
    "#;

    #[test]
    fn schema_declares_concept_class_and_skos_edges() {
        let (o, _act) = import_skos(SAMPLE).unwrap();
        let concept = o.schema.node_type("Concept").expect("Concept declared");
        assert_eq!(concept.iri.as_ref().unwrap().as_str(), "http://www.w3.org/2004/02/skos/core#Concept");
        assert!(concept.properties.iter().any(|p| p.name == "prefLabel"));
        assert!(concept.properties.iter().any(|p| p.name == "altLabel"));
        assert!(concept.properties.iter().any(|p| p.name == "definition"));

        for label in &["broader", "narrower", "related"] {
            assert!(o.schema.edge_type(label).is_some(), "edge type {label} declared");
        }
    }

    #[test]
    fn projects_concepts_and_relationships() {
        let (o, _act) = import_skos(SAMPLE).unwrap();
        assert_eq!(o.node_count(), 3, "Animal, Cat, Dog");
        // 1 broader (Cat -> Animal), 1 broader (Dog -> Animal),
        // 1 related (Dog -> Cat).
        assert_eq!(o.edge_count(), 3);
        let broader_count = o.edges.values().filter(|e| e.label == "broader").count();
        assert_eq!(broader_count, 2);
        let related_count = o.edges.values().filter(|e| e.label == "related").count();
        assert_eq!(related_count, 1);
    }

    #[test]
    fn pref_label_mirrors_into_name() {
        let (o, _act) = import_skos(SAMPLE).unwrap();
        let cat = o
            .nodes
            .values()
            .find(|n| n.iri.as_ref().map(|i| i.as_str()) == Some("https://example.org/concept/Cat"))
            .expect("Cat node present");
        assert_eq!(cat.property("name"), Some(&PropertyValue::String("Cat".into())));
        assert_eq!(cat.property("prefLabel"), Some(&PropertyValue::String("Cat".into())));
        assert_eq!(cat.property("altLabel"), Some(&PropertyValue::String("Feline".into())));
    }

    #[test]
    fn emits_finished_activity() {
        let (_o, act) = import_skos(SAMPLE).unwrap();
        assert_eq!(act.label, "skos-import");
        assert!(act.ended_at.is_some());
    }

    #[test]
    fn rejects_malformed_turtle() {
        let err = import_skos("this is not turtle").unwrap_err();
        matches!(err, ImportError::Adapter(_));
    }
}
