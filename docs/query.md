# Query

Querying the ontology happens at two layers. The **builder API** in
`atomr-ontology-store` constructs `TraversalPlan` values directly
from typed Rust / Python — it is the canonical, programmatic
surface. The **string DSL** in `atomr-ontology-query` parses two
small openCypher and SPARQL subsets into the same `TraversalPlan`,
which is useful when queries arrive as user input or in
configuration. Both layers feed `OntologyStore::traverse`, which on
the in-memory backend (`MemStore`) performs cycle-safe pattern
matching with variable-length expansion.

## When to reach for this

- **Builder API:** anywhere a query is composed in code — pipeline
  stages, library helpers, validation predicates, tests. It is
  type-checked, has no parse failure mode, and supports every
  feature the executor implements (OR / NOT branches, variable
  length, projection, order, skip, limit).
- **String DSL:** when queries are user-authored (notebook input,
  config files) and the supported subset is large enough. Both
  parsers compile to `TraversalPlan`, so once parsed the plan
  behaves identically to a hand-built one.

## Concepts

- `NodePattern` — matches a single node by `types`, `properties`,
  pinned `id`, and optional disjunctive `or` / negative `not`
  sub-patterns. The base predicate is `AND`-joined; if any
  alternatives are present the candidate must additionally satisfy
  at least one. Any `not` branch that matches excludes the candidate.
- `EdgePattern` — matches an edge by `label`, exact `properties`,
  optional `bind` name, and a `repeat: RangeInclusive<usize>` for
  variable-length paths.
- `TraversalStep` — one hop: `(edge_pattern, target_pattern,
  outbound: bool)`. `TraversalStep::outbound` and `::inbound` are
  the constructors.
- `TraversalPlan` — `seed` `NodePattern` plus a vector of
  `TraversalStep`s, followed by post-processing knobs:
  `return_(columns)`, `order_by(name)` / `order_by_desc(name)`,
  `skip(n)`, `limit(n)`.
- `MatchRow` — result row: variable name → `NodeId` map and
  variable name → `EdgeId` map. `OntologyStore::traverse` returns
  `Vec<MatchRow>`.
- `SortOrder` — `Ascending` / `Descending`, used by `order` keys.

### `MemStore::traverse` semantics

The in-memory executor expands the seed pattern against every node,
then for each step walks all matching outbound (or inbound) edges.
Variable-length edges (`EdgePattern::repeat(min..=max)`) expand BFS
from the current frontier:

- At each depth in `[min, max]`, candidates whose terminal node
  matches `step.target` are emitted.
- Already-visited node ids on the current expansion path are
  skipped — cycles do not produce infinite results.
- A row's bindings carry through expansion, so `(start)-[*1..3]->(end)`
  binds `start` once and `end` per terminal node.

After hop expansion, `apply_ordering_and_projection` sorts by the
configured `order` keys, drops `skip` rows, truncates to `limit`,
and finally strips bindings not listed in `return_columns` (empty
columns means "return everything").

## Builder API — Rust example

```rust
use atomr_ontology::core::{Edge, Node};
use atomr_ontology::store::{
    EdgePattern, MemStore, NodePattern, OntologyStore, TraversalPlan,
};
use atomr_ontology_testkit::fixtures::toy_org_ontology;

#[tokio::main]
async fn main() {
let store = MemStore::from_ontology(toy_org_ontology());

// Add a subClassOf chain to demonstrate variable-length paths.
let a = store.upsert_node(Node::new("Class").with_property("name", "A")).await.unwrap();
let b = store.upsert_node(Node::new("Class").with_property("name", "B")).await.unwrap();
let c = store.upsert_node(Node::new("Class").with_property("name", "C")).await.unwrap();
store.upsert_edge(Edge::between(a, "subClassOf", b)).await.unwrap();
store.upsert_edge(Edge::between(b, "subClassOf", c)).await.unwrap();

// Plan: from a Class named "A", follow 1..=3 subClassOf hops to a
// target that is NOT named "B". Project only the start/end nodes,
// order by `end`, and cap to 5 rows.
let plan = TraversalPlan::from(
        NodePattern::any()
            .bind("start")
            .typed("Class")
            .with_property("name", "A"),
    )
    .outbound(
        EdgePattern::any().labeled("subClassOf").repeat(1..=3),
        NodePattern::any()
            .bind("end")
            .typed("Class")
            .not(NodePattern::any().with_property("name", "B")),
    )
    .return_(["start", "end"])
    .order_by("end")
    .skip(0)
    .limit(5);

let rows = store.traverse(&plan).await.unwrap();
for row in &rows {
    let end = row.nodes.get("end").copied().unwrap();
    assert!(end == c);
}
}
```

## Builder API — Python example

```python
import asyncio
from atomr_ontology.core import Edge, Node
from atomr_ontology.store import (
    EdgePattern, MemStore, NodePattern, TraversalPlan,
)
from atomr_ontology.testkit import toy_org_ontology

async def main():
    store = MemStore.from_ontology(toy_org_ontology())
    a = await store.upsert_node(Node("Class").with_property("name", "A"))
    b = await store.upsert_node(Node("Class").with_property("name", "B"))
    c = await store.upsert_node(Node("Class").with_property("name", "C"))
    await store.upsert_edge(Edge.between(a, "subClassOf", b))
    await store.upsert_edge(Edge.between(b, "subClassOf", c))

    plan = (
        TraversalPlan(NodePattern.any()
                      .bind("start").typed("Class")
                      .with_property("name", "A"))
        .outbound(
            EdgePattern.any().labeled("subClassOf").repeat(1, 3),
            NodePattern.any().bind("end").typed("Class")
                .not_(NodePattern.any().with_property("name", "B")),
        )
        .return_(["start", "end"])
        .order_by("end")
        .limit(5)
    )
    rows = await store.traverse(plan)
    for row in rows:
        assert "end" in row.nodes

asyncio.run(main())
```

## String DSL — `parse_cypher` and `parse_sparql`

Both parsers in `atomr-ontology-query` are hand-rolled
recursive-descent over deliberately narrow grammars; both compile
to `TraversalPlan`. Errors come back as
`CypherError` / `SparqlError`, each of which distinguishes
`Parse(...)` (bad syntax) from `Unsupported(...)` (recognized but
outside the subset).

### Cypher subset

Supported:

- `MATCH` followed by a single path: `(n:Label {k: v})` nodes
  joined by `-[:label]->`, `<-[:label]-`, or `-[:label]-` edges.
- Multiple `:Label`s per node, property maps with `string` /
  `integer` / `bool` values, optional edge binding `[e:label]`.
- Variable-length edges: `-[:label*min..max]->`, `*min..`, `*min`, `*`.
- Optional `WHERE NOT (n)` or `WHERE NOT n:Label` — attached as a
  `NodePattern::not` on the matching binding.
- `RETURN <ident> (, <ident>)*` — required, projects bindings.
- Optional trailing `LIMIT <int>`.

Not supported: comparison predicates (`a.foo = 1`), `OPTIONAL MATCH`,
`UNION`, `WITH`, `ORDER BY`, multi-path `MATCH`, parameterized
queries. These return `CypherError::Unsupported`.

### SPARQL subset

Supported:

- Optional `PREFIX name: <iri>` declarations (any number).
- `SELECT ?a ?b ...` or `SELECT *`. The first triple's subject
  must be a variable and becomes the seed binding.
- `WHERE { ?s p ?o . ?s p ?o }` with optional trailing `.`.
  Predicates may be the keyword `a` (compiled as a type
  constraint), an absolute `<iri>`, or a `prefix:local` CURIE
  (collapsed to its local name when no matching prefix is
  declared).
- Optional `LIMIT n` and `OFFSET n` (either order; `OFFSET` maps to
  `plan.skip`).

Not supported: variable predicates, non-variable subjects after
the seed, `FILTER`, `OPTIONAL`, `UNION`, property paths, literal
objects. These return `SparqlError::Unsupported`.

### Side-by-side example

The same two-hop "organizations a member belongs to" query, in
both surfaces:

```rust
use atomr_ontology_query::{parse_cypher, parse_sparql};
use atomr_ontology::store::TraversalPlan;

let cy: TraversalPlan = parse_cypher(
    "MATCH (p:Person)-[:memberOf]->(org:Organization) RETURN p, org LIMIT 10",
).unwrap();

let sp: TraversalPlan = parse_sparql(r#"
    PREFIX ex: <http://example.com/>
    SELECT ?p ?org WHERE {
        ?p a ex:Person .
        ?p ex:memberOf ?org .
    } LIMIT 10
"#).unwrap();

assert_eq!(cy.seed.bind.as_deref(), Some("p"));
assert_eq!(sp.seed.bind.as_deref(), Some("p"));
assert_eq!(cy.steps.len(), sp.steps.len());
```

The Python parity is `atomr_ontology.query.parse_cypher` /
`parse_sparql`, both returning the same `TraversalPlan` consumed
by `MemStore.traverse`.

## Reference

| File | Contents |
| --- | --- |
| `crates/atomr-ontology-store/src/pattern.rs` | `NodePattern`, `EdgePattern`, `TraversalStep`, `TraversalPlan`, `MatchRow`, `SortOrder`. |
| `crates/atomr-ontology-store/src/mem.rs`     | `MemStore::traverse` — variable-length expansion, cycle prevention, `apply_ordering_and_projection`. |
| `crates/atomr-ontology-query/src/lib.rs`     | Crate root; re-exports `parse_cypher`, `parse_sparql`, `CypherError`, `SparqlError`. |
| `crates/atomr-ontology-query/src/cypher.rs`  | Cypher subset grammar and parser. |
| `crates/atomr-ontology-query/src/sparql.rs`  | SPARQL subset grammar, prefix expansion, triple lowering. |
| `crates/atomr-ontology-py/src/store.rs`      | PyO3 wrappers for the pattern types and `MemStore`. |
| `crates/atomr-ontology-py/src/query.rs`      | PyO3 wrappers for `parse_cypher` / `parse_sparql`. |
| `crates/atomr-ontology-py/python/atomr_ontology/store.pyi` | Python type stubs for the builder API. |
| `crates/atomr-ontology-py/python/atomr_ontology/query.pyi` | Python type stubs for the DSL. |

## Cross-links

- [`data-model.md`](data-model.md) — the `Node`, `Edge`, and
  `PropertyValue` types that patterns match against.
- [`reasoning.md`](reasoning.md) — materialize the deductive
  closure first if you want queries to traverse derived edges
  (transitive `subClassOf`, inverse-of pairs, …).
- [`architecture.md`](architecture.md) — where the store and
  query layer sit in the tiered crate stack.
