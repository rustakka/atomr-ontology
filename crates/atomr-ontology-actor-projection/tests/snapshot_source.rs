//! Snapshot-source integration: wrap a `Checkpointer` and treat its
//! ontology nodes as actor records.

#![cfg(feature = "snapshot-source")]

use std::sync::Arc;

use atomr_ontology_actor_projection::{
    ingest::ReplayIngest,
    projection::HierarchicalProjection,
    source::SnapshotActorPersistenceSource,
    strategy::IriMintingStrategy,
    vocab, ProjectorBuilder,
};
use atomr_ontology_core::{Iri, Node, Ontology, PropertyValue};
use atomr_ontology_persist::{Checkpointer, MemCheckpointer, PersistentStore, Snapshot};
use atomr_ontology_provenance::ProvenanceLog;
use atomr_ontology_store::r#trait::OntologyStore;

#[tokio::test]
async fn snapshot_source_projects_actor_records() {
    // Build an upstream snapshot containing a few actor records.
    let upstream_cp = MemCheckpointer::new();
    let mut o = Ontology::new();
    for path in [
        "/workflow/foo/run/1/step/a",
        "/workflow/foo/run/1/step/b",
    ] {
        let node = Node::new("ActorRecord")
            .with_property(vocab::PROP_PATH, path.to_owned())
            .with_property(vocab::PROP_ACTOR_ID, path.rsplit('/').next().unwrap().to_owned())
            .with_property(
                vocab::PROP_STATE,
                PropertyValue::Json(serde_json::json!({"phase": "ready"})),
            );
        o.upsert_node(node);
    }
    upstream_cp
        .save(Snapshot::new(o, ProvenanceLog::new(), 1))
        .await
        .unwrap();
    let upstream: Arc<dyn Checkpointer> = Arc::new(upstream_cp);

    let source = Arc::new(SnapshotActorPersistenceSource::new("snap", upstream));

    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let projector = ProjectorBuilder::new()
        .source(source)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .store(store.clone())
        .build()
        .unwrap();
    projector.run().await.unwrap();

    let ontology = store.snapshot().await.unwrap();
    assert!(
        ontology.node_count() >= 8,
        "expected supervision tree projected, got {} nodes",
        ontology.node_count()
    );
}
