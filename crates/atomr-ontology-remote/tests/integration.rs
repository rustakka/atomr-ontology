//! End-to-end smoke test for the remote `OntologyStore` boundary.
//!
//! Launches the in-process server backed by [`MemStore`] on a random
//! local port, connects a [`RemoteClient`], and exercises the
//! upsert → get → match path along with provenance commit.

#![cfg(all(feature = "client", feature = "server"))]

use std::net::SocketAddr;
use std::sync::Arc;

use atomr_ontology_core::{Edge, Node};
use atomr_ontology_provenance::Activity;
use atomr_ontology_store::{
    EdgePattern, MemStore, NodePattern, OntologyDelta, OntologyStore, TraversalPlan,
};

use atomr_ontology_remote::{serve, RemoteClient};

async fn spin_up() -> (atomr_ontology_remote::ServerHandle, RemoteClient, Arc<MemStore>) {
    let backing = Arc::new(MemStore::new());
    backing.with_mut(|o| {
        o.declare_node_type("Organization");
        o.declare_node_type("Person");
        o.declare_edge_type("memberOf");
    });
    let addr: SocketAddr = "127.0.0.1:0".parse().expect("addr parse");
    let store_dyn: Arc<dyn OntologyStore> = backing.clone();
    let handle = serve(addr, store_dyn).await.expect("serve");
    let url = format!("http://{}", handle.local_addr());
    let client = RemoteClient::new(url).expect("client");
    (handle, client, backing)
}

#[tokio::test]
async fn upsert_get_match_round_trip() {
    let (handle, client, _backing) = spin_up().await;

    let acme = Node::new("Organization").with_property("name", "Acme");
    let acme_id = client.upsert_node(acme.clone()).await.expect("upsert acme");
    let bob = Node::new("Person").with_property("name", "Bob");
    let bob_id = client.upsert_node(bob).await.expect("upsert bob");

    let fetched = client.node(&acme_id).await.expect("get acme").expect("present");
    assert_eq!(fetched.id, acme_id);
    assert!(fetched.has_type("Organization"));

    let rows = client
        .match_pattern(&NodePattern::any().bind("o").typed("Organization"))
        .await
        .expect("match");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].nodes.get("o").copied(), Some(acme_id));

    let edge = Edge::between(bob_id, "memberOf", acme_id);
    let edge_id = client.upsert_edge(edge).await.expect("upsert edge");
    let plan = TraversalPlan::from(NodePattern::any().bind("p").typed("Person"))
        .outbound(EdgePattern::any().labeled("memberOf"), NodePattern::any().bind("o"));
    let rows = client.traverse(&plan).await.expect("traverse");
    assert!(!rows.is_empty(), "expected at least one traversal row");
    assert_eq!(rows[0].nodes.get("p").copied(), Some(bob_id));
    assert_eq!(rows[0].nodes.get("o").copied(), Some(acme_id));

    let fetched_edge = client.edge(&edge_id).await.expect("get edge").expect("present");
    assert_eq!(fetched_edge.label, "memberOf");

    handle.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn snapshot_diff_commit_provenance_round_trip() {
    let (handle, client, _backing) = spin_up().await;

    // Initial snapshot is empty.
    let snap = client.snapshot().await.expect("snapshot");
    assert_eq!(snap.node_count(), 0);

    // Commit a delta with provenance.
    let node = Node::new("Organization").with_property("name", "Globex");
    let delta = OntologyDelta::new().with_node(node.clone());
    let pid = client
        .commit_with_provenance(delta, Activity::started("smoke-test"))
        .await
        .expect("commit");

    // The provenance log should now contain the activity.
    let log = client.provenance().await.expect("provenance");
    assert!(log.activities.contains_key(&pid));

    // Diff against the empty snapshot should report exactly one added node.
    let new_snap = client.snapshot().await.expect("snapshot 2");
    assert_eq!(new_snap.node_count(), 1);
    let diff = client.diff(&snap).await.expect("diff");
    assert_eq!(diff.added_nodes.len(), 1);
    assert!(diff.removed_nodes.is_empty());

    handle.shutdown().await.expect("shutdown");
}
