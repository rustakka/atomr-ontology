//! Higher-order axiom mining (functional, inverse-of, domain, range, …).

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::axiom::{Axiom, AxiomKind};
use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};
use atomr_ontology_extract::terms::strip_code_fence;
use atomr_ontology_provenance::{Activity, AgentRef};

/// A raw axiom proposal from the LLM. Mirrors [`AxiomKind`] but uses
/// strings everywhere for JSON friendliness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AxiomProposal {
    /// `SubClassOf(sub, sup)`.
    SubClassOf { sub: String, sup: String, score: f32 },
    /// `EquivalentClass(a, b)`.
    EquivalentClass { left: String, right: String, score: f32 },
    /// `DisjointWith(a, b)`.
    DisjointWith { left: String, right: String, score: f32 },
    /// `Domain(property, class)`.
    Domain { property: String, class: String, score: f32 },
    /// `Range(property, class)`.
    Range { property: String, class: String, score: f32 },
    /// `Functional(property)`.
    Functional { property: String, score: f32 },
    /// `InverseFunctional(property)`.
    InverseFunctional { property: String, score: f32 },
    /// `InverseOf(left, right)`.
    InverseOf { left: String, right: String, score: f32 },
    /// `Symmetric(property)`.
    Symmetric { property: String, score: f32 },
    /// `Transitive(property)`.
    Transitive { property: String, score: f32 },
}

impl AxiomProposal {
    /// Promote a proposal to a canonical [`Axiom`].
    pub fn into_axiom(self) -> Axiom {
        let kind = match self {
            AxiomProposal::SubClassOf { sub, sup, .. } => AxiomKind::SubClassOf { sub, sup },
            AxiomProposal::EquivalentClass { left, right, .. } => AxiomKind::EquivalentClass { left, right },
            AxiomProposal::DisjointWith { left, right, .. } => AxiomKind::DisjointWith { left, right },
            AxiomProposal::Domain { property, class, .. } => AxiomKind::Domain { property, class },
            AxiomProposal::Range { property, class, .. } => AxiomKind::Range { property, class },
            AxiomProposal::Functional { property, .. } => AxiomKind::Functional { property },
            AxiomProposal::InverseFunctional { property, .. } => AxiomKind::InverseFunctional { property },
            AxiomProposal::InverseOf { left, right, .. } => AxiomKind::InverseOf { left, right },
            AxiomProposal::Symmetric { property, .. } => AxiomKind::Symmetric { property },
            AxiomProposal::Transitive { property, .. } => AxiomKind::Transitive { property },
        };
        Axiom::new(kind)
    }
}

/// Default system prompt.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an ontology engineer proposing OWL axioms. \
Given a schema sketch and example assertions, return a JSON array of axiom proposals \
shaped as {\"kind\": one of [sub_class_of, equivalent_class, disjoint_with, domain, range, \
functional, inverse_functional, inverse_of, symmetric, transitive], plus the appropriate operand \
fields, plus a \"score\" in [0,1]}. JSON only, no prose.";

/// Mine axioms over a schema sketch + sample assertions.
#[derive(Clone)]
pub struct AxiomMiner {
    backend: Arc<dyn Backend>,
    system_prompt: String,
    agent: AgentRef,
}

impl AxiomMiner {
    /// Build a miner.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software("agent://atomr-ontology-induce/AxiomMiner", "AxiomMiner"),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Run mining; `context` should describe the current schema and
    /// sample assertions in a form the model can reason over.
    pub async fn mine(&self, context: &str) -> Result<(Vec<AxiomProposal>, Activity), BackendError> {
        let activity = Activity::started("axiom-mining")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()));
        let prompt = Prompt::user(format!("Context:\n{context}")).with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let parsed = parse_proposals(&response).map_err(BackendError::Parse)?;
        Ok((parsed, activity.finish()))
    }
}

/// Parse a JSON-array of axiom proposals.
pub fn parse_proposals(response: &str) -> Result<Vec<AxiomProposal>, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proposals() {
        let s = r#"[{"kind":"functional","property":"homepage","score":0.9},
                    {"kind":"sub_class_of","sub":"FormalOrganization","sup":"Organization","score":0.95}]"#;
        let parsed = parse_proposals(s).unwrap();
        assert_eq!(parsed.len(), 2);
        let axs: Vec<_> = parsed.into_iter().map(|p| p.into_axiom()).collect();
        assert!(matches!(axs[0].kind, AxiomKind::Functional { .. }));
        assert!(matches!(axs[1].kind, AxiomKind::SubClassOf { .. }));
    }
}
