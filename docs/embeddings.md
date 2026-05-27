# Embeddings

## Purpose

`atomr-ontology-embed` is the vector pre-filter that sits in front of
entity resolution. It turns node labels into fixed-dimension vectors,
stores them in an in-memory index, and answers "given a surface form,
which existing IRIs are most similar?" with cosine-ranked top-k hits.
The downstream `EntityResolver` then hands that short candidate list
to an LLM for disambiguation instead of scanning the entire ontology.

## When to reach for this

- You have an ontology with non-trivial node count (hundreds to
  thousands) and want to narrow the candidate set before invoking the
  LLM during entity resolution.
- You are building a custom extractor that needs cheap nearest-IRI
  lookups by surface text.
- You want a deterministic, offline-only pre-filter for tests
  (`HashEmbedder`) without standing up an embedding service.

## Concepts

| Type | Role |
| --- | --- |
| `EmbeddingBackend` | Pluggable async trait: `embed`, `embed_batch`, `dimensions`, `label`. Implement this against `atomr-infer`, OpenAI, a local Candle model, or any HTTP embedding API. |
| `EmbeddingError` | Two variants: `Transport(String)` for network / IPC failures, `Other(String)` for backend-specific failures. |
| `HashEmbedder` | Deterministic BLAKE3-derived embedder. Stable across runs and platforms; **not** semantically meaningful. Use it for tests and offline development only. |
| `VectorIndex` | Linear-scan cosine-similarity index keyed by IRI. Pins its dimension on the first insert (or via `with_dimensions`). HNSW or another ANN backend is a planned upgrade — see the notes in `index.rs`. |
| `VectorRecord` | `{ iri, vector, meta: serde_json::Value }`. Built via `VectorRecord::new(iri, vec).with_meta(json)`. |
| `EmbeddingResolver` | The canonical wrapper: holds an `Arc<dyn EmbeddingBackend>` plus an `RwLock<VectorIndex>`. Ingest the ontology once; probe with `propose` repeatedly. |

The text used to embed each node, in order of preference:

1. The node's `name` property (when it is a `PropertyValue::String`).
2. The IRI tail (substring after the last `/`, `#`, or `:`).
3. The full IRI.
4. The node id (as a debug string), when nothing else is available.

## Rust example

```rust
use std::sync::Arc;

use atomr_ontology_core::{Iri, Node, Ontology};
use atomr_ontology_embed::{EmbeddingResolver, HashEmbedder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Build an ontology with a few named nodes.
    let mut ontology = Ontology::new();
    ontology.declare_node_type("Organization");
    for (iri, name) in [
        ("https://example.org/Acme", "Acme Corporation"),
        ("https://example.org/Umbrella", "Umbrella Corp"),
        ("https://example.org/Initech", "Initech"),
    ] {
        let node = Node::from_iri(Iri::new(iri)?, "Organization")
            .with_property("name", name);
        ontology.upsert_node(node);
    }

    // 2. Pick a backend. HashEmbedder is fine for tests; for real
    //    similarity you would plug in an EmbeddingBackend impl that
    //    wraps atomr-infer or a remote embedding API.
    let backend = Arc::new(HashEmbedder::new(64));
    let resolver = EmbeddingResolver::new(backend);

    // 3. Ingest the entire ontology — one embed_batch call.
    let inserted = resolver.ingest_ontology(&ontology).await?;
    assert_eq!(inserted, 3);

    // 4. Probe with a surface form. The top hit feeds the LLM-based
    //    EntityResolver as a narrowed candidate set.
    let hits = resolver.propose("Umbrella Corp", 3).await?;
    for (iri, score) in &hits {
        println!("{score:.3}  {iri}");
    }
    Ok(())
}
```

## Python example

```python
import asyncio

from atomr_ontology.core import Iri, Node, Ontology
from atomr_ontology.embed import EmbeddingResolver, HashEmbedder


async def main() -> None:
    # 1. Build an ontology with a few named nodes.
    ontology = Ontology()
    ontology.declare_node_type("Organization")
    for iri, name in [
        ("https://example.org/Acme", "Acme Corporation"),
        ("https://example.org/Umbrella", "Umbrella Corp"),
        ("https://example.org/Initech", "Initech"),
    ]:
        node = Node.from_iri(Iri(iri), "Organization").with_property("name", name)
        ontology.upsert_node(node)

    # 2. Pick a backend. HashEmbedder is for tests/offline use.
    embedder = HashEmbedder(64)
    resolver = EmbeddingResolver(embedder)

    # 3. Ingest the ontology.
    inserted = await resolver.ingest_ontology(ontology)
    assert inserted == 3

    # 4. Probe with a surface form.
    hits = await resolver.propose("Umbrella Corp", 3)
    for iri, score in hits:
        print(f"{score:.3f}  {iri}")


asyncio.run(main())
```

## Pipeline integration

`EntityResolver` (in `atomr-ontology-extract`) currently passes the
ontology context to the LLM as serialized text. Combine it with
`EmbeddingResolver` to make that hint precise: run `propose(surface,
top_k)` for every `TermCandidate`, then construct the LLM prompt with
only those IRIs as legal targets. This drops token cost and prevents
the model from hallucinating IRIs that do not exist.

## Reference

| Path | Contents |
| --- | --- |
| `crates/atomr-ontology-embed/src/lib.rs` | Crate root and public re-exports. |
| `crates/atomr-ontology-embed/src/backend.rs` | `EmbeddingBackend`, `EmbeddingError`, `HashEmbedder`. |
| `crates/atomr-ontology-embed/src/index.rs` | `VectorIndex` (linear-scan), `VectorRecord`, `VectorIndexError`. |
| `crates/atomr-ontology-embed/src/resolver.rs` | `EmbeddingResolver` — `new`, `ingest_ontology`, `propose`. |
| `crates/atomr-ontology-py/src/embed.rs` | PyO3 wrappers. |
| `crates/atomr-ontology-py/python/atomr_ontology/embed.pyi` | Python type stubs. |
| `crates/atomr-ontology-extract/src/entities.rs` | `EntityResolver` — downstream consumer of the proposed candidate set. |

## Cross-links

- [`architecture.md`](architecture.md) — where embedding sits in the
  tiered workspace.
- [`agents.md`](agents.md) — pipeline stages and the `Backend`
  contract that wraps the LLM disambiguator.
- [`providers.md`](providers.md) — how to back `EmbeddingBackend` with
  the same `atomr-infer` runtime used by the extractors.
