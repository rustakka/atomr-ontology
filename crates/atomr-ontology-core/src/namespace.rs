//! Namespaces and vocabularies — CURIE-style prefix maps.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::iri::Iri;

/// A prefix-bound namespace, e.g. `rdf` → `http://www.w3.org/1999/02/22-rdf-syntax-ns#`.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Namespace {
    /// The short prefix label, e.g. `"rdf"`.
    pub prefix: String,
    /// The base IRI the prefix resolves to.
    pub base: Iri,
}

impl Namespace {
    /// Construct a namespace pair.
    pub fn new(prefix: impl Into<String>, base: Iri) -> Self {
        Self { prefix: prefix.into(), base }
    }

    /// Resolve a local name into its expanded IRI by string
    /// concatenation. No percent-encoding is performed.
    pub fn expand(&self, local_name: &str) -> Iri {
        Iri::from_unchecked(format!("{}{}", self.base.as_str(), local_name))
    }
}

/// A collection of [`Namespace`]s keyed by prefix.
///
/// `Vocabulary` is a plain map; iteration order is sorted by prefix.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Vocabulary {
    namespaces: BTreeMap<String, Iri>,
}

impl Vocabulary {
    /// Empty vocabulary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a prefix to a base IRI, replacing any existing binding.
    pub fn bind(&mut self, prefix: impl Into<String>, base: Iri) -> &mut Self {
        self.namespaces.insert(prefix.into(), base);
        self
    }

    /// Look up the base IRI for a prefix.
    pub fn base(&self, prefix: &str) -> Option<&Iri> {
        self.namespaces.get(prefix)
    }

    /// Expand a CURIE of the form `prefix:local` into an IRI.
    /// Returns `None` if the prefix is unknown.
    pub fn expand_curie(&self, curie: &str) -> Option<Iri> {
        let (prefix, local) = curie.split_once(':')?;
        let base = self.namespaces.get(prefix)?;
        Some(Iri::from_unchecked(format!("{}{}", base.as_str(), local)))
    }

    /// Iterate bindings sorted by prefix.
    pub fn iter(&self) -> impl Iterator<Item = Namespace> + '_ {
        self.namespaces.iter().map(|(p, b)| Namespace::new(p.clone(), b.clone()))
    }

    /// The standard W3C / schema.org namespaces commonly needed when
    /// projecting an LPG ontology into RDF.
    pub fn with_standard_bindings() -> Self {
        let mut v = Self::new();
        v.bind("rdf", Iri::from_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
        v.bind("rdfs", Iri::from_unchecked("http://www.w3.org/2000/01/rdf-schema#"));
        v.bind("owl", Iri::from_unchecked("http://www.w3.org/2002/07/owl#"));
        v.bind("xsd", Iri::from_unchecked("http://www.w3.org/2001/XMLSchema#"));
        v.bind("prov", Iri::from_unchecked("http://www.w3.org/ns/prov#"));
        v.bind("schema", Iri::from_unchecked("http://schema.org/"));
        v.bind("org", Iri::from_unchecked("http://www.w3.org/ns/org#"));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_expand() {
        let ns = Namespace::new("ex", Iri::from_unchecked("https://example.org/"));
        assert_eq!(ns.expand("Acme").as_str(), "https://example.org/Acme");
    }

    #[test]
    fn vocabulary_round_trip_curie() {
        let v = Vocabulary::with_standard_bindings();
        let iri = v.expand_curie("org:Organization").unwrap();
        assert_eq!(iri.as_str(), "http://www.w3.org/ns/org#Organization");
        assert!(v.expand_curie("nope:Foo").is_none());
    }

    #[test]
    fn vocabulary_iteration_is_sorted() {
        let v = Vocabulary::with_standard_bindings();
        let prefixes: Vec<_> = v.iter().map(|n| n.prefix).collect();
        let mut sorted = prefixes.clone();
        sorted.sort();
        assert_eq!(prefixes, sorted);
    }
}
