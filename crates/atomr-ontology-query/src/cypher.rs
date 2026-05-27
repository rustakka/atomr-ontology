//! Hand-rolled recursive-descent parser for a tiny openCypher subset.
//!
//! Supported grammar (informally):
//!
//! ```text
//! query        := match where? return limit?
//! match        := 'MATCH' path
//! path         := node ( edge node )*
//! node         := '(' ident? (':' label)* prop_map? ')'
//! edge         := '-' edge_body? arrow
//! edge_body    := '[' ident? (':' label)? var_len? prop_map? ']'
//! arrow        := '->' | '-'
//! var_len      := '*' int ('..' int)?
//! prop_map     := '{' (kv (',' kv)*)? '}'
//! kv           := ident ':' value
//! value        := string | integer | 'true' | 'false'
//! where        := 'WHERE' 'NOT' node          // single negative pattern
//! return       := 'RETURN' ident (',' ident)*
//! limit        := 'LIMIT' int
//! ```
//!
//! The parser is intentionally permissive about whitespace and case
//! for keywords (`MATCH`, `WHERE`, `RETURN`, `LIMIT`, `NOT`, `TRUE`,
//! `FALSE`); identifiers and labels are case-sensitive.

use atomr_ontology_core::PropertyValue;
use atomr_ontology_store::{EdgePattern, NodePattern, TraversalPlan, TraversalStep};
use thiserror::Error;

/// Errors produced by [`parse`].
#[derive(Debug, Error)]
pub enum CypherError {
    /// The input could not be tokenised / parsed.
    #[error("cypher parse error: {0}")]
    Parse(String),
    /// The grammar construct is recognised but not implemented in this
    /// subset.
    #[error("cypher unsupported feature: {0}")]
    Unsupported(String),
}

/// Parse a Cypher-subset query into a [`TraversalPlan`].
pub fn parse(query: &str) -> Result<TraversalPlan, CypherError> {
    let mut p = Parser::new(query);
    let plan = p.parse_query()?;
    p.skip_ws();
    if !p.eof() {
        return Err(CypherError::Parse(format!(
            "unexpected trailing input at position {}: {:?}",
            p.pos,
            p.peek_rest_trim()
        )));
    }
    Ok(plan)
}

/// Internal tracker around the input.
struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src: src.as_bytes(), pos: 0 }
    }

    // ---- low-level utilities --------------------------------------

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, off: usize) -> Option<u8> {
        self.src.get(self.pos + off).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.pos += 1;
            } else {
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

    /// Consume `lit` ASCII-case-insensitively, but only when it is
    /// followed by a non-identifier byte (so `MATCHING` does not match
    /// the keyword `MATCH`).
    fn try_keyword(&mut self, kw: &str) -> bool {
        let bytes = kw.as_bytes();
        if self.src.len() - self.pos < bytes.len() {
            return false;
        }
        let slice = &self.src[self.pos..self.pos + bytes.len()];
        if !slice.eq_ignore_ascii_case(bytes) {
            return false;
        }
        // Look ahead for a word boundary.
        if let Some(&next) = self.src.get(self.pos + bytes.len()) {
            if is_ident_continue(next) {
                return false;
            }
        }
        self.pos += bytes.len();
        true
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<(), CypherError> {
        self.skip_ws();
        if self.try_keyword(kw) {
            Ok(())
        } else {
            Err(CypherError::Parse(format!(
                "expected keyword `{}` at position {} (got {:?})",
                kw,
                self.pos,
                self.peek_rest_trim()
            )))
        }
    }

    fn expect_char(&mut self, c: u8) -> Result<(), CypherError> {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(CypherError::Parse(format!(
                "expected `{}` at position {} (got {:?})",
                c as char,
                self.pos,
                self.peek_rest_trim()
            )))
        }
    }

    // ---- tokens ---------------------------------------------------

    fn parse_ident(&mut self) -> Result<String, CypherError> {
        self.skip_ws();
        let start = self.pos;
        match self.peek() {
            Some(b) if is_ident_start(b) => {
                self.pos += 1;
            }
            _ => {
                return Err(CypherError::Parse(format!(
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
        Ok(std::str::from_utf8(raw)
            .map_err(|e| CypherError::Parse(format!("utf-8 error in identifier: {e}")))?
            .to_string())
    }

    fn parse_uint(&mut self) -> Result<usize, CypherError> {
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
            return Err(CypherError::Parse(format!(
                "expected integer at position {} (got {:?})",
                self.pos,
                self.peek_rest_trim()
            )));
        }
        let raw = std::str::from_utf8(&self.src[start..self.pos])
            .map_err(|e| CypherError::Parse(format!("utf-8 error in integer: {e}")))?;
        raw.parse::<usize>()
            .map_err(|e| CypherError::Parse(format!("invalid integer `{raw}`: {e}")))
    }

    fn parse_int(&mut self) -> Result<i64, CypherError> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let raw = std::str::from_utf8(&self.src[start..self.pos])
            .map_err(|e| CypherError::Parse(format!("utf-8 error in integer: {e}")))?;
        if raw.is_empty() || raw == "-" {
            return Err(CypherError::Parse(format!(
                "expected integer at position {start}"
            )));
        }
        raw.parse::<i64>()
            .map_err(|e| CypherError::Parse(format!("invalid integer `{raw}`: {e}")))
    }

    fn parse_string_literal(&mut self) -> Result<String, CypherError> {
        self.skip_ws();
        if self.peek() != Some(b'"') {
            return Err(CypherError::Parse(format!(
                "expected string literal at position {} (got {:?})",
                self.pos,
                self.peek_rest_trim()
            )));
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            match self.bump() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.bump() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'n') => out.push('\n'),
                    Some(b't') => out.push('\t'),
                    Some(b'r') => out.push('\r'),
                    Some(other) => out.push(other as char),
                    None => {
                        return Err(CypherError::Parse(
                            "unterminated escape in string literal".into(),
                        ));
                    }
                },
                Some(b) => out.push(b as char),
                None => {
                    return Err(CypherError::Parse(
                        "unterminated string literal".into(),
                    ));
                }
            }
        }
    }

    fn parse_value(&mut self) -> Result<PropertyValue, CypherError> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => Ok(PropertyValue::String(self.parse_string_literal()?)),
            Some(b) if b.is_ascii_digit() || b == b'-' => Ok(PropertyValue::Integer(self.parse_int()?)),
            Some(b) if b.is_ascii_alphabetic() => {
                let save = self.pos;
                if self.try_keyword("true") {
                    Ok(PropertyValue::Bool(true))
                } else if self.try_keyword("false") {
                    Ok(PropertyValue::Bool(false))
                } else {
                    self.pos = save;
                    Err(CypherError::Parse(format!(
                        "expected value (string, integer, bool) at position {} (got {:?})",
                        self.pos,
                        self.peek_rest_trim()
                    )))
                }
            }
            _ => Err(CypherError::Parse(format!(
                "expected value at position {} (got {:?})",
                self.pos,
                self.peek_rest_trim()
            ))),
        }
    }

    // ---- pattern parsers ------------------------------------------

    fn parse_prop_map(&mut self, into: &mut std::collections::BTreeMap<String, PropertyValue>) -> Result<(), CypherError> {
        self.expect_char(b'{')?;
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(());
        }
        loop {
            let key = self.parse_ident()?;
            self.expect_char(b':')?;
            let value = self.parse_value()?;
            into.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    continue;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => {
                    return Err(CypherError::Parse(format!(
                        "expected `,` or `}}` in property map at position {} (got {:?})",
                        self.pos,
                        self.peek_rest_trim()
                    )));
                }
            }
        }
    }

    fn parse_node_pattern(&mut self) -> Result<NodePattern, CypherError> {
        self.expect_char(b'(')?;
        let mut node = NodePattern::any();
        self.skip_ws();
        // Optional binding identifier (must not be a `:` label start).
        if let Some(b) = self.peek() {
            if is_ident_start(b) {
                let name = self.parse_ident()?;
                node.bind = Some(name);
            }
        }
        // Optional labels (`:Type`)*.
        loop {
            self.skip_ws();
            if self.peek() == Some(b':') {
                self.pos += 1;
                let label = self.parse_ident()?;
                node.types.push(label);
            } else {
                break;
            }
        }
        // Optional property map.
        self.skip_ws();
        if self.peek() == Some(b'{') {
            self.parse_prop_map(&mut node.properties)?;
        }
        self.expect_char(b')')?;
        Ok(node)
    }

    /// Parse one edge segment plus the trailing node, returning the
    /// step plus direction. Assumes we are positioned at the leading
    /// `-` (or `<-` for inbound).
    fn parse_edge_and_node(&mut self) -> Result<TraversalStep, CypherError> {
        self.skip_ws();
        // Inbound: `<-[...]-`
        let inbound = if self.peek() == Some(b'<') {
            self.pos += 1;
            true
        } else {
            false
        };
        self.expect_char(b'-')?;
        let mut edge = EdgePattern::any();
        self.skip_ws();
        if self.peek() == Some(b'[') {
            self.pos += 1;
            self.skip_ws();
            // Optional binding identifier.
            if let Some(b) = self.peek() {
                if is_ident_start(b) {
                    let name = self.parse_ident()?;
                    edge.bind = Some(name);
                }
            }
            self.skip_ws();
            // Optional label.
            if self.peek() == Some(b':') {
                self.pos += 1;
                let label = self.parse_ident()?;
                edge.label = Some(label);
            }
            // Optional variable-length specifier `*min..max` or `*min` or `*`.
            self.skip_ws();
            if self.peek() == Some(b'*') {
                self.pos += 1;
                self.skip_ws();
                // Parse optional lower bound.
                let mut min: usize = 1;
                let mut max: usize = usize::MAX;
                if let Some(b) = self.peek() {
                    if b.is_ascii_digit() {
                        min = self.parse_uint()?;
                        max = min; // default if no upper bound follows
                    }
                }
                self.skip_ws();
                if self.peek() == Some(b'.') && self.peek_at(1) == Some(b'.') {
                    self.pos += 2;
                    self.skip_ws();
                    if let Some(b) = self.peek() {
                        if b.is_ascii_digit() {
                            max = self.parse_uint()?;
                        } else {
                            max = usize::MAX;
                        }
                    } else {
                        max = usize::MAX;
                    }
                }
                if max < min {
                    return Err(CypherError::Parse(format!(
                        "variable-length range max ({max}) < min ({min})"
                    )));
                }
                edge.repeat = Some(min..=max);
            }
            // Optional property map.
            self.skip_ws();
            if self.peek() == Some(b'{') {
                self.parse_prop_map(&mut edge.properties)?;
            }
            self.skip_ws();
            self.expect_char(b']')?;
        }
        self.skip_ws();
        // Closing dash and optional `>`.
        self.expect_char(b'-')?;
        let outbound_arrow = if self.peek() == Some(b'>') {
            self.pos += 1;
            true
        } else {
            false
        };
        // We treat `-[..]-` (no arrow either side) as outbound.
        // Combined direction logic:
        //   `<-[..]-`  → inbound
        //   `-[..]->`  → outbound
        //   `-[..]-`   → outbound (default)
        //   `<-[..]->` → ambiguous / unsupported
        let outbound = match (inbound, outbound_arrow) {
            (true, true) => {
                return Err(CypherError::Unsupported(
                    "bidirectional edge `<-[..]->` not supported".into(),
                ));
            }
            (true, false) => false,
            (false, true) => true,
            (false, false) => true,
        };
        let target = self.parse_node_pattern()?;
        Ok(TraversalStep { edge, target, outbound })
    }

    // ---- query parser ---------------------------------------------

    fn parse_query(&mut self) -> Result<TraversalPlan, CypherError> {
        self.expect_keyword("MATCH")?;
        let seed = self.parse_node_pattern()?;
        let mut plan = TraversalPlan::from(seed);
        // Zero or more `-[..]-> (node)` segments.
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'-') | Some(b'<') => {
                    let step = self.parse_edge_and_node()?;
                    plan.steps.push(step);
                }
                _ => break,
            }
        }
        // Optional WHERE NOT (node).
        self.skip_ws();
        let save = self.pos;
        if self.try_keyword("WHERE") {
            self.skip_ws();
            if !self.try_keyword("NOT") {
                self.pos = save;
                return Err(CypherError::Unsupported(
                    "only `WHERE NOT (...)` is supported in this subset".into(),
                ));
            }
            self.skip_ws();
            // Two accepted forms:
            //   WHERE NOT (n)
            //   WHERE NOT n:Label
            let neg = if self.peek() == Some(b'(') {
                self.parse_node_pattern()?
            } else {
                self.parse_inline_node_constraint()?
            };
            self.apply_negation(&mut plan, neg)?;
        }
        // Required RETURN.
        self.expect_keyword("RETURN")?;
        let mut cols = Vec::new();
        loop {
            let col = self.parse_ident()?;
            cols.push(col);
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
                continue;
            }
            break;
        }
        plan.return_columns = cols;
        // Optional LIMIT.
        self.skip_ws();
        if self.try_keyword("LIMIT") {
            let n = self.parse_uint()?;
            plan.limit = Some(n);
        }
        Ok(plan)
    }

    fn parse_inline_node_constraint(&mut self) -> Result<NodePattern, CypherError> {
        // Forms: `ident`, `ident:Label`, with optional further `:Label`s.
        let bind = self.parse_ident()?;
        let mut node = NodePattern::any();
        node.bind = Some(bind);
        loop {
            self.skip_ws();
            if self.peek() == Some(b':') {
                self.pos += 1;
                let label = self.parse_ident()?;
                node.types.push(label);
            } else {
                break;
            }
        }
        Ok(node)
    }

    /// Attach a negative pattern to whichever bound variable it refers
    /// to (seed or any target). If the binding name is absent or does
    /// not match a known variable, attach it to the seed.
    fn apply_negation(&self, plan: &mut TraversalPlan, neg: NodePattern) -> Result<(), CypherError> {
        let target_name = neg.bind.clone();
        match target_name.as_deref() {
            Some(name) if plan.seed.bind.as_deref() == Some(name) => {
                attach_not_to(&mut plan.seed, neg);
            }
            Some(name) => {
                let mut placed = false;
                for step in plan.steps.iter_mut() {
                    if step.target.bind.as_deref() == Some(name) {
                        attach_not_to(&mut step.target, neg.clone());
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    // No matching binding — attach to seed as a global filter.
                    attach_not_to(&mut plan.seed, neg);
                }
            }
            None => {
                attach_not_to(&mut plan.seed, neg);
            }
        }
        Ok(())
    }
}

fn attach_not_to(node: &mut NodePattern, mut neg: NodePattern) {
    // The negative sub-pattern should not carry a binding (it filters
    // the parent binding) — drop it so the executor doesn't try to
    // emit a separate binding for it.
    neg.bind = None;
    node.not.push(Box::new(neg));
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
    fn parses_simple_node_pattern() {
        let plan = parse("MATCH (a:Org) RETURN a").expect("parse");
        assert_eq!(plan.seed.bind.as_deref(), Some("a"));
        assert_eq!(plan.seed.types, vec!["Org".to_string()]);
        assert!(plan.steps.is_empty());
        assert_eq!(plan.return_columns, vec!["a".to_string()]);
        assert!(plan.limit.is_none());
    }

    #[test]
    fn parses_two_hop_with_limit() {
        let plan = parse(
            "MATCH (a:Org)-[:memberOf]->(b:Org) RETURN a, b LIMIT 10",
        )
        .expect("parse");
        assert_eq!(plan.seed.bind.as_deref(), Some("a"));
        assert_eq!(plan.steps.len(), 1);
        assert!(plan.steps[0].outbound);
        assert_eq!(
            plan.steps[0].edge.label.as_deref(),
            Some("memberOf")
        );
        assert_eq!(plan.steps[0].target.bind.as_deref(), Some("b"));
        assert_eq!(plan.return_columns, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(plan.limit, Some(10));
    }

    #[test]
    fn parses_variable_length_path() {
        let plan = parse(
            "MATCH (a:Org)-[:subClassOf*1..3]->(b) RETURN b",
        )
        .expect("parse");
        assert_eq!(plan.steps.len(), 1);
        let repeat = plan.steps[0]
            .edge
            .repeat
            .clone()
            .expect("repeat range present");
        assert_eq!(*repeat.start(), 1);
        assert_eq!(*repeat.end(), 3);
        assert_eq!(plan.steps[0].edge.label.as_deref(), Some("subClassOf"));
    }

    #[test]
    fn parses_property_map_and_where_not() {
        let plan = parse(
            r#"MATCH (a:Org {name: "Acme"}) WHERE NOT a:Excluded RETURN a"#,
        )
        .expect("parse");
        let name = plan
            .seed
            .properties
            .get("name")
            .expect("name property bound");
        assert_eq!(name, &PropertyValue::String("Acme".into()));
        assert_eq!(plan.seed.not.len(), 1);
        assert_eq!(plan.seed.not[0].types, vec!["Excluded".to_string()]);
    }

    #[test]
    fn parses_where_not_parenthesised() {
        let plan = parse(
            r#"MATCH (a:Org) WHERE NOT (a:Excluded) RETURN a"#,
        )
        .expect("parse");
        assert_eq!(plan.seed.not.len(), 1);
        assert_eq!(plan.seed.not[0].types, vec!["Excluded".to_string()]);
    }

    #[test]
    fn parses_integer_and_bool_properties() {
        let plan = parse(
            r#"MATCH (a:Org {size: 42, active: true}) RETURN a"#,
        )
        .expect("parse");
        assert_eq!(
            plan.seed.properties.get("size"),
            Some(&PropertyValue::Integer(42))
        );
        assert_eq!(
            plan.seed.properties.get("active"),
            Some(&PropertyValue::Bool(true))
        );
    }

    #[test]
    fn parses_inbound_edge() {
        let plan = parse(
            "MATCH (a:Org)<-[:memberOf]-(b:Org) RETURN a, b",
        )
        .expect("parse");
        assert_eq!(plan.steps.len(), 1);
        assert!(!plan.steps[0].outbound);
    }

    #[test]
    fn rejects_trailing_garbage() {
        let err = parse("MATCH (a:Org) RETURN a GARBAGE").unwrap_err();
        match err {
            CypherError::Parse(_) => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unsupported_where() {
        let err = parse("MATCH (a:Org) WHERE a.foo = 1 RETURN a").unwrap_err();
        match err {
            CypherError::Unsupported(_) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn parses_open_ended_variable_length() {
        let plan = parse(
            "MATCH (a)-[:r*2..]->(b) RETURN b",
        )
        .expect("parse");
        let r = plan.steps[0].edge.repeat.clone().unwrap();
        assert_eq!(*r.start(), 2);
        assert_eq!(*r.end(), usize::MAX);
    }
}
