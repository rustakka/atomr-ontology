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
| 1 | `atomr-ontology-rdf` | RDF/OWL adapter (`Class`, `Individual`, `Triple`); Turtle / N-Triples / JSON-LD I/O behind features. |
| 1 | `atomr-ontology-provenance` | PROV-O-aligned types (`Activity`, `ProvAgent`, `ProvEntity`, lineage edges). |
| 2 | `atomr-ontology-store` | `OntologyStore` trait + in-memory implementation, pattern matching, traversal. |
| 2 | `atomr-ontology-extract` | Term / entity / relation / record extractors as composable backend-driven units. |
| 2 | `atomr-ontology-induce` | Taxonomy induction, concept formation, axiom mining. |
| 2 | `atomr-ontology-validate` | SHACL-style shape validation and axiom-consistency checks. |
| 3 | `atomr-ontology-testkit` | Mock backend, fixtures, assertion helpers. |
| 3 | `atomr-ontology-org` | W3C Org Ontology / schema.org reference vocabulary. |
| 3 | `atomr-ontology` | Umbrella facade re-exporting the rest + a `prelude`. |
| 3 | `atomr-ontology-py` | Python bindings (PyO3 + maturin). Async OntologyStore + extractors as Python coroutines. |

## Quick start

```rust
use atomr_ontology::prelude::*;

let mut ontology = Ontology::new("https://example.org/ontology/v1");
let org_class = ontology.declare_node_type("Organization");
let acme = ontology.upsert_node(Node::new(&org_class).with_property("name", "Acme Inc."));
```

For an end-to-end auto-ontology pipeline, see
[`examples/auto_extract_from_text/`](examples/auto_extract_from_text/)
and the design notes in [`docs/agents.md`](docs/agents.md).

## Python bindings

The full workspace is exposed to Python through
[`crates/atomr-ontology-py`](crates/atomr-ontology-py/) (PyO3 +
maturin). All async APIs return Python coroutines backed by a shared
tokio runtime; provider integrations are gated behind pip extras such
as `atomr-ontology[openai]`, `[anthropic]`, `[vllm]`, etc. See that
crate's [README](crates/atomr-ontology-py/README.md) for a quickstart.

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

- [`docs/architecture.md`](docs/architecture.md) — tier diagram and
  lifecycle of an auto-built ontology.
- [`docs/data-model.md`](docs/data-model.md) — LPG canonical types and
  the RDF/OWL projection.
- [`docs/agents.md`](docs/agents.md) — the 7-step ontology-learning
  pipeline (ingest → terms → entities → concepts → taxonomy →
  relations → validate-and-commit).
- [`docs/providers.md`](docs/providers.md) — how to point at any
  `atomr-infer` runtime (local Candle, vLLM, ONNX, TensorRT, Mistral.rs,
  or remote OpenAI / Anthropic / Gemini / LiteLLM).
- [`docs/naming.md`](docs/naming.md) — vocabulary mapping between LPG,
  RDF/OWL, PROV-O, and schema.org.
- [`docs/initial-implementation-plan.md`](docs/initial-implementation-plan.md)
  — the design intent committed at repo bootstrap (historical).

## License

Apache-2.0. See [`LICENSE`](LICENSE).
