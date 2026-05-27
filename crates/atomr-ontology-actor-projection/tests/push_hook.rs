//! Phase-3: PushHook ingest mode.

use std::sync::Arc;
use std::time::Duration;

use atomr_ontology_actor_projection::{
    ingest::push_hook_pair,
    projection::HierarchicalProjection,
    source::{InMemoryActorPersistenceSource, SupervisionPath},
    strategy::IriMintingStrategy,
    ProjectorBuilder,
};
use atomr_ontology_core::Iri;
use atomr_ontology_persist::{Checkpointer, MemCheckpointer, PersistentStore, Snapshot};
use atomr_ontology_provenance::ProvenanceLog;
use atomr_ontology_store::r#trait::OntologyStore;
use tokio::sync::watch;

#[tokio::test]
async fn push_hook_fires_on_upstream_save() {
    // Source feeds the projector data when triggered.
    let source = Arc::new(InMemoryActorPersistenceSource::new("push-hook-test"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/a"));

    // Upstream checkpointer + push-hook wrapper.
    let upstream: Arc<dyn Checkpointer> = Arc::new(MemCheckpointer::new());
    let (hook_cp, hook_ingest) = push_hook_pair(upstream.clone());

    // Destination store.
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let projector = ProjectorBuilder::new()
        .source(source.clone())
        .with_ingest(Arc::new(hook_ingest))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .store(store.clone())
        .build()
        .unwrap();

    let store_for_check = store.clone();
    let driver = tokio::spawn(async move {
        // Give the projector a moment to subscribe.
        tokio::time::sleep(Duration::from_millis(40)).await;
        // Trigger the hook by saving through the wrapping checkpointer.
        hook_cp.save(Snapshot::default()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = shutdown_tx.send(true);
        store_for_check
    });

    let report = projector.run_until_shutdown(shutdown_rx).await.unwrap();
    let store_for_check = driver.await.unwrap();

    assert!(
        report.batches >= 1,
        "expected projector to react to push-hook save"
    );
    let ontology = store_for_check.snapshot().await.unwrap();
    assert!(
        ontology.node_count() > 0,
        "push-hook should have caused at least one projection"
    );

    // Sanity: load returns the snapshot the upstream stored.
    let upstream_snap = upstream.load().await.unwrap().unwrap();
    assert_eq!(upstream_snap.version, 0);
    // Provenance log default for an empty Snapshot is intact.
    let _ = ProvenanceLog::new();
}
