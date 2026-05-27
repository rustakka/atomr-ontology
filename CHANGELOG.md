# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Recommended provider layering documented end-to-end.** The canonical
  stack is `Backend ← AgentBackend ← atomr_agents::Agent ←
  atomr_infer::Provider`. New top-level diagrams in `README.md` and
  `docs/providers.md`, decision tree in `docs/providers.md`, and
  reframed `docs/agents.md` / `docs/architecture.md` /
  `docs/getting-started.md` / `docs/python.md` /
  `docs/migration-0.1-to-0.2.md` plus per-crate READMEs all point at
  the same canonical wiring.
- **Agentic surface** (`atomr-ontology-extract::agentic`,
  re-exported from `atomr-ontology::agents_integration`): new
  `AgenticDriver` trait, `AgenticAgent`, `AgenticSession`,
  `AgenticOutcome`, `ToolSpec`, `ToolCallRecord`, `TurnRecord`,
  `StopCondition` types. `AgenticAgent` also implements `Backend` so
  it's a drop-in for the existing single-turn extractors.
- **Built-in `OntologyStore` tool palette**
  (`atomr-ontology-extract::store_tools::default_store_tools`):
  `class_exists`, `list_classes`, `list_edge_types`,
  `count_instances`, `subclasses_of`, `supertypes_of`,
  `properties_of`. Hand the palette to an `AgenticAgent` to give it
  live introspection over the in-flight ontology.
- **Agentic inducers** (`atomr-ontology-induce`):
  `AgenticTaxonomyInducer` and `AgenticAxiomMiner`. Multi-turn,
  tool-using variants of the existing one-shot inducers; each emits
  a PROV-O `Activity` tagged with the session's `tool_calls` and
  `turns` counts.
- **`agents-with-*` umbrella meta-features**:
  `agents-with-infer`, `agents-with-openai`, `agents-with-anthropic`,
  `agents-with-gemini`, `agents-with-litellm`, `agents-with-vllm`,
  `agents-with-candle`. Bundle the agent surface with a matching
  `atomr-infer` provider so the recommended stack compiles in one
  shot. Python parity via the matching pip extras.
- **`atomr-ontology-py.agents`**: PyO3 submodule exposing
  `AgenticAgent`, `AgenticSession`, `ToolSpec`, `AgenticOutcome`,
  `TurnRecord`, `ToolCallRecord`, `StopCondition`,
  `AgenticTaxonomyInducer`, `AgenticAxiomMiner`, and
  `default_store_tools_py`. Build with the `agents` feature.
- **`examples/http_driver_migration.rs`**: walkthrough of replacing
  the deprecated `HttpDriver` with `InferBackend` (single-shot) or
  `AgentBackend` (recommended agentic layering).

### Deprecated
- **`HttpDriver` and the `http-driver` feature** (Rust + Python).
  Slated for removal in 0.4. The direct-REST shim is superseded by
  `InferBackend` over `atomr-infer`'s remote providers
  (`provider-openai`, `provider-anthropic`, `provider-litellm`), or
  — for the recommended layering — `AgentBackend` over an
  `atomr_agents::Agent` whose inference goes through `atomr-infer`.
  Construction emits a `#[deprecated]` warning in Rust and a
  `DeprecationWarning` in Python. Existing callers keep working
  through the deprecation window; see
  [`docs/providers.md`](docs/providers.md#http-driver) for the
  migration recipe and
  [`crates/atomr-ontology/examples/http_driver_migration.rs`](crates/atomr-ontology/examples/http_driver_migration.rs)
  for a worked example.

- **RDF read parsers** (`atomr-ontology-rdf`): `turtle::parse/read`,
  `ntriples::parse/read`, `jsonld::parse/read` round-trip the existing
  writers; T-Box and IRI-typed instances are reconstructed via
  `adapter::from_rdf`.
- **HTTP driver** (`atomr-ontology` umbrella, feature `http-driver`):
  `HttpDriver` calls OpenAI Chat Completions / Anthropic Messages /
  LiteLLM-proxied endpoints directly via `reqwest`; no `atomr-infer`
  dep needed. Reads `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` /
  `LITELLM_API_KEY` (with `*_BASE_URL` overrides).
- **Backend trait extensions** (`atomr-ontology-extract`):
  `batch_complete` (concurrent fan-out via `futures::join_all`),
  `stream_complete` (yields `ChunkStream`), `CachedBackend` with
  `CachePolicy::{None, ContentAddressed, Lru(n)}`, convenience
  `lru_cached`/`content_cached` helpers.
- **Richer query patterns** (`atomr-ontology-store`):
  `NodePattern::or/not`, `EdgePattern::repeat(min..=max)` for
  variable-length paths, `TraversalPlan::return_/order_by/order_by_desc/skip/limit`,
  `SortOrder`. Existing `MemStore` executor extended; results
  deterministic.
- **`atomr-ontology-persist`** (Tier 2): `PersistentStore<C: Checkpointer>`
  + bundled `MemCheckpointer`, `FileCheckpointer` (feature `file`),
  `SqliteCheckpointer` (feature `sqlite`). JSON-friendly wire format
  for `Snapshot` (hex-encoded 32-byte IDs).
- **`atomr-ontology-reason`** (Tier 2): forward-chaining OWL 2 RL/EL
  reasoner. `Reasoner` materializes derived `SubClassOf`,
  `EquivalentClass`, `InverseOf`, transitive, symmetric, and property
  consequences with `wasDerivedFrom` provenance.
- **`atomr-ontology-embed`** (Tier 2): vector-similarity entity
  resolution. `EmbeddingBackend` trait, `HashEmbedder`,
  `VectorIndex` (linear scan, HNSW upgrade planned),
  `EmbeddingResolver`.
- **`atomr-ontology-version`** (Tier 2): Git-style branchable
  ontologies. `VersionedStore` with `commit`/`branch`/`checkout`/`merge`/
  `as_of`. Content-addressed `CommitId` via blake3.
- **`atomr-ontology-query`** (Tier 2): hand-rolled Cypher + SPARQL
  subset parsers compiling to `TraversalPlan`. Supports variable-
  length paths, WHERE NOT, RETURN, LIMIT/OFFSET.
- **`atomr-ontology-remote`** (Tier 2): HTTP/JSON RPC
  `OntologyStore`. `RemoteClient` (feature `client`) and `serve`
  (feature `server`, hand-rolled TcpListener). Default features
  enable both.
- **`atomr-ontology-import`** (Tier 3): bulk importers
  `import_skos`, `import_foaf`, `import_schema_org` — each emits a
  PROV-O `Activity`.
- **`atomr-ontology-viz`** (Tier 3): `render_ontology_dot`,
  `render_ontology_mermaid`, `render_provenance_dot`,
  `render_provenance_mermaid`.
- **`atomr-ontology-shacl`** (Tier 3): `to_shacl_turtle(&Schema)`
  emits SHACL shapes; `from_shacl_turtle` parses them back.
- **Python bindings** (`atomr-ontology-py`): PyO3 + maturin wheel
  exposing the umbrella API surface. All Rust async methods
  (`OntologyStore`, extractors, inducers) are exposed as Python
  coroutines via `pyo3-async-runtimes`. The 9 new tier-2/tier-3
  crates each gain a Python submodule
  (`atomr_ontology.{persist, reason, embed, version, query, remote,
  import_, viz, shacl}`). `.pyi` stubs ship for every submodule.
  Provider integrations from `atomr-infer` are opt-in via Python
  extras (`atomr-ontology[openai]`, `[anthropic]`, `[vllm]`, etc.);
  `http-driver` extra exposes the OpenAI/Anthropic/LiteLLM HTTP
  client.
- **Benchmark suite**: `criterion` benches for
  `atomr-ontology-store` (upsert / match / variable-length
  traverse) and `atomr-ontology-rdf` (turtle/ntriples/jsonld
  write+parse, to_rdf projection).
- **Demo pipeline** now exercises all 7 stages: terms → entities →
  relations → concept formation → taxonomy induction → validate →
  commit with provenance.

### Fixed
- `atomr-ontology-core::id::serde_bytes_array::deserialize` now uses
  `serde_bytes::ByteBuf` (owned), restoring JSON round-tripping of
  `NodeId`/`EdgeId`/`AxiomId`/`ProvenanceId`.

## [0.1.0] — initial release

### Added
- Labeled property graph core types (`atomr-ontology-core`):
  `Iri`, `Namespace`, `Vocabulary`, `Ontology`, `Node`, `Edge`,
  `Schema`, `Record`, `Axiom`, content-addressed `NodeId`/`EdgeId`.
- RDF/OWL adapter (`atomr-ontology-rdf`): `Class`, `Individual`,
  `ObjectProperty`, `DataProperty`, `Triple`, `Quad`, Turtle /
  N-Triples / JSON-LD serializers behind feature flags.
- PROV-O-aligned provenance types (`atomr-ontology-provenance`).
- `OntologyStore` trait and in-memory implementation
  (`atomr-ontology-store`) with builder-style pattern matching and
  traversal.
- Extraction primitives (`atomr-ontology-extract`):
  `TermExtractor`, `EntityResolver`, `RelationExtractor`,
  `RecordExtractor`.
- Ontology-learning primitives (`atomr-ontology-induce`):
  `TaxonomyInducer`, `ConceptFormer`, `AxiomMiner`.
- SHACL-style validation (`atomr-ontology-validate`).
- Testkit fixtures and `MockBackend` (`atomr-ontology-testkit`).
- W3C Org Ontology reference vocabulary (`atomr-ontology-org`).
- Umbrella facade with `prelude` (`atomr-ontology`).
- Examples: `org_ontology_demo`, `auto_extract_from_text`.
- xtask: `parity`, `verify`, `audit`.
