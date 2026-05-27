//! Pluggable embedding backends.
//!
//! An [`EmbeddingBackend`] turns text into a fixed-dimension vector.
//! Implementations may call a remote service, a local model, or — as
//! with [`HashEmbedder`] — derive the vector deterministically from a
//! hash. Hash-derived vectors are not semantically meaningful but are
//! stable and dimensioned, which makes them well-suited to tests and
//! offline development.

use async_trait::async_trait;
use thiserror::Error;

/// Error type returned by [`EmbeddingBackend`] implementations.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// A transport-layer failure (HTTP, gRPC, IPC).
    #[error("embedding transport error: {0}")]
    Transport(String),
    /// Any other backend-specific failure.
    #[error("embedding error: {0}")]
    Other(String),
}

/// Pluggable embedding backend.
///
/// Implementations should return vectors of consistent dimension — the
/// value reported by [`EmbeddingBackend::dimensions`]. Index code
/// enforces that invariant on insert.
#[async_trait]
pub trait EmbeddingBackend: Send + Sync {
    /// Embed a single string.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Embed a batch of strings. The default implementation issues
    /// sequential calls to [`embed`](Self::embed); backends that
    /// support native batching should override this method.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.embed(text).await?);
        }
        Ok(out)
    }

    /// Dimension of the vectors returned by this backend.
    fn dimensions(&self) -> usize;

    /// Human-readable label, used in tracing / provenance.
    fn label(&self) -> &str;
}

/// Deterministic hash-based embedder.
///
/// Derives a fixed-dimension vector from BLAKE3 keyed expansion of the
/// input text. The output is stable across runs and platforms, which
/// makes it useful for tests and for offline development where a real
/// embedding service is unavailable. It is **not** a semantically
/// meaningful embedding — two paraphrases hash to unrelated vectors.
#[derive(Clone, Debug)]
pub struct HashEmbedder {
    /// Number of f32 components in the produced vectors.
    pub dim: usize,
    label: String,
}

impl HashEmbedder {
    /// Construct a hash-based embedder with the requested dimension.
    pub fn new(dim: usize) -> Self {
        Self { dim, label: format!("hash-embedder/{dim}") }
    }

    /// Expand the BLAKE3 hash of `text` into `dim` f32 components in
    /// `[-1.0, 1.0]`.
    fn expand(&self, text: &str) -> Vec<f32> {
        // BLAKE3's extendable output gives us as many bytes as we
        // need; map every 4 bytes to a single f32 in [-1, 1].
        let mut hasher = blake3::Hasher::new();
        hasher.update(text.as_bytes());
        let mut reader = hasher.finalize_xof();
        let byte_len = self.dim.saturating_mul(4);
        let mut bytes = vec![0u8; byte_len];
        reader.fill(&mut bytes);
        let mut out = Vec::with_capacity(self.dim);
        for chunk in bytes.chunks_exact(4) {
            let raw = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            // Map u32 onto [-1.0, 1.0] deterministically.
            let normalized = (raw as f64) / (u32::MAX as f64);
            let signed = (normalized * 2.0) - 1.0;
            out.push(signed as f32);
        }
        out
    }
}

#[async_trait]
impl EmbeddingBackend for HashEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(self.expand(text))
    }

    fn dimensions(&self) -> usize {
        self.dim
    }

    fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_embedder_reports_dimensions() {
        let embedder = HashEmbedder::new(32);
        assert_eq!(embedder.dimensions(), 32);
        let v = embedder.embed("hello").await.unwrap();
        assert_eq!(v.len(), 32);
    }

    #[tokio::test]
    async fn hash_embedder_is_deterministic() {
        let embedder = HashEmbedder::new(16);
        let a = embedder.embed("acme").await.unwrap();
        let b = embedder.embed("acme").await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn hash_embedder_batch_matches_embed() {
        let embedder = HashEmbedder::new(8);
        let texts = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let batch = embedder.embed_batch(&texts).await.unwrap();
        assert_eq!(batch.len(), 3);
        for (i, text) in texts.iter().enumerate() {
            let single = embedder.embed(text).await.unwrap();
            assert_eq!(batch[i], single);
        }
    }

    #[tokio::test]
    async fn hash_embedder_label_contains_dim() {
        let embedder = HashEmbedder::new(4);
        assert!(embedder.label().contains('4'));
    }

    #[tokio::test]
    async fn hash_embedder_components_in_unit_range() {
        let embedder = HashEmbedder::new(64);
        let v = embedder.embed("range-check").await.unwrap();
        for x in v {
            assert!((-1.0..=1.0).contains(&x), "component out of range: {x}");
        }
    }
}
