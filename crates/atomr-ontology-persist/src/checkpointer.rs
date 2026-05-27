//! Pluggable [`Checkpointer`] trait and bundled providers.
//!
//! A [`Checkpointer`] persists a [`Snapshot`] consisting of an
//! [`Ontology`] plus its [`ProvenanceLog`]. The trait is intentionally
//! minimal — `save` writes a snapshot, `load` reads the latest, and
//! `label` returns a human-readable identifier for tracing.
//!
//! The bundled [`MemCheckpointer`] keeps the snapshot in an
//! `Arc<parking_lot::Mutex<Option<Snapshot>>>` and is appropriate for
//! tests and ephemeral pipelines. Additional providers
//! ([`FileCheckpointer`](crate::FileCheckpointer),
//! [`SqliteCheckpointer`](crate::SqliteCheckpointer), …) plug in
//! behind cargo features.
//!
//! ## Wire format
//!
//! [`Snapshot`] holds in-memory data verbatim, but its `Serialize` /
//! `Deserialize` impls route through a private wire form (see
//! `wire.rs`). The wire form
//!
//! - flattens the `Ontology`'s `BTreeMap<Id, …>` and the
//!   `ProvenanceLog`'s ledgers to vectors, since JSON cannot use the
//!   32-byte ID newtypes as map keys; and
//! - encodes [`NodeId`](atomr_ontology_core::NodeId),
//!   [`EdgeId`](atomr_ontology_core::EdgeId), and
//!   [`ProvenanceId`](atomr_ontology_core::ProvenanceId) as
//!   lower-case hex strings, since their `serde_bytes`-backed wire
//!   shape does not JSON-round-trip cleanly.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use atomr_ontology_core::Ontology;
use atomr_ontology_provenance::ProvenanceLog;

use crate::wire::SnapshotWire;

/// A single persisted snapshot of an ontology store's state.
///
/// In memory the struct keeps the `Ontology` and `ProvenanceLog`
/// verbatim. On the wire (`Serialize` / `Deserialize`) it routes
/// through a JSON-friendly representation (see [`wire`](crate::wire))
/// that flattens the `BTreeMap<Id, _>` collections into vectors and
/// hex-encodes the 32-byte ID newtypes so JSON can round-trip them.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// The serialized ontology.
    pub ontology: Ontology,
    /// The provenance ledger that was live when the snapshot was taken.
    pub provenance: ProvenanceLog,
    /// Monotonic version. Newer versions supersede older ones.
    pub version: u64,
}

impl Snapshot {
    /// Build a new snapshot from its parts.
    pub fn new(ontology: Ontology, provenance: ProvenanceLog, version: u64) -> Self {
        Self { ontology, provenance, version }
    }
}

impl Serialize for Snapshot {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        SnapshotWire::from(self).serialize(s)
    }
}

impl<'de> Deserialize<'de> for Snapshot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = SnapshotWire::deserialize(d)?;
        w.into_snapshot().map_err(serde::de::Error::custom)
    }
}

/// Errors raised by [`Checkpointer`] providers.
#[derive(Debug, Error)]
pub enum CheckpointerError {
    /// An I/O error reached the checkpointer (file, socket, db handle).
    #[error("io error: {0}")]
    Io(String),
    /// A (de)serialization error occurred.
    #[error("serialize error: {0}")]
    Serialize(String),
    /// Any other failure surfaced from a provider.
    #[error("checkpointer error: {0}")]
    Other(String),
}

/// Persistence provider used by
/// [`PersistentStore`](crate::PersistentStore).
///
/// Implementations must be `Send + Sync` so the store can be shared
/// across async tasks.
#[async_trait]
pub trait Checkpointer: Send + Sync {
    /// Persist `snapshot`. Implementations should treat each `save` as
    /// authoritative — older snapshots may be retained for history,
    /// but `load` must return the most recent one.
    async fn save(&self, snapshot: Snapshot) -> Result<(), CheckpointerError>;

    /// Fetch the most recent snapshot, or `None` if the backing store
    /// is empty.
    async fn load(&self) -> Result<Option<Snapshot>, CheckpointerError>;

    /// Stable, human-readable label for the checkpointer. Used in
    /// tracing spans and error messages.
    fn label(&self) -> &str;
}

/// In-memory [`Checkpointer`]. Keeps the most recent snapshot under an
/// `Arc<Mutex<…>>` so that many [`PersistentStore`](crate::PersistentStore)
/// clones can share the same backing buffer.
#[derive(Clone, Debug, Default)]
pub struct MemCheckpointer {
    slot: Arc<Mutex<Option<Snapshot>>>,
}

impl MemCheckpointer {
    /// Empty checkpointer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populate the checkpointer with an initial snapshot.
    pub fn with_snapshot(snapshot: Snapshot) -> Self {
        Self { slot: Arc::new(Mutex::new(Some(snapshot))) }
    }
}

#[async_trait]
impl Checkpointer for MemCheckpointer {
    async fn save(&self, snapshot: Snapshot) -> Result<(), CheckpointerError> {
        *self.slot.lock() = Some(snapshot);
        Ok(())
    }

    async fn load(&self) -> Result<Option<Snapshot>, CheckpointerError> {
        Ok(self.slot.lock().clone())
    }

    fn label(&self) -> &str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::Node;

    #[tokio::test]
    async fn mem_checkpointer_round_trip() {
        let cp = MemCheckpointer::new();
        assert!(cp.load().await.unwrap().is_none());

        let mut ontology = Ontology::new();
        ontology.declare_node_type("Organization");
        ontology.upsert_node(Node::new("Organization").with_property("name", "Acme"));
        let snap = Snapshot::new(ontology, ProvenanceLog::new(), 1);
        cp.save(snap.clone()).await.unwrap();

        let loaded = cp.load().await.unwrap().expect("snapshot present");
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.ontology.node_count(), 1);
    }

    #[tokio::test]
    async fn mem_checkpointer_overwrites_previous() {
        let cp = MemCheckpointer::new();
        let s1 = Snapshot::new(Ontology::new(), ProvenanceLog::new(), 1);
        let s2 = Snapshot::new(Ontology::new(), ProvenanceLog::new(), 2);
        cp.save(s1).await.unwrap();
        cp.save(s2).await.unwrap();
        let loaded = cp.load().await.unwrap().unwrap();
        assert_eq!(loaded.version, 2);
    }

    #[test]
    fn mem_checkpointer_label() {
        let cp = MemCheckpointer::new();
        assert_eq!(cp.label(), "memory");
    }

    #[test]
    fn snapshot_json_round_trip() {
        // Build a non-trivial snapshot with nodes, edges, axioms, an
        // activity, and provenance lineage to exercise the wire format.
        use atomr_ontology_core::{Axiom, AxiomKind, Edge, Iri, Node};
        use atomr_ontology_provenance::{
            Activity, AgentRef, ProvEntity,
        };

        let mut o = Ontology::with_iri("https://example.org/ontology").unwrap();
        o.declare_node_type("Organization");
        o.declare_edge_type("memberOf");
        let acme_iri = Iri::new("https://example.org/Acme").unwrap();
        let acme = Node::from_iri(acme_iri, "Organization").with_property("name", "Acme");
        let acme_id = o.upsert_node(acme);
        let bob = o.upsert_node(Node::new("Organization").with_property("name", "Bob"));
        o.upsert_edge(Edge::between(bob, "memberOf", acme_id).with_property("since", 2020_i64));
        o.upsert_axiom(Axiom::new(AxiomKind::SubClassOf {
            sub: "FormalOrganization".into(),
            sup: "Organization".into(),
        }));

        let mut prov = ProvenanceLog::new();
        let agent = AgentRef::software("agent://t", "T");
        let act = Activity::started("test")
            .by(agent.clone())
            .with_attribute("model", serde_json::json!("gpt"))
            .finish();
        let act_id = prov.record_activity(act);
        let e1 = prov.record_entity(ProvEntity::new("doc.txt", Some("deadbeef".into())));
        let e2 = prov.record_entity(ProvEntity::new("Term", None));
        prov.used(act_id, e1);
        prov.generated(e2, act_id);
        prov.derived(e2, e1);
        prov.attributed(e2, agent);

        let snap = Snapshot::new(o, prov, 7);
        let json = serde_json::to_string_pretty(&snap).expect("serialize");
        let parsed: Snapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.version, 7);
        assert_eq!(parsed.ontology.node_count(), 2);
        assert_eq!(parsed.ontology.edge_count(), 1);
        assert_eq!(parsed.ontology.axioms.len(), 1);
        assert_eq!(parsed.provenance.activities.len(), 1);
        assert_eq!(parsed.provenance.entities.len(), 2);
        assert_eq!(parsed.provenance.uses.len(), 1);
        assert_eq!(parsed.provenance.generations.len(), 1);
        assert_eq!(parsed.provenance.derivations.len(), 1);
        assert_eq!(parsed.provenance.attributions.len(), 1);
        // ID identity survives the trip.
        assert!(parsed.ontology.node(&acme_id).is_some());
        assert!(parsed.provenance.activities.contains_key(&act_id));
    }
}
