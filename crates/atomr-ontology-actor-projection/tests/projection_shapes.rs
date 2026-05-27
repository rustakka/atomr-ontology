//! Phase-4: All four projection shapes produce the expected output
//! when fed the same fixture.

use std::sync::Arc;

use atomr_ontology_actor_projection::{
    ingest::ReplayIngest,
    projection::{
        EventStreamProjection, FlatProjection, HierarchicalProjection, ProjectionStrategy,
        SnapshotDiffProjection,
    },
    source::{Cursor, InMemoryActorPersistenceSource, JournalEvent, JournalEventKind, SerializedState, SupervisionPath},
    strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy},
    vocab, ProjectorBuilder,
};
use atomr_ontology_core::Iri;
use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;

fn fixture() -> Arc<InMemoryActorPersistenceSource> {
    let src = Arc::new(InMemoryActorPersistenceSource::new("shape-test"));
    src.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/a"));
    src.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/b"));
    src.push_path(SupervisionPath::parse("/workflow/foo/run/2/step/c"));
    for (i, actor) in ["a", "b", "c"].iter().enumerate() {
        src.push_event(
            JournalEvent::new(Cursor::at((i + 1) as u64), *actor, JournalEventKind::Completed)
                .with_path(SupervisionPath::parse(&format!(
                    "/workflow/foo/run/{}/step/{}",
                    if *actor == "c" { 2 } else { 1 },
                    actor
                ))),
        );
        src.put_state(SerializedState::new(*actor, serde_json::json!({"i": i})));
    }
    src
}

async fn run_one(
    projection: Arc<dyn ProjectionStrategy>,
) -> (usize, usize, Vec<String>) {
    let source = fixture();
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());
    let projector = ProjectorBuilder::new()
        .source(source)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(projection)
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .conflict(ConflictResolution::LastWriteWins)
        .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
        .store(store.clone())
        .build()
        .unwrap();
    projector.run().await.unwrap();
    let ontology = store.snapshot().await.unwrap();
    let mut iris: Vec<String> = ontology
        .nodes
        .values()
        .filter_map(|n| n.iri.as_ref().map(|i| i.as_str().to_owned()))
        .collect();
    iris.sort();
    (ontology.node_count(), ontology.edge_count(), iris)
}

#[tokio::test]
async fn hierarchical_shape() {
    let (nodes, edges, _) = run_one(Arc::new(HierarchicalProjection::new())).await;
    // Path tree: workflow / foo / run / {1,2} / step (x2) / {a,b,c} = 10 actor nodes;
    // plus 3 event nodes, 3 state nodes = 16.
    assert_eq!(nodes, 16, "hierarchical shape mismatch");
    // 9 supervises (10 tree-nodes connected) + 3 emitted + 3 holdsState = 15.
    assert_eq!(edges, 15, "hierarchical edge count mismatch");
}

#[tokio::test]
async fn event_stream_shape() {
    let (nodes, edges, _) = run_one(Arc::new(EventStreamProjection::new())).await;
    // 3 event nodes, 2 successor edges.
    assert_eq!(nodes, 3, "event-stream node count");
    assert_eq!(edges, 2, "event-stream successor count");
}

#[tokio::test]
async fn flat_shape() {
    let (nodes, edges, _) = run_one(Arc::new(FlatProjection::new())).await;
    // Buckets: ("foo", "1") and ("foo", "2") plus 3 unrouted state buckets ("<unrouted>", actor).
    assert_eq!(nodes, 5, "flat node count");
    assert_eq!(edges, 0, "flat projection emits no edges");
}

#[tokio::test]
async fn snapshot_diff_emits_only_new_records() {
    let projection = Arc::new(SnapshotDiffProjection::new());
    // First batch sees everything.
    let (n1, _, _) = run_one(projection.clone()).await;
    // Second batch (same source, same projection) sees nothing new.
    let source = fixture();
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());
    let projector = ProjectorBuilder::new()
        .source(source)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(projection.clone())
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
        .store(store.clone())
        .build()
        .unwrap();
    let report = projector.run().await.unwrap();
    assert_eq!(report.batches, 0, "second pass should produce no new work");
    assert!(n1 > 0, "first pass should have produced some work");
}
