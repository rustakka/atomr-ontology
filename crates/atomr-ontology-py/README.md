# atomr-ontology (Python)

Python bindings for the [`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
Rust workspace.

Build, manage, and reason over **labeled property graphs** (LPGs), project them
into **RDF / OWL**, and run a 7-stage **agent-driven ontology-learning
pipeline** — all from Python, backed by the Rust implementation.

## Install

```bash
pip install atomr-ontology
```

To enable a specific inference provider (so extractors can drive a real LLM
instead of the `MockBackend`), install the matching extra. These map 1:1 onto
the upstream `atomr-infer` cargo features:

```bash
pip install atomr-ontology[openai]
pip install atomr-ontology[anthropic]
pip install atomr-ontology[gemini]
pip install atomr-ontology[litellm]
pip install atomr-ontology[vllm]
pip install atomr-ontology[candle]
pip install atomr-ontology[ort]
pip install atomr-ontology[tensorrt]
pip install atomr-ontology[mistralrs]
pip install atomr-ontology[cudarc]
```

## Quickstart

```python
import asyncio
import atomr_ontology as ao

async def main():
    # 1. Seed a store with the W3C Org reference vocabulary.
    store = ao.MemStore.from_ontology(ao.reference_ontology())
    snap = await store.snapshot()
    print(f"seeded {len(snap.schema.node_types)} node types, "
          f"{len(snap.schema.edge_types)} edge types")

    # 2. Build a deterministic test backend (MockBackend replays a queue).
    backend = ao.MockBackend.with_label("demo")
    backend.enqueue('[{"surface":"Acme","score":0.99,"category":"ORG"}]')
    backend.enqueue('[{"surface":"Acme","iri":"https://example.org/Acme",'
                    '"type_name":"Organization","score":0.99,"is_new":true}]')

    # 3. Extract → resolve.
    terms_ex = ao.TermExtractor(backend)
    terms, _ = await terms_ex.extract("Acme Inc. is a corporation.")

    resolver = ao.EntityResolver(backend)
    entities, _ = await resolver.resolve(terms)

    # 4. Commit to the store with provenance.
    nodes = ao.EntityResolver.into_nodes(entities, iri_required=True)
    activity = ao.Activity.started("demo").by(
        ao.AgentRef.software("agent://demo", "demo"),
    )
    prov_id = await store.commit_with_provenance(
        ao.OntologyDelta(nodes=nodes),
        activity,
    )

    # 5. Validate + export.
    snap = await store.snapshot()
    report = ao.run_validate(snap)
    assert report.is_clean()
    print(ao.rdf.turtle_write(snap))

asyncio.run(main())
```

## What's exposed

Every public type from the Rust workspace is wrapped one-for-one. Submodules
follow the Rust crate boundaries:

| Submodule                  | Wraps                              |
| -------------------------- | ---------------------------------- |
| `atomr_ontology.core`      | LPG primitives (Iri, Node, Edge, NodeId, …, Ontology) |
| `atomr_ontology.store`     | `MemStore` (async), patterns, traversal, `OntologyDelta` |
| `atomr_ontology.extract`   | `TermExtractor`, `EntityResolver`, `RelationExtractor`, `RecordExtractor` |
| `atomr_ontology.induce`    | `TaxonomyInducer`, `ConceptFormer`, `AxiomMiner` |
| `atomr_ontology.validate`  | `validate()`, `check_shapes()`, `check_consistency()` |
| `atomr_ontology.provenance`| PROV-O `Activity`, `ProvEntity`, lineage edges, `ProvenanceLog` |
| `atomr_ontology.rdf`       | RDF projection, Turtle / N-Triples / JSON-LD writers |
| `atomr_ontology.org`       | W3C Org Ontology reference vocabulary |
| `atomr_ontology.testkit`   | `MockBackend`, `toy_corpus()`, `toy_org_ontology()` |

The most common types are also re-exported at the package root, so
`from atomr_ontology import Node, Edge, MemStore, …` works.

## Async

`MemStore` and every extractor / inducer call returns a Python coroutine
backed by a single shared tokio runtime. Just `await` them inside an `asyncio`
event loop:

```python
import asyncio, atomr_ontology as ao

async def go():
    store = ao.MemStore()
    nid = await store.upsert_node(ao.Node("Organization"))
    snap = await store.snapshot()
    return snap.node_count()

asyncio.run(go())
```

If you only need to read state synchronously, `MemStore.snapshot_blocking()`
is also available.

## Errors

Every Rust error funnels through a typed Python exception under a common
base, so you can catch broadly or narrowly:

```python
from atomr_ontology import AtomrOntologyError, IriError, StoreError

try:
    ao.Iri("bad iri")
except IriError as e:
    ...
except AtomrOntologyError:
    ...
```

## Building from source

You'll need a Rust toolchain (≥ 1.78) and Python ≥ 3.8.

```bash
pip install maturin
maturin develop --release
pytest tests/ -v
```

To build a wheel with a specific provider enabled, pass the cargo feature
through to maturin:

```bash
maturin build --release --features provider-openai
```

## License

Apache-2.0. See the [workspace LICENSE](../../LICENSE).
