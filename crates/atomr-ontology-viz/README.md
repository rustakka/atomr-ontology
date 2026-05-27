# atomr-ontology-viz

GraphViz DOT + Mermaid renderers for `Ontology` and
`ProvenanceLog`.

## Features

None. Pure-string output; no native rendering dependencies.

## Example

```rust
use atomr_ontology_viz::{
    render_ontology_dot, render_ontology_mermaid,
    render_provenance_dot, render_provenance_mermaid,
};

let dot = render_ontology_dot(&ontology);
let mermaid = render_ontology_mermaid(&ontology);
let prov_dot = render_provenance_dot(&log);
```

Pipe `dot` into `dot -Tsvg -o ontology.svg`; embed Mermaid in
GitHub markdown directly.

## Full guide

[`docs/viz.md`](../../docs/viz.md).
