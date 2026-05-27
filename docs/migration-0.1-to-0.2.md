# Migration: 0.1.0 → unreleased

This guide covers the move from the v0.1.0 surface to the
unreleased (next-minor) release that introduces nine new tier-2/3
crates, extends existing ones, and ships strict 1:1 Python parity.

The bottom line: **no mechanical renames; no required code
changes for v0.1 callers.** Existing code continues to compile.
This guide describes the new features you may want to adopt and
the two behavior changes you should know about.

## Behavior changes

### 1. Backend trait gained default methods

`atomr_ontology_extract::Backend` grew three methods, all with
default implementations:

```rust
async fn batch_complete(&self, prompts: Vec<Prompt>) -> Vec<Result<String, BackendError>>;
async fn stream_complete(&self, prompt: Prompt) -> Result<ChunkStream, BackendError>;
```

Existing `impl Backend for MyBackend` blocks compile unchanged.
For best performance, override the defaults when your provider
exposes native batch / streaming APIs (see
[`agents.md`](agents.md#batching-streaming-caching)).

### 2. ID JSON wire format is now stable

`NodeId` / `EdgeId` / `AxiomId` / `ProvenanceId` had a deserializer
that asked for `&[u8]` (borrowed bytes), which JSON cannot
provide. The deserializer now uses `serde_bytes::ByteBuf` (owned),
so the same byte-array JSON shape that serialization always
produced now round-trips.

Code that already used JSON for these types kept working because
they only **emitted** JSON. Code that **read** JSON-encoded ids
(and worked around the original bug by avoiding it) can now do so
directly. See [`data-model.md`](data-model.md#json-wire-format-for-ids).

## New capabilities — when to adopt

### Persistence

The v0.1 `MemStore` is in-memory only. To survive process
restarts, wrap a `Checkpointer` in `PersistentStore`:

```rust
// Before — v0.1
let store = MemStore::new();

// After — durable
use atomr_ontology_persist::{FileCheckpointer, PersistentStore};
let checkpointer = FileCheckpointer::new("ontology.json");
let store = PersistentStore::new(checkpointer).await?;
```

`PersistentStore<C>` implements `OntologyStore`, so every other
call (`upsert_node`, `commit_with_provenance`, `traverse`, …)
works unchanged. See [`persistence.md`](persistence.md).

### Reasoning

Materialize implicit subclass / equivalent / transitive /
inverse-of facts:

```rust
use atomr_ontology_reason::Reasoner;
let report = Reasoner::new().materialize(&mut ontology)?;
println!("derived {} axioms", report.derived_axioms.len());
```

Derived axioms carry `wasDerivedFrom` provenance. See
[`reasoning.md`](reasoning.md).

### Richer query patterns

The v0.1 `TraversalPlan` only handled fixed-length hops. The new
builders cover variable-length paths, OR/NOT, projection, and
ORDER BY / LIMIT:

```rust
// Before — v0.1 (single hop only)
let plan = TraversalPlan::from(NodePattern::any().bind("a"))
    .outbound(EdgePattern::any().labeled("subClassOf"),
              NodePattern::any().bind("b"));

// After — transitive closure, projected, limited
let plan = TraversalPlan::from(NodePattern::any().bind("a"))
    .outbound(EdgePattern::any().labeled("subClassOf").repeat(1..=5),
              NodePattern::any().bind("b"))
    .return_(["b"])
    .order_by("b")
    .limit(10);
```

The string DSL (Cypher + SPARQL subsets) lowers to the same
`TraversalPlan` IR. See [`query.md`](query.md).

### RDF reads

`atomr-ontology-rdf` gained Turtle / N-Triples / JSON-LD parsers:

```rust
let o = atomr_ontology::rdf::turtle::read(&document)?;
```

Same module structure as the writers; behind the same feature
flags (`turtle`, `ntriples`, `jsonld`, all default-on).

### Provider layering (recommended path) + HTTP driver deprecation

The recommended provider layering for agentic ontology workflows is
**`Backend ← AgentBackend ← atomr_agents::Agent ←
atomr_infer::Provider`**. The umbrella now ships the meta-features
`agents-with-openai`, `agents-with-anthropic`,
`agents-with-litellm`, `agents-with-candle`, etc. that bundle the
agent surface with a matching `atomr-infer` provider so the
recommended stack compiles in one shot.

For multi-turn / tool-using induction the new
`AgenticTaxonomyInducer` and `AgenticAxiomMiner` in
`atomr-ontology-induce` consume an `AgenticAgent` (also in the
umbrella's `agents_integration`). Pair them with
`atomr_ontology::extract::store_tools::default_store_tools(store)`
to give the agent live ontology introspection. See
[`agents.md`](agents.md#agentic-induction) and
[`providers.md`](providers.md#provider-selection).

**`HttpDriver` is deprecated** as of v0.2 and will be removed in v0.4.
The direct-REST driver was introduced as a lightweight
no-extra-deps shim; the recommended replacement is `InferBackend`
over one of the matching `provider-*` features
(`provider-openai`, `provider-anthropic`, `provider-litellm`), or —
for the recommended layering — `AgentBackend` over an
`atomr_agents::Agent` whose inference goes through `atomr-infer`.
Construction emits a `#[deprecated]` warning in Rust and a
`DeprecationWarning` in Python. A worked migration example lives in
[`crates/atomr-ontology/examples/http_driver_migration.rs`](../crates/atomr-ontology/examples/http_driver_migration.rs).
Existing callers keep working through the deprecation window.

### Other adoptable additions

| Capability | Crate | Guide |
| --- | --- | --- |
| Branchable + time-travelable ontologies | `atomr-ontology-version` | [`versioning.md`](versioning.md) |
| Vector-similarity entity resolution | `atomr-ontology-embed` | [`embeddings.md`](embeddings.md) |
| HTTP/JSON RPC for hosted ontologies | `atomr-ontology-remote` | [`remote.md`](remote.md) |
| SKOS / FOAF / schema.org bulk import | `atomr-ontology-import` | [`importers.md`](importers.md) |
| GraphViz / Mermaid rendering | `atomr-ontology-viz` | [`viz.md`](viz.md) |
| SHACL round-trip | `atomr-ontology-shacl` | [`shacl.md`](shacl.md) |
| Pipeline 7-stage demo now exercises stages 4–5 | `examples/auto_extract_from_text` | [`agents.md`](agents.md) |

## Python

The Python bindings are new in this release. Install from PyPI:

```bash
pip install atomr-ontology
```

The wheel ships `.pyi` stubs for every submodule and exposes 1:1
Python wrappers around every public Rust type and function. See
[`python.md`](python.md) for the deep dive.

## Cargo feature changes

The umbrella crate added these features (all default-off so v0.1
builds are unaffected):

| Feature | Pulls |
| --- | --- |
| `http-driver` | **DEPRECATED** (removed in 0.4). `reqwest`, `serde_json`, `tokio`, the deprecated `http_driver` module. |
| `provider-openai` … `provider-cudarc` | `atomr-infer` plus the matching upstream feature |
| `agents-with-openai`, `agents-with-anthropic`, `agents-with-gemini`, `agents-with-litellm`, `agents-with-vllm`, `agents-with-candle` | Meta: `agents` + the matching `provider-*` flag. Recommended for the canonical agentic stack. |
| `agents-with-infer` | Meta: `agents` + `infer`, with no specific provider. |

Existing features (`rdf`, `provenance`, `store`, `extract`,
`induce`, `validate`, `org`, `testkit`, `infer`, `agents`)
behave identically to v0.1. The `agents` feature additionally
re-exports the new `AgenticAgent` / `AgenticDriver` / `ToolSpec`
surface from `atomr-ontology-extract::agentic`.

## Verification

After upgrading the dependency, run:

```bash
cargo build --workspace
cargo test --workspace
```

Both should pass without modification. The benchmark suite
(`cargo bench -p atomr-ontology-store -p atomr-ontology-rdf`) is
new and runs the criterion harness for the workspace's hot paths.
