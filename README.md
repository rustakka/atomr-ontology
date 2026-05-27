# atomr-ontology

Ontology engineering and ontology-learning crate for the
[rustakka/atomr](https://github.com/rustakka/atomr) ecosystem. Build,
manage, and reason over labeled property graphs and their RDF/OWL
projections, with agents from
[`atomr-agents`](https://github.com/rustakka/atomr-agents) and inference
providers from
[`atomr-infer`](https://github.com/rustakka/atomr-infer).

The crate ships a **canonical labeled property graph (LPG)** data model
and a **non-canonical RDF/OWL adapter** alongside it, so authors can
work in the most ergonomic representation while still interoperating
with W3C tooling. The reference vocabulary is the W3C Org Ontology
projected through schema.org's `Organization`, included as a worked
example rather than a privileged core.

## Workspace tiers

| Tier | Crate | Role |
| --- | --- | --- |
| 1 | `atomr-ontology-core` | Pure-data LPG primitives (`Iri`, `Node`, `Edge`, `Schema`, `Record`, `Axiom`, IDs). |
| 1 | `atomr-ontology-rdf` | RDF/OWL adapter (`Class`, `Individual`, `Triple`); Turtle / N-Triples / JSON-LD read+write behind features. |
| 1 | `atomr-ontology-provenance` | PROV-O-aligned types (`Activity`, `ProvAgent`, `ProvEntity`, lineage edges). |
| 2 | `atomr-ontology-store` | `OntologyStore` trait + `MemStore`. Variable-length paths, OR/NOT, projection, ORDER BY, LIMIT. |
| 2 | `atomr-ontology-extract` | Term / entity / relation / record extractors. `Backend` trait with `batch_complete`, `stream_complete`, `CachedBackend`. |
| 2 | `atomr-ontology-induce` | Taxonomy induction, concept formation, axiom mining. |
| 2 | `atomr-ontology-validate` | SHACL-style shape validation and axiom-consistency checks. |
| 2 | `atomr-ontology-persist` | Pluggable persistent `OntologyStore` (memory / file / sqlite checkpointers). |
| 2 | `atomr-ontology-reason` | Forward-chaining OWL 2 RL/EL reasoner. |
| 2 | `atomr-ontology-embed` | Vector-similarity entity resolution (`EmbeddingBackend` + `VectorIndex`). |
| 2 | `atomr-ontology-version` | Git-style branchable, time-travelable ontologies. |
| 2 | `atomr-ontology-query` | Cypher + SPARQL subset parsers compiling to `TraversalPlan`. |
| 2 | `atomr-ontology-remote` | HTTP/JSON `OntologyStore` server + client. |
| 2 | `atomr-ontology-actor-projection` | Project actor-system persistence data (supervision paths, journal events, serialized state) into an `OntologyStore`. Four ingest modes × four projection shapes × three strategies. |
| 3 | `atomr-ontology-testkit` | Mock backend, fixtures, assertion helpers. |
| 3 | `atomr-ontology-org` | W3C Org Ontology / schema.org reference vocabulary. |
| 3 | `atomr-ontology-import` | Bulk importers for SKOS, FOAF, schema.org JSON-LD. |
| 3 | `atomr-ontology-viz` | GraphViz DOT + Mermaid renderers for ontology + provenance graphs. |
| 3 | `atomr-ontology-shacl` | Compile `Schema` → SHACL Turtle and parse it back. |
| 3 | `atomr-ontology` | Umbrella facade re-exporting the rest + a `prelude`. Adds optional `HttpDriver` (`http-driver` feature) for OpenAI / Anthropic / LiteLLM. |
| 3 | `atomr-ontology-py` | Python bindings (PyO3 + maturin). 1:1 parity with Rust; `.pyi` stubs ship in the wheel. Async APIs as Python coroutines. |

## Install

**Rust:**

```toml
[dependencies]
atomr-ontology = "0.1"
# Optional features — pull only what you need.
# atomr-ontology = { version = "0.1", features = ["http-driver", "provider-openai"] }
```

**Python** (PyO3 + maturin wheel):

```bash
pip install atomr-ontology                  # core + mock backend
pip install 'atomr-ontology[http-driver]'   # OpenAI / Anthropic / LiteLLM REST
pip install 'atomr-ontology[openai]'        # atomr-infer-driven OpenAI
# Other provider extras: [anthropic] [gemini] [litellm] [vllm]
#                       [candle] [ort] [tensorrt] [mistralrs]
```

A new walkthrough for both languages lives in
[`docs/getting-started.md`](docs/getting-started.md); the Python deep
dive (asyncio, `.pyi` stubs, parity contract) is in
[`docs/python.md`](docs/python.md).

## Getting started in 60 seconds

**Rust:**

```rust
use atomr_ontology::prelude::*;
use atomr_ontology_org::reference_ontology;

let mut o = reference_ontology();
let acme = o.upsert_node(
    Node::from_iri(Iri::from_unchecked("https://example.org/Acme"), "Organization")
        .with_property("name", "Acme Inc."),
);
println!("nodes: {}", o.node_count());
```

**Python:**

```python
import atomr_ontology as ao

o = ao.reference_ontology()
acme = ao.Node.from_iri(ao.Iri.from_unchecked("https://example.org/Acme"), "Organization") \
    .with_property("name", "Acme Inc.")
o.upsert_node(acme)
print(f"nodes: {len(o.nodes)}")
```

For the full 7-stage auto-extract pipeline see
[`examples/auto_extract_from_text/`](examples/auto_extract_from_text/);
the design walkthrough is in [`docs/agents.md`](docs/agents.md).

## Python bindings

The full workspace is exposed to Python through
[`crates/atomr-ontology-py`](crates/atomr-ontology-py/) (PyO3 +
maturin). All async APIs return Python coroutines backed by a shared
tokio runtime, every public Rust type has a `.pyi` stub for
IDE/`pyright` support, and provider integrations are gated behind pip
extras. The full reference is in [`docs/python.md`](docs/python.md);
the crate-level quickstart is at
[`crates/atomr-ontology-py/README.md`](crates/atomr-ontology-py/README.md).

```python
import asyncio, atomr_ontology as ao

async def main():
    store = ao.MemStore.from_ontology(ao.reference_ontology())
    backend = ao.MockBackend.with_label("demo")
    backend.enqueue('[{"surface":"Acme","score":0.9}]')
    terms, _ = await ao.TermExtractor(backend).extract("Acme Inc.")
    print(terms)

asyncio.run(main())
```

## Documentation

**Concepts**
- [`docs/architecture.md`](docs/architecture.md) — tier diagram and
  lifecycle of an auto-built ontology.
- [`docs/data-model.md`](docs/data-model.md) — LPG canonical types and
  the RDF/OWL projection.
- [`docs/naming.md`](docs/naming.md) — vocabulary mapping between LPG,
  RDF/OWL, PROV-O, and schema.org.
- [`docs/agents.md`](docs/agents.md) — the 7-step ontology-learning
  pipeline (ingest → terms → entities → concepts → taxonomy →
  relations → validate-and-commit) + `Backend` trait surface
  (`batch_complete`, `stream_complete`, `CachedBackend`).

**Usage guides**
- [`docs/getting-started.md`](docs/getting-started.md) — install,
  build, and run the org reference pipeline end-to-end.
- [`docs/python.md`](docs/python.md) — Python bindings deep dive.
- [`docs/persistence.md`](docs/persistence.md) — pluggable
  `Checkpointer` providers and `PersistentStore`.
- [`docs/reasoning.md`](docs/reasoning.md) — OWL 2 RL/EL forward
  chaining.
- [`docs/query.md`](docs/query.md) — builder patterns + Cypher / SPARQL
  string DSL.
- [`docs/versioning.md`](docs/versioning.md) — branchable
  time-travelable ontologies.
- [`docs/embeddings.md`](docs/embeddings.md) — vector-similarity
  entity-resolution pre-filter.
- [`docs/remote.md`](docs/remote.md) — HTTP/JSON RPC server and
  client.
- [`docs/viz.md`](docs/viz.md) — GraphViz DOT + Mermaid renderers.
- [`docs/importers.md`](docs/importers.md) — bulk SKOS / FOAF /
  schema.org ingestion.
- [`docs/shacl.md`](docs/shacl.md) — `Schema` ↔ SHACL Turtle.
- [`docs/actor-projection.md`](docs/actor-projection.md) — project
  actor-system persistence data (supervision paths, journal events,
  serialized state) into an `OntologyStore`.

**Operations**
- [`docs/providers.md`](docs/providers.md) — how to point at any
  `atomr-infer` runtime (local Candle, vLLM, ONNX, TensorRT,
  Mistral.rs, or remote OpenAI / Anthropic / Gemini / LiteLLM) or the
  new `http-driver` for direct REST.
- [`docs/migration-0.1-to-0.2.md`](docs/migration-0.1-to-0.2.md) —
  upgrade path from v0.1 to the unreleased surface.

**Historical**
- [`docs/initial-implementation-plan.md`](docs/initial-implementation-plan.md)
  — the design intent committed at repo bootstrap.

## License

Apache-2.0. See [`LICENSE`](LICENSE).
