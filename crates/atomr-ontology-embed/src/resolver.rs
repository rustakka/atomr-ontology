//! [`EmbeddingResolver`] — embedding-based pre-filter for entity
//! resolution.
//!
//! Given a surface form, `propose` returns the top-k existing node
//! IRIs whose embedded labels are most cosine-similar. The downstream
//! `EntityResolver` in `atomr-ontology-extract` can then ask an LLM to
//! disambiguate among that short list rather than scanning the whole
//! ontology.

use std::sync::Arc;

use parking_lot::RwLock;

use atomr_ontology_core::{Node, Ontology, PropertyValue};

use crate::backend::{EmbeddingBackend, EmbeddingError};
use crate::index::{VectorIndex, VectorRecord};

/// Embedding-based entity-resolution pre-filter.
///
/// Wraps an [`EmbeddingBackend`] and an in-memory [`VectorIndex`].
/// Callers feed an [`Ontology`] in via [`EmbeddingResolver::ingest_ontology`],
/// then probe with surface forms via [`EmbeddingResolver::propose`].
pub struct EmbeddingResolver {
    backend: Arc<dyn EmbeddingBackend>,
    index: RwLock<VectorIndex>,
}

impl EmbeddingResolver {
    /// Build a resolver around an embedding backend. The index is
    /// pinned to the backend's reported dimension.
    pub fn new(backend: Arc<dyn EmbeddingBackend>) -> Self {
        let dim = backend.dimensions();
        Self { backend, index: RwLock::new(VectorIndex::with_dimensions(dim)) }
    }

    /// Embed every node in `ontology` and insert it into the index.
    ///
    /// The text used for embedding is, in order of preference:
    ///   1. the node's `name` property (when it is a string),
    ///   2. the IRI tail (substring after the last `/`, `#`, or `:`),
    ///   3. the full IRI,
    ///   4. the node id (as a debug string).
    ///
    /// Nodes without an IRI are still embedded; their `iri` field in
    /// the stored [`VectorRecord`] is filled with the node id.
    /// Returns the number of records inserted.
    pub async fn ingest_ontology(&self, ontology: &Ontology) -> Result<usize, EmbeddingError> {
        let mut inputs: Vec<(String, String)> = Vec::with_capacity(ontology.nodes.len());
        for node in ontology.nodes.values() {
            let key = node_key(node);
            let text = node_text(node);
            inputs.push((key, text));
        }
        let texts: Vec<String> = inputs.iter().map(|(_, t)| t.clone()).collect();
        let vectors = self.backend.embed_batch(&texts).await?;
        if vectors.len() != inputs.len() {
            return Err(EmbeddingError::Other(format!(
                "backend returned {} vectors for {} inputs",
                vectors.len(),
                inputs.len()
            )));
        }
        let mut index = self.index.write();
        let mut inserted = 0usize;
        for ((key, text), vector) in inputs.into_iter().zip(vectors.into_iter()) {
            let meta = serde_json::json!({ "text": text });
            let record = VectorRecord::new(key, vector).with_meta(meta);
            index.insert(record).map_err(|e| EmbeddingError::Other(e.to_string()))?;
            inserted += 1;
        }
        Ok(inserted)
    }

    /// Embed `surface` and return up to `top_k` (iri, cosine score)
    /// pairs from the index, descending by score.
    pub async fn propose(
        &self,
        surface: &str,
        top_k: usize,
    ) -> Result<Vec<(String, f32)>, EmbeddingError> {
        let query = self.backend.embed(surface).await?;
        let hits = self.index.read().search(&query, top_k);
        Ok(hits.into_iter().map(|(r, s)| (r.iri, s)).collect())
    }

    /// Number of records in the index.
    pub fn len(&self) -> usize {
        self.index.read().len()
    }

    /// True iff the index has no records.
    pub fn is_empty(&self) -> bool {
        self.index.read().is_empty()
    }

    /// Drop every record from the index.
    pub fn clear(&self) {
        self.index.write().clear();
    }
}

/// Pick the stable key used to identify a node in the vector index.
fn node_key(node: &Node) -> String {
    if let Some(iri) = &node.iri {
        return iri.as_str().to_string();
    }
    format!("{:?}", node.id)
}

/// Pick the text used to embed a node.
fn node_text(node: &Node) -> String {
    if let Some(PropertyValue::String(s)) = node.properties.get("name") {
        return s.clone();
    }
    if let Some(iri) = &node.iri {
        let s = iri.as_str();
        let tail = s
            .rsplit_once(|c: char| c == '/' || c == '#' || c == ':')
            .map(|(_, t)| t)
            .unwrap_or(s);
        if !tail.is_empty() {
            return tail.to_string();
        }
        return s.to_string();
    }
    format!("{:?}", node.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{Iri, Node};

    use crate::backend::HashEmbedder;

    fn make_ontology() -> Ontology {
        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        let acme = Node::from_iri(
            Iri::new("https://example.org/Acme").unwrap(),
            "Organization",
        )
        .with_property("name", "Acme Corporation");
        let umbrella = Node::from_iri(
            Iri::new("https://example.org/Umbrella").unwrap(),
            "Organization",
        )
        .with_property("name", "Umbrella Corp");
        let initech = Node::from_iri(
            Iri::new("https://example.org/Initech").unwrap(),
            "Organization",
        )
        .with_property("name", "Initech");
        o.upsert_node(acme);
        o.upsert_node(umbrella);
        o.upsert_node(initech);
        o
    }

    #[tokio::test]
    async fn ingests_every_node() {
        let backend = Arc::new(HashEmbedder::new(32));
        let resolver = EmbeddingResolver::new(backend);
        let o = make_ontology();
        let n = resolver.ingest_ontology(&o).await.unwrap();
        assert_eq!(n, 3);
        assert_eq!(resolver.len(), 3);
    }

    #[tokio::test]
    async fn propose_round_trip_finds_exact_match() {
        let backend = Arc::new(HashEmbedder::new(64));
        let resolver = EmbeddingResolver::new(backend);
        let o = make_ontology();
        resolver.ingest_ontology(&o).await.unwrap();
        // Probing with the exact name of a node should rank it first;
        // the hash embedder is deterministic, so identical text yields
        // identical vectors, which cosine-similar to themselves at 1.0.
        let hits = resolver.propose("Umbrella Corp", 3).await.unwrap();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].0, "https://example.org/Umbrella");
        assert!(
            (hits[0].1 - 1.0).abs() < 1e-5,
            "expected top hit ~1.0, got {}",
            hits[0].1
        );
        assert!(hits[0].1 >= hits[1].1);
        assert!(hits[1].1 >= hits[2].1);
    }

    #[tokio::test]
    async fn propose_top_k_respected() {
        let backend = Arc::new(HashEmbedder::new(16));
        let resolver = EmbeddingResolver::new(backend);
        let o = make_ontology();
        resolver.ingest_ontology(&o).await.unwrap();
        let hits = resolver.propose("Acme Corporation", 1).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "https://example.org/Acme");
    }

    #[tokio::test]
    async fn ingest_uses_iri_tail_when_name_absent() {
        let backend = Arc::new(HashEmbedder::new(16));
        let resolver = EmbeddingResolver::new(backend);
        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        let n = Node::from_iri(
            Iri::new("https://example.org/UnnamedThing").unwrap(),
            "Organization",
        );
        o.upsert_node(n);
        resolver.ingest_ontology(&o).await.unwrap();
        // The IRI tail is "UnnamedThing"; querying with that exact
        // string should hit at ~1.0 cosine similarity.
        let hits = resolver.propose("UnnamedThing", 1).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "https://example.org/UnnamedThing");
        assert!((hits[0].1 - 1.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn clear_empties_the_index() {
        let backend = Arc::new(HashEmbedder::new(8));
        let resolver = EmbeddingResolver::new(backend);
        let o = make_ontology();
        resolver.ingest_ontology(&o).await.unwrap();
        assert!(!resolver.is_empty());
        resolver.clear();
        assert!(resolver.is_empty());
    }
}
