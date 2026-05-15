# Data model

## Labeled property graph (canonical)

```
Ontology
├── iri:        Iri               (optional canonical IRI)
├── vocabulary: Vocabulary        (prefix → base IRI bindings)
├── schema:     Schema
│   ├── node_types: BTreeMap<String, NodeType>
│   └── edge_types: BTreeMap<String, EdgeType>
├── nodes:  BTreeMap<NodeId, Node>
├── edges:  BTreeMap<EdgeId, Edge>
└── axioms: BTreeMap<AxiomId, Axiom>
```

### Identifiers

`NodeId`, `EdgeId`, `RecordId`, `ProvenanceId` are 32-byte newtypes
backed by Blake3. They display as lowercase hex and round-trip
through `FromStr`. Construct one of three ways:

| Constructor | Behavior |
| --- | --- |
| `NodeId::new_random()` | UUID-seeded, content-addressed via Blake3 |
| `NodeId::content_address(bytes)` | Deterministic over `bytes` |
| `NodeId::from_bytes(arr)` | Wrap a raw `[u8;32]` (no validation) |

`Edge::between(source, label, target)` is content-addressed over
`(source ‖ 0 ‖ label ‖ 0 ‖ target)` so identical edges deduplicate
on insert.

### Schema

- **`NodeType`** — the LPG analogue of an OWL `Class`. Carries
  `name`, optional canonical `iri`, `supertypes`, and a list of
  declared `PropertyType`s.
- **`EdgeType`** — the LPG analogue of an OWL `ObjectProperty`.
  Carries `name`, `iri`, `domain`/`range` type names,
  `properties`, a `Cardinality` bound, a `functional` flag,
  and an optional `inverse_of` label.
- **`PropertyType`** — declared property on a node or edge:
  `name`, `datatype` (`Datatype`), `cardinality`, optional
  canonical IRI, optional description.
- **`Cardinality`** — `{ min: u32, max: Option<u32> }` with the
  constants `ANY`, `OPTIONAL`, `ONE`, `AT_LEAST_ONE`.

### Property values

`PropertyValue` is a tagged enum covering the JSON/RDF cross-
section:

| Variant | RDF/XSD analogue |
| --- | --- |
| `String(s)` | `xsd:string` |
| `Integer(i)` | `xsd:integer` |
| `Float(f)` | `xsd:double` |
| `Bool(b)` | `xsd:boolean` |
| `DateTime(d)` | `xsd:dateTime` |
| `Iri(i)` | `xsd:anyURI` |
| `Bytes(b)` | `xsd:base64Binary` |
| `Json(v)` | `rdf:JSON` |
| `Null` | (property absent) |

### Axioms

`AxiomKind` covers the RDFS / OWL constructs the validator can
actually enforce: `SubClassOf`, `EquivalentClass`, `DisjointWith`,
`Domain`, `Range`, `Functional`, `InverseFunctional`, `InverseOf`,
`Symmetric`, `Transitive`. Each axiom carries an `AxiomId`
derived deterministically from its body so duplicates deduplicate.

## RDF/OWL projection (adapter)

`atomr-ontology-rdf` provides two functions:

```rust
fn to_rdf(ontology: &Ontology) -> Vec<Triple>;
fn from_rdf(triples: &[Triple]) -> Result<Ontology, AdapterError>;
```

The forward direction is total; the reverse is partial (T-Box
plus instance assertions whose subject is an IRI). Writers for
Turtle, N-Triples, and JSON-LD ship behind cargo features.

The detailed mapping table — including what gets dropped — lives
in [`naming.md`](naming.md).

## Provenance

`atomr-ontology-provenance` carries the PROV-O surface:

- **`Activity`** — a span over which an agent did something. Carries
  `started_at`, `ended_at`, an optional `AgentRef`, and a free-form
  `attributes` map.
- **`AgentRef`** (alias `ProvAgent`) — software / person /
  organization label.
- **`ProvEntity`** — a snapshot of data that participated in an
  activity.
- Lineage edges: `WasGeneratedBy`, `WasDerivedFrom`,
  `WasAttributedTo`, `Used`.

`ProvenanceLog` aggregates the above and is owned by the
`OntologyStore`. Every commit captures one activity and writes
its id back onto any axiom that lacked a `provenance` annotation.
