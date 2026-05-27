//! Snapshot-diff projection — keeps the last batch's identity set and
//! only projects newly-observed paths/events.

use std::collections::BTreeSet;
use std::sync::Mutex;

use async_trait::async_trait;

use atomr_ontology_core::{Edge, Node, PropertyValue};
use atomr_ontology_store::r#trait::OntologyDelta;

use crate::batch::ActorBatch;
use crate::vocab;
use crate::ProjectionError;

use super::{ProjectionCtx, ProjectionKind, ProjectionStrategy};

/// Remembers which path strings and which `(actor, cursor)` event keys
/// have been emitted, and only emits the new ones.
///
/// The strategy keeps internal mutable state, so it must be wrapped in
/// `Arc` to be reused across `project` calls.
#[derive(Debug, Default)]
pub struct SnapshotDiffProjection {
    label: String,
    seen_paths: Mutex<BTreeSet<String>>,
    seen_events: Mutex<BTreeSet<(String, u64)>>,
    seen_state_digests: Mutex<BTreeSet<(String, Option<String>)>>,
}

impl SnapshotDiffProjection {
    /// Fresh diff projection (knows nothing yet).
    pub fn new() -> Self {
        Self {
            label: "snapshot-diff".into(),
            seen_paths: Mutex::new(BTreeSet::new()),
            seen_events: Mutex::new(BTreeSet::new()),
            seen_state_digests: Mutex::new(BTreeSet::new()),
        }
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl ProjectionStrategy for SnapshotDiffProjection {
    fn label(&self) -> &str {
        if self.label.is_empty() {
            "snapshot-diff"
        } else {
            &self.label
        }
    }

    fn kind(&self) -> ProjectionKind {
        ProjectionKind::SnapshotDiff
    }

    async fn project(
        &self,
        batch: &ActorBatch,
        ctx: &ProjectionCtx,
    ) -> Result<OntologyDelta, ProjectionError> {
        let mut delta = OntologyDelta::new();

        for path in &batch.paths {
            let key = path.render();
            let inserted = self.seen_paths.lock().expect("path-set lock").insert(key.clone());
            if !inserted {
                continue;
            }
            let (id, iri) = ctx.iri.mint_actor(path)?;
            let mut node = Node {
                id,
                iri,
                types: vec![vocab::NODE_ACTOR.into()],
                properties: Default::default(),
            };
            node = node
                .with_property(vocab::PROP_PATH, key)
                .with_property(vocab::PROP_SOURCE, ctx.source_label.clone());
            delta.nodes.push(node);
        }

        let mut prev_event_id = None;
        for event in &batch.events {
            let actor_key = event.actor.as_str().to_owned();
            let key = (actor_key.clone(), event.cursor.version);
            let inserted = self.seen_events.lock().expect("event-set lock").insert(key);
            if !inserted {
                continue;
            }
            let (id, iri) = ctx.iri.mint_event(event)?;
            let mut node = Node {
                id,
                iri,
                types: vec![vocab::NODE_EVENT.into()],
                properties: Default::default(),
            };
            node = node
                .with_property(
                    vocab::PROP_EVENT_KIND,
                    crate::strategy::journal_event_kind_str(&event.kind).to_owned(),
                )
                .with_property(vocab::PROP_ACTOR_ID, actor_key)
                .with_property(vocab::PROP_CURSOR_VERSION, event.cursor.version as i64)
                .with_property(vocab::PROP_EVENT_AT, event.at.to_rfc3339())
                .with_property(vocab::PROP_SOURCE, ctx.source_label.clone());
            if !event.payload.is_null() {
                node = node.with_property(
                    vocab::PROP_EVENT_PAYLOAD,
                    PropertyValue::Json(event.payload.clone()),
                );
            }
            delta.nodes.push(node);
            if let Some(prev) = prev_event_id {
                delta.edges.push(Edge::between(prev, vocab::EDGE_SUCCESSOR, id));
            }
            prev_event_id = Some(id);
        }

        for state in &batch.states {
            let key = (state.actor.as_str().to_owned(), state.digest.clone());
            let inserted = self.seen_state_digests.lock().expect("state-set lock").insert(key);
            if !inserted {
                continue;
            }
            let state_node = Node {
                id: atomr_ontology_core::NodeId::content_address(
                    format!("state:{}:{:?}", state.actor.as_str(), state.digest).as_bytes(),
                ),
                iri: None,
                types: vec![vocab::NODE_STATE.into()],
                properties: [
                    (
                        vocab::PROP_ACTOR_ID.to_string(),
                        PropertyValue::string(state.actor.as_str()),
                    ),
                    (
                        vocab::PROP_STATE.to_string(),
                        PropertyValue::Json(state.payload.clone()),
                    ),
                    (
                        vocab::PROP_SOURCE.to_string(),
                        PropertyValue::string(ctx.source_label.clone()),
                    ),
                ]
                .into_iter()
                .collect(),
            };
            delta.nodes.push(state_node);
        }

        Ok(delta)
    }
}
