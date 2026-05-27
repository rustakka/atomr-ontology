# Actor-persistence projection

`atomr-ontology-actor-projection` turns the runtime persistence trail of an actor system into an `OntologyStore`. It composes the existing checkpointer/store primitives with three small SPI traits: source, ingest mode, and projection strategy.

## When to reach for it

- You have an actor or agent system (atomr-agents, a custom supervision tree, an Akka-style journal) and want a queryable graph of its topology and history.
- You want a single, uniform projection of that data so downstream tools (`atomr-ontology-reason`, `atomr-ontology-validate`, `atomr-ontology-viz`) work against it.
- You want incremental sync (live updates) without writing the glue yourself.

## SPI

```text
ActorPersistenceSource           IngestMode             ProjectionStrategy
       ↓                             ↓                          ↓
   paths / journal / state →   [batch stream] →    OntologyDelta + Activity
                                                          ↓
                                                   OntologyStore
```

- **Source** yields supervision-tree paths, journal events, and serialized state blobs. Built-ins: `InMemoryActorPersistenceSource`, `SnapshotActorPersistenceSource` (wraps a `Checkpointer`).
- **Ingest** drives the source: `ReplayIngest` (one-shot), `PollingIngest` (periodic), `PushHookIngest` (wraps an upstream `Checkpointer`), `EventStreamIngest` (subscribe to a broadcast channel).
- **Projection** shapes the graph: `HierarchicalProjection`, `EventStreamProjection`, `SnapshotDiffProjection`, `FlatProjection`.

## Strategies

Three small enums tune the projector:

- `IriMintingStrategy::{PathBased, ContentAddressed, Uuid}`.
- `ConflictResolution::{LastWriteWins, Merge, SkipExisting}`.
- `SchemaStrategy::{FixedSchema(Schema), InducedSchema, Hybrid(Schema)}`.

All strategies are pure-functional enums (mirrors `CachePolicy` in `atomr-ontology-extract`).

## Composition example

```rust
let projector = ProjectorBuilder::new()
    .source(source)
    .with_ingest(Arc::new(ReplayIngest::once()))     // bootstrap
    .with_ingest(Arc::new(PushHookIngest::new(...))) // live updates
    .projection(Arc::new(HierarchicalProjection::new()))
    .iri(IriMintingStrategy::PathBased { base: Iri::new("https://atomr.dev/actor/")? })
    .conflict(ConflictResolution::Merge)
    .schema(SchemaStrategy::Hybrid(vocab::actor_schema()))
    .store(store)
    .build()?;
projector.run().await?;
```

Multiple ingest modes run concurrently. They multiplex into one batch channel that the projector drains and commits.

## Provenance

Every committed batch records one PROV-O `Activity` carrying:

- `prov:wasAssociatedWith` = the projector software agent.
- `projection` attribute = the projection-strategy label.
- `source` attribute = the source label.
- `cursor_version` attribute = the high-water-mark version at batch end.

Read the activities through `store.provenance()`.

## Extending

The four built-ins are not privileged. To add a new mode or shape, implement the corresponding trait and pass an `Arc` of it to the builder. Custom kinds report `IngestKind::Custom(String)` / `ProjectionKind::Custom(String)` for diagnostics.

## Demo + tests

- Example: `examples/actor_projection_demo` runs all four projections end-to-end against a synthetic three-workflow source.
- Tests: `cargo test -p atomr-ontology-actor-projection --all-features` exercises every projection shape, every ingest mode, every strategy enum, and the provenance contract.

## Python bindings

`atomr_ontology.actor_projection` exposes `InMemoryActorSource`, `ProjectorBuilder`, `Projector`, and `OntologyStoreHandle`. All builders and getters mirror the Rust surface; async methods return `Awaitable`s.
