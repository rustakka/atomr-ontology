# atomr-ontology-version

Git-style branchable, time-travelable ontologies for the
[`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
workspace. Each commit is content-addressed via blake3; branches
are named refs; merges resolve via configurable strategy.

## Features

None.

## Example

```rust
use atomr_ontology_version::{MergeStrategy, VersionedStore};

let mut repo = VersionedStore::init();
let c0 = repo.commit("seed".into(), "alice".into(), ontology.clone());
repo.branch("feature".into())?;
repo.checkout("feature")?;
let c1 = repo.commit("add Acme".into(), "alice".into(), ontology2);
repo.checkout("main")?;
let merge_id = repo.merge("feature", MergeStrategy::Union)?;

let snap = repo.as_of(c0).unwrap();  // time-travel
```

`MergeStrategy::{Ours, Theirs, Union}` mirrors the conventional
three-way semantics.

## Full guide

[`docs/versioning.md`](../../docs/versioning.md).
