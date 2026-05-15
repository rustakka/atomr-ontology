//! N-Triples writer (RFC 7991-style).

use atomr_ontology_core::Ontology;

use crate::adapter::to_rdf;
use crate::triple::{Object, Subject, Triple};

/// Serialize an ontology as N-Triples.
pub fn write(ontology: &Ontology) -> String {
    let mut out = String::new();
    for t in to_rdf(ontology) {
        push_triple(&mut out, &t);
        out.push('\n');
    }
    out
}

fn push_triple(out: &mut String, t: &Triple) {
    push_subject(out, &t.subject);
    out.push(' ');
    out.push('<');
    out.push_str(t.predicate.as_str());
    out.push('>');
    out.push(' ');
    push_object(out, &t.object);
    out.push_str(" .");
}

fn push_subject(out: &mut String, s: &Subject) {
    match s {
        Subject::Iri(iri) => {
            out.push('<');
            out.push_str(iri.as_str());
            out.push('>');
        }
        Subject::Blank(label) => {
            out.push_str("_:");
            out.push_str(label);
        }
    }
}

fn push_object(out: &mut String, o: &Object) {
    match o {
        Object::Iri(iri) => {
            out.push('<');
            out.push_str(iri.as_str());
            out.push('>');
        }
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
            } else {
                out.push_str("^^<");
                out.push_str(datatype.as_str());
                out.push('>');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{
        schema::{Cardinality, NodeType, PropertyType},
        Datatype, Iri, Node, Ontology,
    };

    #[test]
    fn writes_some_triples() {
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
        let nt = write(&o);
        assert!(nt.contains("<https://example.org/Acme>"));
        assert!(nt.contains("\"Acme\""));
        assert!(nt.contains("#type"));
    }
}
