//! Vector embedding + similarity-search layer for entity resolution.
//!
//! - `EmbeddingBackend` trait — pluggable embedder.
//! - `VectorIndex` — in-memory linear-scan index over node IRIs;
//!   HNSW or another ANN backend is a planned upgrade.
//! - `EmbeddingResolver` — entity-resolution pre-filter that proposes
//!   top-k similar candidates the LLM later disambiguates.

#![forbid(unsafe_code)]

pub mod backend;
pub mod index;
pub mod resolver;

pub use backend::{EmbeddingBackend, EmbeddingError, HashEmbedder};
pub use index::{VectorIndex, VectorIndexError, VectorRecord};
pub use resolver::EmbeddingResolver;
