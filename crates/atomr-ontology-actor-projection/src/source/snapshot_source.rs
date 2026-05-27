//! [`ActorPersistenceSource`] backed by an
//! [`atomr_ontology_persist::Checkpointer`].
//!
//! Each node in the upstream snapshot is interpreted as an actor record
//! if it carries a `path` property (a `/`-joined supervision path).
//! Property names follow the conventions in
//! [`crate::vocab`](crate::vocab):
//!
//! - `path` — the actor's supervision path (required).
//! - `state` — JSON-encoded latest state (optional).
//! - `event_kind` + `event_payload` — if present, this snapshot also
//!   contributed a journal event.
//!
//! This source is generic: any subsystem that wants to drive the
//! projector can write its actor records into an `Ontology` snapshot
//! and persist it through a `Checkpointer`.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use atomr_ontology_core::Node;
use atomr_ontology_persist::Checkpointer;

use super::{
    ActorId, ActorPersistenceSource, Cursor, JournalEvent, JournalEventKind, SerializedState,
    SupervisionPath,
};
use crate::SourceError;

/// Wrap a [`Checkpointer`] as an [`ActorPersistenceSource`].
pub struct SnapshotActorPersistenceSource {
    label: String,
    checkpointer: Arc<dyn Checkpointer>,
    last_seen_version: Arc<RwLock<u64>>,
}

impl SnapshotActorPersistenceSource {
    /// Wrap an existing checkpointer.
    pub fn new(label: impl Into<String>, checkpointer: Arc<dyn Checkpointer>) -> Self {
        Self {
            label: label.into(),
            checkpointer,
            last_seen_version: Arc::new(RwLock::new(0)),
        }
    }

    fn extract_path(node: &Node) -> Option<SupervisionPath> {
        let p = node.properties.get(crate::vocab::PROP_PATH)?;
        let s = match p {
            atomr_ontology_core::PropertyValue::String(s) => s.as_str(),
            _ => return None,
        };
        Some(SupervisionPath::parse(s))
    }

    fn extract_actor(node: &Node) -> ActorId {
        if let Some(atomr_ontology_core::PropertyValue::String(s)) =
            node.properties.get(crate::vocab::PROP_ACTOR_ID)
        {
            return ActorId::new(s.clone());
        }
        ActorId::new(node.id.to_string())
    }

    fn extract_event_kind(node: &Node) -> Option<JournalEventKind> {
        let s = match node.properties.get(crate::vocab::PROP_EVENT_KIND)? {
            atomr_ontology_core::PropertyValue::String(s) => s.as_str(),
            _ => return None,
        };
        Some(match s {
            "created" => JournalEventKind::Created,
            "state_changed" => JournalEventKind::StateChanged,
            "completed" => JournalEventKind::Completed,
            "terminated" => JournalEventKind::Terminated,
            other => JournalEventKind::Custom(other.to_owned()),
        })
    }
}

#[async_trait]
impl ActorPersistenceSource for SnapshotActorPersistenceSource {
    fn label(&self) -> &str {
        &self.label
    }

    async fn paths(&self) -> Result<Vec<SupervisionPath>, SourceError> {
        let snap = self
            .checkpointer
            .load()
            .await
            .map_err(|e| SourceError::Io(e.to_string()))?;
        let Some(snap) = snap else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for node in snap.ontology.nodes.values() {
            if let Some(path) = Self::extract_path(node) {
                out.push(path);
            }
        }
        Ok(out)
    }

    async fn journal(&self, since: &Cursor) -> Result<Vec<JournalEvent>, SourceError> {
        let snap = self
            .checkpointer
            .load()
            .await
            .map_err(|e| SourceError::Io(e.to_string()))?;
        let Some(snap) = snap else { return Ok(Vec::new()) };
        if snap.version <= since.version {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut seq = since.version;
        for node in snap.ontology.nodes.values() {
            let Some(kind) = Self::extract_event_kind(node) else { continue };
            seq = seq.saturating_add(1);
            let actor = Self::extract_actor(node);
            let payload = node
                .properties
                .get(crate::vocab::PROP_EVENT_PAYLOAD)
                .and_then(|v| match v {
                    atomr_ontology_core::PropertyValue::Json(j) => Some(j.clone()),
                    atomr_ontology_core::PropertyValue::String(s) => Some(serde_json::Value::String(s.clone())),
                    _ => None,
                })
                .unwrap_or(serde_json::Value::Null);
            let cursor = Cursor::at(seq);
            let mut event = JournalEvent::new(cursor, actor, kind).with_payload(payload);
            if let Some(path) = Self::extract_path(node) {
                event = event.with_path(path);
            }
            out.push(event);
        }
        *self.last_seen_version.write() = snap.version;
        Ok(out)
    }

    async fn state(&self, actor: &ActorId) -> Result<Option<SerializedState>, SourceError> {
        let snap = self
            .checkpointer
            .load()
            .await
            .map_err(|e| SourceError::Io(e.to_string()))?;
        let Some(snap) = snap else { return Ok(None) };
        for node in snap.ontology.nodes.values() {
            if Self::extract_actor(node) != *actor {
                continue;
            }
            if let Some(atomr_ontology_core::PropertyValue::Json(j)) =
                node.properties.get(crate::vocab::PROP_STATE)
            {
                return Ok(Some(SerializedState::new(actor.clone(), j.clone())));
            }
        }
        Ok(None)
    }

    async fn cursor(&self) -> Result<Cursor, SourceError> {
        let snap = self
            .checkpointer
            .load()
            .await
            .map_err(|e| SourceError::Io(e.to_string()))?;
        Ok(Cursor::at(snap.map(|s| s.version).unwrap_or(0)))
    }
}
