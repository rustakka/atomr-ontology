# atomr-ontology-query

Hand-rolled Cypher / SPARQL subset parsers compiling to
[`atomr_ontology_store::TraversalPlan`].

## Features

None.

## Example

```rust
use atomr_ontology_query::{parse_cypher, parse_sparql};

let plan = parse_cypher(
    "MATCH (a:Org)-[:subClassOf*1..3]->(b) RETURN b LIMIT 10"
)?;

let plan2 = parse_sparql(r#"
    PREFIX : <https://example.org/>
    SELECT ?b WHERE { ?a a :Org . ?a :subClassOf ?b . } LIMIT 10
"#)?;
```

Both parsers produce `TraversalPlan` IR that
`atomr_ontology_store::OntologyStore::traverse` executes
unchanged.

Supported Cypher subset: `MATCH`, multi-label, inline property
maps, `*min..max` variable-length, `WHERE NOT`, `RETURN`, `LIMIT`.

Supported SPARQL subset: `PREFIX`, `SELECT`, `WHERE { triples }`
basic graph patterns, `a` shorthand for `rdf:type`, `LIMIT`,
`OFFSET`.

## Full guide

[`docs/query.md`](../../docs/query.md).
