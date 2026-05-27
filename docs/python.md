# Python bindings

`atomr-ontology` ships full Python bindings via PyO3 + maturin.
Every public Rust type has a Python wrapper, every async Rust
method returns a Python coroutine, and every submodule has a
`.pyi` type stub for `pyright` / IDE autocomplete.

The bindings live in
[`crates/atomr-ontology-py`](../crates/atomr-ontology-py); this
guide covers the Python-side ergonomics, the async story, and the
parity contract.

## Installation

```bash
pip install atomr-ontology                  # core + mock backend
pip install 'atomr-ontology[http-driver]'   # OpenAI / Anthropic / LiteLLM REST
pip install 'atomr-ontology[anthropic]'     # atomr-infer-backed Anthropic
```

Provider extras: `[openai]`, `[anthropic]`, `[gemini]`,
`[litellm]`, `[vllm]`, `[candle]`, `[ort]`, `[tensorrt]`,
`[mistralrs]`, `[http-driver]`. Each maps 1:1 to the matching
Cargo feature on the Rust umbrella.

Local development:

```bash
cd crates/atomr-ontology-py
maturin develop --features http-driver
```

## Package layout

```
atomr_ontology
├── core         # Iri, Node, Edge, NodeId, Schema, Ontology, …
├── provenance   # Activity, ProvenanceLog, AgentRef
├── store        # MemStore, NodePattern, EdgePattern, TraversalPlan
├── extract      # Backend, TermExtractor, EntityResolver, RelationExtractor
├── induce       # TaxonomyInducer, ConceptFormer, AxiomMiner
├── validate     # validate(), Severity, ValidationReport
├── rdf          # Subject, Object, Triple, turtle/ntriples/jsonld I/O
├── org          # reference_ontology(), namespace constants
├── testkit      # MockBackend, toy_corpus, toy_org_ontology
│
├── persist      # MemCheckpointer, FileCheckpointer, PersistentStore
├── reason       # Reasoner, RuleSet
├── embed        # HashEmbedder, VectorIndex, EmbeddingResolver
├── version      # VersionedStore, CommitId, MergeStrategy
├── query        # parse_cypher, parse_sparql
├── remote       # RemoteClient
├── viz          # render_ontology_dot, render_ontology_mermaid
├── import_      # import_skos, import_foaf, import_schema_org
├── shacl        # to_shacl_turtle, from_shacl_turtle
│
├── http_driver  # HttpDriver (extra: [http-driver])
└── infer        # InferBackend     (extras: [openai], [anthropic], …)
```

The most common types are re-exported at the top level so plain
`atomr_ontology.X` works:

```python
import atomr_ontology as ao

ao.Iri, ao.Node, ao.Edge, ao.Ontology, ao.MemStore, ao.MockBackend, ...
```

## Async / asyncio

Every async Rust method returns a Python `Awaitable` (a wrapped
tokio future). Use `await` inside an `asyncio` coroutine. The
shared tokio runtime is initialized lazily on first use.

```python
import asyncio, atomr_ontology as ao

async def main():
    store = ao.MemStore.from_ontology(ao.reference_ontology())
    acme = ao.Node.from_iri(
        ao.Iri.from_unchecked("https://example.org/Acme"), "Organization"
    ).with_property("name", "Acme")
    node_id = await store.upsert_node(acme)
    snap = await store.snapshot()
    print(f"node {node_id} in store; total = {len(snap.nodes)}")

asyncio.run(main())
```

The same pattern applies to extractors, inducers, the persistent
store, and the embedding resolver.

## Type stubs

Every submodule ships a `.pyi` file alongside the compiled
extension. `pyright`, `mypy`, and PyCharm pick them up
automatically when `atomr-ontology` is installed in the same
environment:

```bash
$ pyright examples/foo.py
0 errors, 0 warnings, 0 informations
```

Stubs cover constructors, properties, and method signatures. They
do not exhaustively annotate every kwarg default — the runtime
implementation is the source of truth — but every public name has
at minimum a signature so IDE completion works.

## Parity contract

Each Rust public type and free function has a Python counterpart
with the same semantics. Async methods return `Awaitable` rather
than `Future` to keep the surface idiomatic. There is no Python
re-implementation of any hot path: every call dispatches through
the Rust trait object.

The contract has two deliberate carve-outs:

1. **Trait objects** (`Backend`, `OntologyStore`, `Checkpointer`)
   are not directly subclassable from Python. Python sees the
   concrete handles — `MockBackend`, `HttpDriver`, `InferBackend`,
   `MemStore`, `PersistentStore`, `RemoteClient` — and operates
   on them. To plug a custom backend, write a Rust impl and
   expose it through your own PyO3 wrapper, then hand it in via
   the `Arc<dyn Backend>` boundary.
2. **Internal trait composition** types (`Callable`, `Pipeline`)
   stay Rust-only. Python users compose pipelines by chaining
   awaited calls in `async` functions.

Everything else — types, methods, modules, error variants — is
1:1 with the Rust API.

## Backends and providers

```python
import atomr_ontology as ao

mock = ao.MockBackend.with_label("demo")
mock.enqueue('[{"surface":"Acme","score":0.9}]')

# HTTP driver (with http-driver feature):
from atomr_ontology.http_driver import HttpDriver
openai = HttpDriver("openai", "gpt-4o-mini").as_backend()
```

Reads `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `LITELLM_API_KEY`
from the environment by default. See [`providers.md`](providers.md)
for the full provider matrix.

Backends carry the same `batch_complete`, `complete`,
`with_lru_cache`, `with_content_cache` surface as Rust:

```python
cached = openai.with_lru_cache(256)
text = await cached.complete(ao.Prompt.user("Hello").with_max_tokens(64))
```

## Error mapping

Each Rust error enum maps to a Python exception class registered
at the package root:

| Rust | Python |
| --- | --- |
| `IriError` | `atomr_ontology.IriError` |
| `OntologyError` | `atomr_ontology.OntologyError` |
| `StoreError` | `atomr_ontology.StoreError` |
| `BackendError` | `atomr_ontology.BackendError` |
| `AdapterError` | `atomr_ontology.AdapterError` |
| `ValidationError` | `atomr_ontology.ValidationError` |

All inherit from `atomr_ontology.AtomrOntologyError` so a single
`except` clause catches the lot.

## Where to next

- [`getting-started.md`](getting-started.md) — minimal pipeline.
- [`agents.md`](agents.md) — the 7-stage pipeline.
- [`providers.md`](providers.md) — wiring real LLM backends.
- Each topic guide (persistence, reasoning, query, …) carries a
  Python example mirroring the Rust one.
