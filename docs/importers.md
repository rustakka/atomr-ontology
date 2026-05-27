# Importers

`atomr-ontology-import` projects a serialized third-party vocabulary
document (SKOS Turtle, FOAF Turtle, schema.org JSON-LD) into the
canonical [LPG model](data-model.md). Every importer returns
`(Ontology, Activity)`: the ontology carries both a T-Box (declared
`NodeType`s, `EdgeType`s, `PropertyType`s from the source vocabulary)
and the A-Box (the individuals materialized from the document); the
`Activity` is a finished PROV-O record so the import can be committed
to a [`ProvenanceLog`](data-model.md#provenance).

The three importers share an implementation: each declares a
per-vocabulary mapping table of recognized classes, datatype
properties, and object properties, and a small generic projector
walks the parsed triple stream. Triples that don't match the
mapping are silently dropped — by design, since SKOS / FOAF /
schema.org sources routinely include adjacencies (`rdfs:label`,
`dct:creator`, ...) that have no place in the projection.

## When to reach for this

- Seeding a new ontology from an existing standard vocabulary.
- Mixing a published thesaurus or directory into an auto-extracted
  ontology before running [`validate`](data-model.md).
- Producing a graph plus its lineage in one call: the returned
  `Activity` is what the store records on commit.

## Error surface

| Variant | When |
| --- | --- |
| `ImportError::Parse(msg)` | the underlying RDF / JSON-LD adapter rejected the input |
| `ImportError::Mapping(msg)` | parsing succeeded but a triple couldn't be projected |
| `ImportError::Adapter(AdapterError)` | propagated from `atomr_ontology_rdf` |

## SKOS

`import_skos(turtle_input: &str) -> Result<(Ontology, Activity), ImportError>`
consumes a Turtle document using the SKOS core vocabulary at
`http://www.w3.org/2004/02/skos/core#`. The returned `Activity` is
labeled `"skos-import"`. Every `Concept` node has its
`skos:prefLabel` value mirrored onto a `name` property so downstream
code reads it through the same key used by FOAF / schema.org.

| Source IRI | LPG mapping | Kind |
| --- | --- | --- |
| `skos:Concept` | `Concept` | `NodeType` |
| `skos:prefLabel` | `prefLabel` (mirrored to `name`) | `PropertyType` (xsd:string) |
| `skos:altLabel` | `altLabel` | `PropertyType` (xsd:string) |
| `skos:definition` | `definition` | `PropertyType` (xsd:string) |
| `skos:broader` | `broader` | `EdgeType` (Concept → Concept) |
| `skos:narrower` | `narrower` | `EdgeType` (Concept → Concept) |
| `skos:related` | `related` | `EdgeType` (Concept → Concept) |

Tiny Turtle in:

```turtle
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex:   <https://example.org/concept/> .

ex:Animal a skos:Concept ; skos:prefLabel "Animal" .
ex:Cat    a skos:Concept ; skos:prefLabel "Cat" ; skos:broader ex:Animal .
```

Ontology shape out:

- `Schema` declares `NodeType{Concept}` with `prefLabel`, `altLabel`,
  `definition`; `EdgeType{broader, narrower, related}`.
- Two nodes (Animal, Cat) with `name=prefLabel`.
- One `broader` edge: Cat → Animal.

## FOAF

`import_foaf(turtle_input: &str) -> Result<(Ontology, Activity), ImportError>`
consumes Turtle using `http://xmlns.com/foaf/0.1/`. Activity label
`"foaf-import"`. Untyped `foaf:knows` targets default to `Person` so
edges still connect even when the document doesn't restate types.

| Source IRI | LPG mapping | Kind |
| --- | --- | --- |
| `foaf:Person` | `Person` | `NodeType` |
| `foaf:Organization` | `Organization` | `NodeType` |
| `foaf:name` | `name` | `PropertyType` (xsd:string) on Person, Organization |
| `foaf:mbox` | `mbox` | `PropertyType` (xsd:anyURI) on Person, Organization |
| `foaf:homepage` | `homepage` | `PropertyType` (xsd:anyURI) on Person, Organization |
| `foaf:knows` | `knows` | `EdgeType` (Person → Person) |
| `foaf:member` | `member` | `EdgeType` (Organization → Person) |

Tiny Turtle in:

```turtle
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix ex:   <https://example.org/> .

ex:Alice a foaf:Person ; foaf:name "Alice" ; foaf:knows ex:Bob .
ex:Bob   a foaf:Person ; foaf:name "Bob" .
```

Ontology shape out: two `Person` nodes, one `knows` edge from Alice
to Bob.

## schema.org

`import_schema_org(jsonld_input: &str) -> Result<(Ontology, Activity), ImportError>`
consumes JSON-LD using `https://schema.org/`. Activity label
`"schema-org-import"`. Untyped object targets default to
`Organization` (the common case for `memberOf` / `worksFor`).

| Source IRI | LPG mapping | Kind |
| --- | --- | --- |
| `schema:Organization` | `Organization` | `NodeType` |
| `schema:Person` | `Person` | `NodeType` |
| `schema:WebSite` | `WebSite` | `NodeType` |
| `schema:Place` | `Place` | `NodeType` |
| `schema:name` | `name` | `PropertyType` (xsd:string) on all four classes |
| `schema:url` | `url` | `PropertyType` (xsd:anyURI) on all four classes |
| `schema:address` | `address` | `PropertyType` (xsd:string) on Organization, Person, Place |
| `schema:memberOf` | `memberOf` | `EdgeType` (Person, Organization → Organization) |
| `schema:worksFor` | `worksFor` | `EdgeType` (Person → Organization) |

Tiny JSON-LD in:

```json
{
  "@context": { "schema": "https://schema.org/" },
  "@graph": [
    { "@id": "https://example.org/Alice", "@type": "schema:Person",
      "schema:name": "Alice",
      "schema:worksFor": { "@id": "https://example.org/Acme" } },
    { "@id": "https://example.org/Acme", "@type": "schema:Organization",
      "schema:name": "Acme Inc." }
  ]
}
```

Ontology shape out: `Person{Alice}`, `Organization{Acme}`, one
`worksFor` edge Alice → Acme.

## Rust example

```rust
use atomr_ontology_import::{import_skos, ImportError};

let ttl = r#"
    @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
    @prefix ex:   <https://example.org/concept/> .
    ex:Animal a skos:Concept ; skos:prefLabel "Animal" .
    ex:Cat    a skos:Concept ; skos:prefLabel "Cat" ; skos:broader ex:Animal .
"#;

let (ontology, activity) = import_skos(ttl)?;
assert_eq!(ontology.node_count(), 2);
assert_eq!(activity.label, "skos-import");
// `activity` is finished; commit it via store.commit_with_provenance.
# Ok::<(), ImportError>(())
```

## Python example

```python
from atomr_ontology import import_

ttl = """
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex:   <https://example.org/concept/> .
ex:Animal a skos:Concept ; skos:prefLabel "Animal" .
ex:Cat    a skos:Concept ; skos:prefLabel "Cat" ; skos:broader ex:Animal .
"""

ontology, activity = import_.import_skos(ttl)
assert len(ontology.nodes) == 2
assert activity.label == "skos-import"
```

## Reference

| File | Purpose |
| --- | --- |
| `crates/atomr-ontology-import/src/lib.rs` | crate root, re-exports |
| `crates/atomr-ontology-import/src/error.rs` | `ImportError` |
| `crates/atomr-ontology-import/src/mapping.rs` | generic projector (`declare`, `project`) |
| `crates/atomr-ontology-import/src/skos.rs` | SKOS mapping + `import_skos` |
| `crates/atomr-ontology-import/src/foaf.rs` | FOAF mapping + `import_foaf` |
| `crates/atomr-ontology-import/src/schema_org.rs` | schema.org mapping + `import_schema_org` |
| `crates/atomr-ontology-py/src/import_.rs` | PyO3 wrapper |
| `crates/atomr-ontology-py/python/atomr_ontology/import_.pyi` | Python stubs |

## See also

- [`naming.md`](naming.md) — the canonical LPG ↔ RDF/OWL mapping
  table; the per-importer tables above are the same kind of mapping
  applied to one source vocabulary at a time.
- [`data-model.md`](data-model.md#provenance) — what the returned
  `Activity` plugs into.
- [`architecture.md`](architecture.md) — where importers sit in the
  tiered workspace (Tier 2 ingestion side).
