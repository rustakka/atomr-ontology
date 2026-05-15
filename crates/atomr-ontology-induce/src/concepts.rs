//! Concept formation — cluster synonymous terms into candidate
//! `NodeType` candidates.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::schema::NodeType;
use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};
use atomr_ontology_extract::terms::{strip_code_fence, TermCandidate};
use atomr_ontology_provenance::{Activity, AgentRef};

/// A single cluster of synonymous surface forms with a proposed
/// canonical name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConceptCluster {
    /// Proposed canonical type name.
    pub name: String,
    /// Surface forms that fell into this cluster.
    pub members: Vec<String>,
    /// Optional human description.
    pub description: Option<String>,
    /// Optional confidence score.
    #[serde(default)]
    pub score: f32,
}

impl ConceptCluster {
    /// Convert a cluster into a [`NodeType`] candidate.
    pub fn into_node_type(self) -> NodeType {
        let mut ty = NodeType::new(self.name);
        if let Some(d) = self.description {
            ty = ty.with_description(d);
        }
        ty
    }
}

/// Default system prompt.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an ontology engineer clustering surface terms into \
concepts (candidate classes). For each cluster, pick a canonical name. \
Return a JSON array of \
{\"name\": string, \"members\": [string], \"description\": optional string, \"score\": optional number}. \
JSON only.";

/// Cluster surface terms into candidate concepts.
#[derive(Clone)]
pub struct ConceptFormer {
    backend: Arc<dyn Backend>,
    system_prompt: String,
    agent: AgentRef,
}

impl ConceptFormer {
    /// Build a former.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software("agent://atomr-ontology-induce/ConceptFormer", "ConceptFormer"),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Run clustering.
    pub async fn cluster(
        &self,
        terms: &[TermCandidate],
    ) -> Result<(Vec<ConceptCluster>, Activity), BackendError> {
        let activity = Activity::started("concept-formation")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()))
            .with_attribute("term_count", serde_json::json!(terms.len()));
        let body =
            serde_json::to_string(terms).map_err(|e| BackendError::Other(format!("serialize: {e}")))?;
        let prompt = Prompt::user(format!("Terms: {body}")).with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let parsed = parse_clusters(&response).map_err(BackendError::Parse)?;
        Ok((parsed, activity.finish()))
    }
}

/// Parse a JSON-array response into clusters.
pub fn parse_clusters(response: &str) -> Result<Vec<ConceptCluster>, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clusters() {
        let s = r#"[{"name":"Organization","members":["Org","Company","Firm"],"description":"A formal organization","score":0.92}]"#;
        let cs = parse_clusters(s).unwrap();
        assert_eq!(cs[0].name, "Organization");
        let nt = cs[0].clone().into_node_type();
        assert_eq!(nt.name, "Organization");
        assert!(nt.description.is_some());
    }
}
