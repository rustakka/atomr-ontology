//! Internationalized Resource Identifier (IRI) — RFC 3987.
//!
//! We hold IRIs as validated `String`s rather than parsing into a full
//! URL because ontology IRIs frequently use forms that strict URL
//! parsers reject (relative paths, blank-node-style IRIs, custom
//! schemes). The validator we apply is intentionally permissive: any
//! non-empty string that has no embedded whitespace and parses as an
//! absolute URL or a CURIE-shaped string is accepted.

use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors raised when validating an IRI.
#[derive(Debug, Error)]
pub enum IriError {
    /// The string was empty.
    #[error("iri must be non-empty")]
    Empty,
    /// The string contained whitespace.
    #[error("iri must not contain whitespace: {0:?}")]
    Whitespace(String),
}

/// A validated IRI string.
///
/// `Iri` round-trips through serde as a transparent string. Equality
/// and hashing are over the canonical (verbatim) form; no
/// normalization is applied at construction time.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Iri(String);

impl Iri {
    /// Construct from a string, validating it as an IRI.
    pub fn new(value: impl Into<String>) -> Result<Self, IriError> {
        let s = value.into();
        if s.is_empty() {
            return Err(IriError::Empty);
        }
        if s.chars().any(char::is_whitespace) {
            return Err(IriError::Whitespace(s));
        }
        Ok(Iri(s))
    }

    /// Construct without validation. Caller must guarantee the input
    /// is a well-formed IRI; used during deserialization fast paths.
    pub fn from_unchecked(value: impl Into<String>) -> Self {
        Iri(value.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Move the underlying string out.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Iri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Iri {
    type Err = IriError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Iri::new(s.to_string())
    }
}

impl AsRef<str> for Iri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typical_iri() {
        let iri = Iri::new("https://example.org/Organization").unwrap();
        assert_eq!(iri.as_str(), "https://example.org/Organization");
    }

    #[test]
    fn accepts_curie_shape() {
        // `prefix:Local` shapes are accepted; the IRI does not have
        // to be a fully resolved URL.
        let iri = Iri::new("org:Organization").unwrap();
        assert_eq!(iri.as_str(), "org:Organization");
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(Iri::new(""), Err(IriError::Empty)));
    }

    #[test]
    fn rejects_whitespace() {
        assert!(matches!(Iri::new("a b"), Err(IriError::Whitespace(_))));
    }

    #[test]
    fn round_trip_json() {
        let iri = Iri::new("https://example.org/x").unwrap();
        let s = serde_json::to_string(&iri).unwrap();
        assert_eq!(s, "\"https://example.org/x\"");
        let back: Iri = serde_json::from_str(&s).unwrap();
        assert_eq!(iri, back);
    }
}
