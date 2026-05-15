# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Python bindings (`atomr-ontology-py`): PyO3 + maturin wheel exposing
  the umbrella API surface. All Rust async methods (`OntologyStore`,
  extractors, inducers) are exposed as Python coroutines via
  `pyo3-async-runtimes`. Provider integrations from `atomr-infer` are
  opt-in via Python extras (`atomr-ontology[openai]`, `[anthropic]`,
  `[vllm]`, etc.). End-to-end pipeline smoke test mirrors
  `examples/org_ontology_demo`.

## [0.1.0] — initial release

### Added
- Labeled property graph core types (`atomr-ontology-core`):
  `Iri`, `Namespace`, `Vocabulary`, `Ontology`, `Node`, `Edge`,
  `Schema`, `Record`, `Axiom`, content-addressed `NodeId`/`EdgeId`.
- RDF/OWL adapter (`atomr-ontology-rdf`): `Class`, `Individual`,
  `ObjectProperty`, `DataProperty`, `Triple`, `Quad`, Turtle /
  N-Triples / JSON-LD serializers behind feature flags.
- PROV-O-aligned provenance types (`atomr-ontology-provenance`).
- `OntologyStore` trait and in-memory implementation
  (`atomr-ontology-store`) with builder-style pattern matching and
  traversal.
- Extraction primitives (`atomr-ontology-extract`):
  `TermExtractor`, `EntityResolver`, `RelationExtractor`,
  `RecordExtractor`.
- Ontology-learning primitives (`atomr-ontology-induce`):
  `TaxonomyInducer`, `ConceptFormer`, `AxiomMiner`.
- SHACL-style validation (`atomr-ontology-validate`).
- Testkit fixtures and `MockBackend` (`atomr-ontology-testkit`).
- W3C Org Ontology reference vocabulary (`atomr-ontology-org`).
- Umbrella facade with `prelude` (`atomr-ontology`).
- Examples: `org_ontology_demo`, `auto_extract_from_text`.
- xtask: `parity`, `verify`, `audit`.
