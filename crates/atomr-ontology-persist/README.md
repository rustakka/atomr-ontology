# atomr-ontology-persist

Pluggable persistent `OntologyStore` for the
[`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
workspace.

## Features

- `file` (off by default) — JSON file `Checkpointer` via `tokio::fs`.
- `sqlite` (off by default) — SQLite `Checkpointer` via bundled `rusqlite`.

Default build ships only `MemCheckpointer`.

## Example

```rust
use atomr_ontology_persist::{FileCheckpointer, PersistentStore};
use atomr_ontology_store::{OntologyDelta, OntologyStore};
use atomr_ontology_core::Node;
use atomr_ontology_provenance::Activity;

let checkpointer = FileCheckpointer::new("ontology.json");
let store = PersistentStore::new(checkpointer).await?;
let delta = OntologyDelta::new()
    .with_node(Node::new("Organization").with_property("name", "Acme"));
store.commit_with_provenance(delta, Activity::started("seed")).await?;
```

Reload by constructing a fresh `PersistentStore` over the same
checkpointer path; the previous state is restored automatically.

## Full guide

[`docs/persistence.md`](../../docs/persistence.md) in the workspace
root.
