# Getting started

This walkthrough gets you from a fresh checkout to a working
auto-extract pipeline in both Rust and Python in about ten
minutes. It assumes [`cargo`](https://www.rust-lang.org/tools/install)
and a recent Python (3.8+) on PATH.

## Install

### Rust

Add the umbrella crate to your `Cargo.toml`:

```toml
[dependencies]
atomr-ontology = "0.1"
```

Default features pull in `rdf`, `provenance`, `store`, `extract`,
`induce`, `validate`, and `org`. Opt-in features layer on the
optional capabilities — for example:

```toml
atomr-ontology = { version = "0.1", features = [
    "agents-with-anthropic",  # RECOMMENDED — AgentBackend + atomr-infer/anthropic
] }
# Or, no agent loop:
# atomr-ontology = { version = "0.1", features = ["provider-anthropic"] }
# DEPRECATED (removed in 0.4) — prefer the provider-* features above.
# atomr-ontology = { version = "0.1", features = ["http-driver"] }
```

See [`providers.md`](providers.md#provider-selection) for the full
feature matrix and the decision tree.

### Python

The Python bindings ship as a maturin-built wheel:

```bash
pip install atomr-ontology
```

Provider extras follow the same pattern as the Rust features:

```bash
pip install 'atomr-ontology[agents-with-anthropic]'  # RECOMMENDED agentic stack
pip install 'atomr-ontology[anthropic]'              # atomr-infer-backed, no agent loop
pip install 'atomr-ontology[http-driver]'            # DEPRECATED — prefer the provider-* extras
```

Building from source needs [`maturin`](https://www.maturin.rs/):

```bash
pip install maturin
cd crates/atomr-ontology-py
maturin develop
```

`maturin develop` produces an importable `atomr_ontology` Python
package in your current environment plus `.pyi` stubs for
`pyright` / IDE autocomplete.

## Build a tiny ontology

### Rust

```rust
use atomr_ontology::prelude::*;
use atomr_ontology_org::reference_ontology;

fn main() {
    let mut o = reference_ontology();
    let acme = Node::from_iri(
        Iri::from_unchecked("https://example.org/Acme"),
        "Organization",
    )
    .with_property("name", "Acme Inc.");
    let acme_id = o.upsert_node(acme);

    let bob = Node::from_iri(
        Iri::from_unchecked("https://example.org/Bob"),
        "Person",
    )
    .with_property("name", "Bob");
    let bob_id = o.upsert_node(bob);

    o.upsert_edge(Edge::between(bob_id, "memberOf", acme_id));

    println!("nodes: {}, edges: {}", o.node_count(), o.edge_count());
}
```

### Python

```python
import atomr_ontology as ao

o = ao.reference_ontology()
acme = ao.Node.from_iri(
    ao.Iri.from_unchecked("https://example.org/Acme"), "Organization"
).with_property("name", "Acme Inc.")
acme_id = o.upsert_node(acme)

bob = ao.Node.from_iri(
    ao.Iri.from_unchecked("https://example.org/Bob"), "Person"
).with_property("name", "Bob")
bob_id = o.upsert_node(bob)

o.upsert_edge(ao.Edge.between(bob_id, "memberOf", acme_id))
print(f"nodes: {len(o.nodes)}, edges: {len(o.edges)}")
```

## Project to RDF

```rust
let ttl = atomr_ontology::rdf::turtle::write(&o);
let jsonld = atomr_ontology::rdf::jsonld::write(&o);
std::fs::write("ontology.ttl", ttl).unwrap();
std::fs::write("ontology.jsonld", jsonld).unwrap();
```

Round-trip through the parsers works the same way:

```rust
let parsed = atomr_ontology::rdf::turtle::read(&ttl).unwrap();
assert!(parsed.schema.node_type("Organization").is_some());
```

The Python surface mirrors these as `ao.rdf.turtle_write`,
`ao.rdf.turtle_read`, etc. See [`naming.md`](naming.md) for the
projection rules.

## Run the auto-extract pipeline

The full 7-stage demo is in
[`examples/auto_extract_from_text`](../examples/auto_extract_from_text):

```bash
cargo run -p auto_extract_from_text -- --provider mock --out-dir out
```

The mock provider replays a deterministic JSON queue, so the
command runs hermetically without any network access. Outputs:

- `out/ontology.ttl` — Turtle dump of the resulting ontology.
- `out/ontology.jsonld` — JSON-LD with `@context`.
- `out/trace.json` — PROV-O log of every stage's `Activity`.

To run against a real LLM, build with one of the provider features
and pass `--provider openai` / `anthropic` / `litellm`:

```bash
export ANTHROPIC_API_KEY=...
cargo run -p auto_extract_from_text --features provider-anthropic -- \
    --provider anthropic --model claude-3-5-sonnet --out-dir out
# Or, with the deprecated direct-REST shim (will be removed in 0.4):
# cargo run -p auto_extract_from_text --features http-driver -- \
#     --provider openai --model gpt-4o-mini --out-dir out
```

The pipeline is described in [`agents.md`](agents.md); the provider
matrix and the canonical
`AgentBackend → atomr_agents::Agent → atomr_infer::Provider`
layering are in
[`providers.md`](providers.md#provider-selection).

## Where to next

- **Persistence** — keep your ontology on disk: [`persistence.md`](persistence.md).
- **Versioning** — branch / merge / time-travel: [`versioning.md`](versioning.md).
- **Reasoning** — derive implicit facts: [`reasoning.md`](reasoning.md).
- **Query** — variable-length paths and the string DSL: [`query.md`](query.md).
- **Embeddings** — vector-similarity entity resolution: [`embeddings.md`](embeddings.md).
- **Remote** — hosted `OntologyStore` over HTTP: [`remote.md`](remote.md).
- **Python** — full bindings walkthrough: [`python.md`](python.md).
- **Upgrade from v0.1** — [`migration-0.1-to-0.2.md`](migration-0.1-to-0.2.md).
