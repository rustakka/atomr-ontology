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

use crate::pattern::{EdgePattern, MatchRow, NodePattern, TraversalPlan};
use crate::r#trait::{OntologyDelta, OntologyStore, StoreDiff, StoreError};

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
            for (row, current) in frontier.drain(..) {
                let candidates: Vec<&Edge> = if step.outbound {
                    guard.ontology.edges.values().filter(|e| e.source == current).collect()
                } else {
                    guard.ontology.edges.values().filter(|e| e.target == current).collect()
                };
                for edge in candidates {
                    if !Self::match_edge(edge, &step.edge) {
                        continue;
                    }
                    let target = if step.outbound { edge.target } else { edge.source };
                    let Some(target_node) = guard.ontology.node(&target) else { continue };
                    if !Self::match_node(target_node, &step.target) {
                        continue;
                    }
                    let mut next_row = row.clone();
                    if let Some(name) = &step.edge.bind {
                        next_row = next_row.bind_edge(name.clone(), edge.id);
                    }
                    if let Some(name) = &step.target.bind {
                        next_row = next_row.bind_node(name.clone(), target);
                    }
                    next.push((next_row, target));
                }
            }
            frontier = next;
        }
        Ok(frontier.into_iter().map(|(row, _)| row).collect())
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
    async fn commit_with_provenance_writes_activity() {
        let store = MemStore::new();
        let delta = OntologyDelta::new().with_node(Node::new("Organization"));
        let act = Activity::started("test-commit");
        let pid = store.commit_with_provenance(delta, act).await.unwrap();
        let log = store.provenance().await.unwrap();
        assert!(log.activities.contains_key(&pid));
    }
}
