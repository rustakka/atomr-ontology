# Architecture

`atomr-ontology` is a tiered Rust workspace. Each tier has an
explicit responsibility and depends only on tiers below it.

```
┌─────────────────────────────────────────────────────────────┐
│ Tier 3 — facade and reference                                │
│   atomr-ontology (umbrella, prelude, feature-gated re-exports)│
│   atomr-ontology-org      (W3C Org + schema.org reference)   │
│   atomr-ontology-testkit  (MockBackend, fixtures, assertions)│
├─────────────────────────────────────────────────────────────┤
│ Tier 2 — runtime                                             │
│   atomr-ontology-store    (OntologyStore + MemStore)         │
│   atomr-ontology-extract  (Term/Entity/Relation/Record)      │
│   atomr-ontology-induce   (Taxonomy/Concepts/Axioms)         │
│   atomr-ontology-validate (Shapes + axiom consistency)       │
├─────────────────────────────────────────────────────────────┤
│ Tier 1 — pure data                                           │
│   atomr-ontology-core       (LPG types: Iri, Node, Edge…)    │
│   atomr-ontology-rdf        (RDF/OWL adapter, TTL/N-Triples) │
│   atomr-ontology-provenance (PROV-O Activity/Entity/Agent)   │
└─────────────────────────────────────────────────────────────┘
```

Tier 1 crates have no I/O, no actors, and no async surface; they
are pure data plus a few traversal helpers. Tier 2 crates depend on
Tier 1 and on each other through stable trait surfaces. Tier 3 is
ergonomic glue.

## Lifecycle of an auto-built ontology

1. **Seed.** A `MemStore` is constructed (optionally from a
   reference vocabulary like the one in `atomr-ontology-org`).
2. **Ingest.** Documents enter via plain text or a structured
   record source; they are passed downstream as `&str`.
3. **Extract.** `TermExtractor` → `EntityResolver` →
   `RelationExtractor` propose candidates. Each stage produces a
   PROV-O `Activity` attached to its output.
4. **Induce** (optional). `ConceptFormer`, `TaxonomyInducer`,
   `AxiomMiner` lift candidates into a schema sketch.
5. **Validate.** `atomr-ontology-validate::validate` runs SHACL-
   style shape checks and axiom consistency over the proposed
   delta plus the live state.
6. **Commit.** `OntologyStore::commit_with_provenance` applies
   the delta atomically and records the activity in the
   provenance log.
7. **Project.** `atomr-ontology-rdf` writes the live snapshot to
   Turtle, N-Triples, or JSON-LD as needed for export.

## Backend abstraction

Each extractor depends on a narrow `Backend` trait (one async
`complete(prompt) -> String` method). This deliberately stays
smaller than `atomr_agents::InferenceClient` and
`atomr_infer::ModelRunner` so the workspace is decoupled from
the upstream generics machinery. Adapters live in the umbrella
facade (`atomr_ontology::agents_integration`,
`atomr_ontology::infer_integration`) behind cargo features.

## Why labeled property graph as canonical?

- **Ergonomics.** Node-with-properties matches the shape of most
  application data sources (CSV rows, JSON documents, database
  rows) without forcing reification.
- **Performance.** Pattern matching against a typed property bag is
  cheaper than running a SPARQL query against a triple store.
- **Interop.** RDF/OWL projection is straightforward and
  documented in [`naming.md`](naming.md); the reverse direction
  is offered for T-Box import.
