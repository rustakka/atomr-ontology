//! PROV-O-aligned provenance for ontology assertions.
//!
//! Every fact written into an
//! [`OntologyStore`](https://docs.rs/atomr-ontology-store) carries a
//! [`ProvenanceId`] that resolves to an [`Activity`] in this module's
//! ledger. The shape of the types follows [PROV-O][prov-o]:
//!
//! - [`Activity`] — something that happened over a span of time
//!   (an LLM call, a manual edit, an inducer run).
//! - [`ProvAgent`] — the agent responsible for the activity. Named
//!   `ProvAgent` rather than `Agent` to disambiguate from
//!   `atomr_agents::Agent`.
//! - [`ProvEntity`] — a snapshot of data that was used or produced.
//! - Lineage edges: [`WasGeneratedBy`], [`WasDerivedFrom`],
//!   [`WasAttributedTo`], [`Used`].
//!
//! [prov-o]: https://www.w3.org/TR/prov-o/

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use atomr_ontology_core::ProvenanceId;

/// A PROV-O activity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    /// Stable identifier.
    pub id: ProvenanceId,
    /// Human-readable label.
    pub label: String,
    /// Activity start.
    pub started_at: DateTime<Utc>,
    /// Activity end (None while running).
    pub ended_at: Option<DateTime<Utc>>,
    /// The agent responsible for the activity.
    pub agent: Option<AgentRef>,
    /// Free-form attributes (provider, model, pipeline stage, etc).
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl Activity {
    /// Build a freshly-started activity.
    pub fn started(label: impl Into<String>) -> Self {
        Self {
            id: ProvenanceId::new_random(),
            label: label.into(),
            started_at: Utc::now(),
            ended_at: None,
            agent: None,
            attributes: BTreeMap::new(),
        }
    }

    /// Mark the activity finished now.
    pub fn finish(mut self) -> Self {
        self.ended_at = Some(Utc::now());
        self
    }

    /// Attach an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Attach an agent reference.
    pub fn by(mut self, agent: AgentRef) -> Self {
        self.agent = Some(agent);
        self
    }
}

/// PROV-O agent kind.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// A human author.
    Person,
    /// A software agent (LLM, atomr-agents Agent, deterministic
    /// program).
    Software,
    /// An organization.
    Organization,
}

/// Reference to a PROV-O agent.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentRef {
    /// Stable identifier (free-form, e.g. `"agent://term-extractor"`).
    pub id: String,
    /// Agent kind.
    pub kind: AgentKind,
    /// Human-readable label.
    pub label: String,
}

impl AgentRef {
    /// Build a software-agent reference.
    pub fn software(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), kind: AgentKind::Software, label: label.into() }
    }

    /// Build a person reference.
    pub fn person(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), kind: AgentKind::Person, label: label.into() }
    }
}

/// Reference to an externally-known prov:Agent (kept as a separate
/// type so it can carry richer metadata in the future without
/// breaking [`AgentRef`]).
pub type ProvAgent = AgentRef;

/// A PROV-O entity — a snapshot of data that participated in an
/// activity (e.g. a source document, a record, an extracted node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProvEntity {
    /// Stable identifier.
    pub id: ProvenanceId,
    /// Human-readable label.
    pub label: String,
    /// Optional content-hash digest of the entity body.
    pub digest: Option<String>,
    /// Free-form attributes.
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl ProvEntity {
    /// New entity with a label and optional digest.
    pub fn new(label: impl Into<String>, digest: Option<String>) -> Self {
        Self { id: ProvenanceId::new_random(), label: label.into(), digest, attributes: BTreeMap::new() }
    }
}

/// `prov:wasGeneratedBy` — entity was produced by an activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WasGeneratedBy {
    /// The generated entity.
    pub entity: ProvenanceId,
    /// The generating activity.
    pub activity: ProvenanceId,
}

/// `prov:wasDerivedFrom` — entity is derived from another entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WasDerivedFrom {
    /// The new entity.
    pub derived: ProvenanceId,
    /// The source entity.
    pub source: ProvenanceId,
}

/// `prov:wasAttributedTo` — entity is attributed to an agent.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WasAttributedTo {
    /// The entity.
    pub entity: ProvenanceId,
    /// The agent the entity is attributed to.
    pub agent: AgentRef,
}

/// `prov:used` — activity consumed an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Used {
    /// The using activity.
    pub activity: ProvenanceId,
    /// The used entity.
    pub entity: ProvenanceId,
}

/// In-memory provenance ledger.
///
/// `ProvenanceLog` is suitable for tests, examples, and ontology
/// runs that fit in memory. Larger deployments should persist the
/// log to an [`OntologyStore`](https://docs.rs/atomr-ontology-store)
/// or external store.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProvenanceLog {
    /// Activities keyed by id.
    pub activities: BTreeMap<ProvenanceId, Activity>,
    /// Entities keyed by id.
    pub entities: BTreeMap<ProvenanceId, ProvEntity>,
    /// `wasGeneratedBy` records.
    pub generations: Vec<WasGeneratedBy>,
    /// `wasDerivedFrom` records.
    pub derivations: Vec<WasDerivedFrom>,
    /// `wasAttributedTo` records.
    pub attributions: Vec<WasAttributedTo>,
    /// `used` records.
    pub uses: Vec<Used>,
}

impl ProvenanceLog {
    /// Empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an activity; returns its id.
    pub fn record_activity(&mut self, activity: Activity) -> ProvenanceId {
        let id = activity.id;
        self.activities.insert(id, activity);
        id
    }

    /// Record an entity; returns its id.
    pub fn record_entity(&mut self, entity: ProvEntity) -> ProvenanceId {
        let id = entity.id;
        self.entities.insert(id, entity);
        id
    }

    /// Append a `wasGeneratedBy` edge.
    pub fn generated(&mut self, entity: ProvenanceId, activity: ProvenanceId) {
        self.generations.push(WasGeneratedBy { entity, activity });
    }

    /// Append a `wasDerivedFrom` edge.
    pub fn derived(&mut self, derived: ProvenanceId, source: ProvenanceId) {
        self.derivations.push(WasDerivedFrom { derived, source });
    }

    /// Append a `wasAttributedTo` edge.
    pub fn attributed(&mut self, entity: ProvenanceId, agent: AgentRef) {
        self.attributions.push(WasAttributedTo { entity, agent });
    }

    /// Append a `used` edge.
    pub fn used(&mut self, activity: ProvenanceId, entity: ProvenanceId) {
        self.uses.push(Used { activity, entity });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_activity_and_finish() {
        let mut log = ProvenanceLog::new();
        let agent = AgentRef::software("agent://term-extractor", "TermExtractor");
        let act = Activity::started("term-extraction")
            .by(agent.clone())
            .with_attribute("model", serde_json::json!("gpt-4o"))
            .finish();
        let id = log.record_activity(act);
        let act_ref = log.activities.get(&id).unwrap();
        assert!(act_ref.ended_at.is_some());
        assert_eq!(act_ref.agent.as_ref().unwrap().label, "TermExtractor");
    }

    #[test]
    fn lineage_edges_accumulate() {
        let mut log = ProvenanceLog::new();
        let a = log.record_entity(ProvEntity::new("doc.txt", None));
        let b = log.record_entity(ProvEntity::new("Term: ACME", None));
        let act = log.record_activity(Activity::started("extract"));
        log.used(act, a);
        log.generated(b, act);
        log.derived(b, a);
        assert_eq!(log.uses.len(), 1);
        assert_eq!(log.generations.len(), 1);
        assert_eq!(log.derivations.len(), 1);
    }
}
