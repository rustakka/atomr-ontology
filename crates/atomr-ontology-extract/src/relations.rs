//! `RelationExtractor` — propose object-property edges between resolved entities.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::{Edge, NodeId};
use atomr_ontology_provenance::{Activity, AgentRef};

use crate::backend::{Backend, BackendError, Prompt};
use crate::entities::EntityCandidate;
use crate::terms::strip_code_fence;

/// A proposed object-property assertion between two entities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationCandidate {
    /// Source surface (matches an entity surface).
    pub source: String,
    /// Edge label.
    pub label: String,
    /// Target surface (matches an entity surface).
    pub target: String,
    /// Confidence score in `[0, 1]`.
    pub score: f32,
}

/// The default system prompt used by [`RelationExtractor`].
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are an ontology engineer proposing typed relations between entities. \
For the document and entity list provided, return a JSON array of \
{\"source\": string, \"label\": string, \"target\": string, \"score\": number in [0,1]} objects. \
Use only entity surfaces from the supplied list. Return JSON only, no prose.";

/// Propose relations from a document + an entity catalog.
#[derive(Clone)]
pub struct RelationExtractor {
    backend: Arc<dyn Backend>,
    system_prompt: String,
    agent: AgentRef,
}

impl RelationExtractor {
    /// Build a relation extractor.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software(
                "agent://atomr-ontology-extract/RelationExtractor",
                "RelationExtractor",
            ),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Extract relations.
    pub async fn extract(
        &self,
        text: &str,
        entities: &[EntityCandidate],
    ) -> Result<(Vec<RelationCandidate>, Activity), BackendError> {
        let activity = Activity::started("relation-extraction")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()))
            .with_attribute("entity_count", serde_json::json!(entities.len()));
        let body =
            serde_json::to_string(entities).map_err(|e| BackendError::Other(format!("serialize: {e}")))?;
        let prompt = Prompt::user(format!("Document:\n---\n{text}\n---\nEntities: {body}"))
            .with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let parsed = parse_relations(&response).map_err(BackendError::Parse)?;
        Ok((parsed, activity.finish()))
    }

    /// Convert a relation candidate into an [`Edge`] by looking up
    /// the source / target node ids from a surface ⇒ id map.
    pub fn into_edges(
        candidates: &[RelationCandidate],
        surface_to_id: &std::collections::HashMap<String, NodeId>,
    ) -> Vec<Edge> {
        candidates
            .iter()
            .filter_map(|c| {
                let source = surface_to_id.get(&c.source)?;
                let target = surface_to_id.get(&c.target)?;
                Some(Edge::between(*source, c.label.clone(), *target))
            })
            .collect()
    }
}

/// Parse a JSON-array response into relations.
pub fn parse_relations(response: &str) -> Result<Vec<RelationCandidate>, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parses_relations() {
        let s = r#"[{"source":"Acme","label":"memberOf","target":"Globex","score":0.9}]"#;
        let p = parse_relations(s).unwrap();
        assert_eq!(p[0].label, "memberOf");
    }

    #[test]
    fn into_edges_filters_unknown_surfaces() {
        let mut map = HashMap::new();
        let a = NodeId::new_random();
        let b = NodeId::new_random();
        map.insert("Acme".to_string(), a);
        map.insert("Globex".to_string(), b);
        let cs = vec![
            RelationCandidate {
                source: "Acme".into(),
                label: "memberOf".into(),
                target: "Globex".into(),
                score: 0.9,
            },
            RelationCandidate {
                source: "Unknown".into(),
                label: "memberOf".into(),
                target: "Globex".into(),
                score: 0.5,
            },
        ];
        let edges = RelationExtractor::into_edges(&cs, &map);
        assert_eq!(edges.len(), 1);
    }
}
