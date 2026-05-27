//! In-memory vector index used to pre-filter entity-resolution
//! candidates.
//!
//! The current implementation is a linear-scan store ordered by cosine
//! similarity. Linear scan is fine at the scale of a single ontology
//! snapshot (thousands of nodes). HNSW or another ANN backend is a
//! planned upgrade — see the workspace roadmap.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error raised when inserting into a [`VectorIndex`].
#[derive(Debug, Error)]
pub enum VectorIndexError {
    /// The inserted vector's dimension did not match the index.
    #[error("vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension supplied.
        actual: usize,
    },
}

/// A stored vector keyed by IRI with attached metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VectorRecord {
    /// Canonical IRI of the node this vector represents.
    pub iri: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Arbitrary metadata payload, e.g. the original surface form.
    pub meta: serde_json::Value,
}

impl VectorRecord {
    /// Build a record with a `null` metadata payload.
    pub fn new(iri: impl Into<String>, vector: Vec<f32>) -> Self {
        Self { iri: iri.into(), vector, meta: serde_json::Value::Null }
    }

    /// Attach a metadata payload.
    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = meta;
        self
    }
}

/// Linear-scan in-memory vector index.
///
/// Stores [`VectorRecord`]s and ranks them by cosine similarity at
/// query time. The index pins itself to the dimension of the first
/// inserted record (or to the explicit dimension supplied via
/// [`VectorIndex::with_dimensions`]) and rejects mismatched inserts.
///
/// Future work: swap the inner `Vec` for an HNSW graph or a faiss
/// adapter without changing the public surface.
#[derive(Debug, Default)]
pub struct VectorIndex {
    records: Vec<VectorRecord>,
    dim: Option<usize>,
}

impl VectorIndex {
    /// Empty index with no fixed dimension yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty index pinned to a known dimension.
    pub fn with_dimensions(dim: usize) -> Self {
        Self { records: Vec::new(), dim: Some(dim) }
    }

    /// Insert a record. Returns an error when its vector dimension
    /// disagrees with the index.
    pub fn insert(&mut self, record: VectorRecord) -> Result<(), VectorIndexError> {
        let actual = record.vector.len();
        match self.dim {
            Some(expected) if expected != actual => {
                return Err(VectorIndexError::DimensionMismatch { expected, actual });
            }
            None => self.dim = Some(actual),
            _ => {}
        }
        self.records.push(record);
        Ok(())
    }

    /// Search the index by cosine similarity. Returns up to `top_k`
    /// `(record, score)` pairs in descending score order.
    ///
    /// Queries with a mismatched dimension yield an empty result —
    /// callers that need a hard error should validate dimensions
    /// against [`VectorIndex::dimensions`] before calling.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(VectorRecord, f32)> {
        if top_k == 0 || self.records.is_empty() {
            return Vec::new();
        }
        if let Some(dim) = self.dim {
            if query.len() != dim {
                return Vec::new();
            }
        }
        let mut scored: Vec<(usize, f32)> = self
            .records
            .iter()
            .enumerate()
            .map(|(i, r)| (i, cosine_similarity(query, &r.vector)))
            .collect();
        // Sort descending; NaNs sort to the end via `partial_cmp` fallback.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(top_k)
            .map(|(i, s)| (self.records[i].clone(), s))
            .collect()
    }

    /// Number of stored records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True iff the index has no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Drop all stored records. The pinned dimension (if any) is kept
    /// so subsequent inserts retain the same dimensional invariant.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// The dimension this index is pinned to, if any.
    pub fn dimensions(&self) -> Option<usize> {
        self.dim
    }
}

/// Cosine similarity in `[-1, 1]`. Returns 0.0 when either input is
/// the zero vector or when the lengths disagree.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = *x as f64;
        let yf = *y as f64;
        dot += xf * yf;
        na += xf * xf;
        nb += yf * yf;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_orders_by_cosine_similarity() {
        let mut idx = VectorIndex::new();
        // The query vector — `c` aligns perfectly, `a` opposes, `b` is orthogonal-ish.
        idx.insert(VectorRecord::new("iri:a", vec![-1.0, 0.0, 0.0])).unwrap();
        idx.insert(VectorRecord::new("iri:b", vec![0.0, 1.0, 0.0])).unwrap();
        idx.insert(VectorRecord::new("iri:c", vec![1.0, 0.0, 0.0])).unwrap();
        let hits = idx.search(&[1.0, 0.0, 0.0], 3);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].0.iri, "iri:c");
        assert!(hits[0].1 > hits[1].1);
        assert!(hits[1].1 > hits[2].1);
        assert_eq!(hits[2].0.iri, "iri:a");
    }

    #[test]
    fn search_top_k_truncates() {
        let mut idx = VectorIndex::new();
        idx.insert(VectorRecord::new("iri:a", vec![1.0, 0.0])).unwrap();
        idx.insert(VectorRecord::new("iri:b", vec![0.0, 1.0])).unwrap();
        idx.insert(VectorRecord::new("iri:c", vec![1.0, 1.0])).unwrap();
        let hits = idx.search(&[1.0, 0.0], 2);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn search_empty_returns_empty() {
        let idx = VectorIndex::new();
        assert!(idx.search(&[1.0, 0.0], 5).is_empty());
    }

    #[test]
    fn search_top_k_zero_returns_empty() {
        let mut idx = VectorIndex::new();
        idx.insert(VectorRecord::new("iri:a", vec![1.0, 0.0])).unwrap();
        assert!(idx.search(&[1.0, 0.0], 0).is_empty());
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let mut idx = VectorIndex::with_dimensions(3);
        let err = idx
            .insert(VectorRecord::new("iri:a", vec![1.0, 0.0]))
            .expect_err("expected dimension mismatch");
        match err {
            VectorIndexError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 2);
            }
        }
    }

    #[test]
    fn first_insert_pins_dimension() {
        let mut idx = VectorIndex::new();
        idx.insert(VectorRecord::new("iri:a", vec![1.0, 0.0, 0.0])).unwrap();
        assert_eq!(idx.dimensions(), Some(3));
        let err = idx
            .insert(VectorRecord::new("iri:b", vec![1.0, 0.0]))
            .expect_err("expected dimension mismatch");
        assert!(matches!(err, VectorIndexError::DimensionMismatch { .. }));
    }

    #[test]
    fn len_clear_is_empty() {
        let mut idx = VectorIndex::new();
        assert!(idx.is_empty());
        idx.insert(VectorRecord::new("iri:a", vec![1.0])).unwrap();
        idx.insert(VectorRecord::new("iri:b", vec![0.5])).unwrap();
        assert_eq!(idx.len(), 2);
        idx.clear();
        assert!(idx.is_empty());
        // Dimension is retained across clear().
        assert_eq!(idx.dimensions(), Some(1));
    }

    #[test]
    fn cosine_handles_zero_vector() {
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 0.0]), 0.0);
    }
}
