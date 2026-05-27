//! Vocabulary for actor projection: node-type names, edge labels, and
//! property keys, plus a baseline [`Schema`] to seed
//! [`SchemaStrategy::Hybrid`](crate::strategy::SchemaStrategy::Hybrid).

use atomr_ontology_core::{
    Cardinality, Datatype, EdgeType, NodeType, PropertyType, Schema,
};

// --- Node types -----------------------------------------------------

/// Generic supertype for any actor-projected node.
pub const NODE_ACTOR: &str = "Actor";
/// Top-level supervisor of an actor workflow.
pub const NODE_WORKFLOW: &str = "Workflow";
/// A single run of a workflow.
pub const NODE_RUN: &str = "Run";
/// A single step within a run.
pub const NODE_STEP: &str = "Step";
/// A journal event reified as a node.
pub const NODE_EVENT: &str = "ActorEvent";
/// A serialized-state snapshot reified as a node.
pub const NODE_STATE: &str = "ActorState";
/// An auxiliary path segment with no matching kind (intermediate
/// segments in the supervision tree).
pub const NODE_PATH_SEGMENT: &str = "ActorPathSegment";
/// Cursor singleton — one per source.
pub const NODE_CURSOR: &str = "ActorProjectionCursor";

// --- Edge types -----------------------------------------------------

/// `supervises` — parent → child supervision edge.
pub const EDGE_SUPERVISES: &str = "supervises";
/// `emitted` — actor → event edge.
pub const EDGE_EMITTED: &str = "emitted";
/// `holdsState` — actor → state edge.
pub const EDGE_HOLDS_STATE: &str = "holdsState";
/// `successor` — event → event chronological edge.
pub const EDGE_SUCCESSOR: &str = "successor";

// --- Property keys --------------------------------------------------

/// Supervision-tree path string (e.g. `/workflow/foo/run/1/step/2`).
pub const PROP_PATH: &str = "path";
/// Free-form actor identity within the source.
pub const PROP_ACTOR_ID: &str = "actor_id";
/// Cursor version (`u64`).
pub const PROP_CURSOR_VERSION: &str = "cursor_version";
/// Cursor opaque token (string).
pub const PROP_CURSOR_TOKEN: &str = "cursor_token";
/// Event kind (e.g. `"created"`, `"completed"`).
pub const PROP_EVENT_KIND: &str = "event_kind";
/// Event payload (JSON).
pub const PROP_EVENT_PAYLOAD: &str = "event_payload";
/// Event timestamp (ISO-8601).
pub const PROP_EVENT_AT: &str = "event_at";
/// Latest serialized state (JSON).
pub const PROP_STATE: &str = "state";
/// Optional digest of a state blob.
pub const PROP_STATE_DIGEST: &str = "state_digest";
/// Source label that produced the node.
pub const PROP_SOURCE: &str = "source_label";
/// Depth within the supervision tree (`i64`).
pub const PROP_DEPTH: &str = "depth";
/// Local segment name at this depth.
pub const PROP_SEGMENT: &str = "segment";

/// Base schema describing the vocabulary above.
///
/// Use this as the seed for
/// [`SchemaStrategy::Hybrid`](crate::strategy::SchemaStrategy::Hybrid)
/// — the projector will keep these declared types and add new ones as
/// they appear.
pub fn actor_schema() -> Schema {
    let mut s = Schema::new();

    s.declare_node_type(NodeType::new(NODE_ACTOR));
    s.declare_node_type(
        NodeType::new(NODE_WORKFLOW)
            .with_supertype(NODE_ACTOR)
            .with_property(PropertyType {
                name: PROP_PATH.into(),
                datatype: Datatype::String,
                cardinality: Cardinality::ONE,
                iri: None,
                description: Some("Supervision path".into()),
            }),
    );
    s.declare_node_type(NodeType::new(NODE_RUN).with_supertype(NODE_ACTOR));
    s.declare_node_type(NodeType::new(NODE_STEP).with_supertype(NODE_ACTOR));
    s.declare_node_type(NodeType::new(NODE_PATH_SEGMENT).with_supertype(NODE_ACTOR));
    s.declare_node_type(
        NodeType::new(NODE_EVENT).with_property(PropertyType {
            name: PROP_EVENT_KIND.into(),
            datatype: Datatype::String,
            cardinality: Cardinality::ONE,
            iri: None,
            description: Some("Journal event classification".into()),
        }),
    );
    s.declare_node_type(NodeType::new(NODE_STATE));
    s.declare_node_type(NodeType::new(NODE_CURSOR));

    s.declare_edge_type(
        EdgeType::new(EDGE_SUPERVISES)
            .with_domain(NODE_ACTOR)
            .with_range(NODE_ACTOR),
    );
    s.declare_edge_type(
        EdgeType::new(EDGE_EMITTED)
            .with_domain(NODE_ACTOR)
            .with_range(NODE_EVENT),
    );
    s.declare_edge_type(
        EdgeType::new(EDGE_HOLDS_STATE)
            .with_domain(NODE_ACTOR)
            .with_range(NODE_STATE)
            .functional(),
    );
    s.declare_edge_type(EdgeType::new(EDGE_SUCCESSOR).with_domain(NODE_EVENT).with_range(NODE_EVENT));

    s
}

/// Map a path segment depth + name pair to the most specific node type
/// the default vocabulary knows about. Falls back to
/// [`NODE_PATH_SEGMENT`] when no specialization applies.
pub fn type_for_segment(depth: usize, segment: &str) -> &'static str {
    match (depth, segment) {
        (0, "workflow") | (_, "workflow") => NODE_WORKFLOW,
        (_, "run") => NODE_RUN,
        (_, "step") => NODE_STEP,
        _ => NODE_PATH_SEGMENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_schema_declares_core_types() {
        let s = actor_schema();
        assert!(s.node_type(NODE_WORKFLOW).is_some());
        assert!(s.node_type(NODE_EVENT).is_some());
        assert!(s.edge_type(EDGE_SUPERVISES).is_some());
    }
}
