//! Phase-2 MVP: Replay ingest + Hierarchical projection, end-to-end.

use std::sync::Arc;

use atomr_ontology_actor_projection::{
    ingest::ReplayIngest,
    projection::HierarchicalProjection,
    source::{InMemoryActorPersistenceSource, SupervisionPath},
    strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy},
    vocab, ProjectorBuilder,
};
use atomr_ontology_core::Iri;
use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;

#[tokio::test]
async fn replay_hierarchical_shapes_supervision_tree() {
    let source = Arc::new(InMemoryActorPersistenceSource::new("test"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/1"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/2"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/2/step/1"));

    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let projector = ProjectorBuilder::new()
        .source(source.clone())
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .conflict(ConflictResolution::LastWriteWins)
        .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
        .store(store.clone())
        .build()
        .unwrap();

    let report = projector.run().await.unwrap();
    assert!(report.batches >= 1, "expected at least one batch, got {report:?}");
    assert!(report.activities_recorded >= 1);

    let ontology = store.snapshot().await.unwrap();
    // Expected unique path nodes (shared by prefix):
    //   /workflow                     -> 1 Workflow
    //   /workflow/foo                 -> 1 PathSegment
    //   /workflow/foo/run             -> 1 Run
    //   /workflow/foo/run/1           -> 1 PathSegment
    //   /workflow/foo/run/2           -> 1 PathSegment
    //   /workflow/foo/run/1/step      -> 1 Step
    //   /workflow/foo/run/1/step/1    -> 1 PathSegment
    //   /workflow/foo/run/1/step/2    -> 1 PathSegment
    //   /workflow/foo/run/2/step      -> 1 Step
    //   /workflow/foo/run/2/step/1    -> 1 PathSegment
    // 10 distinct nodes.
    assert_eq!(ontology.node_count(), 10, "node tree shape mismatch");

    // Edges: each segment beyond the root has a supervises edge from its parent.
    // 10 nodes form 9 supervises edges.
    let supervises = ontology
        .edges
        .values()
        .filter(|e| e.label == vocab::EDGE_SUPERVISES)
        .count();
    assert_eq!(supervises, 9, "expected 9 supervises edges, got {supervises}");

    // Provenance: one activity recorded by the projector agent.
    let prov = store.provenance().await.unwrap();
    assert!(!prov.activities.is_empty(), "expected at least one activity");
    let activity = prov.activities.values().next().unwrap();
    assert!(activity.label.starts_with("actor-projection:"));
    let agent = activity.agent.as_ref().expect("projector agent attached");
    assert_eq!(agent.id, "agent://atomr-ontology-actor-projection");
}

#[tokio::test]
async fn replay_attaches_events_and_state() {
    use atomr_ontology_actor_projection::source::{JournalEvent, JournalEventKind, SerializedState};

    let source = Arc::new(InMemoryActorPersistenceSource::new("test"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/alpha"));
    source.push_event(
        JournalEvent::new(
            atomr_ontology_actor_projection::source::Cursor::beginning(),
            "alpha",
            JournalEventKind::Created,
        )
        .with_path(SupervisionPath::parse("/workflow/foo/run/1/step/alpha"))
        .with_payload(serde_json::json!({"detail": "ok"})),
    );
    source.put_state(SerializedState::new(
        "alpha",
        serde_json::json!({"phase": "ready"}),
    ));

    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let projector = ProjectorBuilder::new()
        .source(source.clone())
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
        .store(store.clone())
        .build()
        .unwrap();
    projector.run().await.unwrap();

    let ontology = store.snapshot().await.unwrap();
    let event_count = ontology
        .nodes
        .values()
        .filter(|n| n.has_type(vocab::NODE_EVENT))
        .count();
    let state_count = ontology
        .nodes
        .values()
        .filter(|n| n.has_type(vocab::NODE_STATE))
        .count();
    assert_eq!(event_count, 1, "expected one event node");
    assert_eq!(state_count, 1, "expected one state node");

    let emitted = ontology
        .edges
        .values()
        .filter(|e| e.label == vocab::EDGE_EMITTED)
        .count();
    assert_eq!(emitted, 1, "expected one emitted edge from actor → event");
    let holds_state = ontology
        .edges
        .values()
        .filter(|e| e.label == vocab::EDGE_HOLDS_STATE)
        .count();
    assert_eq!(holds_state, 1, "expected one holdsState edge");
}
