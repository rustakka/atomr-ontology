//! `RecordExtractor` — convert structured / semi-structured input
//! (CSV rows, JSON documents) into flat [`Record`]s for downstream
//! resolution.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::{Iri, PropertyValue, Record};
use atomr_ontology_provenance::{Activity, AgentRef};

use crate::backend::{Backend, BackendError, Prompt};
use crate::terms::strip_code_fence;

/// JSON-friendly view of a [`Record`] as emitted by an LLM.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawRecord {
    /// Optional canonical IRI.
    pub iri: Option<String>,
    /// LPG type label.
    pub type_name: String,
    /// Property bag (string → string for LLM friendliness).
    #[serde(default)]
    pub properties: std::collections::BTreeMap<String, serde_json::Value>,
    /// Outbound edges `(label, target_iri)`.
    #[serde(default)]
    pub outbound: Vec<(String, String)>,
    /// Source citation.
    pub source: Option<String>,
}

impl RawRecord {
    /// Convert to a canonical [`Record`].
    pub fn into_record(self) -> Result<Record, String> {
        let mut record = Record::new(self.type_name);
        if let Some(iri) = self.iri {
            record = record.with_iri(Iri::new(iri).map_err(|e| e.to_string())?);
        }
        for (k, v) in self.properties {
            let value = match v {
                serde_json::Value::String(s) => PropertyValue::String(s),
                serde_json::Value::Number(n) if n.is_i64() => PropertyValue::Integer(n.as_i64().unwrap()),
                serde_json::Value::Number(n) => PropertyValue::Float(n.as_f64().unwrap_or(0.0)),
                serde_json::Value::Bool(b) => PropertyValue::Bool(b),
                serde_json::Value::Null => PropertyValue::Null,
                other => PropertyValue::Json(other),
            };
            record = record.with_property(k, value);
        }
        for (label, iri) in self.outbound {
            record = record.with_outbound(label, Iri::new(iri).map_err(|e| e.to_string())?);
        }
        if let Some(source) = self.source {
            record = record.with_source(source);
        }
        Ok(record)
    }
}

/// The default system prompt used by [`RecordExtractor`].
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are an ontology engineer turning a structured row into an LPG record. \
Return a JSON object with fields {\"iri\": optional string, \"type_name\": string, \
\"properties\": object, \"outbound\": array of [label, target_iri] pairs, \"source\": optional string}. \
JSON only, no prose.";

/// Extract one record per input row.
#[derive(Clone)]
pub struct RecordExtractor {
    backend: Arc<dyn Backend>,
    system_prompt: String,
    agent: AgentRef,
}

impl RecordExtractor {
    /// Build a record extractor.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            agent: AgentRef::software("agent://atomr-ontology-extract/RecordExtractor", "RecordExtractor"),
        }
    }

    /// Override the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Extract a single record from arbitrary text.
    pub async fn extract(&self, row: &str) -> Result<(Record, Activity), BackendError> {
        let activity = Activity::started("record-extraction")
            .by(self.agent.clone())
            .with_attribute("backend", serde_json::json!(self.backend.label()));
        let prompt = Prompt::user(format!("Row:\n{row}")).with_system(self.system_prompt.clone());
        let response = self.backend.complete(prompt).await?;
        let raw = parse_record(&response).map_err(BackendError::Parse)?;
        let record = raw.into_record().map_err(BackendError::Parse)?;
        Ok((record, activity.finish()))
    }
}

/// Parse a single JSON object into a [`RawRecord`].
pub fn parse_record(response: &str) -> Result<RawRecord, String> {
    let trimmed = strip_code_fence(response.trim());
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_record() {
        let s = r#"{"iri":"https://example.org/Acme","type_name":"Organization","properties":{"name":"Acme","founded":1995},"outbound":[["hasMember","https://example.org/Bob"]],"source":"row#1"}"#;
        let raw = parse_record(s).unwrap();
        let record = raw.into_record().unwrap();
        assert_eq!(record.type_name, "Organization");
        assert_eq!(record.outbound.len(), 1);
        assert!(record.iri.is_some());
    }
}
