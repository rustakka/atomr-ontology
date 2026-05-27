//! Smoke test for the agentic inducers — runs a scripted
//! `AgenticDriver` that pretends to call the store tools, then
//! parses the final-turn JSON.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;

use atomr_ontology_core::{Iri, Node, NodeType};
use atomr_ontology_extract::agentic::{
    AgenticAgent, AgenticDriver, AgenticOutcome, AgenticSession, ToolCallRecord, TurnRecord,
};
use atomr_ontology_extract::backend::{BackendError, Prompt};
use atomr_ontology_extract::store_tools::default_store_tools;
use atomr_ontology_induce::{AgenticAxiomMiner, AgenticTaxonomyInducer};
use atomr_ontology_store::{MemStore, OntologyStore};

/// Scripted driver that, on each session, walks every tool with the
/// supplied arguments (recording the real results) and then returns
/// `final_text` verbatim.
struct ScriptedDriver {
    final_text: String,
    tool_calls: Vec<(String, serde_json::Value)>,
    sessions_seen: Mutex<usize>,
}

#[async_trait]
impl AgenticDriver for ScriptedDriver {
    async fn run_session(&self, session: AgenticSession) -> Result<AgenticOutcome, BackendError> {
        let mut turns = vec![TurnRecord { role: "user".into(), text: session.seed_user.clone() }];
        let mut invocations = Vec::new();
        for (name, args) in &self.tool_calls {
            let tool = session
                .tools
                .iter()
                .find(|t| &*t.name == name)
                .ok_or_else(|| BackendError::Other(format!("missing tool {name}")))?;
            let result = (tool.handler)(args.clone()).await?;
            turns.push(TurnRecord {
                role: "tool".into(),
                text: serde_json::to_string(&result).unwrap(),
            });
            invocations.push(ToolCallRecord {
                tool: name.clone(),
                arguments: args.clone(),
                result,
            });
        }
        turns.push(TurnRecord { role: "assistant".into(), text: self.final_text.clone() });
        *self.sessions_seen.lock() += 1;
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

fn org_store() -> Arc<dyn OntologyStore> {
    let mut o = atomr_ontology_core::Ontology::new();
    o.schema.declare_node_type(NodeType::new("Organization").with_description("formal org"));
    o.schema.declare_node_type(NodeType::new("FormalOrganization").with_supertype("Organization"));
    o.upsert_node(
        Node::from_iri(Iri::from_unchecked("https://example.org/Acme"), "Organization")
            .with_property("name", "Acme"),
    );
    Arc::new(MemStore::from_ontology(o))
}

#[tokio::test]
async fn agentic_taxonomy_inducer_uses_store_tools_then_parses_final_json() {
    let store = org_store();
    let tools = default_store_tools(store);
    let driver = Arc::new(ScriptedDriver {
        final_text: r#"[{"sub":"FormalOrganization","sup":"Organization","score":0.95}]"#.into(),
        tool_calls: vec![
            ("class_exists".into(), serde_json::json!({"name":"Organization"})),
            ("supertypes_of".into(), serde_json::json!({"class":"FormalOrganization"})),
        ],
        sessions_seen: Mutex::new(0),
    });
    let agent = Arc::new(AgenticAgent::new("scripted", driver.clone()));
    let inducer = AgenticTaxonomyInducer::new(agent, tools);
    let (proposals, activity) = inducer
        .induce(&["Organization".to_string(), "FormalOrganization".to_string()])
        .await
        .expect("induce ok");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].sub, "FormalOrganization");
    assert_eq!(proposals[0].sup, "Organization");
    assert_eq!(*driver.sessions_seen.lock(), 1);
    // The activity tracks the tool-call and turn counts the agent loop reported.
    let tool_calls = activity
        .attributes
        .get("tool_calls")
        .and_then(|v| v.as_u64())
        .expect("tool_calls attribute");
    assert_eq!(tool_calls, 2);
}

#[tokio::test]
async fn agentic_axiom_miner_uses_store_tools_then_parses_final_json() {
    let store = org_store();
    let tools = default_store_tools(store);
    let driver = Arc::new(ScriptedDriver {
        final_text: r#"[{"kind":"sub_class_of","sub":"FormalOrganization","sup":"Organization","score":0.9},
                        {"kind":"functional","property":"name","score":0.7}]"#
            .into(),
        tool_calls: vec![
            ("list_classes".into(), serde_json::json!({})),
            ("properties_of".into(), serde_json::json!({"class":"Organization"})),
        ],
        sessions_seen: Mutex::new(0),
    });
    let agent = Arc::new(AgenticAgent::new("scripted", driver));
    let miner = AgenticAxiomMiner::new(agent, tools);
    let (proposals, _activity) =
        miner.mine("Schema: Organization, FormalOrganization").await.expect("mine ok");
    assert_eq!(proposals.len(), 2);
}
