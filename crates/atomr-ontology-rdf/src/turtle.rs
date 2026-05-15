//! Turtle (TTL) writer.
//!
//! The output is correct but not pretty-printed: each triple sits on
//! its own line preceded by `@prefix` declarations expanded from the
//! ontology's vocabulary.

use atomr_ontology_core::namespace::Vocabulary;
use atomr_ontology_core::{Iri, Ontology};

use crate::adapter::to_rdf;
use crate::triple::{Object, Subject, Triple};

/// Serialize an ontology as Turtle.
pub fn write(ontology: &Ontology) -> String {
    let vocab = if ontology.vocabulary.iter().next().is_some() {
        ontology.vocabulary.clone()
    } else {
        Vocabulary::with_standard_bindings()
    };
    let mut out = String::new();
    for ns in vocab.iter() {
        out.push_str(&format!("@prefix {}: <{}> .\n", ns.prefix, ns.base.as_str()));
    }
    if !out.is_empty() {
        out.push('\n');
    }
    for t in to_rdf(ontology) {
        push_triple(&mut out, &vocab, &t);
        out.push_str(" .\n");
    }
    out
}

fn push_triple(out: &mut String, vocab: &Vocabulary, t: &Triple) {
    push_subject(out, vocab, &t.subject);
    out.push(' ');
    push_iri(out, vocab, &t.predicate);
    out.push(' ');
    push_object(out, vocab, &t.object);
}

fn push_subject(out: &mut String, vocab: &Vocabulary, s: &Subject) {
    match s {
        Subject::Iri(iri) => push_iri(out, vocab, iri),
        Subject::Blank(label) => {
            out.push_str("_:");
            out.push_str(label);
        }
    }
}

fn push_object(out: &mut String, vocab: &Vocabulary, o: &Object) {
    match o {
        Object::Iri(iri) => push_iri(out, vocab, iri),
        Object::Blank(label) => {
            out.push_str("_:");
            out.push_str(label);
        }
        Object::Literal { lexical, datatype, language } => {
            out.push('"');
            for c in lexical.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c => out.push(c),
                }
            }
            out.push('"');
            if let Some(lang) = language {
                out.push('@');
                out.push_str(lang);
            } else if datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                out.push_str("^^");
                push_iri(out, vocab, datatype);
            }
        }
    }
}

fn push_iri(out: &mut String, vocab: &Vocabulary, iri: &Iri) {
    for ns in vocab.iter() {
        if let Some(local) = iri.as_str().strip_prefix(ns.base.as_str()) {
            if !local.contains(' ') && !local.contains('/') {
                out.push_str(&ns.prefix);
                out.push(':');
                out.push_str(local);
                return;
            }
        }
    }
    out.push('<');
    out.push_str(iri.as_str());
    out.push('>');
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{
        schema::{Cardinality, NodeType, PropertyType},
        Datatype, Iri, Node,
    };

    #[test]
    fn writes_prefix_header_and_triples() {
        let mut o = Ontology::new();
        o.vocabulary = Vocabulary::with_standard_bindings();
        o.schema.declare_node_type(
            NodeType::new("Organization")
                .with_iri(Iri::from_unchecked("http://www.w3.org/ns/org#Organization"))
                .with_property(PropertyType {
                    name: "name".into(),
                    datatype: Datatype::String,
                    cardinality: Cardinality::ONE,
                    iri: Some(Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#label")),
                    description: None,
                }),
        );
        o.upsert_node(
            Node::from_iri(Iri::from_unchecked("https://example.org/Acme"), "Organization")
                .with_property("name", "Acme"),
        );
        let ttl = write(&o);
        assert!(ttl.contains("@prefix rdf:"));
        assert!(ttl.contains("org:Organization"));
        assert!(ttl.contains("\"Acme\""));
    }
}
