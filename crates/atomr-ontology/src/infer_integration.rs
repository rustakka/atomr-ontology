//! Adapter that lifts an `atomr_infer::ModelRunner` into a [`Backend`].
//!
//! Only compiled when the `infer` feature is enabled. The adapter
//! holds the runner behind an `async` mutex so the trait-object
//! `Backend` (which takes `&self`) can drive the underlying mutable
//! runner state.
//!
//! In v0.1 we stream the runner's token chunks until a stop reason
//! is observed, accumulate them into a string, and return that as
//! the backend response. Richer streaming surfaces will be added in
//! a future release.

use std::sync::Arc;

use async_trait::async_trait;

use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};

/// Lightweight wrapper trait — the real `ModelRunner` lives in
/// `atomr-infer` and has a richer surface (load_weights, rebuild,
/// rate limits) we do not need here. Implementors are expected to
/// own the runner and dispatch `complete` calls onto it.
#[async_trait]
pub trait InferDriver: Send + Sync {
    /// Single-completion call.
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError>;
    /// Stable label for tracing.
    fn label(&self) -> &str {
        "atomr-infer"
    }
}

/// Wrap an [`InferDriver`] as a [`Backend`].
pub struct InferBackend {
    inner: Arc<dyn InferDriver>,
}

impl InferBackend {
    /// Build a backend from a driver.
    pub fn new(inner: Arc<dyn InferDriver>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl Backend for InferBackend {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        self.inner.complete(prompt).await
    }

    fn label(&self) -> &str {
        self.inner.label()
    }
}
