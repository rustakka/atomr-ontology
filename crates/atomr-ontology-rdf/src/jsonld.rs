//! Minimal JSON-LD writer.
//!
//! Produces a `{ "@context": {...}, "@graph": [...] }` document
//! suitable for further processing by a full JSON-LD processor. The
//! writer is intentionally simple: each subject becomes one node
//! object; properties are emitted using the local name from their
//! IRI plus a `@context` mapping that records the full IRI.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use atomr_ontology_core::namespace::Vocabulary;
use atomr_ontology_core::Ontology;

use crate::adapter::to_rdf;
use crate::triple::{Object, Subject, Triple};

/// Serialize an ontology as a JSON-LD document (pretty-printed).
pub fn write(ontology: &Ontology) -> String {
    let triples = to_rdf(ontology);
    let vocab = if ontology.vocabulary.iter().next().is_some() {
        ontology.vocabulary.clone()
    } else {
        Vocabulary::with_standard_bindings()
    };

    let mut context = Map::new();
    for ns in vocab.iter() {
        context.insert(ns.prefix.clone(), json!(ns.base.as_str()));
    }

    // Group triples by subject.
    let mut by_subject: BTreeMap<String, Vec<&Triple>> = BTreeMap::new();
    for t in &triples {
        let key = match &t.subject {
            Subject::Iri(iri) => iri.as_str().to_string(),
            Subject::Blank(b) => format!("_:{b}"),
        };
        by_subject.entry(key).or_default().push(t);
    }

    let mut graph = Vec::new();
    for (subj_key, ts) in by_subject {
        let mut obj = Map::new();
        obj.insert("@id".into(), json!(subj_key));
        for t in ts {
            let pred_key = t.predicate.as_str();
            let value = object_to_json(&t.object);
            obj.entry(pred_key.to_string()).or_insert_with(|| json!([])).as_array_mut().unwrap().push(value);
        }
        graph.push(Value::Object(obj));
    }

    let doc = json!({
        "@context": Value::Object(context),
        "@graph": graph,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

fn object_to_json(o: &Object) -> Value {
    match o {
        Object::Iri(iri) => json!({ "@id": iri.as_str() }),
        Object::Blank(label) => json!({ "@id": format!("_:{label}") }),
        Object::Literal { lexical, datatype, language } => {
            let mut m = Map::new();
            m.insert("@value".into(), json!(lexical));
            if let Some(lang) = language {
                m.insert("@language".into(), json!(lang));
            } else {
                m.insert("@type".into(), json!(datatype.as_str()));
            }
            Value::Object(m)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{
        schema::{Cardinality, NodeType, PropertyType},
        Datatype, Iri, Node,
    };

    #[test]
    fn produces_graph_doc() {
        let mut o = Ontology::new();
        o.schema.declare_node_type(
            NodeType::new("Organization")
                .with_iri(Iri::from_unchecked("http://www.w3.org/ns/org#Organization"))
                .with_property(PropertyType {
                    name: "name".into(),
                    datatype: Datatype::String,
                    cardinality: Cardinality::ONE,
                    iri: None,
                    description: None,
                }),
        );
        o.upsert_node(
            Node::from_iri(Iri::from_unchecked("https://example.org/Acme"), "Organization")
                .with_property("name", "Acme"),
        );
        let doc: serde_json::Value = serde_json::from_str(&write(&o)).unwrap();
        assert!(doc.get("@graph").is_some());
        assert!(!doc["@graph"].as_array().unwrap().is_empty());
    }
}
