//! Hand-rolled recursive-descent parser for a tiny SPARQL subset.
//!
//! Supported grammar (informally):
//!
//! ```text
//! query     := prefixes select where solution_mods
//! prefixes  := ('PREFIX' pname_ns ':' '<' iri '>')*
//! select    := 'SELECT' ('*' | var+)
//! where     := 'WHERE' '{' triple ('.' triple)* '.'? '}'
//! triple    := term verb term
//! verb      := 'a' | iri | curie       // 'a' ⇒ rdf:type
//! term      := var | iri | curie
//! var       := '?' ident
//! curie     := pname_ns? ':' ident
//! iri       := '<' [^>]* '>'
//! solution_mods := ('LIMIT' uint)? ('OFFSET' uint)?
//!                | ('OFFSET' uint)? ('LIMIT' uint)?
//! ```
//!
//! ## Compilation to TraversalPlan
//!
//! Triples are translated as follows:
//! * `?s a :T`    — declare that `?s` carries type label `T` (or the
//!   suffix of an absolute IRI).
//! * `?s :p ?o`   — emit a hop from `?s` to `?o` labelled by the
//!   predicate's local name. The seed is the subject of the first
//!   triple; subsequent triples either further constrain a binding
//!   already seen, or extend the chain via the most recently bound
//!   subject.
//!
//! This is a deliberately narrow subset — enough to round-trip the
//! kinds of BGPs used by ontology smoke tests.

use std::collections::BTreeMap;

use atomr_ontology_store::{EdgePattern, NodePattern, TraversalPlan, TraversalStep};
use thiserror::Error;

/// Errors produced by [`parse`].
#[derive(Debug, Error)]
pub enum SparqlError {
    /// The input could not be tokenised / parsed.
    #[error("sparql parse error: {0}")]
    Parse(String),
    /// The grammar construct is recognised but not implemented in this
    /// subset.
    #[error("sparql unsupported feature: {0}")]
    Unsupported(String),
}

/// Parse a SPARQL-subset query into a [`TraversalPlan`].
pub fn parse(query: &str) -> Result<TraversalPlan, SparqlError> {
    let mut p = Parser::new(query);
    let plan = p.parse_query()?;
    p.skip_ws();
    if !p.eof() {
        return Err(SparqlError::Parse(format!(
            "unexpected trailing input at position {}: {:?}",
            p.pos,
            p.peek_rest_trim()
        )));
    }
    Ok(plan)
}

/// Internal parser state.
struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
    prefixes: BTreeMap<String, String>,
}

/// A parsed term occurring in a triple pattern.
#[derive(Debug, Clone)]
enum Term {
    /// A SPARQL variable (`?x`).
    Var(String),
    /// A literal IRI in angle brackets, or the local name of a CURIE
    /// expanded via a prefix.
    Iri(String),
    /// The unexpanded local name from a `:foo` or `prefix:foo` CURIE.
    /// We track these alongside `Iri` because in the absence of any
    /// matching `PREFIX`, the local name still serves as a label.
    Local(String),
}

impl Term {
    fn label(&self) -> Option<&str> {
        match self {
            Term::Iri(s) | Term::Local(s) => Some(s.as_str()),
            Term::Var(_) => None,
        }
    }

    fn as_var(&self) -> Option<&str> {
        if let Term::Var(v) = self {
            Some(v.as_str())
        } else {
            None
        }
    }
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
            prefixes: BTreeMap::new(),
        }
    }

    // ---- low-level utilities --------------------------------------

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_ws(&mut self) {
        loop {
            let mut moved = false;
            while let Some(b) = self.peek() {
                if b.is_ascii_whitespace() {
                    self.pos += 1;
                    moved = true;
                } else {
                    break;
                }
            }
            // Line comments `# ...` (SPARQL convention).
            if self.peek() == Some(b'#') {
                while let Some(b) = self.peek() {
                    self.pos += 1;
                    if b == b'\n' {
                        break;
                    }
                }
                moved = true;
            }
            if !moved {
                break;
            }
        }
    }

    fn peek_rest_trim(&self) -> String {
        let rest = &self.src[self.pos..];
        let s = std::str::from_utf8(rest).unwrap_or("<non-utf8>");
        let s = s.trim();
        if s.len() > 40 {
            format!("{}…", &s[..40])
        } else {
            s.to_string()
        }
    }

    fn try_keyword(&mut self, kw: &str) -> bool {
        let bytes = kw.as_bytes();
        if self.src.len() - self.pos < bytes.len() {
            return false;
        }
        let slice = &self.src[self.pos..self.pos + bytes.len()];
        if !slice.eq_ignore_ascii_case(bytes) {
            return false;
        }
        if let Some(&next) = self.src.get(self.pos + bytes.len()) {
            if is_ident_continue(next) {
                return false;
            }
        }
        self.pos += bytes.len();
        true
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<(), SparqlError> {
        self.skip_ws();
        if self.try_keyword(kw) {
            Ok(())
        } else {
            Err(SparqlError::Parse(format!(
                "expected keyword `{}` at position {} (got {:?})",
                kw,
                self.pos,
                self.peek_rest_trim()
            )))
        }
    }

    fn expect_char(&mut self, c: u8) -> Result<(), SparqlError> {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(SparqlError::Parse(format!(
                "expected `{}` at position {} (got {:?})",
                c as char,
                self.pos,
                self.peek_rest_trim()
            )))
        }
    }

    // ---- token parsers --------------------------------------------

    fn parse_ident(&mut self) -> Result<String, SparqlError> {
        self.skip_ws();
        let start = self.pos;
        match self.peek() {
            Some(b) if is_ident_start(b) => {
                self.pos += 1;
            }
            _ => {
                return Err(SparqlError::Parse(format!(
                    "expected identifier at position {} (got {:?})",
                    self.pos,
                    self.peek_rest_trim()
                )));
            }
        }
        while let Some(b) = self.peek() {
            if is_ident_continue(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let raw = &self.src[start..self.pos];
        Ok(String::from_utf8_lossy(raw).into_owned())
    }

    fn parse_uint(&mut self) -> Result<usize, SparqlError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(SparqlError::Parse(format!(
                "expected integer at position {} (got {:?})",
                self.pos,
                self.peek_rest_trim()
            )));
        }
        let raw = std::str::from_utf8(&self.src[start..self.pos])
            .map_err(|e| SparqlError::Parse(format!("utf-8 error in integer: {e}")))?;
        raw.parse::<usize>()
            .map_err(|e| SparqlError::Parse(format!("invalid integer `{raw}`: {e}")))
    }

    fn parse_var(&mut self) -> Result<String, SparqlError> {
        self.skip_ws();
        if self.peek() != Some(b'?') {
            return Err(SparqlError::Parse(format!(
                "expected `?var` at position {} (got {:?})",
                self.pos,
                self.peek_rest_trim()
            )));
        }
        self.pos += 1;
        self.parse_ident()
    }

    fn parse_iri_in_angles(&mut self) -> Result<String, SparqlError> {
        self.skip_ws();
        if self.peek() != Some(b'<') {
            return Err(SparqlError::Parse(format!(
                "expected `<iri>` at position {} (got {:?})",
                self.pos,
                self.peek_rest_trim()
            )));
        }
        self.pos += 1;
        let start = self.pos;
        loop {
            match self.bump() {
                Some(b'>') => {
                    let raw = &self.src[start..self.pos - 1];
                    return std::str::from_utf8(raw)
                        .map(str::to_owned)
                        .map_err(|e| SparqlError::Parse(format!("utf-8 in IRI: {e}")));
                }
                None => {
                    return Err(SparqlError::Parse("unterminated IRI".into()));
                }
                _ => {}
            }
        }
    }

    /// Try to parse a CURIE `prefix:local` or `:local`, expanding the
    /// prefix if known. Returns the term plus whether it came from a
    /// known prefix.
    fn try_parse_curie(&mut self) -> Option<Result<Term, SparqlError>> {
        let save = self.pos;
        self.skip_ws();
        let prefix_start = self.pos;
        // Parse optional prefix part.
        if let Some(b) = self.peek() {
            if is_ident_start(b) {
                while let Some(b2) = self.peek() {
                    if is_ident_continue(b2) {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        if self.peek() != Some(b':') {
            self.pos = save;
            return None;
        }
        let prefix = std::str::from_utf8(&self.src[prefix_start..self.pos])
            .unwrap_or("")
            .to_owned();
        self.pos += 1; // consume ':'
        // Parse local name.
        let local_start = self.pos;
        if let Some(b) = self.peek() {
            if is_ident_start(b) || b.is_ascii_digit() {
                self.pos += 1;
                while let Some(b2) = self.peek() {
                    if is_ident_continue(b2) || b2 == b'.' || b2 == b'-' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        if self.pos == local_start {
            // Empty local — only valid in declarations, not as a term.
            self.pos = save;
            return None;
        }
        let local = std::str::from_utf8(&self.src[local_start..self.pos])
            .unwrap_or("")
            .to_owned();
        if let Some(iri_base) = self.prefixes.get(&prefix) {
            Some(Ok(Term::Iri(format!("{iri_base}{local}"))))
        } else {
            Some(Ok(Term::Local(local)))
        }
    }

    fn parse_term(&mut self) -> Result<Term, SparqlError> {
        self.skip_ws();
        match self.peek() {
            Some(b'?') => Ok(Term::Var(self.parse_var()?)),
            Some(b'<') => Ok(Term::Iri(self.parse_iri_in_angles()?)),
            _ => {
                if let Some(res) = self.try_parse_curie() {
                    res
                } else {
                    Err(SparqlError::Parse(format!(
                        "expected term at position {} (got {:?})",
                        self.pos,
                        self.peek_rest_trim()
                    )))
                }
            }
        }
    }

    fn parse_verb(&mut self) -> Result<Term, SparqlError> {
        self.skip_ws();
        // The bare keyword `a` is shorthand for rdf:type.
        let save = self.pos;
        if self.try_keyword("a") {
            return Ok(Term::Local("a".into()));
        }
        self.pos = save;
        self.parse_term()
    }

    // ---- query parser ---------------------------------------------

    fn parse_prefixes(&mut self) -> Result<(), SparqlError> {
        loop {
            self.skip_ws();
            let save = self.pos;
            if !self.try_keyword("PREFIX") {
                return Ok(());
            }
            self.skip_ws();
            // Parse prefix name (possibly empty).
            let name_start = self.pos;
            while let Some(b) = self.peek() {
                if is_ident_continue(b) {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let prefix = std::str::from_utf8(&self.src[name_start..self.pos])
                .map(str::to_owned)
                .map_err(|e| SparqlError::Parse(format!("utf-8 in prefix: {e}")))?;
            if self.peek() != Some(b':') {
                // Roll back — not a PREFIX after all.
                self.pos = save;
                return Ok(());
            }
            self.pos += 1;
            let iri = self.parse_iri_in_angles()?;
            self.prefixes.insert(prefix, iri);
        }
    }

    fn parse_query(&mut self) -> Result<TraversalPlan, SparqlError> {
        self.parse_prefixes()?;
        self.expect_keyword("SELECT")?;
        self.skip_ws();
        let mut select_vars: Vec<String> = Vec::new();
        let mut select_star = false;
        if self.peek() == Some(b'*') {
            self.pos += 1;
            select_star = true;
        } else {
            loop {
                self.skip_ws();
                if self.peek() == Some(b'?') {
                    select_vars.push(self.parse_var()?);
                } else {
                    break;
                }
            }
            if select_vars.is_empty() {
                return Err(SparqlError::Parse(
                    "SELECT requires `*` or at least one `?var`".into(),
                ));
            }
        }
        self.expect_keyword("WHERE")?;
        self.expect_char(b'{')?;
        let triples = self.parse_triple_block()?;
        self.expect_char(b'}')?;
        // Optional LIMIT / OFFSET, in either order.
        let mut limit: Option<usize> = None;
        let mut offset: usize = 0;
        for _ in 0..2 {
            self.skip_ws();
            if self.try_keyword("LIMIT") {
                limit = Some(self.parse_uint()?);
            } else if self.try_keyword("OFFSET") {
                offset = self.parse_uint()?;
            } else {
                break;
            }
        }
        let mut plan = self.lower_triples(triples)?;
        if select_star {
            plan.return_columns.clear();
        } else {
            plan.return_columns = select_vars;
        }
        plan.skip = offset;
        plan.limit = limit;
        Ok(plan)
    }

    fn parse_triple_block(&mut self) -> Result<Vec<(Term, Term, Term)>, SparqlError> {
        let mut triples = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                return Ok(triples);
            }
            let s = self.parse_term()?;
            let p = self.parse_verb()?;
            let o = self.parse_term()?;
            triples.push((s, p, o));
            self.skip_ws();
            if self.peek() == Some(b'.') {
                self.pos += 1;
                continue;
            } else if self.peek() == Some(b'}') {
                return Ok(triples);
            } else {
                return Err(SparqlError::Parse(format!(
                    "expected `.` or `}}` after triple at position {} (got {:?})",
                    self.pos,
                    self.peek_rest_trim()
                )));
            }
        }
    }

    fn lower_triples(
        &self,
        triples: Vec<(Term, Term, Term)>,
    ) -> Result<TraversalPlan, SparqlError> {
        if triples.is_empty() {
            return Err(SparqlError::Parse(
                "WHERE block must contain at least one triple".into(),
            ));
        }
        // We compile triples one at a time. The first triple's subject
        // becomes the seed binding.
        let mut bindings: BTreeMap<String, BindingSite> = BTreeMap::new();
        let seed_var = match &triples[0].0 {
            Term::Var(v) => v.clone(),
            _ => {
                return Err(SparqlError::Unsupported(
                    "the first triple's subject must be a variable in this subset".into(),
                ));
            }
        };
        let seed = NodePattern::any().bind(&seed_var);
        let mut plan = TraversalPlan::from(seed);
        bindings.insert(seed_var.clone(), BindingSite::Seed);

        for (s, p, o) in triples.into_iter() {
            self.compile_triple(&mut plan, &mut bindings, s, p, o)?;
        }
        Ok(plan)
    }

    fn compile_triple(
        &self,
        plan: &mut TraversalPlan,
        bindings: &mut BTreeMap<String, BindingSite>,
        s: Term,
        p: Term,
        o: Term,
    ) -> Result<(), SparqlError> {
        let s_var = s.as_var().ok_or_else(|| {
            SparqlError::Unsupported(
                "triple subjects must be variables in this subset".into(),
            )
        })?;
        // Ensure subject is bound.
        if !bindings.contains_key(s_var) {
            return Err(SparqlError::Unsupported(format!(
                "subject `?{s_var}` was not introduced by the seed triple"
            )));
        }
        let predicate_label = match p.label() {
            Some(s) => s.to_string(),
            None => {
                return Err(SparqlError::Unsupported(
                    "predicate may not be a variable in this subset".into(),
                ));
            }
        };
        // Detect `?s a :Type` — type constraint on the subject.
        if predicate_label == "a" {
            let ty = match o.label() {
                Some(s) => label_localname(s).to_string(),
                None => {
                    return Err(SparqlError::Unsupported(
                        "`rdf:type` object must be an IRI or CURIE".into(),
                    ));
                }
            };
            attach_type_to_binding(plan, bindings, s_var, ty);
            return Ok(());
        }

        // Otherwise: emit an outbound edge hop.
        let edge_label = label_localname(&predicate_label).to_string();
        let edge = EdgePattern::any().labeled(edge_label);
        let target = match &o {
            Term::Var(v) => {
                let np = NodePattern::any().bind(v);
                // Record where the variable was first introduced; if
                // it was already bound, we leave the join semantics to
                // the executor and still emit the hop.
                bindings
                    .entry(v.clone())
                    .or_insert(BindingSite::Step(plan.steps.len()));
                np
            }
            Term::Iri(s) | Term::Local(s) => NodePattern::any().typed(label_localname(s).to_string()),
        };
        plan.steps.push(TraversalStep::outbound(edge, target));
        Ok(())
    }
}

/// Where a variable was first introduced in a plan.
enum BindingSite {
    Seed,
    Step(usize),
}

fn attach_type_to_binding(
    plan: &mut TraversalPlan,
    bindings: &BTreeMap<String, BindingSite>,
    var: &str,
    ty: String,
) {
    match bindings.get(var) {
        Some(BindingSite::Seed) => {
            plan.seed.types.push(ty);
        }
        Some(BindingSite::Step(idx)) => {
            if let Some(step) = plan.steps.get_mut(*idx) {
                step.target.types.push(ty);
            }
        }
        None => {
            // Should not happen if compile_triple checks pre-existence
            // first, but fall back to attaching to the seed.
            plan.seed.types.push(ty);
        }
    }
}

/// Extract the local name from an IRI: the suffix after the last `#`,
/// `/`, or `:`.
fn label_localname(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut cut = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'#' || b == b'/' || b == b':' {
            cut = i + 1;
        }
    }
    &s[cut..]
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

// =================================================================
// Tests
// =================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_select_where() {
        let plan = parse(
            "SELECT ?a ?b WHERE { ?a a :Org . ?a :memberOf ?b . } LIMIT 10",
        )
        .expect("parse");
        assert_eq!(plan.seed.bind.as_deref(), Some("a"));
        assert_eq!(plan.seed.types, vec!["Org".to_string()]);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].edge.label.as_deref(), Some("memberOf"));
        assert!(plan.steps[0].outbound);
        assert_eq!(plan.steps[0].target.bind.as_deref(), Some("b"));
        assert_eq!(plan.return_columns, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(plan.limit, Some(10));
    }

    #[test]
    fn expands_prefixes() {
        let q = r#"
            PREFIX ex: <http://example.com/>
            SELECT ?x WHERE { ?x a ex:Person . ?x ex:knows ?y . }
        "#;
        let plan = parse(q).expect("parse");
        // The local name of `ex:Person` should be "Person".
        assert_eq!(plan.seed.types, vec!["Person".to_string()]);
        assert_eq!(plan.steps[0].edge.label.as_deref(), Some("knows"));
    }

    #[test]
    fn parses_limit_and_offset() {
        let plan = parse(
            "SELECT ?a WHERE { ?a a :T . } LIMIT 5 OFFSET 7",
        )
        .expect("parse");
        assert_eq!(plan.limit, Some(5));
        assert_eq!(plan.skip, 7);
    }

    #[test]
    fn parses_offset_before_limit() {
        let plan = parse(
            "SELECT ?a WHERE { ?a a :T . } OFFSET 3 LIMIT 2",
        )
        .expect("parse");
        assert_eq!(plan.limit, Some(2));
        assert_eq!(plan.skip, 3);
    }

    #[test]
    fn parses_select_star() {
        let plan = parse(
            "SELECT * WHERE { ?a a :Org . }",
        )
        .expect("parse");
        assert!(plan.return_columns.is_empty());
    }

    #[test]
    fn rejects_unknown_trailing_input() {
        let err = parse("SELECT ?a WHERE { ?a a :Org . } JUNK").unwrap_err();
        match err {
            SparqlError::Parse(_) => {}
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn rejects_non_variable_subject() {
        let err = parse("SELECT ?a WHERE { :foo a :Org . }").unwrap_err();
        match err {
            SparqlError::Unsupported(_) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn handles_optional_trailing_dot() {
        let plan = parse(
            "SELECT ?a WHERE { ?a a :Org }",
        )
        .expect("parse");
        assert_eq!(plan.seed.types, vec!["Org".to_string()]);
    }

    #[test]
    fn iri_local_name_extraction() {
        assert_eq!(super::label_localname("http://example.com/Person"), "Person");
        assert_eq!(super::label_localname("http://example.com#Person"), "Person");
        assert_eq!(super::label_localname("Person"), "Person");
        assert_eq!(super::label_localname("ex:Person"), "Person");
    }
}
