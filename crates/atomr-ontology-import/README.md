# atomr-ontology-import

Bulk importers for SKOS, FOAF, and schema.org JSON-LD into the
[`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
canonical LPG model.

## Features

None.

## Example

```rust
use atomr_ontology_import::{import_skos, import_foaf, import_schema_org};

let (ontology, activity) = import_skos(turtle_input)?;
let (ontology, activity) = import_foaf(turtle_input)?;
let (ontology, activity) = import_schema_org(jsonld_input)?;
```

Each importer returns `(Ontology, Activity)` so the PROV-O
lineage of the import is captured.

## Full guide

[`docs/importers.md`](../../docs/importers.md) with mapping tables
for each vocabulary.
