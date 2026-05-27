//! Event-stream projection — one node per journal event, connected by
//! `successor` edges in chronological order.

use std::collections::BTreeMap;

use async_trait::async_trait;

use atomr_ontology_core::{Edge, Node, PropertyValue};
use atomr_ontology_store::r#trait::OntologyDelta;

use crate::batch::ActorBatch;
use crate::vocab;
use crate::ProjectionError;

use super::{ProjectionCtx, ProjectionKind, ProjectionStrategy};

/// Each `JournalEvent` becomes an `ActorEvent` node. Successive events
/// (within the batch) are linked by `successor` edges.
#[derive(Clone, Debug, Default)]
pub struct EventStreamProjection {
    label: String,
}

impl EventStreamProjection {
    /// Construct with default label.
    pub fn new() -> Self {
        Self { label: "event-stream".into() }
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl ProjectionStrategy for EventStreamProjection {
    fn label(&self) -> &str {
        if self.label.is_empty() {
            "event-stream"
        } else {
            &self.label
        }
    }

    fn kind(&self) -> ProjectionKind {
        ProjectionKind::EventStream
    }

    async fn project(
        &self,
        batch: &ActorBatch,
        ctx: &ProjectionCtx,
    ) -> Result<OntologyDelta, ProjectionError> {
        let mut delta = OntologyDelta::new();
        let mut prev_id = None;
        for event in &batch.events {
            let (id, iri) = ctx.iri.mint_event(event)?;
            let mut node = Node {
                id,
                iri,
                types: vec![vocab::NODE_EVENT.into()],
                properties: BTreeMap::new(),
            };
            node = node
                .with_property(
                    vocab::PROP_EVENT_KIND,
                    crate::strategy::journal_event_kind_str(&event.kind).to_owned(),
                )
                .with_property(vocab::PROP_ACTOR_ID, event.actor.as_str().to_owned())
                .with_property(vocab::PROP_CURSOR_VERSION, event.cursor.version as i64)
                .with_property(vocab::PROP_EVENT_AT, event.at.to_rfc3339())
                .with_property(vocab::PROP_SOURCE, ctx.source_label.clone());
            if !event.payload.is_null() {
                node = node.with_property(
                    vocab::PROP_EVENT_PAYLOAD,
                    PropertyValue::Json(event.payload.clone()),
                );
            }
            if let Some(path) = &event.path {
                node = node.with_property(vocab::PROP_PATH, path.render());
            }
            delta.nodes.push(node);

            if let Some(prev) = prev_id {
                delta.edges.push(Edge::between(prev, vocab::EDGE_SUCCESSOR, id));
            }
            prev_id = Some(id);
        }
        Ok(delta)
    }
}
