# atomr-ontology-remote

HTTP/JSON RPC `OntologyStore` server and client for the
[`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
workspace.

## Features

- `client` (default) — `RemoteClient` (uses `reqwest`).
- `server` (default) — `serve` + `ServerHandle` (hand-rolled
  `tokio::net::TcpListener`-based HTTP/1.1).

Build with `--no-default-features --features client` (or
`server`) to slim either side independently.

## Example

```rust
use std::sync::Arc;
use atomr_ontology_remote::{serve, RemoteClient};
use atomr_ontology_store::MemStore;
use atomr_ontology_store::OntologyStore;

let store = Arc::new(MemStore::new()) as Arc<dyn OntologyStore>;
let handle = serve("127.0.0.1:0".parse()?, store).await?;
let client = RemoteClient::new(format!("http://{}", handle.local_addr()))?;
// client now satisfies OntologyStore and dispatches over HTTP.
handle.shutdown().await?;
```

## Full guide

[`docs/remote.md`](../../docs/remote.md).
