//! Turtle (TTL) writer.
//!
//! The output is correct but not pretty-printed: each triple sits on
//! its own line preceded by `@prefix` declarations expanded from the
//! ontology's vocabulary.

use atomr_ontology_core::namespace::Vocabulary;
use atomr_ontology_core::{Iri, Ontology};

use crate::adapter::{from_rdf, to_rdf, AdapterError};
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

/// Parse a Turtle document into a triple stream.
///
/// Supports the Turtle subset emitted by [`write`]: `@prefix`
/// directives, IRI-or-CURIE subjects, predicate-object triples
/// separated by ` ` and terminated by ` .`, blank-node literals,
/// typed literals (`"x"^^xsd:string`), and language-tagged literals
/// (`"x"@en`). Predicate-object lists (`;`) and object lists (`,`)
/// are also supported. Multiline content within triples is allowed;
/// comments start with `#`.
pub fn parse(input: &str) -> Result<Vec<Triple>, AdapterError> {
    let mut p = Parser::new(input);
    p.parse_directives()?;
    let mut triples = Vec::new();
    while !p.eof() {
        p.skip_ws();
        if p.eof() {
            break;
        }
        p.parse_triples(&mut triples)?;
    }
    Ok(triples)
}

/// Parse a Turtle document directly into an [`Ontology`] via
/// [`from_rdf`].
pub fn read(input: &str) -> Result<Ontology, AdapterError> {
    let triples = parse(input)?;
    from_rdf(&triples)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    vocab: Vocabulary,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0, vocab: Vocabulary::new() }
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    fn skip_ws(&mut self) {
        loop {
            let rest = self.rest();
            let mut consumed = 0;
            for (i, c) in rest.char_indices() {
                if c.is_whitespace() {
                    consumed = i + c.len_utf8();
                    continue;
                }
                if c == '#' {
                    let nl = rest[i..].find('\n').map(|p| i + p + 1).unwrap_or(rest.len());
                    consumed = nl;
                    continue;
                }
                consumed = i;
                break;
            }
            // If we never broke, consumed includes the trailing whitespace.
            self.advance(consumed);
            let r2 = self.rest();
            if r2.is_empty() {
                return;
            }
            let first = r2.chars().next().unwrap();
            if !first.is_whitespace() && first != '#' {
                return;
            }
        }
    }

    fn parse_directives(&mut self) -> Result<(), AdapterError> {
        loop {
            self.skip_ws();
            if !self.rest().starts_with("@prefix") {
                return Ok(());
            }
            self.advance("@prefix".len());
            self.skip_ws();
            let prefix_end = self.rest().find(':').ok_or_else(|| AdapterError::Parse("missing ':' in @prefix".into()))?;
            let prefix = self.rest()[..prefix_end].trim().to_string();
            self.advance(prefix_end + 1);
            self.skip_ws();
            if !self.rest().starts_with('<') {
                return Err(AdapterError::Parse("expected <iri> after @prefix".into()));
            }
            self.advance(1);
            let close = self.rest().find('>').ok_or_else(|| AdapterError::Parse("unterminated @prefix IRI".into()))?;
            let base = self.rest()[..close].to_string();
            self.advance(close + 1);
            self.skip_ws();
            if !self.rest().starts_with('.') {
                return Err(AdapterError::Parse("expected '.' after @prefix".into()));
            }
            self.advance(1);
            self.vocab.bind(prefix, Iri::from_unchecked(base));
        }
    }

    fn parse_triples(&mut self, out: &mut Vec<Triple>) -> Result<(), AdapterError> {
        let subject = self.parse_subject()?;
        loop {
            self.skip_ws();
            let predicate = self.parse_predicate()?;
            loop {
                self.skip_ws();
                let object = self.parse_object()?;
                out.push(Triple { subject: subject.clone(), predicate: predicate.clone(), object });
                self.skip_ws();
                if self.rest().starts_with(',') {
                    self.advance(1);
                    continue;
                }
                break;
            }
            self.skip_ws();
            if self.rest().starts_with(';') {
                self.advance(1);
                continue;
            }
            if self.rest().starts_with('.') {
                self.advance(1);
                return Ok(());
            }
            return Err(AdapterError::Parse(format!("expected ';' or '.' at: {:?}", self.peek_snippet())));
        }
    }

    fn peek_snippet(&self) -> String {
        let end = self.rest().char_indices().take(40).map(|(i, c)| i + c.len_utf8()).last().unwrap_or(0);
        self.rest()[..end].to_string()
    }

    fn parse_subject(&mut self) -> Result<Subject, AdapterError> {
        let token = self.parse_iri_or_curie_or_blank()?;
        match token {
            ParsedTerm::Iri(iri) => Ok(Subject::Iri(iri)),
            ParsedTerm::Blank(label) => Ok(Subject::Blank(label)),
            ParsedTerm::Literal { .. } => Err(AdapterError::Parse("subject cannot be a literal".into())),
        }
    }

    fn parse_predicate(&mut self) -> Result<Iri, AdapterError> {
        if self.rest().starts_with("a") {
            let after = &self.rest()[1..];
            if after.starts_with(|c: char| c.is_whitespace()) {
                self.advance(1);
                return Ok(Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type".to_string()));
            }
        }
        let token = self.parse_iri_or_curie_or_blank()?;
        match token {
            ParsedTerm::Iri(iri) => Ok(iri),
            _ => Err(AdapterError::Parse("predicate must be an IRI".into())),
        }
    }

    fn parse_object(&mut self) -> Result<Object, AdapterError> {
        if self.rest().starts_with('"') {
            return self.parse_literal();
        }
        let token = self.parse_iri_or_curie_or_blank()?;
        Ok(match token {
            ParsedTerm::Iri(iri) => Object::Iri(iri),
            ParsedTerm::Blank(label) => Object::Blank(label),
            ParsedTerm::Literal { lexical, datatype, language } => Object::Literal { lexical, datatype, language },
        })
    }

    fn parse_iri_or_curie_or_blank(&mut self) -> Result<ParsedTerm, AdapterError> {
        let rest = self.rest();
        if rest.starts_with('<') {
            let close = rest.find('>').ok_or_else(|| AdapterError::Parse("unterminated IRI".into()))?;
            let iri = rest[1..close].to_string();
            self.advance(close + 1);
            return Ok(ParsedTerm::Iri(Iri::from_unchecked(iri)));
        }
        if let Some(after) = rest.strip_prefix("_:") {
            let end = after.find(|c: char| c.is_whitespace() || matches!(c, '.' | ';' | ',')).unwrap_or(after.len());
            let label = after[..end].to_string();
            self.advance(2 + end);
            return Ok(ParsedTerm::Blank(label));
        }
        // CURIE: prefix:local — read until whitespace or punctuator.
        let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '.' | ';' | ',')).unwrap_or(rest.len());
        let token = &rest[..end];
        let Some(colon) = token.find(':') else {
            return Err(AdapterError::Parse(format!("unrecognized token {token:?}")));
        };
        let prefix = &token[..colon];
        let local = &token[colon + 1..];
        let iri = if let Some(ns) = self.vocab.iter().find(|n| n.prefix == prefix) {
            Iri::from_unchecked(format!("{}{}", ns.base.as_str(), local))
        } else {
            return Err(AdapterError::Parse(format!("unknown prefix {prefix:?}")));
        };
        self.advance(end);
        Ok(ParsedTerm::Iri(iri))
    }

    fn parse_literal(&mut self) -> Result<Object, AdapterError> {
        let rest = self.rest();
        let bytes = rest.as_bytes();
        let mut i = 1;
        let mut lexical = String::new();
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\\' {
                i += 1;
                if i >= bytes.len() {
                    return Err(AdapterError::Parse("dangling escape".into()));
                }
                match bytes[i] {
                    b'n' => lexical.push('\n'),
                    b'r' => lexical.push('\r'),
                    b't' => lexical.push('\t'),
                    b'"' => lexical.push('"'),
                    b'\\' => lexical.push('\\'),
                    other => lexical.push(other as char),
                }
                i += 1;
            } else if b == b'"' {
                self.advance(i + 1);
                let mut datatype = Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#string");
                let mut language = None;
                if self.rest().starts_with('@') {
                    self.advance(1);
                    let lang_end = self
                        .rest()
                        .find(|c: char| c.is_whitespace() || matches!(c, '.' | ';' | ','))
                        .unwrap_or(self.rest().len());
                    language = Some(self.rest()[..lang_end].to_string());
                    datatype = Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString");
                    self.advance(lang_end);
                } else if self.rest().starts_with("^^") {
                    self.advance(2);
                    let dt = self.parse_iri_or_curie_or_blank()?;
                    if let ParsedTerm::Iri(iri) = dt {
                        datatype = iri;
                    }
                }
                return Ok(Object::Literal { lexical, datatype, language });
            } else {
                lexical.push(b as char);
                i += 1;
            }
        }
        Err(AdapterError::Parse("unterminated literal".into()))
    }
}

enum ParsedTerm {
    Iri(Iri),
    Blank(String),
    #[allow(dead_code)]
    Literal { lexical: String, datatype: Iri, language: Option<String> },
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

    #[test]
    fn parses_simple_turtle() {
        let ttl = "@prefix ex: <http://example.org/> .\n\nex:Acme <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ex:Org .\n";
        let triples = parse(ttl).unwrap();
        assert_eq!(triples.len(), 1);
    }

    #[test]
    fn parses_literal_object() {
        let ttl = "@prefix ex: <http://example.org/> .\nex:Acme ex:name \"Acme\" .\n";
        let triples = parse(ttl).unwrap();
        match &triples[0].object {
            Object::Literal { lexical, .. } => assert_eq!(lexical, "Acme"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn parses_predicate_object_list() {
        let ttl = "@prefix ex: <http://example.org/> .\nex:Acme a ex:Org ; ex:name \"Acme\" , \"ACME\" .\n";
        let triples = parse(ttl).unwrap();
        assert_eq!(triples.len(), 3);
    }

    #[test]
    fn round_trip_via_write_and_parse() {
        let mut o = Ontology::new();
        o.vocabulary = Vocabulary::with_standard_bindings();
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
        let ttl = write(&o);
        let triples = parse(&ttl).unwrap();
        assert!(!triples.is_empty());
    }
}
