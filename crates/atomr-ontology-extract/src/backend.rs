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

use async_trait::async_trait;
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

/// The inference contract used by the extractors.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Run a single completion and return the full response text.
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError>;

    /// Human-readable label for tracing.
    fn label(&self) -> &str {
        "unnamed-backend"
    }
}
