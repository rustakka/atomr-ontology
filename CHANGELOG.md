# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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
