//! The `Backend` trait — the narrow inference contract every
//! extractor depends on.
//!
//! `Backend` is intentionally smaller than
//! `atomr_agents::InferenceClient` or `atomr_infer::ModelRunner`:
//! it exists so this crate stays decoupled from the agents / infer
//! generics, and so the testkit can plug in a deterministic mock
//! without dragging in the full runtime stack.
//!
//! Adapters from `atomr_infer::ModelRunner` and `atomr_agents::Agent`
//! live in `atomr-ontology-testkit` and the umbrella facade.
//!
//! The trait surface includes default implementations of:
//! - `batch_complete` — concurrent fan-out over a slice of prompts
//!   using `futures::future::join_all`. Concrete drivers can override
//!   to call native batch APIs.
//! - `stream_complete` — token-by-token streaming. Default impl
//!   wraps `complete` and yields a single chunk; HTTP/SSE drivers
//!   override to yield real tokens.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::Stream;
use parking_lot::Mutex;
use thiserror::Error;

/// Errors raised by a backend.
#[derive(Debug, Error)]
pub enum BackendError {
    /// Network / transport failure.
    #[error("transport: {0}")]
    Transport(String),
    /// Model refused or filtered the request.
    #[error("filtered: {0}")]
    Filtered(String),
    /// Output did not parse as expected.
    #[error("parse: {0}")]
    Parse(String),
    /// Catch-all.
    #[error("{0}")]
    Other(String),
}

/// A prompt to send to the backend.
#[derive(Clone, Debug)]
pub struct Prompt {
    /// Optional system prompt.
    pub system: Option<String>,
    /// User-facing prompt body.
    pub user: String,
    /// Maximum number of output tokens (advisory).
    pub max_tokens: Option<u32>,
}

impl Prompt {
    /// Build a prompt with just a user body.
    pub fn user(body: impl Into<String>) -> Self {
        Self { system: None, user: body.into(), max_tokens: None }
    }

    /// Attach a system prompt.
    pub fn with_system(mut self, body: impl Into<String>) -> Self {
        self.system = Some(body.into());
        self
    }

    /// Set the max-tokens advisory.
    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = Some(n);
        self
    }
}

/// A streaming token (or chunk) emitted by a backend.
#[derive(Clone, Debug)]
pub struct StreamChunk {
    /// The text fragment.
    pub text: String,
    /// `true` when this chunk is the last in the stream.
    pub done: bool,
}

/// A boxed `Stream` of [`StreamChunk`]s.
pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<StreamChunk, BackendError>> + Send>>;

/// The inference contract used by the extractors.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Run a single completion and return the full response text.
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError>;

    /// Run a batch of completions. Default fans out concurrently via
    /// `futures::future::join_all`; drivers that support native batch
    /// APIs (vLLM, batched ONNX) should override.
    async fn batch_complete(&self, prompts: Vec<Prompt>) -> Vec<Result<String, BackendError>> {
        use futures::future::join_all;
        let futs = prompts.into_iter().map(|p| self.complete(p));
        join_all(futs).await
    }

    /// Stream a completion. Default impl wraps `complete` and yields
    /// a single terminal chunk; drivers with SSE/WebSocket transports
    /// should override to yield incremental tokens.
    async fn stream_complete(&self, prompt: Prompt) -> Result<ChunkStream, BackendError> {
        let text = self.complete(prompt).await?;
        let chunk = StreamChunk { text, done: true };
        Ok(Box::pin(futures::stream::once(async move { Ok(chunk) })))
    }

    /// Human-readable label for tracing.
    fn label(&self) -> &str {
        "unnamed-backend"
    }
}

/// Cache policy for [`CachedBackend`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CachePolicy {
    /// Do not cache.
    None,
    /// Cache keyed by content hash; unbounded.
    ContentAddressed,
    /// LRU cache with the given capacity.
    Lru(usize),
}

/// Backend wrapper that caches `complete` responses by prompt content.
///
/// Streaming and batch calls fall through to the inner backend uncached.
pub struct CachedBackend<B: Backend> {
    inner: B,
    policy: CachePolicy,
    cache: Mutex<HashMap<u64, String>>,
    lru_order: Mutex<Vec<u64>>,
}

impl<B: Backend> CachedBackend<B> {
    /// Wrap `inner` with the given policy.
    pub fn new(inner: B, policy: CachePolicy) -> Self {
        Self { inner, policy, cache: Mutex::new(HashMap::new()), lru_order: Mutex::new(Vec::new()) }
    }

    /// Return cache hit count (for tests / introspection).
    pub fn cache_size(&self) -> usize {
        self.cache.lock().len()
    }

    fn key(prompt: &Prompt) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        prompt.system.hash(&mut h);
        prompt.user.hash(&mut h);
        prompt.max_tokens.hash(&mut h);
        h.finish()
    }
}

#[async_trait]
impl<B: Backend> Backend for CachedBackend<B> {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        if matches!(self.policy, CachePolicy::None) {
            return self.inner.complete(prompt).await;
        }
        let key = Self::key(&prompt);
        if let Some(hit) = self.cache.lock().get(&key).cloned() {
            return Ok(hit);
        }
        let response = self.inner.complete(prompt).await?;
        let mut cache = self.cache.lock();
        if let CachePolicy::Lru(cap) = self.policy {
            let mut order = self.lru_order.lock();
            if cache.len() >= cap {
                if let Some(evict) = order.first().copied() {
                    cache.remove(&evict);
                    order.remove(0);
                }
            }
            order.push(key);
        }
        cache.insert(key, response.clone());
        Ok(response)
    }

    fn label(&self) -> &str {
        self.inner.label()
    }
}

/// Convenience: wrap any backend in an LRU cache of the given size.
pub fn lru_cached<B: Backend>(backend: B, capacity: usize) -> CachedBackend<B> {
    CachedBackend::new(backend, CachePolicy::Lru(capacity))
}

/// Convenience: wrap any backend in an unbounded content-addressed cache.
pub fn content_cached<B: Backend>(backend: B) -> CachedBackend<B> {
    CachedBackend::new(backend, CachePolicy::ContentAddressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingBackend {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl Backend for CountingBackend {
        async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!("echo:{}", prompt.user))
        }
    }

    #[tokio::test]
    async fn cached_backend_short_circuits_on_repeat() {
        let inner = CountingBackend { calls: AtomicUsize::new(0) };
        let backend = CachedBackend::new(inner, CachePolicy::ContentAddressed);
        let p = Prompt::user("ping");
        let a = backend.complete(p.clone()).await.unwrap();
        let b = backend.complete(p.clone()).await.unwrap();
        assert_eq!(a, b);
        // Second call must be cached.
        assert_eq!(backend.cache_size(), 1);
    }

    #[tokio::test]
    async fn lru_cache_evicts_oldest() {
        let inner = CountingBackend { calls: AtomicUsize::new(0) };
        let backend = CachedBackend::new(inner, CachePolicy::Lru(2));
        backend.complete(Prompt::user("a")).await.unwrap();
        backend.complete(Prompt::user("b")).await.unwrap();
        backend.complete(Prompt::user("c")).await.unwrap(); // evicts "a"
        assert_eq!(backend.cache_size(), 2);
    }

    #[tokio::test]
    async fn batch_complete_fans_out() {
        let inner = CountingBackend { calls: AtomicUsize::new(0) };
        let prompts = vec![Prompt::user("a"), Prompt::user("b"), Prompt::user("c")];
        let results = inner.batch_complete(prompts).await;
        assert_eq!(results.len(), 3);
        for r in results {
            assert!(r.is_ok());
        }
    }

    #[tokio::test]
    async fn stream_default_yields_one_chunk() {
        let inner = CountingBackend { calls: AtomicUsize::new(0) };
        let mut s = inner.stream_complete(Prompt::user("hi")).await.unwrap();
        let mut total = 0;
        while let Some(item) = s.next().await {
            let chunk = item.unwrap();
            total += 1;
            if chunk.done {
                break;
            }
        }
        assert_eq!(total, 1);
    }
}

// Helper so trait objects (`Arc<dyn Backend>`) can be cached too.
#[async_trait]
impl Backend for Arc<dyn Backend> {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        (**self).complete(prompt).await
    }
    async fn batch_complete(&self, prompts: Vec<Prompt>) -> Vec<Result<String, BackendError>> {
        (**self).batch_complete(prompts).await
    }
    async fn stream_complete(&self, prompt: Prompt) -> Result<ChunkStream, BackendError> {
        (**self).stream_complete(prompt).await
    }
    fn label(&self) -> &str {
        (**self).label()
    }
}
