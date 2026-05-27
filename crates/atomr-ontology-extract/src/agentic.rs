//! Multi-turn / tool-using agentic surface.
//!
//! Where [`Backend`] describes the narrow
//! single-completion contract, [`AgenticDriver`] describes the richer
//! contract used by ontology-induction workflows that need the LLM to
//! plan, call tools, and iterate against the live ontology before
//! committing a proposal.
//!
//! The canonical layering is:
//!
//! ```text
//! agentic workflow (AgenticTaxonomyInducer, AgenticAxiomMiner, …)
//!         │  takes Arc<AgenticAgent>
//!         ▼
//!   AgenticDriver  (this module)
//!         │  implemented over
//!         ▼
//!   atomr_agents::Agent   (planning / tools / multi-turn)
//!         │  inference via
//!         ▼
//!   atomr_infer::Provider (OpenAI, Anthropic, Candle, vLLM, …)
//! ```
//!
//! The contract is deliberately decoupled from upstream `atomr_agents`
//! types: we accept a generic [`AgenticDriver`] trait so the workspace
//! stays loosely coupled to upstream version churn. A thin user-side
//! adapter translates [`ToolSpec`] / [`AgenticSession`] into the
//! upstream agent / tool builders.
//!
//! [`AgenticAgent`] also implements [`Backend`]
//! (via [`AgenticDriver::complete_one`]) so the same handle can drive
//! both the new agentic inducers and the existing narrow extractors.

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::backend::{Backend, BackendError, Prompt};

/// A tool the agent can invoke during a session.
///
/// The handler is an async closure that takes a JSON arguments object
/// (matching `json_schema`) and returns a JSON result. The closure must
/// be `Send + Sync` so the agent loop can call it from any task.
#[derive(Clone)]
pub struct ToolSpec {
    /// Tool name (the identifier the model uses).
    pub name: Arc<str>,
    /// Human-readable tool description shown to the model.
    pub description: Arc<str>,
    /// JSON schema describing the tool's argument shape.
    pub json_schema: serde_json::Value,
    /// Async handler. Receives the parsed arguments, returns a JSON
    /// result the agent appends to the conversation as a tool message.
    pub handler: Arc<
        dyn Fn(serde_json::Value) -> BoxFuture<'static, Result<serde_json::Value, BackendError>>
            + Send
            + Sync,
    >,
}

impl ToolSpec {
    /// Construct a tool from its parts.
    pub fn new<F>(
        name: impl Into<Arc<str>>,
        description: impl Into<Arc<str>>,
        json_schema: serde_json::Value,
        handler: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> BoxFuture<'static, Result<serde_json::Value, BackendError>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            json_schema,
            handler: Arc::new(handler),
        }
    }
}

impl std::fmt::Debug for ToolSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSpec")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("json_schema", &self.json_schema)
            .finish()
    }
}

/// When the session should stop accepting more turns.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum StopCondition {
    /// Stop when the agent emits a turn with no tool calls.
    #[default]
    NoMoreToolCalls,
    /// Stop when the agent's final-text response parses as JSON
    /// matching the named schema discriminator. The driver is
    /// responsible for the actual matching; this just signals intent.
    FirstJsonMatching(String),
    /// Stop after exactly N turns regardless.
    FixedTurns(u32),
}

/// A single turn in an agent session — what was said, by which role.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnRecord {
    /// `"system"`, `"user"`, `"assistant"`, or `"tool"`.
    pub role: String,
    /// The text body for this turn (or the JSON-encoded tool result).
    pub text: String,
}

/// A record of one tool call the agent made during a session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Name of the tool invoked.
    pub tool: String,
    /// Arguments the agent passed (JSON).
    pub arguments: serde_json::Value,
    /// Result returned by the handler (JSON).
    pub result: serde_json::Value,
}

/// Description of one agent session — what to run.
#[derive(Clone, Debug)]
pub struct AgenticSession {
    /// Optional system prompt.
    pub system: Option<String>,
    /// Initial user message that seeds the loop.
    pub seed_user: String,
    /// Tools the agent may call.
    pub tools: Vec<ToolSpec>,
    /// Maximum number of agent turns before the driver must stop.
    pub max_turns: u32,
    /// Stop condition. See [`StopCondition`].
    pub stop_on: StopCondition,
}

impl AgenticSession {
    /// Start a new session with the given seed user message.
    pub fn new(seed_user: impl Into<String>) -> Self {
        Self {
            system: None,
            seed_user: seed_user.into(),
            tools: Vec::new(),
            max_turns: 8,
            stop_on: StopCondition::default(),
        }
    }

    /// Attach a system prompt.
    pub fn with_system(mut self, body: impl Into<String>) -> Self {
        self.system = Some(body.into());
        self
    }

    /// Add a tool.
    pub fn with_tool(mut self, tool: ToolSpec) -> Self {
        self.tools.push(tool);
        self
    }

    /// Replace the tool list.
    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    /// Override max turns.
    pub fn with_max_turns(mut self, n: u32) -> Self {
        self.max_turns = n;
        self
    }

    /// Override the stop condition.
    pub fn with_stop_on(mut self, stop: StopCondition) -> Self {
        self.stop_on = stop;
        self
    }
}

/// Result of running an agent session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgenticOutcome {
    /// Final assistant text emitted by the agent.
    pub final_text: String,
    /// Ordered transcript of every turn (system, user, assistant, tool).
    pub turns: Vec<TurnRecord>,
    /// Ordered record of every tool call the agent made.
    pub tool_invocations: Vec<ToolCallRecord>,
}

impl AgenticOutcome {
    /// Convenience constructor for drivers that don't produce a full
    /// transcript (e.g. when bridging single-turn completion).
    pub fn from_text(final_text: impl Into<String>) -> Self {
        Self { final_text: final_text.into(), turns: Vec::new(), tool_invocations: Vec::new() }
    }
}

/// The full agentic-driver contract. Implementors typically wrap an
/// `atomr_agents::Agent` (which uses an `atomr_infer` provider to talk
/// to the model).
#[async_trait]
pub trait AgenticDriver: Send + Sync {
    /// Run a multi-turn session and return its outcome.
    async fn run_session(&self, session: AgenticSession) -> Result<AgenticOutcome, BackendError>;

    /// Single-turn shortcut — used to satisfy the narrow [`Backend`]
    /// contract for callers that don't need the planning loop.
    async fn complete_one(&self, prompt: Prompt) -> Result<String, BackendError>;
}

/// An agentic [`Backend`] adapter.
///
/// Wraps an [`AgenticDriver`] and exposes both:
///
/// 1. The full [`AgenticDriver::run_session`] surface via
///    [`AgenticAgent::run`] for tool-using workflows.
/// 2. The narrow [`Backend::complete`] surface so the same handle can
///    drive any existing extractor unchanged.
#[derive(Clone)]
pub struct AgenticAgent {
    label: Arc<str>,
    inner: Arc<dyn AgenticDriver>,
}

impl AgenticAgent {
    /// Construct from a driver and label.
    pub fn new(label: impl Into<Arc<str>>, inner: Arc<dyn AgenticDriver>) -> Self {
        Self { label: label.into(), inner }
    }

    /// Run a multi-turn session.
    pub async fn run(&self, session: AgenticSession) -> Result<AgenticOutcome, BackendError> {
        self.inner.run_session(session).await
    }

    /// Underlying driver, for callers that want to share the same
    /// driver between an `AgenticAgent` and a custom adapter.
    pub fn driver(&self) -> Arc<dyn AgenticDriver> {
        self.inner.clone()
    }
}

#[async_trait]
impl Backend for AgenticAgent {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        self.inner.complete_one(prompt).await
    }

    fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    /// Toy driver used in unit tests — records sessions, replays a
    /// scripted final text, and pretends to call tools by hand.
    struct ScriptedDriver {
        final_text: String,
        scripted_tool_calls: Vec<(String, serde_json::Value)>,
        _seen: Mutex<Vec<AgenticSession>>,
    }

    impl ScriptedDriver {
        fn new(final_text: impl Into<String>, calls: Vec<(String, serde_json::Value)>) -> Self {
            Self {
                final_text: final_text.into(),
                scripted_tool_calls: calls,
                _seen: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AgenticDriver for ScriptedDriver {
        async fn run_session(
            &self,
            session: AgenticSession,
        ) -> Result<AgenticOutcome, BackendError> {
            let mut turns =
                vec![TurnRecord { role: "user".into(), text: session.seed_user.clone() }];
            let mut invocations = Vec::new();
            for (tool_name, args) in &self.scripted_tool_calls {
                let tool = session
                    .tools
                    .iter()
                    .find(|t| &*t.name == tool_name)
                    .ok_or_else(|| BackendError::Other(format!("no tool {tool_name}")))?;
                let result = (tool.handler)(args.clone()).await?;
                turns.push(TurnRecord {
                    role: "tool".into(),
                    text: serde_json::to_string(&result).unwrap(),
                });
                invocations.push(ToolCallRecord {
                    tool: tool_name.clone(),
                    arguments: args.clone(),
                    result,
                });
            }
            turns.push(TurnRecord { role: "assistant".into(), text: self.final_text.clone() });
            self._seen.lock().push(session);
            Ok(AgenticOutcome {
                final_text: self.final_text.clone(),
                turns,
                tool_invocations: invocations,
            })
        }

        async fn complete_one(&self, _prompt: Prompt) -> Result<String, BackendError> {
            Ok(self.final_text.clone())
        }
    }

    #[tokio::test]
    async fn agentic_agent_runs_session_with_tool() {
        let echo_tool = ToolSpec::new(
            "echo",
            "Echo back the args.",
            serde_json::json!({"type":"object"}),
            |args| Box::pin(async move { Ok(args) }),
        );
        let driver = Arc::new(ScriptedDriver::new(
            r#"[{"sub":"X","sup":"Y","score":0.9}]"#,
            vec![("echo".to_string(), serde_json::json!({"hello":"world"}))],
        ));
        let agent = AgenticAgent::new("test", driver);
        let session = AgenticSession::new("seed").with_tool(echo_tool);
        let outcome = agent.run(session).await.unwrap();
        assert_eq!(outcome.tool_invocations.len(), 1);
        assert_eq!(outcome.tool_invocations[0].tool, "echo");
        assert_eq!(outcome.tool_invocations[0].result, serde_json::json!({"hello":"world"}));
        assert!(outcome.final_text.contains("\"sub\":\"X\""));
    }

    #[tokio::test]
    async fn agentic_agent_impls_backend() {
        let driver = Arc::new(ScriptedDriver::new("hello", Vec::new()));
        let agent = AgenticAgent::new("be", driver);
        let response = agent.complete(Prompt::user("ping")).await.unwrap();
        assert_eq!(response, "hello");
        assert_eq!(Backend::label(&agent), "be");
    }
}
