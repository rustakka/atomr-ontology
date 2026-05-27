//! actor_projection_demo
//!
//! Project a synthetic actor system (three workflows, six runs, twelve
//! steps, twenty journal events) into an ontology using each of the
//! four built-in projection shapes. Print a summary of each.

use std::sync::Arc;

use anyhow::Result;

use atomr_ontology_actor_projection::{
    ingest::ReplayIngest,
    projection::{
        EventStreamProjection, FlatProjection, HierarchicalProjection, ProjectionStrategy,
        SnapshotDiffProjection,
    },
    source::{
        Cursor, InMemoryActorPersistenceSource, JournalEvent, JournalEventKind, SerializedState,
        SupervisionPath,
    },
    strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy},
    vocab, ProjectorBuilder,
};
use atomr_ontology_core::Iri;
use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;

fn build_source() -> Arc<InMemoryActorPersistenceSource> {
    let src = Arc::new(InMemoryActorPersistenceSource::new("demo"));
    // Three workflows, two runs each, two steps per run.
    let mut seq = 0u64;
    for wf in ["ingest", "transform", "publish"] {
        for run in 1..=2 {
            for (i, step) in ["fetch", "validate"].iter().enumerate() {
                let actor = format!("{wf}-{run}-{step}");
                let path = SupervisionPath::parse(&format!(
                    "/workflow/{wf}/run/{run}/step/{actor}"
                ));
                src.push_path(path.clone());
                seq += 1;
                src.push_event(
                    JournalEvent::new(Cursor::at(seq), actor.clone(), JournalEventKind::Created)
                        .with_path(path.clone()),
                );
                seq += 1;
                src.push_event(
                    JournalEvent::new(
                        Cursor::at(seq),
                        actor.clone(),
                        JournalEventKind::Completed,
                    )
                    .with_path(path)
                    .with_payload(serde_json::json!({"index": i})),
                );
                src.put_state(SerializedState::new(
                    actor,
                    serde_json::json!({"workflow": wf, "run": run, "step": step}),
                ));
            }
        }
    }
    src
}

async fn run_shape(
    name: &str,
    projection: Arc<dyn ProjectionStrategy>,
) -> Result<()> {
    let source = build_source();
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await?);

    let projector = ProjectorBuilder::new()
        .source(source)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(projection)
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/")?,
        })
        .conflict(ConflictResolution::LastWriteWins)
        .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
        .store(store.clone())
        .build()?;

    let report = projector.run().await?;
    let snapshot = store.snapshot().await?;

    println!(
        "[{name}] batches={} nodes_written={} edges_written={} activities={} (final: {} nodes, {} edges)",
        report.batches,
        report.nodes_written,
        report.edges_written,
        report.activities_recorded,
        snapshot.node_count(),
        snapshot.edge_count(),
    );

    let event_count = snapshot
        .nodes
        .values()
        .filter(|n| n.has_type(vocab::NODE_EVENT))
        .count();
    let supervises = snapshot
        .edges
        .values()
        .filter(|e| e.label == vocab::EDGE_SUPERVISES)
        .count();
    if event_count > 0 || supervises > 0 {
        println!("            events={event_count} supervises={supervises}");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("atomr-ontology-actor-projection demo");
    println!("====================================");

    run_shape("hierarchical", Arc::new(HierarchicalProjection::new())).await?;
    run_shape("event-stream", Arc::new(EventStreamProjection::new())).await?;
    run_shape("snapshot-diff", Arc::new(SnapshotDiffProjection::new())).await?;
    run_shape("flat", Arc::new(FlatProjection::new())).await?;

    println!();
    println!("Each shape projected the same actor data with different graph topologies.");
    println!("Pick a shape based on whether you need navigability (hierarchical),");
    println!("audit (event-stream), incremental sync (snapshot-diff), or query speed (flat).");
    Ok(())
}
