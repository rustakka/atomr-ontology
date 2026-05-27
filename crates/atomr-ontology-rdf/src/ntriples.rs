//! N-Triples writer + parser (RFC 7991-style).
//!
//! The parser is intentionally minimal: it accepts well-formed
//! N-Triples (one triple per line, IRIs in angle brackets, blank nodes
//! with `_:` prefix, typed/lang literals in canonical form), tolerates
//! `#`-prefixed comments and blank lines, and returns an
//! [`AdapterError`](crate::AdapterError) on hard parse failures.

use atomr_ontology_core::{Iri, Ontology};

use crate::adapter::{from_rdf, to_rdf, AdapterError};
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

/// Parse an N-Triples document into a vector of triples.
///
/// Empty lines and lines starting with `#` are skipped. Returns an
/// error on the first malformed triple, with a 1-based line number.
pub fn parse(input: &str) -> Result<Vec<Triple>, AdapterError> {
    let mut out = Vec::new();
    for (lineno, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_triple(line) {
            Ok(t) => out.push(t),
            Err(e) => return Err(AdapterError::Parse(format!("line {}: {}", lineno + 1, e))),
        }
    }
    Ok(out)
}

/// Parse an N-Triples document directly into an [`Ontology`] via
/// [`from_rdf`].
pub fn read(input: &str) -> Result<Ontology, AdapterError> {
    let triples = parse(input)?;
    from_rdf(&triples)
}

fn parse_triple(line: &str) -> Result<Triple, String> {
    // Strip trailing `.` plus surrounding whitespace.
    let line = line.trim_end_matches(|c: char| c.is_whitespace());
    let line = line.strip_suffix('.').ok_or_else(|| "missing trailing dot".to_string())?;
    let line = line.trim();

    let (subject, rest) = parse_term(line)?;
    let (predicate_obj, rest) = parse_term(rest.trim_start())?;
    let predicate = match predicate_obj {
        Term::Iri(iri) => iri,
        _ => return Err("predicate must be an IRI".into()),
    };
    let (object_term, rest) = parse_term(rest.trim_start())?;
    if !rest.trim().is_empty() {
        return Err(format!("trailing data after object: {rest:?}"));
    }
    let subject = match subject {
        Term::Iri(iri) => Subject::Iri(iri),
        Term::Blank(label) => Subject::Blank(label),
        Term::Literal { .. } => return Err("subject cannot be a literal".into()),
    };
    let object = match object_term {
        Term::Iri(iri) => Object::Iri(iri),
        Term::Blank(label) => Object::Blank(label),
        Term::Literal { lexical, datatype, language } => Object::Literal { lexical, datatype, language },
    };
    Ok(Triple { subject, predicate, object })
}

/// Internal term abstraction used by the N-Triples parser. Public to
/// the crate so `turtle::parse` can reuse the same building blocks.
pub(crate) enum Term {
    Iri(Iri),
    Blank(String),
    Literal { lexical: String, datatype: Iri, language: Option<String> },
}

pub(crate) fn parse_term(input: &str) -> Result<(Term, &str), String> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return Err("expected term, got end of input".into());
    }
    match bytes[0] {
        b'<' => {
            let end = input.find('>').ok_or_else(|| "unterminated IRI".to_string())?;
            let iri = &input[1..end];
            Ok((Term::Iri(Iri::from_unchecked(iri.to_string())), &input[end + 1..]))
        }
        b'_' if bytes.len() > 1 && bytes[1] == b':' => {
            let rest = &input[2..];
            let split = rest
                .find(|c: char| c.is_whitespace() || c == '.')
                .unwrap_or(rest.len());
            let label = &rest[..split];
            Ok((Term::Blank(label.to_string()), &rest[split..]))
        }
        b'"' => parse_literal(input),
        _ => Err(format!("unexpected term start: {:?}", &input.chars().next())),
    }
}

fn parse_literal(input: &str) -> Result<(Term, &str), String> {
    let bytes = input.as_bytes();
    let mut i = 1; // skip opening quote
    let mut lexical = String::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            i += 1;
            if i >= bytes.len() {
                return Err("dangling escape in literal".into());
            }
            match bytes[i] {
                b'n' => lexical.push('\n'),
                b'r' => lexical.push('\r'),
                b't' => lexical.push('\t'),
                b'"' => lexical.push('"'),
                b'\\' => lexical.push('\\'),
                b'u' => {
                    if i + 4 >= bytes.len() {
                        return Err("short \\u escape".into());
                    }
                    let hex = &input[i + 1..i + 5];
                    let cp = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                    if let Some(c) = char::from_u32(cp) {
                        lexical.push(c);
                    }
                    i += 4;
                }
                b'U' => {
                    if i + 8 >= bytes.len() {
                        return Err("short \\U escape".into());
                    }
                    let hex = &input[i + 1..i + 9];
                    let cp = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                    if let Some(c) = char::from_u32(cp) {
                        lexical.push(c);
                    }
                    i += 8;
                }
                other => return Err(format!("unknown escape \\{}", other as char)),
            }
            i += 1;
        } else if b == b'"' {
            i += 1; // skip closing quote
            // Look for ^^ (datatype) or @lang.
            let mut language = None;
            let mut datatype = Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#string");
            let rest = &input[i..];
            if let Some(lang_rest) = rest.strip_prefix('@') {
                let end = lang_rest
                    .find(|c: char| c.is_whitespace() || c == '.')
                    .unwrap_or(lang_rest.len());
                language = Some(lang_rest[..end].to_string());
                datatype = Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString");
                return Ok((Term::Literal { lexical, datatype, language }, &lang_rest[end..]));
            } else if let Some(dt_rest) = rest.strip_prefix("^^") {
                let dt_rest = dt_rest.trim_start();
                let dt_bytes = dt_rest.as_bytes();
                if dt_bytes.first() != Some(&b'<') {
                    return Err("expected <iri> after ^^".into());
                }
                let end = dt_rest.find('>').ok_or_else(|| "unterminated datatype IRI".to_string())?;
                datatype = Iri::from_unchecked(dt_rest[1..end].to_string());
                return Ok((Term::Literal { lexical, datatype, language }, &dt_rest[end + 1..]));
            }
            return Ok((Term::Literal { lexical, datatype, language }, rest));
        } else {
            lexical.push(b as char);
            i += 1;
        }
    }
    Err("unterminated literal".into())
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

    #[test]
    fn parses_simple_triples() {
        let nt = "<https://example.org/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Class> .\n";
        let triples = parse(nt).unwrap();
        assert_eq!(triples.len(), 1);
    }

    #[test]
    fn parses_literal_with_datatype() {
        let nt = r#"<https://example.org/Acme> <http://example.org/name> "Acme"^^<http://www.w3.org/2001/XMLSchema#string> ."#;
        let triples = parse(nt).unwrap();
        assert_eq!(triples.len(), 1);
        match &triples[0].object {
            Object::Literal { lexical, .. } => assert_eq!(lexical, "Acme"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn parses_literal_with_language() {
        let nt = r#"<https://example.org/Acme> <http://example.org/label> "Acme"@en ."#;
        let triples = parse(nt).unwrap();
        match &triples[0].object {
            Object::Literal { language, .. } => assert_eq!(language.as_deref(), Some("en")),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn parses_blank_node_subject() {
        let nt = "_:b0 <http://example.org/p> <http://example.org/o> .\n";
        let triples = parse(nt).unwrap();
        assert!(matches!(triples[0].subject, Subject::Blank(_)));
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
        let nt = write(&o);
        let triples = parse(&nt).unwrap();
        assert!(!triples.is_empty());
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let nt = "# a comment\n\n<https://example.org/x> <http://example.org/p> <http://example.org/o> .\n# trailing\n";
        let triples = parse(nt).unwrap();
        assert_eq!(triples.len(), 1);
    }
}
