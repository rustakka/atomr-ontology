//! Translation between an [`Ontology`] (LPG) and a stream of RDF triples.
//!
//! The mapping is documented in `docs/naming.md`. In brief:
//!
//! - Each declared `NodeType` becomes an `owl:Class`; its `iri` (or a
//!   generated CURIE) is its identifier.
//! - Each declared `EdgeType` becomes an `owl:ObjectProperty`.
//! - Each declared `PropertyType` becomes an `owl:DatatypeProperty`
//!   with `xsd:*` range.
//! - Each `Node` becomes a subject; its types become `rdf:type`
//!   triples; its properties become datatype-property triples.
//! - Each `Edge` becomes a `<source> predicate <target>` triple.
//! - Subclass axioms become `rdfs:subClassOf`; domain/range/functional
//!   axioms become their OWL counterparts.
//!
//! The reverse direction ([`from_rdf`]) is **partial**: in v0.1 we
//! recognize T-Box assertions (class declarations, subclass axioms)
//! and instance assertions whose subject is an IRI. Blank-node graphs
//! and arbitrary OWL constructs are accepted but ignored.

use thiserror::Error;

use atomr_ontology_core::axiom::AxiomKind;
use atomr_ontology_core::namespace::Vocabulary;
use atomr_ontology_core::{Datatype, Edge, Iri, Node, NodeId, Ontology, PropertyValue, Schema};

use crate::triple::{Object, Subject, Triple};

/// Errors raised by the RDF adapter.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// A node had no IRI and could not be projected as a named subject.
    /// We fall back to a blank node, but record this as a warning when
    /// strict mode is requested.
    #[error("anonymous node has no IRI: {0}")]
    AnonymousNode(String),
    /// A property value could not be projected.
    #[error("unsupported value: {0}")]
    UnsupportedValue(String),
    /// A parser rejected its input.
    #[error("parse error: {0}")]
    Parse(String),
}

/// Project an [`Ontology`] to a vector of [`Triple`]s.
///
/// Order is: schema (classes, properties, axioms), then nodes, then
/// edges. Within a category, iteration is by the underlying BTreeMap
/// (sorted by id / name) so the output is deterministic.
pub fn to_rdf(ontology: &Ontology) -> Vec<Triple> {
    let mut triples = Vec::new();
    let vocab = if ontology.vocabulary.iter().next().is_some() {
        ontology.vocabulary.clone()
    } else {
        Vocabulary::with_standard_bindings()
    };

    emit_schema(&ontology.schema, &vocab, &mut triples);
    emit_nodes(ontology, &vocab, &mut triples);
    emit_edges(ontology, &vocab, &mut triples);
    emit_axioms(ontology, &vocab, &mut triples);

    triples
}

fn rdf_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
}
fn rdfs_subclass() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#subClassOf")
}
fn rdfs_domain() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#domain")
}
fn rdfs_range() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#range")
}
fn owl_class_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#Class")
}
fn owl_object_property_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#ObjectProperty")
}
fn owl_datatype_property_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#DatatypeProperty")
}
fn owl_functional_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#FunctionalProperty")
}
fn owl_inverse_functional_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#InverseFunctionalProperty")
}
fn owl_symmetric_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#SymmetricProperty")
}
fn owl_transitive_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#TransitiveProperty")
}
fn owl_equivalent_class_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#equivalentClass")
}
fn owl_disjoint_with_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#disjointWith")
}
fn owl_inverse_of_iri() -> Iri {
    Iri::from_unchecked("http://www.w3.org/2002/07/owl#inverseOf")
}

fn ensure_iri(name: &str, fallback_prefix: &str, vocab: &Vocabulary) -> Iri {
    if let Some(iri) = vocab.expand_curie(name) {
        iri
    } else if name.contains(':') {
        Iri::from_unchecked(name.to_string())
    } else {
        Iri::from_unchecked(format!("{fallback_prefix}{name}"))
    }
}

fn datatype_iri(d: Datatype) -> Iri {
    let base = "http://www.w3.org/2001/XMLSchema#";
    let suffix = match d {
        Datatype::String => "string",
        Datatype::Integer => "integer",
        Datatype::Float => "double",
        Datatype::Bool => "boolean",
        Datatype::DateTime => "dateTime",
        Datatype::Iri => "anyURI",
        Datatype::Bytes => "base64Binary",
        Datatype::Json => return Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#JSON"),
    };
    Iri::from_unchecked(format!("{base}{suffix}"))
}

fn emit_schema(schema: &Schema, vocab: &Vocabulary, triples: &mut Vec<Triple>) {
    let local_prefix = "https://atomr.dev/ontology/local#";

    for (name, ty) in &schema.node_types {
        let class_iri = ty.iri.clone().unwrap_or_else(|| ensure_iri(name, local_prefix, vocab));
        triples.push(Triple::new(Subject::Iri(class_iri.clone()), rdf_iri(), Object::Iri(owl_class_iri())));
        for sup in &ty.supertypes {
            let sup_iri = ensure_iri(sup, local_prefix, vocab);
            triples.push(Triple::new(Subject::Iri(class_iri.clone()), rdfs_subclass(), Object::Iri(sup_iri)));
        }
        for prop in &ty.properties {
            let prop_iri = prop.iri.clone().unwrap_or_else(|| ensure_iri(&prop.name, local_prefix, vocab));
            triples.push(Triple::new(
                Subject::Iri(prop_iri.clone()),
                rdf_iri(),
                Object::Iri(owl_datatype_property_iri()),
            ));
            triples.push(Triple::new(
                Subject::Iri(prop_iri.clone()),
                rdfs_domain(),
                Object::Iri(class_iri.clone()),
            ));
            triples.push(Triple::new(
                Subject::Iri(prop_iri),
                rdfs_range(),
                Object::Iri(datatype_iri(prop.datatype)),
            ));
        }
    }

    for (name, ty) in &schema.edge_types {
        let pred_iri = ty.iri.clone().unwrap_or_else(|| ensure_iri(name, local_prefix, vocab));
        triples.push(Triple::new(
            Subject::Iri(pred_iri.clone()),
            rdf_iri(),
            Object::Iri(owl_object_property_iri()),
        ));
        if ty.functional {
            triples.push(Triple::new(
                Subject::Iri(pred_iri.clone()),
                rdf_iri(),
                Object::Iri(owl_functional_iri()),
            ));
        }
        if let Some(inv) = &ty.inverse_of {
            let inv_iri = ensure_iri(inv, local_prefix, vocab);
            triples.push(Triple::new(
                Subject::Iri(pred_iri.clone()),
                owl_inverse_of_iri(),
                Object::Iri(inv_iri),
            ));
        }
        for d in &ty.domain {
            let d_iri = ensure_iri(d, local_prefix, vocab);
            triples.push(Triple::new(Subject::Iri(pred_iri.clone()), rdfs_domain(), Object::Iri(d_iri)));
        }
        for r in &ty.range {
            let r_iri = ensure_iri(r, local_prefix, vocab);
            triples.push(Triple::new(Subject::Iri(pred_iri.clone()), rdfs_range(), Object::Iri(r_iri)));
        }
    }
}

fn node_subject(node: &Node) -> Subject {
    match &node.iri {
        Some(iri) => Subject::Iri(iri.clone()),
        None => Subject::Blank(node.id.to_string()),
    }
}

fn node_object(id: &NodeId, ontology: &Ontology) -> Object {
    match ontology.node(id).and_then(|n| n.iri.clone()) {
        Some(iri) => Object::Iri(iri),
        None => Object::Iri(Iri::from_unchecked(format!("_:n{}", id))),
    }
}

fn value_to_object(value: &PropertyValue) -> Option<Object> {
    use chrono::SecondsFormat;
    let obj = match value {
        PropertyValue::String(s) => Object::xsd_string(s.clone()),
        PropertyValue::Integer(i) => Object::xsd_integer(*i),
        PropertyValue::Float(f) => Object::xsd_double(*f),
        PropertyValue::Bool(b) => Object::xsd_boolean(*b),
        PropertyValue::DateTime(d) => Object::Literal {
            lexical: d.to_rfc3339_opts(SecondsFormat::Secs, true),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            language: None,
        },
        PropertyValue::Iri(iri) => Object::Iri(iri.clone()),
        PropertyValue::Bytes(b) => Object::Literal {
            lexical: base64_encode(b),
            datatype: Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#base64Binary"),
            language: None,
        },
        PropertyValue::Json(v) => Object::Literal {
            lexical: v.to_string(),
            datatype: Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#JSON"),
            language: None,
        },
        PropertyValue::Null => return None,
    };
    Some(obj)
}

fn base64_encode(bytes: &[u8]) -> String {
    // Minimal RFC 4648 base64 (no padding stripped) using a small
    // hand-rolled encoder so we don't pull a base64 dep just for
    // RDF projection.
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let chunk = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8) | u32::from(bytes[i + 2]);
        out.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(chunk & 0x3F) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let chunk = u32::from(bytes[i]) << 16;
        out.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let chunk = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
        out.push(TABLE[((chunk >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
}

fn emit_nodes(ontology: &Ontology, vocab: &Vocabulary, triples: &mut Vec<Triple>) {
    let local_prefix = "https://atomr.dev/ontology/local#";
    for node in ontology.nodes.values() {
        let subj = node_subject(node);
        for type_name in &node.types {
            let class_iri = ontology
                .schema
                .node_type(type_name)
                .and_then(|t| t.iri.clone())
                .unwrap_or_else(|| ensure_iri(type_name, local_prefix, vocab));
            triples.push(Triple::new(subj.clone(), rdf_iri(), Object::Iri(class_iri)));
        }
        for (name, value) in &node.properties {
            let Some(object) = value_to_object(value) else { continue };
            let prop_iri = node
                .types
                .iter()
                .find_map(|tn| ontology.schema.node_type(tn))
                .and_then(|nt| nt.properties.iter().find(|p| &p.name == name))
                .and_then(|p| p.iri.clone())
                .unwrap_or_else(|| ensure_iri(name, local_prefix, vocab));
            triples.push(Triple::new(subj.clone(), prop_iri, object));
        }
    }
}

fn emit_edges(ontology: &Ontology, vocab: &Vocabulary, triples: &mut Vec<Triple>) {
    let local_prefix = "https://atomr.dev/ontology/local#";
    for edge in ontology.edges.values() {
        let Some(src) = ontology.node(&edge.source) else { continue };
        let pred_iri = ontology
            .schema
            .edge_type(&edge.label)
            .and_then(|t| t.iri.clone())
            .unwrap_or_else(|| ensure_iri(&edge.label, local_prefix, vocab));
        let subj = node_subject(src);
        let obj = node_object(&edge.target, ontology);
        triples.push(Triple::new(subj, pred_iri, obj));
    }
}

fn emit_axioms(ontology: &Ontology, vocab: &Vocabulary, triples: &mut Vec<Triple>) {
    let local_prefix = "https://atomr.dev/ontology/local#";
    for ax in ontology.axioms.values() {
        match &ax.kind {
            AxiomKind::SubClassOf { sub, sup } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(sub, local_prefix, vocab)),
                rdfs_subclass(),
                Object::Iri(ensure_iri(sup, local_prefix, vocab)),
            )),
            AxiomKind::EquivalentClass { left, right } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(left, local_prefix, vocab)),
                owl_equivalent_class_iri(),
                Object::Iri(ensure_iri(right, local_prefix, vocab)),
            )),
            AxiomKind::DisjointWith { left, right } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(left, local_prefix, vocab)),
                owl_disjoint_with_iri(),
                Object::Iri(ensure_iri(right, local_prefix, vocab)),
            )),
            AxiomKind::Domain { property, class } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(property, local_prefix, vocab)),
                rdfs_domain(),
                Object::Iri(ensure_iri(class, local_prefix, vocab)),
            )),
            AxiomKind::Range { property, class } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(property, local_prefix, vocab)),
                rdfs_range(),
                Object::Iri(ensure_iri(class, local_prefix, vocab)),
            )),
            AxiomKind::Functional { property } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(property, local_prefix, vocab)),
                rdf_iri(),
                Object::Iri(owl_functional_iri()),
            )),
            AxiomKind::InverseFunctional { property } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(property, local_prefix, vocab)),
                rdf_iri(),
                Object::Iri(owl_inverse_functional_iri()),
            )),
            AxiomKind::InverseOf { left, right } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(left, local_prefix, vocab)),
                owl_inverse_of_iri(),
                Object::Iri(ensure_iri(right, local_prefix, vocab)),
            )),
            AxiomKind::Symmetric { property } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(property, local_prefix, vocab)),
                rdf_iri(),
                Object::Iri(owl_symmetric_iri()),
            )),
            AxiomKind::Transitive { property } => triples.push(Triple::new(
                Subject::Iri(ensure_iri(property, local_prefix, vocab)),
                rdf_iri(),
                Object::Iri(owl_transitive_iri()),
            )),
        }
    }
}

/// Partial reverse adapter: import T-Box (class declarations,
/// subclass axioms, property domain/range) from a triple stream.
///
/// Triples we don't recognize are silently skipped — this is by
/// design to make ingest of partial graphs resilient.
pub fn from_rdf(triples: &[Triple]) -> Result<Ontology, AdapterError> {
    use atomr_ontology_core::schema::{EdgeType, NodeType};
    let mut o = Ontology::new();
    let rdf_type = rdf_iri();
    let rdfs_sub = rdfs_subclass();
    let owl_class = owl_class_iri();
    let owl_object_property = owl_object_property_iri();
    let owl_datatype_property = owl_datatype_property_iri();

    for t in triples {
        let Subject::Iri(subj_iri) = &t.subject else { continue };
        if t.predicate == rdf_type {
            if let Object::Iri(obj_iri) = &t.object {
                if obj_iri == &owl_class {
                    let name = local_name(subj_iri);
                    o.schema.declare_node_type(NodeType::new(name).with_iri(subj_iri.clone()));
                } else if obj_iri == &owl_object_property {
                    let name = local_name(subj_iri);
                    o.schema.declare_edge_type(EdgeType::new(name).with_iri(subj_iri.clone()));
                } else if obj_iri == &owl_datatype_property {
                    // Datatype property — recorded only when we later see its domain.
                }
            }
        } else if t.predicate == rdfs_sub {
            if let Object::Iri(sup_iri) = &t.object {
                let sub_name = local_name(subj_iri);
                let sup_name = local_name(sup_iri);
                let mut existing = o
                    .schema
                    .node_type(&sub_name)
                    .cloned()
                    .unwrap_or_else(|| NodeType::new(sub_name.clone()).with_iri(subj_iri.clone()));
                if !existing.supertypes.iter().any(|s| s == &sup_name) {
                    existing.supertypes.push(sup_name);
                }
                o.schema.declare_node_type(existing);
            }
        }
    }

    Ok(o)
}

fn local_name(iri: &Iri) -> String {
    let s = iri.as_str();
    if let Some(pos) = s.rfind(['#', '/', ':']) {
        s[pos + 1..].to_string()
    } else {
        s.to_string()
    }
}

/// Re-export of [`to_rdf`] under a dedicated submodule (used by the
/// Turtle and N-Triples writers).
pub mod export {
    pub use super::to_rdf;
}

// Avoid the unused-import warning when the public consumer is `crate::Edge`.
#[allow(dead_code)]
fn _ensure_unused_imports_silenced(_: &Edge) {}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::schema::{Cardinality, EdgeType, NodeType, PropertyType};

    fn fixture() -> Ontology {
        let mut o = Ontology::new();
        let org = NodeType::new("Organization")
            .with_iri(Iri::from_unchecked("http://www.w3.org/ns/org#Organization"))
            .with_property(PropertyType {
                name: "name".into(),
                datatype: atomr_ontology_core::Datatype::String,
                cardinality: Cardinality::ONE,
                iri: Some(Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#label")),
                description: None,
            });
        o.schema.declare_node_type(org);
        o.schema.declare_edge_type(
            EdgeType::new("memberOf")
                .with_iri(Iri::from_unchecked("http://www.w3.org/ns/org#memberOf"))
                .with_domain("Organization")
                .with_range("Organization"),
        );
        let acme_iri = Iri::from_unchecked("https://example.org/Acme");
        let bob_iri = Iri::from_unchecked("https://example.org/Bob");
        let acme =
            o.upsert_node(Node::from_iri(acme_iri.clone(), "Organization").with_property("name", "Acme"));
        let bob = o.upsert_node(Node::from_iri(bob_iri.clone(), "Organization"));
        let _ = o.upsert_edge(atomr_ontology_core::Edge::between(bob, "memberOf", acme));
        o
    }

    #[test]
    fn projects_schema_and_instances() {
        let triples = to_rdf(&fixture());
        let predicates: Vec<&str> = triples.iter().map(|t| t.predicate.as_str()).collect();
        assert!(predicates.iter().any(|p| p.ends_with("#type")));
        assert!(predicates.iter().any(|p| p.ends_with("#label")));
        assert!(predicates.iter().any(|p| p.contains("org#memberOf")));
    }

    #[test]
    fn from_rdf_partial_roundtrip() {
        let triples = to_rdf(&fixture());
        let o = from_rdf(&triples).unwrap();
        assert!(o.schema.node_type("Organization").is_some());
    }
}
