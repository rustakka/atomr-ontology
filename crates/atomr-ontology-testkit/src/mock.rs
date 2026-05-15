//! A deterministic [`Backend`] implementation that replays a queue
//! of pre-scripted responses.
//!
//! `MockBackend` is the analogue of `atomr_infer::testkit::MockRunner`
//! for the narrower [`Backend`] contract this crate uses for
//! extractors. Use it to drive unit tests and golden-output examples
//! without touching a network.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;

use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};

/// Programmable backend that returns prepared responses in order.
#[derive(Clone)]
pub struct MockBackend {
    label: Arc<str>,
    inner: Arc<Mutex<MockState>>,
}

struct MockState {
    queue: VecDeque<MockResponse>,
    captured: Vec<Prompt>,
}

enum MockResponse {
    Text(String),
    Error(BackendError),
}

impl MockBackend {
    /// Empty queue (every call yields a parse error).
    pub fn new() -> Self {
        Self::with_label("mock-backend")
    }

    /// Build with a custom label (visible in provenance).
    pub fn with_label(label: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            inner: Arc::new(Mutex::new(MockState { queue: VecDeque::new(), captured: Vec::new() })),
        }
    }

    /// Push a response that the next call will return.
    pub fn enqueue(&self, response: impl Into<String>) -> &Self {
        self.inner.lock().queue.push_back(MockResponse::Text(response.into()));
        self
    }

    /// Push a failure that the next call will return.
    pub fn enqueue_error(&self, error: BackendError) -> &Self {
        self.inner.lock().queue.push_back(MockResponse::Error(error));
        self
    }

    /// Inspect the prompts the mock has seen so far.
    pub fn captured(&self) -> Vec<Prompt> {
        self.inner.lock().captured.clone()
    }

    /// Convenience: enqueue a JSON value as a stringified response.
    pub fn enqueue_json<T: serde::Serialize>(&self, value: &T) -> &Self {
        let body = serde_json::to_string(value).expect("serializable");
        self.enqueue(body)
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for MockBackend {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        let mut guard = self.inner.lock();
        guard.captured.push(prompt);
        match guard.queue.pop_front() {
            Some(MockResponse::Text(s)) => Ok(s),
            Some(MockResponse::Error(e)) => Err(e),
            None => Err(BackendError::Other("mock backend queue is empty".into())),
        }
    }

    fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replays_in_order() {
        let mock = MockBackend::new();
        mock.enqueue("hello").enqueue("world");
        let a = mock.complete(Prompt::user("a")).await.unwrap();
        let b = mock.complete(Prompt::user("b")).await.unwrap();
        assert_eq!(a, "hello");
        assert_eq!(b, "world");
        assert_eq!(mock.captured().len(), 2);
    }

    #[tokio::test]
    async fn empty_queue_errors() {
        let mock = MockBackend::new();
        assert!(mock.complete(Prompt::user("x")).await.is_err());
    }
}
