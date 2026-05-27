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
use atomr_ontology_core::{Iri, Ontology};

use crate::adapter::{from_rdf, to_rdf, AdapterError};
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

/// Parse a JSON-LD document of the form produced by [`write`] back
/// into a triple stream.
///
/// The parser supports the subset of JSON-LD that this crate emits:
/// a top-level `@context` mapping prefixes to base IRIs plus a top-
/// level `@graph` array of subject objects. Each subject object has
/// `@id` plus predicate keys (either compact CURIEs or full IRIs)
/// whose values are arrays of `{ "@id": ... }` or `{ "@value": ...,
/// "@type": ... | "@language": ... }`. Unrecognized constructs are
/// silently skipped.
pub fn parse(input: &str) -> Result<Vec<Triple>, AdapterError> {
    let doc: Value = serde_json::from_str(input).map_err(|e| AdapterError::Parse(e.to_string()))?;
    let mut vocab = Vocabulary::new();
    if let Some(ctx) = doc.get("@context").and_then(|c| c.as_object()) {
        for (prefix, base) in ctx {
            if let Some(base_str) = base.as_str() {
                vocab.bind(prefix, Iri::from_unchecked(base_str.to_string()));
            }
        }
    }
    let mut triples = Vec::new();
    let Some(graph) = doc.get("@graph").and_then(|g| g.as_array()) else {
        return Ok(triples);
    };
    for node in graph {
        let Some(node_obj) = node.as_object() else { continue };
        let Some(id_val) = node_obj.get("@id").and_then(|i| i.as_str()) else { continue };
        let subject = subject_from_id(id_val);
        for (key, values) in node_obj {
            if key == "@id" || key == "@type" {
                if key == "@type" {
                    for v in flatten(values) {
                        if let Some(s) = v.as_str() {
                            triples.push(Triple {
                                subject: subject.clone(),
                                predicate: expand_iri(s, &vocab),
                                object: Object::Iri(expand_iri(s, &vocab)),
                            });
                        }
                    }
                }
                continue;
            }
            let predicate = expand_iri(key, &vocab);
            for v in flatten(values) {
                if let Some(obj) = value_to_object(v, &vocab) {
                    triples.push(Triple { subject: subject.clone(), predicate: predicate.clone(), object: obj });
                }
            }
        }
    }
    Ok(triples)
}

/// Parse a JSON-LD document directly into an [`Ontology`] via
/// [`from_rdf`].
pub fn read(input: &str) -> Result<Ontology, AdapterError> {
    let triples = parse(input)?;
    from_rdf(&triples)
}

fn subject_from_id(id: &str) -> Subject {
    if let Some(label) = id.strip_prefix("_:") {
        Subject::Blank(label.to_string())
    } else {
        Subject::Iri(Iri::from_unchecked(id.to_string()))
    }
}

fn expand_iri(s: &str, vocab: &Vocabulary) -> Iri {
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("urn:") {
        return Iri::from_unchecked(s.to_string());
    }
    if let Some(iri) = vocab.expand_curie(s) {
        return iri;
    }
    Iri::from_unchecked(s.to_string())
}

fn flatten(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(arr) => arr.iter().collect(),
        _ => vec![v],
    }
}

fn value_to_object(value: &Value, vocab: &Vocabulary) -> Option<Object> {
    let obj = value.as_object()?;
    if let Some(id) = obj.get("@id").and_then(|v| v.as_str()) {
        return Some(match id.strip_prefix("_:") {
            Some(label) => Object::Blank(label.to_string()),
            None => Object::Iri(expand_iri(id, vocab)),
        });
    }
    if let Some(val) = obj.get("@value") {
        let lexical = match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let language = obj.get("@language").and_then(|l| l.as_str()).map(String::from);
        let datatype = if language.is_some() {
            Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString")
        } else if let Some(dt) = obj.get("@type").and_then(|t| t.as_str()) {
            expand_iri(dt, vocab)
        } else {
            Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#string")
        };
        return Some(Object::Literal { lexical, datatype, language });
    }
    None
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

    #[test]
    fn parses_minimal_doc() {
        let doc = r#"{
            "@context": { "ex": "http://example.org/" },
            "@graph": [
                {
                    "@id": "ex:Acme",
                    "http://example.org/name": [{"@value": "Acme"}]
                }
            ]
        }"#;
        let triples = parse(doc).unwrap();
        assert!(!triples.is_empty());
    }

    #[test]
    fn round_trip_via_write_and_parse() {
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
        let doc = write(&o);
        let triples = parse(&doc).unwrap();
        assert!(!triples.is_empty());
    }
}
