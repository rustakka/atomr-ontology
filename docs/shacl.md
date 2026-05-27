# SHACL

`atomr-ontology-shacl` round-trips a [`Schema`](data-model.md#schema)
between the canonical LPG T-Box and a SHACL Turtle document.
The compiler emits `sh:NodeShape`s with `sh:targetClass`,
per-property `sh:property` blocks, cardinality bounds, and `xsd:*`
datatype constraints; the parser consumes the same subset of SHACL
back into a `Schema`. The parser is forgiving: anything beyond the
emitted subset is silently ignored so externally-authored shapes can
still drive the validate pipeline.

## When to reach for this

- Exporting a built ontology's schema as portable SHACL for other
  tooling.
- Importing externally-authored SHACL shapes so
  [`atomr_ontology_validate`](architecture.md) can use them.
- Reviewing the constraints induced by `AxiomMiner` in a standard
  format.

## Concepts

The round-trip is `Schema → Turtle → Schema`, **not**
`Ontology → Turtle → Ontology`. Only the T-Box (node types,
property types, edge types) participates. Instance data, axioms,
and provenance are out of scope.

What survives the round-trip:

| Surface | Preserved? |
| --- | --- |
| `NodeType.iri`, `NodeType.name` | yes (via `sh:targetClass`) |
| `PropertyType.iri`, `name` | yes (via `sh:path`; name derived from IRI local segment) |
| `PropertyType.datatype` | yes (via `sh:datatype xsd:*`) |
| `PropertyType.cardinality` | yes (via `sh:minCount` / `sh:maxCount`) |
| `EdgeType.cardinality`, `nodeKind`, target classes | emitted (via `sh:nodeKind sh:IRI` + one `sh:class` per range) |
| `EdgeType` parsed back as `EdgeType` | **no** — edge blocks lack `sh:datatype` and are skipped on parse |
| `Axiom::Domain` / `Range` / `DisjointWith` / `SubClassOf` | **no** — axiom-level constraints need separate handling |

## Datatype mapping

| `Datatype` | `xsd:*` IRI |
| --- | --- |
| `String` | `xsd:string` |
| `Integer` | `xsd:integer` |
| `Float` | `xsd:double` |
| `Bool` | `xsd:boolean` |
| `DateTime` | `xsd:dateTime` |
| `Iri` | `xsd:anyURI` |
| `Bytes` | `xsd:base64Binary` |
| `Json` | `xsd:string` (lossy: lexical form only) |

On parse, the inverse is applied; common synonyms (`xsd:int`,
`xsd:long`, `xsd:date`, `xsd:hexBinary`, ...) fold to the nearest
canonical `Datatype`.

## Error surface

| Error | When |
| --- | --- |
| `ShaclCompileError::MissingIri(name)` | a schema type required an IRI that wasn't set |
| `ShaclCompileError::Other(msg)` | catch-all for compile failures |
| `ShaclParseError::Adapter(AdapterError)` | the Turtle parser rejected the input |
| `ShaclParseError::MissingRequired(term)` | a NodeShape lacked `sh:targetClass` |
| `ShaclParseError::Other(msg)` | catch-all for parse failures |

## Rust example

The canonical round-trip pattern, from
`crates/atomr-ontology-shacl/tests/round_trip.rs`:

```rust
use atomr_ontology_core::{
    schema::{Cardinality, NodeType, PropertyType},
    Datatype, Iri, Schema,
};
use atomr_ontology_shacl::{from_shacl_turtle, to_shacl_turtle};

let mut original = Schema::new();
original.declare_node_type(
    NodeType::new("Organization")
        .with_iri(Iri::from_unchecked("http://example.org/Organization"))
        .with_property(PropertyType {
            name: "name".into(),
            datatype: Datatype::String,
            cardinality: Cardinality::ONE,
            iri: Some(Iri::from_unchecked("http://example.org/name")),
            description: None,
        }),
);

let ttl = to_shacl_turtle(&original)?;
let parsed = from_shacl_turtle(&ttl)?;

let ty = parsed.node_type("Organization").unwrap();
assert_eq!(ty.properties[0].datatype, Datatype::String);
assert_eq!(ty.properties[0].cardinality, Cardinality::ONE);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Python example

```python
from atomr_ontology import Cardinality, Datatype, Iri, NodeType, PropertyType, Schema
from atomr_ontology import shacl

original = Schema()
original.declare_node_type(
    NodeType("Organization")
        .with_iri(Iri.from_unchecked("http://example.org/Organization"))
        .with_property(PropertyType(
            name="name",
            datatype=Datatype.string(),
            cardinality=Cardinality.one(),
            iri=Iri.from_unchecked("http://example.org/name"),
        )),
)

ttl = shacl.to_shacl_turtle(original)
parsed = shacl.from_shacl_turtle(ttl)
assert parsed.node_type("Organization").properties[0].datatype.name == "String"
```

## Feeding external SHACL into validate

`atomr_ontology_validate::validate` takes an `Ontology`, not a bare
`Schema` — the shape checks run against the schema embedded in the
ontology. To use externally-authored SHACL, parse it and graft the
resulting schema onto a fresh `Ontology`:

```rust
use atomr_ontology_core::Ontology;
use atomr_ontology_shacl::from_shacl_turtle;
use atomr_ontology_validate::validate;

let external_ttl = std::fs::read_to_string("shapes.ttl")?;
let schema = from_shacl_turtle(&external_ttl)?;

let mut ontology = Ontology::new();
ontology.schema = schema;
// ... populate `ontology.nodes` / `ontology.edges` with the data
// you want to validate against the imported shapes ...

let report = validate(&ontology);
assert!(report.is_clean());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Reference

| File | Purpose |
| --- | --- |
| `crates/atomr-ontology-shacl/src/lib.rs` | crate root + re-exports |
| `crates/atomr-ontology-shacl/src/ns.rs` | SHACL / XSD namespace constants |
| `crates/atomr-ontology-shacl/src/compile.rs` | `to_shacl_turtle` + `ShaclCompileError` |
| `crates/atomr-ontology-shacl/src/parse.rs` | `from_shacl_turtle` + `ShaclParseError` |
| `crates/atomr-ontology-shacl/tests/round_trip.rs` | canonical round-trip example |
| `crates/atomr-ontology-py/src/shacl.rs` | PyO3 wrapper |
| `crates/atomr-ontology-py/python/atomr_ontology/shacl.pyi` | Python stubs |

## See also

- [`data-model.md`](data-model.md#schema) — what a `Schema` looks
  like before compilation.
- [`naming.md`](naming.md) — `Datatype` ↔ `xsd:*` mapping shared with
  the RDF projection.
- [`architecture.md`](architecture.md) — where SHACL sits relative to
  `atomr-ontology-validate`.
