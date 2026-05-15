//! Adapter that lifts an `atomr_agents::Agent` into a [`Backend`].
//!
//! Only compiled when the `agents` feature is enabled.

use std::sync::Arc;

use async_trait::async_trait;

use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};

/// Wrap an `atomr_agents::agent::AgentRef` as a [`Backend`].
///
/// The agent must expose a single-turn completion surface. We do
/// not depend on the full pipeline machinery here — calls go through
/// the agent's [`Tool`] interface so this stays minimal.
pub struct AgentBackend {
    label: Arc<str>,
    inner: Arc<dyn AgentDriver>,
}

/// Internal trait describing the bare minimum we need from an agent
/// reference; the actual implementor adapts the upstream API.
#[async_trait]
pub trait AgentDriver: Send + Sync {
    /// Single-turn completion.
    async fn run(&self, prompt: Prompt) -> Result<String, BackendError>;
}

impl AgentBackend {
    /// Build a backend from an opaque driver.
    pub fn new(label: impl Into<Arc<str>>, inner: Arc<dyn AgentDriver>) -> Self {
        Self { label: label.into(), inner }
    }
}

#[async_trait]
impl Backend for AgentBackend {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        self.inner.run(prompt).await
    }

    fn label(&self) -> &str {
        &self.label
    }
}
