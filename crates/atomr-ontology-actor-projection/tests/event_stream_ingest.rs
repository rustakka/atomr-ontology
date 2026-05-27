//! Phase-3: EventStream ingest mode.

use std::sync::Arc;
use std::time::Duration;

use atomr_ontology_actor_projection::{
    ingest::EventStreamIngest,
    projection::EventStreamProjection,
    source::{Cursor, InMemoryActorPersistenceSource, JournalEvent, JournalEventKind},
    vocab, ProjectorBuilder,
};
use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;
use tokio::sync::{broadcast, watch};

#[tokio::test]
async fn event_stream_projects_each_published_event() {
    let source = Arc::new(InMemoryActorPersistenceSource::new("event-stream"));
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let (tx, _rx) = broadcast::channel::<JournalEvent>(16);
    let ingest = EventStreamIngest::subscribe(&tx);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let projector = ProjectorBuilder::new()
        .source(source.clone())
        .with_ingest(Arc::new(ingest))
        .projection(Arc::new(EventStreamProjection::new()))
        .store(store.clone())
        .build()
        .unwrap();

    let store_for_check = store.clone();
    let driver = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        for i in 0..4 {
            tx.send(JournalEvent::new(
                Cursor::at(i + 1),
                format!("evt-{i}"),
                JournalEventKind::StateChanged,
            ))
            .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = shutdown_tx.send(true);
        store_for_check
    });

    let report = projector.run_until_shutdown(shutdown_rx).await.unwrap();
    let store_for_check = driver.await.unwrap();

    assert!(report.batches >= 4, "expected per-event batches, got {report:?}");
    let ontology = store_for_check.snapshot().await.unwrap();
    let events: Vec<_> = ontology
        .nodes
        .values()
        .filter(|n| n.has_type(vocab::NODE_EVENT))
        .collect();
    assert_eq!(events.len(), 4, "every published event should project");
}
