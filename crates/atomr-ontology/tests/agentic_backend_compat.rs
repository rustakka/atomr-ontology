//! Asserts that an `AgenticAgent` is a drop-in `Backend` — so legacy
//! extractors (TermExtractor, EntityResolver, RelationExtractor, the
//! one-shot inducers) can be driven by an agentic driver without code
//! changes.

#![cfg(feature = "agents")]

use std::sync::Arc;

use async_trait::async_trait;

use atomr_ontology::agents_integration::{
    AgenticAgent, AgenticDriver, AgenticOutcome, AgenticSession,
};
use atomr_ontology::extract::backend::{Backend, BackendError, Prompt};
use atomr_ontology::extract::TermExtractor;

struct StaticDriver {
    text: String,
}

#[async_trait]
impl AgenticDriver for StaticDriver {
    async fn run_session(&self, _session: AgenticSession) -> Result<AgenticOutcome, BackendError> {
        Ok(AgenticOutcome::from_text(self.text.clone()))
    }

    async fn complete_one(&self, _prompt: Prompt) -> Result<String, BackendError> {
        Ok(self.text.clone())
    }
}

#[tokio::test]
async fn agentic_agent_drives_term_extractor() {
    let driver = Arc::new(StaticDriver {
        text: r#"[{"surface":"Acme","score":0.9}]"#.to_string(),
    });
    let agent = Arc::new(AgenticAgent::new("static", driver));
    // Legacy extractor accepts an Arc<dyn Backend>; AgenticAgent impls
    // Backend via complete_one, so this is a single-shot drop-in.
    let backend: Arc<dyn Backend> = agent.clone();
    let extractor = TermExtractor::new(backend);
    let (terms, _activity) = extractor.extract("Acme Inc.").await.expect("extract ok");
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].surface, "Acme");
}
