//! Subclass-axiom induction.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::axiom::{Axiom, AxiomKind};
use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};
use atomr_ontology_extract::terms::strip_code_fence;
use atomr_ontology_provenance::{Activity, AgentRef};

/// A `(sub, sup)` subclass proposal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubclassProposal {
    /// Subclass name (the more specific class).
    pub sub: String,
    /// Superclass name.
    pub sup: String,
    /// Confidence in `[0, 1]`.
    pub score: f32,
}

/// Default system prompt.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an ontology engineer proposing subclass-of relations. \
For each candidate class, decide which of the other candidates is a more general supertype. \
Return a JSON array of {\"sub\": string, \"sup\": string, \"score\": number in [0,1]}. \
Return JSON only, no prose.";

/// Induce subclass axioms over a candidate class list.
#[derive(Clone)]
pub struct TaxonomyInducer {
    backend: Arc<dyn Backend>,
    system_prompt: String,
    agent: AgentRef,
}

impl TaxonomyInducer {
    /// Build an inducer.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software("agent://atomr-ontology-induce/TaxonomyInducer", "TaxonomyInducer"),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Run induction over a candidate class list.
    pub async fn induce(
        &self,
        candidate_classes: &[String],
    ) -> Result<(Vec<SubclassProposal>, Activity), BackendError> {
        let activity = Activity::started("taxonomy-induction")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()))
            .with_attribute("candidate_count", serde_json::json!(candidate_classes.len()));
        let body = serde_json::to_string(candidate_classes)
            .map_err(|e| BackendError::Other(format!("serialize: {e}")))?;
        let prompt = Prompt::user(format!("Classes: {body}")).with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let parsed = parse_proposals(&response).map_err(BackendError::Parse)?;
        Ok((parsed, activity.finish()))
    }

    /// Promote subclass proposals to canonical axioms (with the
    /// induction activity recorded as provenance).
    pub fn into_axioms(
        proposals: &[SubclassProposal],
        provenance: Option<atomr_ontology_provenance::ProvenanceId>,
    ) -> Vec<Axiom> {
        proposals
            .iter()
            .map(|p| {
                let mut a = Axiom::new(AxiomKind::SubClassOf { sub: p.sub.clone(), sup: p.sup.clone() });
                if let Some(pid) = provenance {
                    a = a.with_provenance(pid);
                }
                a
            })
            .collect()
    }
}

/// Parse a JSON-array response.
pub fn parse_proposals(response: &str) -> Result<Vec<SubclassProposal>, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_promotes() {
        let s = r#"[{"sub":"FormalOrganization","sup":"Organization","score":0.95}]"#;
        let ps = parse_proposals(s).unwrap();
        let axs = TaxonomyInducer::into_axioms(&ps, None);
        assert_eq!(axs.len(), 1);
        match &axs[0].kind {
            AxiomKind::SubClassOf { sub, sup } => {
                assert_eq!(sub, "FormalOrganization");
                assert_eq!(sup, "Organization");
            }
            _ => panic!("expected SubClassOf"),
        }
    }
}
