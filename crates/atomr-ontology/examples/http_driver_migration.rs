//! Migration guide: replacing the deprecated `HttpDriver` with the
//! canonical `atomr-infer`-backed wiring.
//!
//! `HttpDriver` is deprecated as of v0.2 and slated for removal in v0.4.
//! The two recommended replacements, in order of preference:
//!
//! 1. **`AgentBackend` over `atomr-agents` over `atomr-infer`** —
//!    the canonical layering for agentic ontology workflows. Get tools,
//!    planning, multi-turn refinement, plus the full provider matrix
//!    (Anthropic, OpenAI, Gemini, LiteLLM, Candle, vLLM, …).
//!
//! 2. **`InferBackend` over `atomr-infer`** — single-shot completion
//!    against the same provider matrix, without the agent loop. Drop-in
//!    for `HttpDriver` callers that don't need tools or planning.
//!
//! This file is documentation-only: it doesn't compile without one of
//! the matching feature combinations, so the snippets are kept as
//! `ignore` doctests rather than a runnable `main`.
//!
//! See `docs/providers.md` for the full decision tree.

/// `HttpDriver` → `InferBackend` (single-shot).
///
/// **Before** (`features = ["http-driver"]`):
///
/// ```ignore
/// use std::sync::Arc;
/// use atomr_ontology::extract::Backend;
/// use atomr_ontology::http_driver::HttpDriver;
///
/// let backend: Arc<dyn Backend> =
///     Arc::new(HttpDriver::from_provider("openai", "gpt-4o-mini")?);
/// ```
///
/// **After** (`features = ["provider-openai"]`):
///
/// ```ignore
/// use std::sync::Arc;
/// use atomr_ontology::extract::Backend;
/// use atomr_ontology::infer_integration::{InferBackend, InferDriver};
///
/// // The InferDriver implementation lives in your own crate or a
/// // helper. It wraps an `atomr_infer::ModelRunner` configured for
/// // the OpenAI provider — see the `atomr-infer` docs for the
/// // matching builder.
/// let driver: Arc<dyn InferDriver> = my_openai_driver()?;
/// let backend: Arc<dyn Backend> = Arc::new(InferBackend::new(driver));
/// # fn my_openai_driver() -> Result<Arc<dyn InferDriver>, Box<dyn std::error::Error>> { unimplemented!() }
/// ```
///
/// The API-key env-var contract is unchanged: `atomr-infer`'s OpenAI
/// provider also reads `OPENAI_API_KEY` (plus the more granular
/// `OPENAI_BASE_URL` / `OPENAI_ORG` knobs).
pub fn migration_infer_backend() {}

/// `HttpDriver` → `AgentBackend` (recommended; agent loop + tools).
///
/// **Before** (`features = ["http-driver"]`):
///
/// ```ignore
/// use std::sync::Arc;
/// use atomr_ontology::extract::Backend;
/// use atomr_ontology::http_driver::HttpDriver;
///
/// let backend: Arc<dyn Backend> =
///     Arc::new(HttpDriver::from_provider("anthropic", "claude-3-5-sonnet")?);
/// let extractor = atomr_ontology::extract::TermExtractor::new(backend);
/// ```
///
/// **After** (`features = ["agents-with-anthropic"]`):
///
/// ```ignore
/// use std::sync::Arc;
/// use atomr_ontology::agents_integration::{AgenticAgent, AgenticDriver};
/// // Implement `AgenticDriver` over `atomr_agents::Agent` (which
/// // wraps an `atomr_infer::Provider` for Anthropic underneath).
/// let driver: Arc<dyn AgenticDriver> = my_anthropic_agent_driver()?;
/// let agent = Arc::new(AgenticAgent::new("anthropic", driver));
///
/// // Narrow single-shot use: `AgenticAgent` also impls `Backend`.
/// let extractor = atomr_ontology::extract::TermExtractor::new(agent.clone());
///
/// // Multi-turn / tool-using use: pass to one of the agentic inducers.
/// let inducer = atomr_ontology::induce::AgenticAxiomMiner::new(
///     agent,
///     Vec::new(), // or pass `default_store_tools(store)`
/// );
/// # fn my_anthropic_agent_driver() -> Result<Arc<dyn AgenticDriver>, Box<dyn std::error::Error>> { unimplemented!() }
/// ```
///
/// The agent driver is implemented in your own crate (or a thin
/// adapter crate) since `atomr-agents` is an external sibling crate
/// whose `Agent` builders evolve faster than the ontology crate.
pub fn migration_agent_backend() {}

fn main() {
    // This example is documentation-only — see the items above.
    println!("See module-level docs for the HttpDriver migration guide.");
}
