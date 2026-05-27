//! Agentic subclass induction — multi-turn, tool-using variant of
//! [`crate::taxonomy::TaxonomyInducer`].
//!
//! Where [`TaxonomyInducer`](crate::taxonomy::TaxonomyInducer) issues a
//! single LLM call against a static list of class names,
//! [`AgenticTaxonomyInducer`] runs an
//! [`AgenticSession`]
//! against an
//! [`AgenticAgent`] so
//! the LLM can:
//!
//! - Inspect the live ontology via the bundled store tools
//!   ([`atomr_ontology_extract::store_tools::default_store_tools`]).
//! - Detect cycles before proposing a `sub :> sup` relation.
//! - Iterate / refine until the final-turn assistant message is a
//!   parseable JSON array of [`SubclassProposal`].
//!
//! Recommended layering: `AgenticTaxonomyInducer → AgenticAgent →
//! atomr_agents::Agent → atomr_infer::Provider`. See
//! [`docs/providers.md`](https://github.com/rustakka/atomr-ontology/blob/main/docs/providers.md)
//! for the canonical wiring.

use std::sync::Arc;

use atomr_ontology_extract::agentic::{AgenticAgent, AgenticSession, ToolSpec};
use atomr_ontology_extract::backend::{Backend, BackendError};
use atomr_ontology_provenance::{Activity, AgentRef};

use crate::taxonomy::{parse_proposals, SubclassProposal};

/// Default system prompt — extends
/// [`crate::taxonomy::DEFAULT_SYSTEM_PROMPT`] with explicit guidance
/// to use the available tools before committing.
pub const AGENTIC_SYSTEM_PROMPT: &str = "You are an ontology engineer proposing subclass-of relations. \
For each candidate class, decide which of the other candidates is a more general supertype. \
You have tools to inspect the live ontology — use `class_exists`, `supertypes_of`, and \
`subclasses_of` to avoid proposing cycles or duplicating existing relations. When you are \
finished, return a JSON array of {\"sub\": string, \"sup\": string, \"score\": number in [0,1]} \
as your final message. JSON only, no prose.";

/// Multi-turn taxonomy inducer driven by an [`AgenticAgent`].
#[derive(Clone)]
pub struct AgenticTaxonomyInducer {
    agent: Arc<AgenticAgent>,
    system_prompt: String,
    tools: Vec<ToolSpec>,
    max_turns: u32,
    agent_ref: AgentRef,
}

impl AgenticTaxonomyInducer {
    /// Build an inducer. `tools` is the agent's tool palette — pass
    /// [`atomr_ontology_extract::store_tools::default_store_tools`]
    /// to give the agent ontology-introspection power.
    pub fn new(agent: Arc<AgenticAgent>, tools: Vec<ToolSpec>) -> Self {
        Self {
            agent,
            system_prompt: AGENTIC_SYSTEM_PROMPT.to_string(),
            tools,
            max_turns: 8,
            agent_ref: AgentRef::software(
                "agent://atomr-ontology-induce/AgenticTaxonomyInducer",
                "AgenticTaxonomyInducer",
            ),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Override the per-session turn budget (default `8`).
    pub fn with_max_turns(mut self, n: u32) -> Self {
        self.max_turns = n;
        self
    }

    /// Run agentic induction over a candidate class list. Returns the
    /// parsed subclass proposals and an [`Activity`] tracking the
    /// session (tool-call count, turn count, backend label).
    pub async fn induce(
        &self,
        candidate_classes: &[String],
    ) -> Result<(Vec<SubclassProposal>, Activity), BackendError> {
        let activity = Activity::started("taxonomy-induction-agentic")
            .by(self.agent_ref.clone())
            .with_attribute("backend", serde_json::json!(self.agent.label()))
            .with_attribute("candidate_count", serde_json::json!(candidate_classes.len()));
        let body = serde_json::to_string(candidate_classes)
            .map_err(|e| BackendError::Other(format!("serialize: {e}")))?;
        let session = AgenticSession::new(format!("Classes: {body}"))
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
