# atomr-ontology-shacl

Compile `Schema` to SHACL Turtle and parse SHACL Turtle back into
`Schema`.

## Features

None. Depends on `atomr-ontology-rdf` (turtle feature).

## Example

```rust
use atomr_ontology_shacl::{to_shacl_turtle, from_shacl_turtle};

let ttl = to_shacl_turtle(&ontology.schema)?;
let schema_back = from_shacl_turtle(&ttl)?;
```

Round-trip preserves `NodeType` targetClass, `PropertyType`
cardinality, datatype, and IRI. Axiom-level constraints
(`Domain`/`Range`/`DisjointWith`) are not yet reflected on the
SHACL side.

## Full guide

[`docs/shacl.md`](../../docs/shacl.md).
