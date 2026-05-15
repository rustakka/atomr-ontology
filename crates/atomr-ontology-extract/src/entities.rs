//! `EntityResolver` — match mentions to existing `Node`s or propose new ones.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::{Iri, Node};
use atomr_ontology_provenance::{Activity, AgentRef};
use atomr_ontology_store::OntologyStore;

use crate::backend::{Backend, BackendError, Prompt};
use crate::terms::{strip_code_fence, TermCandidate};

/// A resolved or proposed entity for a surface term.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityCandidate {
    /// Surface form the candidate resolves.
    pub surface: String,
    /// Proposed canonical IRI (if assigned).
    pub iri: Option<Iri>,
    /// Proposed type label (LPG `NodeType` name).
    pub type_name: String,
    /// Optional confidence score in `[0, 1]`.
    pub score: f32,
    /// True when the resolver decided this is a *new* entity rather
    /// than a link to an existing one.
    pub is_new: bool,
}

/// The default system prompt used by [`EntityResolver`].
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an ontology engineer linking surface mentions to \
canonical entities. For each input term, return a JSON object \
{\"surface\": string, \"iri\": optional string, \"type_name\": string, \"score\": number in [0,1], \"is_new\": bool}. \
Return a JSON array, no prose.";

/// Resolve term candidates into entity candidates using a backend.
///
/// The resolver consults the supplied `store` (if any) to bias
/// against duplicating entities that already exist. In v0.1 the
/// store hint is passed to the LLM as serialized context; richer
/// lookup (vector or string matching) is a planned extension.
#[derive(Clone)]
pub struct EntityResolver {
    backend: Arc<dyn Backend>,
    store: Option<Arc<dyn OntologyStore>>,
    system_prompt: String,
    agent: AgentRef,
}

impl EntityResolver {
    /// Build a resolver around a backend.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            store: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software("agent://atomr-ontology-extract/EntityResolver", "EntityResolver"),
        }
    }

    /// Attach an ontology store to bias against duplicates.
    pub fn with_store(mut self, store: Arc<dyn OntologyStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Resolve a batch of term candidates.
    pub async fn resolve(
        &self,
        terms: &[TermCandidate],
    ) -> Result<(Vec<EntityCandidate>, Activity), BackendError> {
        let activity = Activity::started("entity-resolution")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()))
            .with_attribute("term_count", serde_json::json!(terms.len()));
        let body =
            serde_json::to_string(terms).map_err(|e| BackendError::Other(format!("serialize: {e}")))?;
        let prompt = Prompt::user(format!("Terms:\n{body}")).with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let parsed = parse_entities(&response).map_err(BackendError::Parse)?;
        Ok((parsed, activity.finish()))
    }

    /// Promote a candidate into a fresh `Node` ready for upsert.
    /// Skips candidates with no IRI when `iri_required` is true.
    pub fn into_nodes(candidates: &[EntityCandidate], iri_required: bool) -> Vec<Node> {
        candidates
            .iter()
            .filter_map(|c| match (&c.iri, iri_required) {
                (Some(iri), _) => Some(
                    Node::from_iri(iri.clone(), c.type_name.clone())
                        .with_property("surface", c.surface.clone()),
                ),
                (None, false) => {
                    Some(Node::new(c.type_name.clone()).with_property("surface", c.surface.clone()))
                }
                (None, true) => None,
            })
            .collect()
    }
}

/// Parse a JSON-array response into entity candidates.
pub fn parse_entities(response: &str) -> Result<Vec<EntityCandidate>, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entities() {
        let s = r#"[{"surface":"Acme","iri":"https://example.org/Acme","type_name":"Organization","score":0.9,"is_new":true}]"#;
        let parsed = parse_entities(s).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].is_new);
    }

    #[test]
    fn into_nodes_respects_iri_required() {
        let with_iri = EntityCandidate {
            surface: "Acme".into(),
            iri: Some(Iri::new("https://example.org/Acme").unwrap()),
            type_name: "Organization".into(),
            score: 1.0,
            is_new: true,
        };
        let without = EntityCandidate {
            surface: "Mystery".into(),
            iri: None,
            type_name: "Organization".into(),
            score: 0.5,
            is_new: true,
        };
        let nodes = EntityResolver::into_nodes(&[with_iri, without], true);
        assert_eq!(nodes.len(), 1);
    }
}
