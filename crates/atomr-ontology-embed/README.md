# atomr-ontology-embed

Vector-embedding + similarity-search layer for entity resolution
in the [`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
workspace.

## Features

None. The crate ships a deterministic `HashEmbedder` for tests; a
real embedding provider plugs in by implementing
`EmbeddingBackend`. The canonical wiring mirrors the rest of the
workspace: wrap an `atomr_infer::Provider` for embeddings rather than
hand-rolling a REST client. See
[`docs/providers.md`](../../docs/providers.md#provider-selection) for
the full layering and decision tree.

## Example

```rust
use std::sync::Arc;
use atomr_ontology_embed::{EmbeddingResolver, HashEmbedder};

let embedder = Arc::new(HashEmbedder::new(64));
let resolver = EmbeddingResolver::new(embedder);
resolver.ingest_ontology(&ontology).await?;
let candidates = resolver.propose("Acme", 5).await?;
```

`VectorIndex` is a linear-scan implementation; HNSW or another
ANN backend is a planned upgrade.

## Full guide

[`docs/embeddings.md`](../../docs/embeddings.md).
