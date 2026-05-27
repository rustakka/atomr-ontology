//! [`PersistentStore`] — an [`OntologyStore`] wrapper that flushes
//! state to a pluggable [`Checkpointer`] on every commit.
//!
//! Construction calls [`Checkpointer::load`] to populate the in-memory
//! state from whatever is already persisted. After that, reads serve
//! from memory; writes mutate memory only. The
//! [`OntologyStore::commit_with_provenance`] path additionally calls
//! [`Checkpointer::save`] with a fresh [`Snapshot`] so the durable
//! copy stays in sync at commit boundaries.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use atomr_ontology_core::{Axiom, Edge, EdgeId, Node, NodeId, Ontology};
use atomr_ontology_provenance::{Activity, ProvenanceId, ProvenanceLog};
use atomr_ontology_store::pattern::{EdgePattern, MatchRow, NodePattern, SortOrder, TraversalPlan};
use atomr_ontology_store::r#trait::{OntologyDelta, OntologyStore, StoreDiff, StoreError};

use crate::checkpointer::{Checkpointer, CheckpointerError, Snapshot};

/// Map a `CheckpointerError` into a `StoreError::Io` so the wrapper
/// fits cleanly inside the `OntologyStore` contract.
fn checkpointer_err(label: &str, err: CheckpointerError) -> StoreError {
    StoreError::Io(format!("checkpointer({label}): {err}"))
}

/// In-memory state guarded by `parking_lot::RwLock`, mirroring
/// `MemStore::State` and carrying a monotonic version counter.
#[derive(Default)]
struct State {
    ontology: Ontology,
    provenance: ProvenanceLog,
    version: u64,
}

/// Apply ORDER BY / SKIP / LIMIT / RETURN projection from the plan.
/// Mirrors the implementation in `atomr_ontology_store::mem` so the
/// behavior remains identical without exposing the helper publicly.
fn apply_ordering_and_projection(rows: &mut Vec<MatchRow>, plan: &TraversalPlan) {
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
        let allowed: BTreeSet<&str> = plan.return_columns.iter().map(String::as_str).collect();
        for row in rows.iter_mut() {
            row.nodes.retain(|k, _| allowed.contains(k.as_str()));
            row.edges.retain(|k, _| allowed.contains(k.as_str()));
        }
    }
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
    if !pattern.or.is_empty() && !pattern.or.iter().any(|alt| match_node(node, alt)) {
        return false;
    }
    if pattern.not.iter().any(|neg| match_node(node, neg)) {
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

/// Persistent [`OntologyStore`]. Cheap to `clone`; clones share the
/// same underlying in-memory state and checkpointer handle.
pub struct PersistentStore<C: Checkpointer> {
    state: Arc<RwLock<State>>,
    checkpointer: Arc<C>,
}

impl<C: Checkpointer> Clone for PersistentStore<C> {
    fn clone(&self) -> Self {
        Self { state: Arc::clone(&self.state), checkpointer: Arc::clone(&self.checkpointer) }
    }
}

impl<C: Checkpointer> PersistentStore<C> {
    /// Construct a store, populating in-memory state from the latest
    /// snapshot returned by `checkpointer.load()`. If the checkpointer
    /// is empty, the store starts empty (version `0`).
    pub async fn new(checkpointer: C) -> Result<Self, StoreError> {
        let label = checkpointer.label().to_string();
        let loaded = checkpointer.load().await.map_err(|e| checkpointer_err(&label, e))?;
        let state = match loaded {
            Some(snap) => State {
                ontology: snap.ontology,
                provenance: snap.provenance,
                version: snap.version,
            },
            None => State::default(),
        };
        Ok(Self { state: Arc::new(RwLock::new(state)), checkpointer: Arc::new(checkpointer) })
    }

    /// Construct a store without consulting the checkpointer. The
    /// in-memory state starts empty; the first commit will overwrite
    /// whatever is durably stored. Useful for fresh deployments or
    /// tests that want a clean slate against an existing backend.
    pub fn from_memory(checkpointer: C) -> Self {
        Self {
            state: Arc::new(RwLock::new(State::default())),
            checkpointer: Arc::new(checkpointer),
        }
    }

    /// Borrow the checkpointer handle.
    pub fn checkpointer(&self) -> &C {
        &self.checkpointer
    }

    /// Current monotonic version counter.
    pub fn version(&self) -> u64 {
        self.state.read().version
    }

    /// Take a [`Snapshot`] from the current in-memory state. Useful
    /// for manual flushes or for migrating between checkpointer
    /// implementations.
    pub fn snapshot_now(&self) -> Snapshot {
        let guard = self.state.read();
        Snapshot::new(guard.ontology.clone(), guard.provenance.clone(), guard.version)
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut Ontology) -> R) -> R {
        let mut guard = self.state.write();
        f(&mut guard.ontology)
    }
}

#[async_trait]
impl<C: Checkpointer> OntologyStore for PersistentStore<C> {
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
            if match_node(node, pattern) {
                let mut row = MatchRow::new();
                if let Some(name) = &pattern.bind {
                    row = row.bind_node(name.clone(), node.id);
                }
                out.push(row);
            }
        }
        out.sort_by(|a, b| a.nodes.values().next().cmp(&b.nodes.values().next()));
        Ok(out)
    }

    async fn traverse(&self, plan: &TraversalPlan) -> Result<Vec<MatchRow>, StoreError> {
        let guard = self.state.read();
        let mut frontier: Vec<(MatchRow, NodeId)> = Vec::new();
        for node in guard.ontology.nodes.values() {
            if match_node(node, &plan.seed) {
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
                let range = step.edge.repeat.clone().unwrap_or(1..=1);
                let mut visited: std::collections::HashSet<NodeId> =
                    std::collections::HashSet::new();
                visited.insert(start);
                let mut layer: Vec<(MatchRow, NodeId, Option<EdgeId>)> =
                    vec![(row.clone(), start, None)];
                let min = *range.start();
                let max = *range.end();
                for depth in 0..=max {
                    if depth >= min {
                        for (acc_row, cur, last_edge) in &layer {
                            let Some(target_node) = guard.ontology.node(cur) else { continue };
                            if !match_node(target_node, &step.target) {
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
                            if !match_edge(edge, &step.edge) {
                                continue;
                            }
                            let target = if step.outbound { edge.target } else { edge.source };
                            if !visited.insert(target) {
                                continue;
                            }
                            next_layer.push((acc_row.clone(), target, Some(edge.id)));
                        }
                    }
                    layer = next_layer;
                    if layer.is_empty() {
                        break;
                    }
                }
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
        // Apply the delta + activity under the write lock, then build
        // a snapshot of the new state. We do not hold the lock across
        // the await on `save()` — release it first so concurrent
        // readers are not blocked while the (potentially slow)
        // checkpointer flushes.
        let (prov_id, snapshot) = {
            let mut guard = self.state.write();
            let pid = activity.id;
            for node in delta.nodes {
                guard.ontology.upsert_node(node);
            }
            for edge in delta.edges {
                guard.ontology.upsert_edge(edge);
            }
            for axiom in delta.axioms {
                let mut a = axiom;
                if a.provenance.is_none() {
                    a.provenance = Some(pid);
                }
                guard.ontology.upsert_axiom(a);
            }
            guard.provenance.record_activity(activity);
            guard.version = guard.version.saturating_add(1);
            let snap = Snapshot::new(
                guard.ontology.clone(),
                guard.provenance.clone(),
                guard.version,
            );
            (pid, snap)
        };

        let label = self.checkpointer.label().to_string();
        tracing::debug!(
            target: "atomr_ontology_persist",
            checkpointer = %label,
            version = snapshot.version,
            "flushing snapshot"
        );
        self.checkpointer
            .save(snapshot)
            .await
            .map_err(|e| checkpointer_err(&label, e))?;

        Ok(prov_id)
    }

    async fn provenance(&self) -> Result<ProvenanceLog, StoreError> {
        Ok(self.state.read().provenance.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpointer::MemCheckpointer;
    use atomr_ontology_core::Node;

    #[tokio::test]
    async fn loads_empty_when_checkpointer_empty() {
        let cp = MemCheckpointer::new();
        let store = PersistentStore::new(cp).await.unwrap();
        assert_eq!(store.version(), 0);
        assert_eq!(store.snapshot().await.unwrap().node_count(), 0);
    }

    #[tokio::test]
    async fn upsert_and_fetch() {
        let cp = MemCheckpointer::new();
        let store = PersistentStore::new(cp).await.unwrap();
        store.with_mut(|o| {
            o.declare_node_type("Organization");
        });
        let id = store
            .upsert_node(Node::new("Organization").with_property("name", "Acme"))
            .await
            .unwrap();
        let got = store.node(&id).await.unwrap().unwrap();
        assert_eq!(got.id, id);
    }

    #[tokio::test]
    async fn commit_flushes_to_checkpointer_and_reloads() {
        let cp = MemCheckpointer::new();
        let store = PersistentStore::new(cp.clone()).await.unwrap();
        let delta = OntologyDelta::new()
            .with_node(Node::new("Organization").with_property("name", "Acme"));
        let activity = Activity::started("commit-test");
        let pid = store.commit_with_provenance(delta, activity).await.unwrap();

        // The provenance id is in the live log.
        let log = store.provenance().await.unwrap();
        assert!(log.activities.contains_key(&pid));
        assert_eq!(store.version(), 1);

        // The checkpointer has the snapshot.
        let snap = cp.load().await.unwrap().expect("snapshot present after commit");
        assert_eq!(snap.version, 1);
        assert_eq!(snap.ontology.node_count(), 1);
        assert!(snap.provenance.activities.contains_key(&pid));

        // A fresh store built on the same checkpointer recovers state.
        let store2 = PersistentStore::new(cp).await.unwrap();
        assert_eq!(store2.version(), 1);
        assert_eq!(store2.snapshot().await.unwrap().node_count(), 1);
        let log2 = store2.provenance().await.unwrap();
        assert!(log2.activities.contains_key(&pid));
    }

    #[tokio::test]
    async fn multiple_commits_increment_version() {
        let cp = MemCheckpointer::new();
        let store = PersistentStore::new(cp.clone()).await.unwrap();
        for i in 0..3 {
            let delta = OntologyDelta::new().with_node(
                Node::new("Organization").with_property("seq", i as i64),
            );
            let activity = Activity::started(format!("commit-{i}"));
            store.commit_with_provenance(delta, activity).await.unwrap();
        }
        assert_eq!(store.version(), 3);
        let snap = cp.load().await.unwrap().unwrap();
        assert_eq!(snap.version, 3);
        assert_eq!(snap.ontology.node_count(), 3);
    }

    #[tokio::test]
    async fn from_memory_skips_initial_load() {
        // Pre-populate the checkpointer.
        let cp = MemCheckpointer::new();
        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        o.upsert_node(Node::new("Organization"));
        cp.save(Snapshot::new(o, ProvenanceLog::new(), 7)).await.unwrap();

        // from_memory does NOT consult the checkpointer.
        let store = PersistentStore::from_memory(cp.clone());
        assert_eq!(store.version(), 0);
        assert_eq!(store.snapshot().await.unwrap().node_count(), 0);

        // First commit overwrites the durable snapshot.
        let delta = OntologyDelta::new().with_node(Node::new("Organization"));
        store
            .commit_with_provenance(delta, Activity::started("first"))
            .await
            .unwrap();
        let snap = cp.load().await.unwrap().unwrap();
        assert_eq!(snap.version, 1);
        assert_eq!(snap.ontology.node_count(), 1);
    }
}
