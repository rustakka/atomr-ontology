# Visualization

`atomr-ontology-viz` renders an [`Ontology`](data-model.md) or a
[`ProvenanceLog`](data-model.md#provenance) as either a GraphViz DOT
document or a Mermaid `graph` document. Both outputs are plain
strings — there is no native GraphViz dependency at build time, no
async surface, and no I/O. The caller is responsible for piping the
output through `dot`, embedding it in a Markdown file, or otherwise
materializing it as an image.

## When to reach for this

- Eyeballing the shape of an auto-built ontology mid-pipeline.
- Embedding the structure of a small schema in design docs / READMEs.
- Drawing the PROV-O lineage that an extraction produced.

## Concepts

| Renderer | Output format | Subject |
| --- | --- | --- |
| `render_ontology_dot` | GraphViz DOT | nodes + edges (one box per `Node`, labeled by `name` if present) |
| `render_ontology_mermaid` | Mermaid `graph LR` | same, suitable for GitHub Markdown |
| `render_provenance_dot` | GraphViz DOT | `Activity`s + `wasDerivedFrom` edges |
| `render_provenance_mermaid` | Mermaid `graph TD` | same |

## Rust example

```rust
use atomr_ontology_core::{Edge, Iri, Node, Ontology};
use atomr_ontology_viz::{render_ontology_dot, render_ontology_mermaid};

let mut o = Ontology::new();
o.declare_node_type("Organization");
let acme = o.upsert_node(Node::new("Organization").with_property("name", "Acme"));
let beta = o.upsert_node(Node::from_iri(Iri::from_unchecked("https://example.org/Beta"), "Organization"));
o.upsert_edge(Edge::between(acme, "partner", beta));

let dot = render_ontology_dot(&o);
// digraph ontology {
//   rankdir=LR;
//   node [shape=box, style=rounded];
//   n_<hash> [label="\"Acme\" | Organization"];
//   n_<hash> [label="Organization"];
//   n_<hash> -> n_<hash> [label="partner"];
// }

let mermaid = render_ontology_mermaid(&o);
// graph LR
//   n<id>["\"Acme\""]
//   n<id>["Organization"]
//   n<id> -- "partner" --> n<id>
```

For provenance, pass a `ProvenanceLog`:

```rust
use atomr_ontology_viz::render_provenance_dot;
let dot = render_provenance_dot(&store.provenance());
```

## Python example

```python
from atomr_ontology import Edge, Iri, Node, Ontology
from atomr_ontology import viz

o = Ontology()
o.declare_node_type("Organization")
acme = o.upsert_node(Node("Organization").with_property("name", "Acme"))
beta = o.upsert_node(Node.from_iri(Iri.from_unchecked("https://example.org/Beta"), "Organization"))
o.upsert_edge(Edge.between(acme, "partner", beta))

print(viz.render_ontology_dot(o))
print(viz.render_ontology_mermaid(o))
```

## Recipes

Render DOT to SVG via the `dot` CLI:

```sh
cargo run --example my_app | dot -Tsvg -o ontology.svg
```

Embed Mermaid in a GitHub-flavored Markdown file: wrap the output in a
fenced ```mermaid block and GitHub renders it inline. No image
pipeline needed.

## Reference

| File | Purpose |
| --- | --- |
| `crates/atomr-ontology-viz/src/lib.rs` | crate root + re-exports |
| `crates/atomr-ontology-viz/src/dot.rs` | DOT renderer (ontology + provenance) |
| `crates/atomr-ontology-viz/src/mermaid.rs` | Mermaid renderer (ontology + provenance) |
| `crates/atomr-ontology-py/src/viz.rs` | PyO3 wrapper |
| `crates/atomr-ontology-py/python/atomr_ontology/viz.pyi` | Python stubs |

## See also

- [`data-model.md`](data-model.md) — what gets rendered.
- [`naming.md`](naming.md) — how labels map to RDF/OWL terms.
