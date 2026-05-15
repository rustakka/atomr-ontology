//! `TermExtractor` — propose candidate surface terms from a document.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_provenance::{Activity, AgentRef};

use crate::backend::{Backend, BackendError, Prompt};

/// A proposed surface term plus a confidence score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TermCandidate {
    /// The surface form.
    pub surface: String,
    /// Confidence score in `[0, 1]`.
    pub score: f32,
    /// Optional category hint (e.g. `"ORG"`, `"PERSON"`, `"CONCEPT"`).
    pub category: Option<String>,
    /// Optional context window around the mention.
    pub context: Option<String>,
}

impl TermCandidate {
    /// Build a candidate.
    pub fn new(surface: impl Into<String>, score: f32) -> Self {
        Self { surface: surface.into(), score, category: None, context: None }
    }

    /// Attach a category hint.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Attach a context window.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// The default system prompt used by [`TermExtractor`].
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an ontology engineer extracting candidate \
terms from a corpus. Output a JSON array of objects with fields \
{\"surface\": string, \"score\": number in [0,1], \"category\": optional string}. \
Return JSON only, no prose.";

/// Extract candidate terms from text using a [`Backend`].
#[derive(Clone)]
pub struct TermExtractor {
    backend: Arc<dyn Backend>,
    system_prompt: String,
    agent: AgentRef,
}

impl TermExtractor {
    /// Build a term extractor around a backend.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software("agent://atomr-ontology-extract/TermExtractor", "TermExtractor"),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Run extraction on `text`. Returns the candidates plus the
    /// activity record that captured the run.
    pub async fn extract(&self, text: &str) -> Result<(Vec<TermCandidate>, Activity), BackendError> {
        let activity = Activity::started("term-extraction")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()));
        let prompt =
            Prompt::user(format!("Document:\n---\n{text}\n---")).with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let parsed: Vec<TermCandidate> = parse_terms(&response).map_err(BackendError::Parse)?;
        Ok((parsed, activity.finish()))
    }
}

/// Parse a JSON-array response into terms. Tolerates surrounding
/// whitespace and code-fence wrappers (``` ... ```).
pub fn parse_terms(response: &str) -> Result<Vec<TermCandidate>, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

/// Strip a fenced code block (``` ... ```) wrapping, returning the
/// inner body. Exposed for use by other crates in the workspace
/// that need to parse JSON output of the same shape.
pub fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```") {
        // Strip optional `json` language tag and final fence.
        let rest = rest.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_');
        let rest = rest.trim_start_matches('\n');
        if let Some(end) = rest.rfind("```") {
            return &rest[..end];
        }
        return rest;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_array() {
        let s = r#"[{"surface":"Acme Inc.","score":0.97,"category":"ORG"}]"#;
        let parsed = parse_terms(s).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].surface, "Acme Inc.");
    }

    #[test]
    fn tolerates_code_fence() {
        let s = "```json\n[{\"surface\":\"X\",\"score\":0.5}]\n```";
        let parsed = parse_terms(s).unwrap();
        assert_eq!(parsed[0].surface, "X");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_terms("nope").is_err());
    }
}
