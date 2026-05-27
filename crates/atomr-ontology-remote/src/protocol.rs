//! Wire protocol for the remote `OntologyStore` RPC.
//!
//! The protocol is JSON-over-HTTP. Every request is a [`RpcEnvelope`]
//! whose `params` is one of the per-method payload structs in this
//! module; every response is a [`RpcResponse`] carrying either the
//! per-method result or a `RemoteError` payload.
//!
//! Several `atomr-ontology-store` types (`NodePattern`, `EdgePattern`,
//! `TraversalPlan`, `MatchRow`, `StoreDiff`, `OntologyDelta`) do not
//! derive `serde::{Serialize, Deserialize}`. This module mirrors them
//! with wire-shaped structs that *do* implement serde, plus `From`
//! conversions in both directions so the in-process trait surface
//! remains untouched.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use atomr_ontology_core::{
    Axiom, Edge, EdgeId, Iri, Node, NodeId, Ontology, PropertyValue, Vocabulary,
};
use atomr_ontology_core::axiom::AxiomId;
use atomr_ontology_core::schema::Schema;
use atomr_ontology_provenance::{
    Activity, ProvEntity, ProvenanceId, ProvenanceLog, Used, WasAttributedTo, WasDerivedFrom,
    WasGeneratedBy,
};
use atomr_ontology_store::{
    EdgePattern, MatchRow, NodePattern, OntologyDelta, SortOrder, StoreDiff, TraversalPlan,
    TraversalStep,
};

/// Method names used on the wire. Mirrors the `OntologyStore` trait
/// surface. Kept as `&'static str` constants so both the client and
/// server reference the same identifiers.
pub mod method {
    /// `upsert_node`.
    pub const UPSERT_NODE: &str = "upsert_node";
    /// `upsert_edge`.
    pub const UPSERT_EDGE: &str = "upsert_edge";
    /// `upsert_axiom`.
    pub const UPSERT_AXIOM: &str = "upsert_axiom";
    /// `node` (get by id).
    pub const GET_NODE: &str = "get_node";
    /// `edge` (get by id).
    pub const GET_EDGE: &str = "get_edge";
    /// `match_pattern`.
    pub const MATCH_PATTERN: &str = "match_pattern";
    /// `traverse`.
    pub const TRAVERSE: &str = "traverse";
    /// `snapshot`.
    pub const SNAPSHOT: &str = "snapshot";
    /// `diff`.
    pub const DIFF: &str = "diff";
    /// `commit_with_provenance`.
    pub const COMMIT: &str = "commit";
    /// `provenance`.
    pub const PROVENANCE: &str = "provenance";
}

// --- envelope ---------------------------------------------------------------

/// Outbound RPC envelope. `method` is one of the strings in [`method`];
/// `params` is the matching `*Request` struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcEnvelope<T> {
    /// Method identifier (see [`method`]).
    pub method: String,
    /// Method-specific request payload.
    pub params: T,
}

impl<T> RpcEnvelope<T> {
    /// Build an envelope for a known method.
    pub fn new(method: impl Into<String>, params: T) -> Self {
        Self { method: method.into(), params }
    }
}

/// Inbound RPC response. Exactly one of `result` / `error` is present.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    /// Successful payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    /// Failure payload — server-side errors serialized as strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> RpcResponse<T> {
    /// Build a successful response.
    pub fn ok(value: T) -> Self {
        Self { result: Some(value), error: None }
    }

    /// Build a failure response carrying a server-rendered message.
    pub fn err(message: impl Into<String>) -> Self {
        Self { result: None, error: Some(message.into()) }
    }
}

// --- error ------------------------------------------------------------------

/// Errors raised by the remote client / server boundary.
#[derive(Debug, Error)]
pub enum RemoteError {
    /// Network / I/O failure (connect, send, receive).
    #[error("transport error: {0}")]
    Transport(String),
    /// The server returned an error response.
    #[error("server error: {0}")]
    Server(String),
    /// JSON encode / decode failed.
    #[error("encoding error: {0}")]
    Encoding(String),
}

// --- request / response payloads ------------------------------------------

/// `upsert_node` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpsertNodeRequest {
    /// Node to insert or replace.
    pub node: Node,
}

/// `upsert_node` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpsertNodeResponse {
    /// Id of the stored node.
    pub id: NodeId,
}

/// `upsert_edge` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpsertEdgeRequest {
    /// Edge to insert or replace.
    pub edge: Edge,
}

/// `upsert_edge` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpsertEdgeResponse {
    /// Id of the stored edge.
    pub id: EdgeId,
}

/// `upsert_axiom` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpsertAxiomRequest {
    /// Axiom to insert or replace.
    pub axiom: Axiom,
}

/// `upsert_axiom` response (empty success marker).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpsertAxiomResponse {}

/// `get_node` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetNodeRequest {
    /// Identifier to look up.
    pub id: NodeId,
}

/// `get_node` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetNodeResponse {
    /// `None` if the id was not present.
    pub node: Option<Node>,
}

/// `get_edge` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetEdgeRequest {
    /// Identifier to look up.
    pub id: EdgeId,
}

/// `get_edge` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetEdgeResponse {
    /// `None` if the id was not present.
    pub edge: Option<Edge>,
}

/// `match_pattern` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchPatternRequest {
    /// Pattern to apply.
    pub pattern: WireNodePattern,
}

/// `match_pattern` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchPatternResponse {
    /// Bindings produced by the pattern.
    pub rows: Vec<WireMatchRow>,
}

/// `traverse` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraverseRequest {
    /// Plan to execute.
    pub plan: WireTraversalPlan,
}

/// `traverse` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraverseResponse {
    /// Bindings produced by the plan.
    pub rows: Vec<WireMatchRow>,
}

/// `snapshot` request (no parameters).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SnapshotRequest {}

/// `snapshot` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotResponse {
    /// Snapshot of the current ontology state.
    pub ontology: WireOntology,
}

/// `diff` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffRequest {
    /// Counterpart to diff against.
    pub other: WireOntology,
}

/// `diff` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffResponse {
    /// Computed coarse diff.
    pub diff: WireStoreDiff,
}

/// `commit_with_provenance` request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitRequest {
    /// Delta to apply.
    pub delta: WireOntologyDelta,
    /// Activity to record alongside the delta.
    pub activity: WireActivity,
}

/// `commit_with_provenance` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitResponse {
    /// Identifier of the recorded activity.
    pub provenance_id: ProvenanceId,
}

/// `provenance` request (no parameters).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProvenanceRequest {}

/// `provenance` response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvenanceResponse {
    /// Snapshot of the provenance log.
    pub log: WireProvenanceLog,
}

// --- wire mirrors for non-serializable store types -----------------------

/// Wire mirror of [`atomr_ontology_store::NodePattern`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireNodePattern {
    /// See [`NodePattern::bind`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// See [`NodePattern::types`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// See [`NodePattern::properties`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, PropertyValue>,
    /// See [`NodePattern::id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<NodeId>,
    /// See [`NodePattern::or`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub or: Vec<WireNodePattern>,
    /// See [`NodePattern::not`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not: Vec<WireNodePattern>,
}

impl From<NodePattern> for WireNodePattern {
    fn from(p: NodePattern) -> Self {
        Self {
            bind: p.bind,
            types: p.types,
            properties: p.properties,
            id: p.id,
            or: p.or.into_iter().map(|b| Self::from(*b)).collect(),
            not: p.not.into_iter().map(|b| Self::from(*b)).collect(),
        }
    }
}

impl From<&NodePattern> for WireNodePattern {
    fn from(p: &NodePattern) -> Self {
        Self {
            bind: p.bind.clone(),
            types: p.types.clone(),
            properties: p.properties.clone(),
            id: p.id,
            or: p.or.iter().map(|b| Self::from(b.as_ref())).collect(),
            not: p.not.iter().map(|b| Self::from(b.as_ref())).collect(),
        }
    }
}

impl From<WireNodePattern> for NodePattern {
    fn from(p: WireNodePattern) -> Self {
        Self {
            bind: p.bind,
            types: p.types,
            properties: p.properties,
            id: p.id,
            or: p.or.into_iter().map(|w| Box::new(NodePattern::from(w))).collect(),
            not: p.not.into_iter().map(|w| Box::new(NodePattern::from(w))).collect(),
        }
    }
}

/// Wire mirror of [`atomr_ontology_store::EdgePattern`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireEdgePattern {
    /// See [`EdgePattern::bind`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// See [`EdgePattern::label`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// See [`EdgePattern::properties`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, PropertyValue>,
    /// Variable-length repetition expressed as `[min, max]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<[usize; 2]>,
}

impl From<EdgePattern> for WireEdgePattern {
    fn from(p: EdgePattern) -> Self {
        Self {
            bind: p.bind,
            label: p.label,
            properties: p.properties,
            repeat: p.repeat.map(|r| [*r.start(), *r.end()]),
        }
    }
}

impl From<&EdgePattern> for WireEdgePattern {
    fn from(p: &EdgePattern) -> Self {
        Self {
            bind: p.bind.clone(),
            label: p.label.clone(),
            properties: p.properties.clone(),
            repeat: p.repeat.as_ref().map(|r| [*r.start(), *r.end()]),
        }
    }
}

impl From<WireEdgePattern> for EdgePattern {
    fn from(p: WireEdgePattern) -> Self {
        Self {
            bind: p.bind,
            label: p.label,
            properties: p.properties,
            repeat: p.repeat.map(|[a, b]| RangeInclusive::new(a, b)),
        }
    }
}

/// Sort order as serialized on the wire (mirrors [`SortOrder`]).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireSortOrder {
    /// Ascending.
    Ascending,
    /// Descending.
    Descending,
}

impl From<SortOrder> for WireSortOrder {
    fn from(value: SortOrder) -> Self {
        match value {
            SortOrder::Ascending => WireSortOrder::Ascending,
            SortOrder::Descending => WireSortOrder::Descending,
        }
    }
}

impl From<WireSortOrder> for SortOrder {
    fn from(value: WireSortOrder) -> Self {
        match value {
            WireSortOrder::Ascending => SortOrder::Ascending,
            WireSortOrder::Descending => SortOrder::Descending,
        }
    }
}

/// Wire mirror of [`atomr_ontology_store::TraversalStep`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireTraversalStep {
    /// See [`TraversalStep::edge`].
    pub edge: WireEdgePattern,
    /// See [`TraversalStep::target`].
    pub target: WireNodePattern,
    /// See [`TraversalStep::outbound`].
    pub outbound: bool,
}

impl From<TraversalStep> for WireTraversalStep {
    fn from(s: TraversalStep) -> Self {
        Self { edge: s.edge.into(), target: s.target.into(), outbound: s.outbound }
    }
}

impl From<&TraversalStep> for WireTraversalStep {
    fn from(s: &TraversalStep) -> Self {
        Self {
            edge: (&s.edge).into(),
            target: (&s.target).into(),
            outbound: s.outbound,
        }
    }
}

impl From<WireTraversalStep> for TraversalStep {
    fn from(s: WireTraversalStep) -> Self {
        Self {
            edge: s.edge.into(),
            target: s.target.into(),
            outbound: s.outbound,
        }
    }
}

/// Wire mirror of [`atomr_ontology_store::TraversalPlan`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireTraversalPlan {
    /// See [`TraversalPlan::seed`].
    pub seed: WireNodePattern,
    /// See [`TraversalPlan::steps`].
    #[serde(default)]
    pub steps: Vec<WireTraversalStep>,
    /// See [`TraversalPlan::return_columns`].
    #[serde(default)]
    pub return_columns: Vec<String>,
    /// See [`TraversalPlan::order`].
    #[serde(default)]
    pub order: Vec<(String, WireSortOrder)>,
    /// See [`TraversalPlan::skip`].
    #[serde(default)]
    pub skip: usize,
    /// See [`TraversalPlan::limit`].
    #[serde(default)]
    pub limit: Option<usize>,
}

impl From<TraversalPlan> for WireTraversalPlan {
    fn from(p: TraversalPlan) -> Self {
        Self {
            seed: p.seed.into(),
            steps: p.steps.into_iter().map(WireTraversalStep::from).collect(),
            return_columns: p.return_columns,
            order: p.order.into_iter().map(|(k, o)| (k, o.into())).collect(),
            skip: p.skip,
            limit: p.limit,
        }
    }
}

impl From<&TraversalPlan> for WireTraversalPlan {
    fn from(p: &TraversalPlan) -> Self {
        Self {
            seed: (&p.seed).into(),
            steps: p.steps.iter().map(WireTraversalStep::from).collect(),
            return_columns: p.return_columns.clone(),
            order: p.order.iter().map(|(k, o)| (k.clone(), (*o).into())).collect(),
            skip: p.skip,
            limit: p.limit,
        }
    }
}

impl From<WireTraversalPlan> for TraversalPlan {
    fn from(p: WireTraversalPlan) -> Self {
        Self {
            seed: p.seed.into(),
            steps: p.steps.into_iter().map(TraversalStep::from).collect(),
            return_columns: p.return_columns,
            order: p.order.into_iter().map(|(k, o)| (k, o.into())).collect(),
            skip: p.skip,
            limit: p.limit,
        }
    }
}

/// Wire mirror of [`atomr_ontology_store::MatchRow`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireMatchRow {
    /// Node bindings — variable name → matched node id.
    #[serde(default)]
    pub nodes: BTreeMap<String, NodeId>,
    /// Edge bindings — variable name → matched edge id.
    #[serde(default)]
    pub edges: BTreeMap<String, EdgeId>,
}

impl From<MatchRow> for WireMatchRow {
    fn from(r: MatchRow) -> Self {
        Self { nodes: r.nodes, edges: r.edges }
    }
}

impl From<WireMatchRow> for MatchRow {
    fn from(r: WireMatchRow) -> Self {
        let mut row = MatchRow::new();
        row.nodes = r.nodes;
        row.edges = r.edges;
        row
    }
}

/// Wire mirror of [`atomr_ontology_store::StoreDiff`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireStoreDiff {
    /// See [`StoreDiff::added_nodes`].
    #[serde(default)]
    pub added_nodes: Vec<NodeId>,
    /// See [`StoreDiff::removed_nodes`].
    #[serde(default)]
    pub removed_nodes: Vec<NodeId>,
    /// See [`StoreDiff::added_edges`].
    #[serde(default)]
    pub added_edges: Vec<EdgeId>,
    /// See [`StoreDiff::removed_edges`].
    #[serde(default)]
    pub removed_edges: Vec<EdgeId>,
}

impl From<StoreDiff> for WireStoreDiff {
    fn from(d: StoreDiff) -> Self {
        Self {
            added_nodes: d.added_nodes,
            removed_nodes: d.removed_nodes,
            added_edges: d.added_edges,
            removed_edges: d.removed_edges,
        }
    }
}

impl From<WireStoreDiff> for StoreDiff {
    fn from(d: WireStoreDiff) -> Self {
        Self {
            added_nodes: d.added_nodes,
            removed_nodes: d.removed_nodes,
            added_edges: d.added_edges,
            removed_edges: d.removed_edges,
        }
    }
}

/// Wire mirror of [`atomr_ontology_store::OntologyDelta`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireOntologyDelta {
    /// Nodes to upsert.
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// Edges to upsert.
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// Axioms to upsert.
    #[serde(default)]
    pub axioms: Vec<Axiom>,
}

impl From<OntologyDelta> for WireOntologyDelta {
    fn from(d: OntologyDelta) -> Self {
        Self { nodes: d.nodes, edges: d.edges, axioms: d.axioms }
    }
}

impl From<WireOntologyDelta> for OntologyDelta {
    fn from(d: WireOntologyDelta) -> Self {
        Self { nodes: d.nodes, edges: d.edges, axioms: d.axioms }
    }
}

// --- wire mirrors for maps keyed by 32-byte IDs ---------------------------
//
// JSON requires map keys to be strings, but `NodeId` / `EdgeId` / `AxiomId`
// / `ProvenanceId` serialize as byte arrays (`#[serde(transparent)]` over
// `[u8; 32]`). The wire mirrors below replace the offending `BTreeMap<Id, V>`
// fields with `Vec<V>` — `V` already carries its id, so no information is
// lost on either direction.

/// Wire mirror of [`atomr_ontology_core::Ontology`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireOntology {
    /// See [`Ontology::iri`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iri: Option<Iri>,
    /// See [`Ontology::vocabulary`].
    #[serde(default)]
    pub vocabulary: Vocabulary,
    /// See [`Ontology::schema`].
    #[serde(default)]
    pub schema: Schema,
    /// Nodes by-id (id lives inside `Node`).
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// Edges by-id (id lives inside `Edge`).
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// Axioms by-id (id lives inside `Axiom`).
    #[serde(default)]
    pub axioms: Vec<Axiom>,
}

impl From<Ontology> for WireOntology {
    fn from(o: Ontology) -> Self {
        Self {
            iri: o.iri,
            vocabulary: o.vocabulary,
            schema: o.schema,
            nodes: o.nodes.into_values().collect(),
            edges: o.edges.into_values().collect(),
            axioms: o.axioms.into_values().collect(),
        }
    }
}

impl From<&Ontology> for WireOntology {
    fn from(o: &Ontology) -> Self {
        Self {
            iri: o.iri.clone(),
            vocabulary: o.vocabulary.clone(),
            schema: o.schema.clone(),
            nodes: o.nodes.values().cloned().collect(),
            edges: o.edges.values().cloned().collect(),
            axioms: o.axioms.values().cloned().collect(),
        }
    }
}

impl From<WireOntology> for Ontology {
    fn from(o: WireOntology) -> Self {
        let mut out = Ontology {
            iri: o.iri,
            vocabulary: o.vocabulary,
            schema: o.schema,
            ..Ontology::default()
        };
        for n in o.nodes {
            let _: NodeId = out.upsert_node(n);
        }
        for e in o.edges {
            let _: EdgeId = out.upsert_edge(e);
        }
        for a in o.axioms {
            let _: AxiomId = out.upsert_axiom(a);
        }
        out
    }
}

/// Wire mirror of [`atomr_ontology_provenance::Activity`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireActivity {
    /// Stable identifier.
    pub id: ProvenanceId,
    /// Human-readable label.
    pub label: String,
    /// Activity start.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Activity end (None while running).
    #[serde(default)]
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Agent attribution.
    #[serde(default)]
    pub agent: Option<atomr_ontology_provenance::AgentRef>,
    /// Free-form attributes.
    #[serde(default)]
    pub attributes: std::collections::BTreeMap<String, serde_json::Value>,
}

impl From<Activity> for WireActivity {
    fn from(a: Activity) -> Self {
        Self {
            id: a.id,
            label: a.label,
            started_at: a.started_at,
            ended_at: a.ended_at,
            agent: a.agent,
            attributes: a.attributes,
        }
    }
}

impl From<WireActivity> for Activity {
    fn from(a: WireActivity) -> Self {
        Activity {
            id: a.id,
            label: a.label,
            started_at: a.started_at,
            ended_at: a.ended_at,
            agent: a.agent,
            attributes: a.attributes,
        }
    }
}

/// Wire mirror of [`atomr_ontology_provenance::ProvEntity`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireProvEntity {
    /// Stable identifier.
    pub id: ProvenanceId,
    /// Human-readable label.
    pub label: String,
    /// Optional digest.
    #[serde(default)]
    pub digest: Option<String>,
    /// Free-form attributes.
    #[serde(default)]
    pub attributes: std::collections::BTreeMap<String, serde_json::Value>,
}

impl From<ProvEntity> for WireProvEntity {
    fn from(e: ProvEntity) -> Self {
        Self { id: e.id, label: e.label, digest: e.digest, attributes: e.attributes }
    }
}

impl From<WireProvEntity> for ProvEntity {
    fn from(e: WireProvEntity) -> Self {
        ProvEntity { id: e.id, label: e.label, digest: e.digest, attributes: e.attributes }
    }
}

/// Wire mirror of [`atomr_ontology_provenance::ProvenanceLog`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireProvenanceLog {
    /// Activities (id is in-band).
    #[serde(default)]
    pub activities: Vec<WireActivity>,
    /// Entities (id is in-band).
    #[serde(default)]
    pub entities: Vec<WireProvEntity>,
    /// `wasGeneratedBy` records.
    #[serde(default)]
    pub generations: Vec<WasGeneratedBy>,
    /// `wasDerivedFrom` records.
    #[serde(default)]
    pub derivations: Vec<WasDerivedFrom>,
    /// `wasAttributedTo` records.
    #[serde(default)]
    pub attributions: Vec<WasAttributedTo>,
    /// `used` records.
    #[serde(default)]
    pub uses: Vec<Used>,
}

impl From<ProvenanceLog> for WireProvenanceLog {
    fn from(l: ProvenanceLog) -> Self {
        Self {
            activities: l.activities.into_values().map(WireActivity::from).collect(),
            entities: l.entities.into_values().map(WireProvEntity::from).collect(),
            generations: l.generations,
            derivations: l.derivations,
            attributions: l.attributions,
            uses: l.uses,
        }
    }
}

impl From<WireProvenanceLog> for ProvenanceLog {
    fn from(l: WireProvenanceLog) -> Self {
        let mut log = ProvenanceLog::new();
        for a in l.activities {
            log.record_activity(a.into());
        }
        for e in l.entities {
            log.record_entity(e.into());
        }
        log.generations = l.generations;
        log.derivations = l.derivations;
        log.attributions = l.attributions;
        log.uses = l.uses;
        log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips() {
        let env = RpcEnvelope::new(method::GET_NODE, GetNodeRequest { id: NodeId::new_random() });
        let s = serde_json::to_string(&env).expect("ser");
        let parsed: RpcEnvelope<GetNodeRequest> = serde_json::from_str(&s).expect("de");
        assert_eq!(parsed.method, method::GET_NODE);
        assert_eq!(parsed.params.id, env.params.id);
    }

    #[test]
    fn node_pattern_round_trips() {
        let p = NodePattern::any()
            .bind("a")
            .typed("Organization")
            .with_property("name", "Acme")
            .or(NodePattern::any().with_property("name", "Other"))
            .not(NodePattern::any().with_property("hidden", true));
        let w: WireNodePattern = (&p).into();
        let s = serde_json::to_string(&w).expect("ser");
        let round: WireNodePattern = serde_json::from_str(&s).expect("de");
        let back: NodePattern = round.into();
        assert_eq!(back.bind, p.bind);
        assert_eq!(back.types, p.types);
        assert_eq!(back.or.len(), 1);
        assert_eq!(back.not.len(), 1);
    }

    #[test]
    fn traversal_plan_round_trips() {
        let plan = TraversalPlan::from(NodePattern::any().bind("a").typed("Org"))
            .outbound(
                EdgePattern::any().bind("e").labeled("memberOf").repeat(1..=3),
                NodePattern::any().bind("b").typed("Org"),
            )
            .order_by_desc("b")
            .limit(10)
            .skip(2)
            .return_(["a", "b"]);
        let wire: WireTraversalPlan = (&plan).into();
        let s = serde_json::to_string(&wire).expect("ser");
        let round: WireTraversalPlan = serde_json::from_str(&s).expect("de");
        let back: TraversalPlan = round.into();
        assert_eq!(back.limit, Some(10));
        assert_eq!(back.skip, 2);
        assert_eq!(back.steps.len(), 1);
        assert!(back.steps[0].edge.repeat.is_some());
        let r = back.steps[0].edge.repeat.clone().unwrap();
        assert_eq!(*r.start(), 1);
        assert_eq!(*r.end(), 3);
    }
}
