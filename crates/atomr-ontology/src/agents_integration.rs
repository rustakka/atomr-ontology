//! Adapters that lift `atomr_agents`-driven agents into the workflow
//! [`Backend`] contract.
//!
//! This module is the recommended seam for plugging agentic ontology
//! workflows on top of the [`atomr-agents`] crate (which itself uses
//! [`atomr-infer`] to talk to OpenAI, Anthropic, Candle, vLLM, and the
//! other supported providers). The canonical layering is:
//!
//! ```text
//! agentic workflow (TermExtractor, AxiomMiner, AgenticAxiomMiner, …)
//!         │  takes Arc<dyn Backend>  (or Arc<AgenticAgent> for the
//!         ▼                            multi-turn surface)
//!   Backend trait  (atomr-ontology-extract)
//!         │
//!         ▼
//!   AgentBackend / AgenticAgent (this module — recommended)
//!         │  drives
//!         ▼
//!   atomr_agents::Agent  (planning / tools / multi-turn)
//!         │  inference via
//!         ▼
//!   atomr_infer::Provider (OpenAI, Anthropic, Candle, vLLM, …)
//! ```
//!
//! Two surfaces are exposed:
//!
//! - [`AgentBackend`] (+ [`AgentDriver`]) — narrow single-turn shim,
//!   kept for back-compat. Implementors wrap an `atomr_agents::Agent`
//!   and answer one prompt at a time.
//! - [`AgenticAgent`] (+ [`AgenticDriver`]) — the richer multi-turn /
//!   tool-using surface that ontology induction workflows
//!   (`AgenticTaxonomyInducer`, `AgenticAxiomMiner` in
//!   `atomr-ontology-induce`) actually exercise. Re-exported from
//!   `atomr-ontology-extract::agentic` so the workflow crates can use
//!   the same types without depending on this umbrella.
//!
//! Both ultimately implement [`Backend`] so they remain drop-in for
//! the existing single-shot extractors.
//!
//! Only compiled when the `agents` feature is enabled.
//!
//! [`atomr-agents`]: https://github.com/rustakka/atomr-agents
//! [`atomr-infer`]: https://github.com/rustakka/atomr-infer

use std::sync::Arc;

use async_trait::async_trait;

use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};

pub use atomr_ontology_extract::agentic::{
    AgenticAgent, AgenticDriver, AgenticOutcome, AgenticSession, StopCondition, ToolCallRecord,
    ToolSpec, TurnRecord,
};

/// Built-in [`ToolSpec`] adapters over a live
/// [`OntologyStore`](atomr_ontology_store::OntologyStore).
///
/// Re-export of `atomr_ontology_extract::store_tools` for ergonomics
/// when wiring an [`AgenticAgent`] against the live store.
#[cfg(feature = "store")]
pub mod tools {
    pub use atomr_ontology_extract::store_tools::*;
}

/// Wrap an `atomr_agents`-driven agent as a [`Backend`].
///
/// This is the narrow single-turn surface. Use it when the workflow
/// already orchestrates its own loop over `Backend::complete` and only
/// needs the agent to answer one prompt per call (term extraction,
/// entity resolution, the simpler inducers). For multi-turn /
/// tool-using flows, reach for [`AgenticAgent`] instead.
pub struct AgentBackend {
    label: Arc<str>,
    inner: Arc<dyn AgentDriver>,
}

/// Bare-minimum surface needed from an `atomr_agents::Agent` to satisfy
/// the [`Backend`] trait. Implementors adapt the upstream API.
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
