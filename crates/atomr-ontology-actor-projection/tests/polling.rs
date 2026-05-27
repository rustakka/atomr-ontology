//! Phase-3: Polling ingest mode.

use std::sync::Arc;
use std::time::Duration;

use atomr_ontology_actor_projection::{
    ingest::PollingIngest,
    projection::EventStreamProjection,
    source::{InMemoryActorPersistenceSource, JournalEvent, JournalEventKind},
    vocab, ProjectorBuilder,
};
use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;
use tokio::sync::watch;

#[tokio::test]
async fn polling_pulls_events_until_shutdown() {
    let source = Arc::new(InMemoryActorPersistenceSource::new("polling-test"));
    // Pre-load three events so the first tick has work.
    for i in 0..3 {
        source.push_event(JournalEvent::new(
            atomr_ontology_actor_projection::source::Cursor::beginning(),
            format!("actor-{i}"),
            JournalEventKind::Created,
        ));
    }

    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let projector = ProjectorBuilder::new()
        .source(source.clone())
        .with_ingest(Arc::new(PollingIngest::every(Duration::from_millis(50))))
        .projection(Arc::new(EventStreamProjection::new()))
        .store(store.clone())
        .build()
        .unwrap();

    let store_for_check = store.clone();
    let source_for_drive = source.clone();
    let driver = tokio::spawn(async move {
        // After 80ms, append two more events; after another 80ms, shut down.
        tokio::time::sleep(Duration::from_millis(80)).await;
        source_for_drive.push_event(JournalEvent::new(
            atomr_ontology_actor_projection::source::Cursor::beginning(),
            "later-1",
            JournalEventKind::Completed,
        ));
        source_for_drive.push_event(JournalEvent::new(
            atomr_ontology_actor_projection::source::Cursor::beginning(),
            "later-2",
            JournalEventKind::Completed,
        ));
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = shutdown_tx.send(true);
        store_for_check
    });

    let report = projector.run_until_shutdown(shutdown_rx).await.unwrap();
    let store_for_check = driver.await.unwrap();

    assert!(report.batches >= 1, "expected at least one batch, got {report:?}");
    let ontology = store_for_check.snapshot().await.unwrap();
    let events: Vec<_> = ontology
        .nodes
        .values()
        .filter(|n| n.has_type(vocab::NODE_EVENT))
        .collect();
    assert_eq!(events.len(), 5, "all five events should be projected");
}
