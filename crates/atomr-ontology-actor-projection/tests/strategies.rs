//! Phase-5: Strategy variants and provenance assertions.

use std::sync::Arc;

use atomr_ontology_actor_projection::{
    ingest::ReplayIngest,
    projection::HierarchicalProjection,
    source::{InMemoryActorPersistenceSource, SupervisionPath},
    strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy},
    vocab, ProjectorBuilder, ProjectorError,
};
use atomr_ontology_core::{Iri, Schema};
use atomr_ontology_persist::{MemCheckpointer, PersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;

async fn build_projector_with(
    iri: IriMintingStrategy,
    conflict: ConflictResolution,
    schema: SchemaStrategy,
) -> (Arc<dyn OntologyStore>, atomr_ontology_actor_projection::ProjectorReport) {
    let source = Arc::new(InMemoryActorPersistenceSource::new("strategies"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/a"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/b"));

    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let projector = ProjectorBuilder::new()
        .source(source)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(iri)
        .conflict(conflict)
        .schema(schema)
        .store(store.clone())
        .build()
        .unwrap();
    let report = projector.run().await.unwrap();
    (store, report)
}

#[tokio::test]
async fn iri_strategies_yield_distinct_ids() {
    let (path_store, _) = build_projector_with(
        IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        },
        ConflictResolution::LastWriteWins,
        SchemaStrategy::InducedSchema,
    )
    .await;
    let (cont_store, _) = build_projector_with(
        IriMintingStrategy::ContentAddressed,
        ConflictResolution::LastWriteWins,
        SchemaStrategy::InducedSchema,
    )
    .await;

    let path_ids: std::collections::BTreeSet<_> =
        path_store.snapshot().await.unwrap().nodes.keys().copied().collect();
    let cont_ids: std::collections::BTreeSet<_> =
        cont_store.snapshot().await.unwrap().nodes.keys().copied().collect();
    assert!(
        path_ids.intersection(&cont_ids).count() == 0,
        "PathBased and ContentAddressed should produce disjoint ids"
    );
}

#[tokio::test]
async fn merge_combines_repeated_writes() {
    // Run twice with `Merge`: first run with one attribute, second with another.
    // Both should land on the leaf node.
    let source1 = Arc::new(InMemoryActorPersistenceSource::new("strategies"));
    source1.push_path(
        SupervisionPath::parse("/workflow/foo/run/1/step/a")
            .with_attribute("first", serde_json::json!("yes")),
    );

    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    ProjectorBuilder::new()
        .source(source1)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .conflict(ConflictResolution::Merge)
        .store(store.clone())
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    let source2 = Arc::new(InMemoryActorPersistenceSource::new("strategies"));
    source2.push_path(
        SupervisionPath::parse("/workflow/foo/run/1/step/a")
            .with_attribute("second", serde_json::json!("yes")),
    );

    ProjectorBuilder::new()
        .source(source2)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .conflict(ConflictResolution::Merge)
        .store(store.clone())
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    let ontology = store.snapshot().await.unwrap();
    let leaf = ontology
        .nodes
        .values()
        .find(|n| {
            n.properties
                .get(vocab::PROP_PATH)
                .and_then(|p| match p {
                    atomr_ontology_core::PropertyValue::String(s) => Some(s.as_str()),
                    _ => None,
                })
                == Some("/workflow/foo/run/1/step/a")
        })
        .expect("leaf node present");
    assert!(leaf.properties.contains_key("first"), "first-run property preserved");
    assert!(leaf.properties.contains_key("second"), "second-run property merged");
}

#[tokio::test]
async fn skip_existing_is_idempotent() {
    let source = Arc::new(InMemoryActorPersistenceSource::new("strategies"));
    source.push_path(SupervisionPath::parse("/workflow/foo/run/1"));
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let make = || {
        ProjectorBuilder::new()
            .source(source.clone())
            .with_ingest(Arc::new(ReplayIngest::once()))
            .projection(Arc::new(HierarchicalProjection::new()))
            .iri(IriMintingStrategy::PathBased {
                base: Iri::new("https://atomr.dev/actor/").unwrap(),
            })
            .conflict(ConflictResolution::SkipExisting)
            .store(store.clone())
            .build()
            .unwrap()
    };
    make().run().await.unwrap();
    let count_after_first = store.snapshot().await.unwrap().node_count();
    make().run().await.unwrap();
    let count_after_second = store.snapshot().await.unwrap().node_count();
    assert_eq!(count_after_first, count_after_second, "SkipExisting must not grow the store");
}

#[tokio::test]
async fn fixed_schema_rejects_unknown_types() {
    // Empty schema — projector will emit "Actor" / "Workflow" etc., none declared.
    let empty = Schema::new();
    let source = Arc::new(InMemoryActorPersistenceSource::new("strategies"));
    source.push_path(SupervisionPath::parse("/workflow/foo"));
    let cp = MemCheckpointer::new();
    let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(cp).await.unwrap());

    let projector = ProjectorBuilder::new()
        .source(source)
        .with_ingest(Arc::new(ReplayIngest::once()))
        .projection(Arc::new(HierarchicalProjection::new()))
        .iri(IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        })
        .conflict(ConflictResolution::LastWriteWins)
        .schema(SchemaStrategy::FixedSchema(empty))
        .store(store)
        .build()
        .unwrap();
    let result = projector.run().await;
    assert!(
        matches!(result, Err(ProjectorError::Projection(_))),
        "fixed schema should reject unknown types, got {result:?}"
    );
}

#[tokio::test]
async fn provenance_records_one_activity_per_batch() {
    let (store, report) = build_projector_with(
        IriMintingStrategy::PathBased {
            base: Iri::new("https://atomr.dev/actor/").unwrap(),
        },
        ConflictResolution::LastWriteWins,
        SchemaStrategy::Hybrid(vocab::actor_schema()),
    )
    .await;
    let prov = store.provenance().await.unwrap();
    assert_eq!(prov.activities.len() as u64, report.activities_recorded);
    assert!(prov.activities.len() >= 1);
    let activity = prov.activities.values().next().unwrap();
    let attrs = &activity.attributes;
    assert!(attrs.contains_key("projection"), "activity carries projection attr");
    assert!(attrs.contains_key("source"), "activity carries source attr");
}
