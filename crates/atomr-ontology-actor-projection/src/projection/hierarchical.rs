//! Hierarchical projection — projects supervision paths into a
//! Workflow → Run → Step → Actor tree of nodes connected by
//! `supervises` edges.

use std::collections::BTreeMap;

use async_trait::async_trait;

use atomr_ontology_core::{Edge, Node, NodeId, PropertyValue};
use atomr_ontology_store::r#trait::OntologyDelta;

use crate::batch::ActorBatch;
use crate::source::SupervisionPath;
use crate::vocab::{self, type_for_segment};
use crate::ProjectionError;

use super::{ProjectionCtx, ProjectionKind, ProjectionStrategy};

/// One node per path segment; `supervises` edges connect adjacent
/// depths.
///
/// State blobs become `ActorState` nodes attached by `holdsState`
/// edges. Events are projected as `ActorEvent` nodes with `emitted`
/// edges from the actor.
#[derive(Clone, Debug, Default)]
pub struct HierarchicalProjection {
    label: String,
}

impl HierarchicalProjection {
    /// Construct with default label.
    pub fn new() -> Self {
        Self { label: "hierarchical".into() }
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl ProjectionStrategy for HierarchicalProjection {
    fn label(&self) -> &str {
        if self.label.is_empty() {
            "hierarchical"
        } else {
            &self.label
        }
    }

    fn kind(&self) -> ProjectionKind {
        ProjectionKind::Hierarchical
    }

    async fn project(
        &self,
        batch: &ActorBatch,
        ctx: &ProjectionCtx,
    ) -> Result<OntologyDelta, ProjectionError> {
        let mut delta = OntologyDelta::new();
        // Map from path prefix → node id so we share intermediate
        // segments across paths and events.
        let mut prefix_to_id: BTreeMap<String, NodeId> = BTreeMap::new();

        for path in &batch.paths {
            insert_path(path, ctx, &mut delta, &mut prefix_to_id)?;
        }
        for event in &batch.events {
            if let Some(path) = &event.path {
                insert_path(path, ctx, &mut delta, &mut prefix_to_id)?;
            }
            let (event_id, event_iri) = ctx.iri.mint_event(event)?;
            let mut node = Node {
                id: event_id,
                iri: event_iri,
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
            delta.nodes.push(node);
            // Wire emitted edge if we know the actor node.
            if let Some(path) = &event.path {
                let actor_id = prefix_to_id.get(&path.render()).copied();
                if let Some(actor_id) = actor_id {
                    delta.edges.push(Edge::between(actor_id, vocab::EDGE_EMITTED, event_id));
                }
            }
        }
        for state in &batch.states {
            // Find an actor node by id-via-paths; fall back to a
            // minted-by-actor-id state node.
            let state_node_id = NodeId::content_address(
                format!("state:{}", state.actor.as_str()).as_bytes(),
            );
            let state_node = Node {
                id: state_node_id,
                iri: None,
                types: vec![vocab::NODE_STATE.into()],
                properties: BTreeMap::from_iter([
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
                ]),
            };
            delta.nodes.push(state_node);
            // Best-effort: attach to the first path whose leaf
            // matches this actor.
            for (rendered, actor_id) in &prefix_to_id {
                if rendered.ends_with(&format!("/{}", state.actor.as_str())) {
                    delta.edges.push(Edge::between(*actor_id, vocab::EDGE_HOLDS_STATE, state_node_id));
                    break;
                }
            }
        }
        Ok(delta)
    }
}

fn insert_path(
    path: &SupervisionPath,
    ctx: &ProjectionCtx,
    delta: &mut OntologyDelta,
    prefix_to_id: &mut BTreeMap<String, NodeId>,
) -> Result<(), ProjectionError> {
    let mut current_prefix = String::new();
    let mut prev_id: Option<NodeId> = None;
    let segment_count = path.segments.len();

    for (depth, segment) in path.iter_segments() {
        current_prefix.push('/');
        current_prefix.push_str(segment);

        // Build a synthetic path-prefix path so the IRI strategy can
        // mint a deterministic id.
        let prefix_path = SupervisionPath::parse(&current_prefix);
        let (id, iri) = ctx.iri.mint_actor(&prefix_path)?;

        if !prefix_to_id.contains_key(&current_prefix) {
            let ty = type_for_segment(depth, segment);
            let mut node = Node {
                id,
                iri,
                types: vec![vocab::NODE_ACTOR.into(), ty.into()],
                properties: BTreeMap::new(),
            };
            node = node
                .with_property(vocab::PROP_PATH, current_prefix.clone())
                .with_property(vocab::PROP_SEGMENT, segment.to_owned())
                .with_property(vocab::PROP_DEPTH, depth as i64)
                .with_property(vocab::PROP_SOURCE, ctx.source_label.clone());
            // Attach path attributes onto the leaf node only.
            let is_leaf = depth + 1 == segment_count;
            if is_leaf {
                for (k, v) in &path.attributes {
                    node = node.with_property(k.clone(), PropertyValue::Json(v.clone()));
                }
            }
            delta.nodes.push(node);
            prefix_to_id.insert(current_prefix.clone(), id);
        }

        if let Some(parent) = prev_id {
            delta.edges.push(Edge::between(parent, vocab::EDGE_SUPERVISES, id));
        }
        prev_id = Some(id);
    }
    Ok(())
}
