//! Agentic axiom mining — multi-turn, tool-using variant of
//! [`crate::axioms::AxiomMiner`].
//!
//! Where [`AxiomMiner`](crate::axioms::AxiomMiner) issues a single LLM
//! call over a static context string, [`AgenticAxiomMiner`] drives an
//! [`AgenticSession`]
//! against an
//! [`AgenticAgent`] so
//! the LLM can:
//!
//! - Inspect the live ontology via the bundled store tools.
//! - Validate each proposed axiom family (domain/range, functional,
//!   inverse-of, …) against the schema before emitting it.
//! - Iterate / refine until the final-turn assistant message is a
//!   parseable JSON array of [`AxiomProposal`].
//!
//! Recommended layering: `AgenticAxiomMiner → AgenticAgent →
//! atomr_agents::Agent → atomr_infer::Provider`. See
//! [`docs/providers.md`](https://github.com/rustakka/atomr-ontology/blob/main/docs/providers.md)
//! for the canonical wiring.

use std::sync::Arc;

use atomr_ontology_extract::agentic::{AgenticAgent, AgenticSession, ToolSpec};
use atomr_ontology_extract::backend::{Backend, BackendError};
use atomr_ontology_provenance::{Activity, AgentRef};

use crate::axioms::{parse_proposals, AxiomProposal};

/// Default system prompt — extends
/// [`crate::axioms::DEFAULT_SYSTEM_PROMPT`] with explicit guidance to
/// use the available tools before committing.
pub const AGENTIC_SYSTEM_PROMPT: &str = "You are an ontology engineer proposing OWL axioms. \
Given a schema sketch and example assertions, you have tools to inspect the live ontology — use \
`class_exists`, `list_classes`, `list_edge_types`, `supertypes_of`, and `properties_of` to \
verify each axiom against the schema before proposing it. When you are finished, return a JSON \
array of axiom proposals shaped as {\"kind\": one of [sub_class_of, equivalent_class, \
disjoint_with, domain, range, functional, inverse_functional, inverse_of, symmetric, \
transitive], plus the appropriate operand fields, plus a \"score\" in [0,1]} as your final \
message. JSON only, no prose.";

/// Multi-turn axiom miner driven by an [`AgenticAgent`].
#[derive(Clone)]
pub struct AgenticAxiomMiner {
    agent: Arc<AgenticAgent>,
    system_prompt: String,
    tools: Vec<ToolSpec>,
    max_turns: u32,
    agent_ref: AgentRef,
}

impl AgenticAxiomMiner {
    /// Build a miner. `tools` is the agent's tool palette — pass
    /// [`atomr_ontology_extract::store_tools::default_store_tools`]
    /// to give the agent ontology-introspection power.
    pub fn new(agent: Arc<AgenticAgent>, tools: Vec<ToolSpec>) -> Self {
        Self {
            agent,
            system_prompt: AGENTIC_SYSTEM_PROMPT.to_string(),
            tools,
            max_turns: 12,
            agent_ref: AgentRef::software(
                "agent://atomr-ontology-induce/AgenticAxiomMiner",
                "AgenticAxiomMiner",
            ),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Override the per-session turn budget (default `12`).
    pub fn with_max_turns(mut self, n: u32) -> Self {
        self.max_turns = n;
        self
    }

    /// Run agentic mining; `context` should describe the current
    /// schema and sample assertions in a form the model can reason
    /// over. Returns the parsed axiom proposals and an [`Activity`]
    /// tracking the session.
    pub async fn mine(
        &self,
        context: &str,
    ) -> Result<(Vec<AxiomProposal>, Activity), BackendError> {
        let activity = Activity::started("axiom-mining-agentic")
            .by(self.agent_ref.clone())
            .with_attribute("backend", serde_json::json!(self.agent.label()));
        let session = AgenticSession::new(format!("Context:\n{context}"))
            .with_system(self.system_prompt.clone())
            .with_tools(self.tools.clone())
            .with_max_turns(self.max_turns);
        let outcome = self.agent.run(session).await?;
        let proposals = parse_proposals(&outcome.final_text).map_err(BackendError::Parse)?;
        let activity = activity
            .with_attribute("tool_calls", serde_json::json!(outcome.tool_invocations.len()))
            .with_attribute("turns", serde_json::json!(outcome.turns.len()))
            .finish();
        Ok((proposals, activity))
    }
}
