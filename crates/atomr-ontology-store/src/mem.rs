//! In-memory implementation of [`OntologyStore`].
//!
//! `MemStore` is the reference implementation used in tests,
//! examples, and small workflows. It guards an `Ontology` and a
//! `ProvenanceLog` under a `parking_lot::RwLock`. The async surface
//! is satisfied trivially (every op acquires the lock and returns
//! synchronously) so the type works under any runtime.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use atomr_ontology_core::{Axiom, Edge, EdgeId, Node, NodeId, Ontology, PropertyValue};
use atomr_ontology_provenance::{Activity, ProvenanceId, ProvenanceLog};

use crate::pattern::{EdgePattern, MatchRow, NodePattern, SortOrder, TraversalPlan};
use crate::r#trait::{OntologyDelta, OntologyStore, StoreDiff, StoreError};

/// Apply ORDER BY / SKIP / LIMIT / RETURN projection from the plan.
pub(crate) fn apply_ordering_and_projection(rows: &mut Vec<MatchRow>, plan: &TraversalPlan) {
    if !plan.order.is_empty() {
        rows.sort_by(|a, b| {
            for (binding, ord) in &plan.order {
                let av = a.nodes.get(binding.as_str()).copied();
                let bv = b.nodes.get(binding.as_str()).copied();
                let cmp = av.cmp(&bv);
                if cmp != std::cmp::Ordering::Equal {
                    return match ord {
                        SortOrder::Ascending => cmp,
                        SortOrder::Descending => cmp.reverse(),
                    };
                }
            }
            std::cmp::Ordering::Equal
        });
    }
    if plan.skip > 0 {
        let drop = plan.skip.min(rows.len());
        rows.drain(..drop);
    }
    if let Some(n) = plan.limit {
        rows.truncate(n);
    }
    if !plan.return_columns.is_empty() {
        let allowed: std::collections::BTreeSet<&str> =
            plan.return_columns.iter().map(String::as_str).collect();
        for row in rows.iter_mut() {
            row.nodes.retain(|k, _| allowed.contains(k.as_str()));
            row.edges.retain(|k, _| allowed.contains(k.as_str()));
        }
    }
}

/// In-memory ontology store. Cheap to `clone`; shares the same
/// underlying state.
#[derive(Clone, Default)]
pub struct MemStore {
    state: Arc<RwLock<State>>,
}

#[derive(Default)]
struct State {
    ontology: Ontology,
    provenance: ProvenanceLog,
}

impl MemStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize the store from an existing `Ontology`.
    pub fn from_ontology(ontology: Ontology) -> Self {
        let state = State { ontology, provenance: ProvenanceLog::new() };
        Self { state: Arc::new(RwLock::new(state)) }
    }

    /// Borrow a clone of the underlying ontology snapshot.
    pub fn snapshot_blocking(&self) -> Ontology {
        self.state.read().ontology.clone()
    }

    /// Apply a function under the write lock.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut Ontology) -> R) -> R {
        let mut guard = self.state.write();
        f(&mut guard.ontology)
    }

    fn match_node(node: &Node, pattern: &NodePattern) -> bool {
        if let Some(id) = pattern.id {
            if node.id != id {
                return false;
            }
        }
        for ty in &pattern.types {
            if !node.has_type(ty) {
                return false;
            }
        }
        for (k, v) in &pattern.properties {
            if node.properties.get(k) != Some(v) {
                return false;
            }
        }
        // OR-branches: at least one must match if any present.
        if !pattern.or.is_empty() && !pattern.or.iter().any(|alt| Self::match_node(node, alt)) {
            return false;
        }
        // NOT-branches: none may match.
        if pattern.not.iter().any(|neg| Self::match_node(node, neg)) {
            return false;
        }
        true
    }

    fn match_edge(edge: &Edge, pattern: &EdgePattern) -> bool {
        if let Some(label) = &pattern.label {
            if &edge.label != label {
                return false;
            }
        }
        for (k, v) in &pattern.properties {
            if edge.properties.get(k) != Some(v) {
                return false;
            }
        }
        true
    }
}

#[async_trait]
impl OntologyStore for MemStore {
    async fn upsert_node(&self, node: Node) -> Result<NodeId, StoreError> {
        Ok(self.with_mut(|o| o.upsert_node(node)))
    }

    async fn upsert_edge(&self, edge: Edge) -> Result<EdgeId, StoreError> {
        Ok(self.with_mut(|o| o.upsert_edge(edge)))
    }

    async fn upsert_axiom(&self, axiom: Axiom) -> Result<(), StoreError> {
        self.with_mut(|o| {
            o.upsert_axiom(axiom);
        });
        Ok(())
    }

    async fn node(&self, id: &NodeId) -> Result<Option<Node>, StoreError> {
        Ok(self.state.read().ontology.node(id).cloned())
    }

    async fn edge(&self, id: &EdgeId) -> Result<Option<Edge>, StoreError> {
        Ok(self.state.read().ontology.edge(id).cloned())
    }

    async fn match_pattern(&self, pattern: &NodePattern) -> Result<Vec<MatchRow>, StoreError> {
        let guard = self.state.read();
        let mut out = Vec::new();
        for node in guard.ontology.nodes.values() {
            if Self::match_node(node, pattern) {
                let mut row = MatchRow::new();
                if let Some(name) = &pattern.bind {
                    row = row.bind_node(name.clone(), node.id);
                }
                out.push(row);
            }
        }
        // Sort by node id for deterministic ordering before any caller-
        // applied LIMIT logic.
        out.sort_by(|a, b| a.nodes.values().next().cmp(&b.nodes.values().next()));
        Ok(out)
    }

    async fn traverse(&self, plan: &TraversalPlan) -> Result<Vec<MatchRow>, StoreError> {
        let guard = self.state.read();
        let mut frontier: Vec<(MatchRow, NodeId)> = Vec::new();
        for node in guard.ontology.nodes.values() {
            if Self::match_node(node, &plan.seed) {
                let mut row = MatchRow::new();
                if let Some(name) = &plan.seed.bind {
                    row = row.bind_node(name.clone(), node.id);
                }
                frontier.push((row, node.id));
            }
        }

        for step in &plan.steps {
            let mut next = Vec::new();
            for (row, start) in frontier.drain(..) {
                // Variable-length matching: if `edge.repeat` is set,
                // expand from `start` over `min..=max` hops following
                // the same edge constraint, then require the terminal
                // node to match `step.target` before binding.
                let range = step.edge.repeat.clone().unwrap_or(1..=1);
                let mut visited: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
                visited.insert(start);
                let mut layer: Vec<(MatchRow, NodeId, Option<EdgeId>)> = vec![(row.clone(), start, None)];
                // Track per-row layer for each depth so we can emit
                // results at any depth within the range.
                let min = *range.start();
                let max = *range.end();
                let mut emitted = false;
                for depth in 0..=max {
                    // Emit rows from this depth if depth ∈ [min, max] AND
                    // (for repeated edges) depth > 0 (zero-length paths
                    // imply unchanged frontier — we still emit if seed
                    // already matches step.target and min == 0).
                    if depth >= min {
                        for (acc_row, cur, last_edge) in &layer {
                            let Some(target_node) = guard.ontology.node(cur) else { continue };
                            if !Self::match_node(target_node, &step.target) {
                                continue;
                            }
                            let mut nr = acc_row.clone();
                            if let (Some(name), Some(eid)) = (&step.edge.bind, last_edge) {
                                nr = nr.bind_edge(name.clone(), *eid);
                            }
                            if let Some(name) = &step.target.bind {
                                nr = nr.bind_node(name.clone(), *cur);
                            }
                            next.push((nr, *cur));
                            emitted = true;
                        }
                    }
                    if depth == max {
                        break;
                    }
                    let mut next_layer = Vec::new();
                    for (acc_row, cur, _) in &layer {
                        let candidates: Vec<&Edge> = if step.outbound {
                            guard.ontology.edges.values().filter(|e| e.source == *cur).collect()
                        } else {
                            guard.ontology.edges.values().filter(|e| e.target == *cur).collect()
                        };
                        for edge in candidates {
                            if !Self::match_edge(edge, &step.edge) {
                                continue;
                            }
                            let target = if step.outbound { edge.target } else { edge.source };
                            if !visited.insert(target) {
                                continue; // cycle prevention
                            }
                            next_layer.push((acc_row.clone(), target, Some(edge.id)));
                        }
                    }
                    layer = next_layer;
                    if layer.is_empty() {
                        break;
                    }
                }
                let _ = emitted; // suppress unused warning if min == 0
            }
            frontier = next;
        }
        let mut rows: Vec<MatchRow> = frontier.into_iter().map(|(row, _)| row).collect();
        apply_ordering_and_projection(&mut rows, plan);
        Ok(rows)
    }

    async fn snapshot(&self) -> Result<Ontology, StoreError> {
        Ok(self.state.read().ontology.clone())
    }

    async fn diff(&self, other: &Ontology) -> Result<StoreDiff, StoreError> {
        let guard = self.state.read();
        let self_nodes: BTreeSet<_> = guard.ontology.nodes.keys().copied().collect();
        let other_nodes: BTreeSet<_> = other.nodes.keys().copied().collect();
        let self_edges: BTreeSet<_> = guard.ontology.edges.keys().copied().collect();
        let other_edges: BTreeSet<_> = other.edges.keys().copied().collect();
        Ok(StoreDiff {
            added_nodes: self_nodes.difference(&other_nodes).copied().collect(),
            removed_nodes: other_nodes.difference(&self_nodes).copied().collect(),
            added_edges: self_edges.difference(&other_edges).copied().collect(),
            removed_edges: other_edges.difference(&self_edges).copied().collect(),
        })
    }

    async fn commit_with_provenance(
        &self,
        delta: OntologyDelta,
        activity: Activity,
    ) -> Result<ProvenanceId, StoreError> {
        let mut guard = self.state.write();
        let prov_id = activity.id;
        for node in delta.nodes {
            guard.ontology.upsert_node(node);
        }
        for edge in delta.edges {
            guard.ontology.upsert_edge(edge);
        }
        for axiom in delta.axioms {
            let mut a = axiom;
            if a.provenance.is_none() {
                a.provenance = Some(prov_id);
            }
            guard.ontology.upsert_axiom(a);
        }
        guard.provenance.record_activity(activity);
        Ok(prov_id)
    }

    async fn provenance(&self) -> Result<ProvenanceLog, StoreError> {
        Ok(self.state.read().provenance.clone())
    }
}

// Silence unused-import warning when `PropertyValue` is only used through pattern
// matching against the `Node`/`Edge` property maps.
#[allow(dead_code)]
fn _silence_unused(_: PropertyValue) {}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::Iri;

    #[tokio::test]
    async fn upsert_and_fetch() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Organization");
        });
        let n = Node::from_iri(Iri::new("https://example.org/Acme").unwrap(), "Organization");
        let id = store.upsert_node(n.clone()).await.unwrap();
        let got = store.node(&id).await.unwrap().unwrap();
        assert_eq!(got.id, id);
    }

    #[tokio::test]
    async fn match_pattern_filters_by_type_and_property() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Organization");
            o.declare_node_type("Person");
        });
        let _ = store.upsert_node(Node::new("Organization").with_property("name", "Acme")).await.unwrap();
        let _ = store.upsert_node(Node::new("Person").with_property("name", "Bob")).await.unwrap();

        let rows = store
            .match_pattern(
                &NodePattern::any().bind("org").typed("Organization").with_property("name", "Acme"),
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].nodes.contains_key("org"));
    }

    #[tokio::test]
    async fn traversal_follows_edges() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Organization");
            o.declare_edge_type("memberOf");
        });
        let acme = store.upsert_node(Node::new("Organization")).await.unwrap();
        let bob = store.upsert_node(Node::new("Organization")).await.unwrap();
        let _ = store.upsert_edge(Edge::between(bob, "memberOf", acme)).await.unwrap();

        let plan = TraversalPlan::from(NodePattern::any().bind("a").typed("Organization"))
            .outbound(EdgePattern::any().labeled("memberOf"), NodePattern::any().bind("b"));
        let rows = store.traverse(&plan).await.unwrap();
        assert!(!rows.is_empty());
        assert!(rows[0].nodes.contains_key("a"));
        assert!(rows[0].nodes.contains_key("b"));
    }

    #[tokio::test]
    async fn variable_length_path() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Class");
            o.declare_edge_type("subClassOf");
        });
        let a = store.upsert_node(Node::new("Class").with_property("name", "A")).await.unwrap();
        let b = store.upsert_node(Node::new("Class").with_property("name", "B")).await.unwrap();
        let c = store.upsert_node(Node::new("Class").with_property("name", "C")).await.unwrap();
        store.upsert_edge(Edge::between(a, "subClassOf", b)).await.unwrap();
        store.upsert_edge(Edge::between(b, "subClassOf", c)).await.unwrap();

        // 1..=2 hops from A: should reach B (1 hop) and C (2 hops).
        let plan = TraversalPlan::from(NodePattern::any().bind("start").typed("Class").with_property("name", "A"))
            .outbound(
                EdgePattern::any().labeled("subClassOf").repeat(1..=2),
                NodePattern::any().bind("end"),
            );
        let rows = store.traverse(&plan).await.unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn or_alternative_in_node_pattern() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Organization");
            o.declare_node_type("Person");
        });
        store.upsert_node(Node::new("Organization").with_property("name", "Acme")).await.unwrap();
        store.upsert_node(Node::new("Person").with_property("name", "Bob")).await.unwrap();
        store.upsert_node(Node::new("Person").with_property("name", "Carol")).await.unwrap();

        // OR: name == "Acme" OR name == "Carol".
        let p = NodePattern::any()
            .bind("x")
            .or(NodePattern::any().with_property("name", "Acme"))
            .or(NodePattern::any().with_property("name", "Carol"));
        let rows = store.match_pattern(&p).await.unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn not_excludes_matches() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Person");
        });
        store.upsert_node(Node::new("Person").with_property("name", "Alice")).await.unwrap();
        store.upsert_node(Node::new("Person").with_property("name", "Bob")).await.unwrap();

        // typed Person AND NOT name == "Bob".
        let p = NodePattern::any()
            .typed("Person")
            .not(NodePattern::any().with_property("name", "Bob"));
        let rows = store.match_pattern(&p).await.unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn limit_and_order_by() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Organization");
            o.declare_edge_type("partner");
        });
        let a = store.upsert_node(Node::new("Organization").with_property("n", "A")).await.unwrap();
        let b = store.upsert_node(Node::new("Organization").with_property("n", "B")).await.unwrap();
        let c = store.upsert_node(Node::new("Organization").with_property("n", "C")).await.unwrap();
        store.upsert_edge(Edge::between(a, "partner", b)).await.unwrap();
        store.upsert_edge(Edge::between(a, "partner", c)).await.unwrap();

        let plan = TraversalPlan::from(NodePattern::any().bind("a").typed("Organization").with_property("n", "A"))
            .outbound(EdgePattern::any().labeled("partner"), NodePattern::any().bind("b"))
            .order_by("b")
            .limit(1);
        let rows = store.traverse(&plan).await.unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn projection_strips_other_columns() {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Org");
            o.declare_edge_type("memberOf");
        });
        let x = store.upsert_node(Node::new("Org")).await.unwrap();
        let y = store.upsert_node(Node::new("Org")).await.unwrap();
        store.upsert_edge(Edge::between(x, "memberOf", y)).await.unwrap();
        let plan = TraversalPlan::from(NodePattern::any().bind("a").typed("Org"))
            .outbound(
                EdgePattern::any().bind("e").labeled("memberOf"),
                NodePattern::any().bind("b"),
            )
            .return_(["a"]);
        let rows = store.traverse(&plan).await.unwrap();
        assert!(rows.iter().all(|r| r.nodes.contains_key("a")));
        assert!(rows.iter().all(|r| !r.nodes.contains_key("b")));
        assert!(rows.iter().all(|r| !r.edges.contains_key("e")));
    }

    #[tokio::test]
    async fn commit_with_provenance_writes_activity() {
        let store = MemStore::new();
        let delta = OntologyDelta::new().with_node(Node::new("Organization"));
        let act = Activity::started("test-commit");
        let pid = store.commit_with_provenance(delta, act).await.unwrap();
        let log = store.provenance().await.unwrap();
        assert!(log.activities.contains_key(&pid));
    }
}
