# atomr-ontology-actor-projection

Project actor-system persistence data — **supervision-tree paths**, **journal events**, and **serialized state** — into an [`OntologyStore`](https://docs.rs/atomr-ontology-store). The crate is purely additive: it composes existing primitives (Node/Edge builders, `Checkpointer`, `OntologyStore`, PROV-O `Activity`) and adds three small SPI traits plus four built-in implementations of each.

## Why

Other crates in the workspace build ontologies from text (LLM extractors), from RDF (`atomr-ontology-rdf` + `atomr-ontology-import`), or from a programmatic seed. This crate adds a fourth source: the **runtime persistence trail** of an actor or agent system. Given that trail it produces an ontology you can reason over, query, validate, and reuse as input to extractors.

## Concepts

| Concept | Trait | Built-ins |
|---|---|---|
| **Source** — actor data | `ActorPersistenceSource` | `InMemoryActorPersistenceSource`, `SnapshotActorPersistenceSource` (feature `snapshot-source`) |
| **Ingest** — how to read | `IngestMode` | `ReplayIngest`, `PollingIngest`, `PushHookIngest`, `EventStreamIngest` |
| **Projection** — graph shape | `ProjectionStrategy` | `HierarchicalProjection`, `EventStreamProjection`, `SnapshotDiffProjection`, `FlatProjection` |
| **IRI minting** | `IriMintingStrategy` enum | `PathBased { base }`, `ContentAddressed`, `Uuid` |
| **Conflict resolution** | `ConflictResolution` enum | `LastWriteWins`, `Merge`, `SkipExisting` |
| **Schema** | `SchemaStrategy` enum | `FixedSchema(_)`, `InducedSchema`, `Hybrid(_)` |

`ProjectorBuilder` composes one source + N ingest modes + one projection + one of each strategy enum + an `OntologyStore` into a `Projector`. The projector multiplexes ingest into one batch stream, projects each batch, applies the conflict policy against the store, and commits with a PROV-O activity per batch.

## Quickstart

```rust
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

# async fn run() -> anyhow::Result<()> {
let source = Arc::new(InMemoryActorPersistenceSource::new("demo"));
source.push_path(SupervisionPath::parse("/workflow/foo/run/1/step/a"));

let store: Arc<dyn OntologyStore> = Arc::new(PersistentStore::new(MemCheckpointer::new()).await?);

let projector = ProjectorBuilder::new()
    .source(source)
    .with_ingest(Arc::new(ReplayIngest::once()))
    .projection(Arc::new(HierarchicalProjection::new()))
    .iri(IriMintingStrategy::PathBased { base: Iri::new("https://atomr.dev/actor/")? })
    .conflict(ConflictResolution::Merge)
    .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
    .store(store.clone())
    .build()?;

let report = projector.run().await?;
println!("{report:?}");
# Ok(()) }
```

## Strategy matrix

Pick the projection shape that fits your query needs:

| Shape | Best for | Trade-off |
|---|---|---|
| `HierarchicalProjection` | Supervision navigation, "which workflow owns this step?" | Many small nodes; deep paths |
| `EventStreamProjection` | Audit timelines, chronological replay | No structural navigation |
| `SnapshotDiffProjection` | Incremental sync against a long-running source | Stateful; must be kept alive between batches |
| `FlatProjection` | Fastest queries on `(workflow, run)` | Loses graph navigability |

Combine multiple shapes by spawning multiple `Projector`s sharing the same `source: Arc<dyn ActorPersistenceSource>`.

Mix-and-match ingest modes by chaining `.with_ingest(...)`. Each mode runs concurrently and feeds the same projection pipeline. A common combination is `Replay::once()` + `PushHookIngest` for bootstrap + live updates.

## Extending

To add a new source, ingest mode, or projection shape, implement the corresponding trait and pass it to the builder. The built-ins are not privileged — they are simply additional impls.

## Crate features

- `snapshot-source` — enable `SnapshotActorPersistenceSource`, which wraps any `Checkpointer` whose snapshots already store actor records as ontology nodes.
- `agents` — reserved for future direct integration with `atomr-agents`.

## See also

- [`examples/actor_projection_demo`](../../examples/actor_projection_demo/) — runs all four projection shapes against a synthetic workflow.
- [`atomr-ontology-py` Python bindings](../atomr-ontology-py/) — exposes `Projector`, `ProjectorBuilder`, and `InMemoryActorSource` to Python.
