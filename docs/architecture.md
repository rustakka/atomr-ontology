# Architecture

`atomr-ontology` is a tiered Rust workspace. Each tier has an
explicit responsibility and depends only on tiers below it.

```
┌───────────────────────────────────────────────────────────────────────────┐
│ Tier 3 — facade, reference, and presentation                              │
│   atomr-ontology         (umbrella, prelude, feature gates, http_driver)  │
│   atomr-ontology-org     (W3C Org + schema.org reference vocabulary)      │
│   atomr-ontology-testkit (MockBackend, fixtures, assertion helpers)       │
│   atomr-ontology-import  (SKOS / FOAF / schema.org bulk importers)        │
│   atomr-ontology-viz     (GraphViz DOT + Mermaid renderers)               │
│   atomr-ontology-shacl   (Schema ↔ SHACL Turtle round-trip)               │
│   atomr-ontology-py      (PyO3 bindings, .pyi stubs, asyncio coroutines)  │
├───────────────────────────────────────────────────────────────────────────┤
│ Tier 2 — runtime                                                          │
│   atomr-ontology-store    (OntologyStore trait + MemStore + query IR)     │
│   atomr-ontology-extract  (Term/Entity/Relation/Record + Backend trait)   │
│   atomr-ontology-induce   (Taxonomy/Concepts/Axioms induction)            │
│   atomr-ontology-validate (Shapes + axiom consistency checks)             │
│   atomr-ontology-persist  (Pluggable Checkpointer → PersistentStore)      │
│   atomr-ontology-reason   (Forward-chaining OWL 2 RL / EL reasoner)       │
│   atomr-ontology-embed    (Vector-similarity entity-resolution prefilter) │
│   atomr-ontology-version  (Branchable, time-travelable ontologies)        │
│   atomr-ontology-query    (Cypher / SPARQL subset → TraversalPlan)        │
│   atomr-ontology-remote   (HTTP/JSON OntologyStore server + client)       │
├───────────────────────────────────────────────────────────────────────────┤
│ Tier 1 — pure data                                                        │
│   atomr-ontology-core       (LPG types: Iri, Node, Edge, Schema, Axiom)   │
│   atomr-ontology-rdf        (RDF/OWL adapter + Turtle/NT/JSON-LD I/O)     │
│   atomr-ontology-provenance (PROV-O Activity/Entity/Agent + lineage)      │
└───────────────────────────────────────────────────────────────────────────┘
```

Tier 1 crates have no I/O, no actors, and no async surface; they
are pure data plus a few traversal helpers. Tier 2 crates depend on
Tier 1 and on each other through stable trait surfaces. Tier 3 is
ergonomic glue, presentation layers, and language bindings.

## Lifecycle of an auto-built ontology

1. **Seed.** A `MemStore` is constructed (optionally from a
   reference vocabulary like the one in `atomr-ontology-org`).
2. **Ingest.** Documents enter via plain text or a structured
   record source; they are passed downstream as `&str`.
3. **Extract.** `TermExtractor` → `EntityResolver` →
   `RelationExtractor` propose candidates. Each stage produces a
   PROV-O `Activity` attached to its output.
4. **Induce** (optional). `ConceptFormer`, `TaxonomyInducer`,
   `AxiomMiner` lift candidates into a schema sketch.
5. **Validate.** `atomr-ontology-validate::validate` runs SHACL-
   style shape checks and axiom consistency over the proposed
   delta plus the live state.
6. **Commit.** `OntologyStore::commit_with_provenance` applies
   the delta atomically and records the activity in the
   provenance log.
7. **Project.** `atomr-ontology-rdf` writes the live snapshot to
   Turtle, N-Triples, or JSON-LD as needed for export.

## Lifecycle when reasoning and versioning are involved

The seven steps above remain unchanged; the following stages slot in
*around* them when the workflow opts into the richer surface:

- **5a. Reason** (between *Induce* and *Validate*).
  `atomr-ontology-reason::Reasoner` runs forward-chaining OWL 2
  RL/EL closure over the proposed delta + live ontology. Derived
  axioms carry `wasDerivedFrom` provenance pointing at the
  reasoning `Activity` so downstream consumers can distinguish
  asserted from inferred facts. See [`reasoning.md`](reasoning.md).
- **5b. Embed** (parallel to *Extract*). When entity resolution
  benefits from a top-k pre-filter, an `EmbeddingResolver` ingests
  the live ontology once and then proposes candidates per surface
  form before the LLM disambiguates. See
  [`embeddings.md`](embeddings.md).
- **6a. Branch + commit**. Instead of mutating a single store,
  `atomr-ontology-version::VersionedStore` lets the workflow open
  a branch, validate a delta, and either merge into `main` or
  discard. Time-travel queries via `as_of(commit_id)`. See
  [`versioning.md`](versioning.md).
- **6b. Persist**. The default `MemStore` is in-memory; for durable
  workflows wrap a `Checkpointer` (memory / file / SQLite) in
  `PersistentStore` and the same `OntologyStore` trait works
  unchanged. See [`persistence.md`](persistence.md).
- **6c. Serve**. A hosted `OntologyStore` ships behind
  `atomr-ontology-remote::serve` (HTTP/JSON RPC); clients implement
  the same trait via `RemoteClient`. See [`remote.md`](remote.md).
- **7a. Render**. Beyond RDF projection,
  `atomr-ontology-viz::render_ontology_dot` / `_mermaid` emits
  GraphViz / Mermaid for documentation and debugging. See
  [`viz.md`](viz.md).
- **7b. Import external standards**.
  `atomr-ontology-import` ingests SKOS / FOAF / schema.org JSON-LD
  with PROV-O activity records. See [`importers.md`](importers.md).
- **7c. Export SHACL**. `atomr-ontology-shacl::to_shacl_turtle`
  emits SHACL shapes derived from the live `Schema` for
  interoperation with W3C tooling. See [`shacl.md`](shacl.md).

The optional stages are independent and composable: a pipeline can
reason without versioning, persist without remoting, or render
without ever calling the extractors.

## Backend abstraction

Each extractor depends on a narrow `Backend` trait (one async
`complete(prompt) -> String` method). The trait stays deliberately
smaller than `atomr_agents::Agent` or `atomr_infer::Provider` so the
workspace is decoupled from upstream generics churn and so the
testkit can plug in a deterministic mock without dragging in the full
runtime stack.

The recommended layering for agentic workflows is
**`Backend ← AgentBackend ← atomr_agents::Agent ← atomr_infer::Provider`**:
extractors hold an `Arc<dyn Backend>`, `AgentBackend` (in
`atomr_ontology::agents_integration`) drives an `atomr_agents::Agent`,
and the agent uses an `atomr_infer` provider for the underlying
inference call. For multi-turn / tool-using workflows
(`AgenticTaxonomyInducer`, `AgenticAxiomMiner` in
`atomr-ontology-induce`), the same module exposes `AgenticAgent` /
`AgenticDriver` / `ToolSpec` — defined in
`atomr-ontology-extract::agentic` so the workflow crates can use them
directly. Direct seams (`InferBackend`, `MockBackend`) and the
deprecated `HttpDriver` (slated for removal in 0.4) live on the same
`Backend` trait. See [`providers.md`](providers.md) for the decision
tree and [`agents.md`](agents.md) for the agent-loop semantics.

## Why labeled property graph as canonical?

- **Ergonomics.** Node-with-properties matches the shape of most
  application data sources (CSV rows, JSON documents, database
  rows) without forcing reification.
- **Performance.** Pattern matching against a typed property bag is
  cheaper than running a SPARQL query against a triple store.
- **Interop.** RDF/OWL projection is straightforward and
  documented in [`naming.md`](naming.md); the reverse direction
  is offered for T-Box import.
