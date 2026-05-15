# Initial Implementation Plan: `atomr-ontology`

> Status: approved, pre-implementation. This document is the initial plan
> committed at repo bootstrap. Subsequent design decisions belong in
> `docs/architecture.md` and per-topic docs (`data-model.md`, `agents.md`,
> `providers.md`, `naming.md`); they should not retroactively edit this
> file. Treat it as a historical record of the starting design intent.

## Context

`/home/cognect/source/atomr-ontology/` is an empty directory that will become a
new sibling crate in the `rustakka/atomr` ecosystem. It inherits from
`atomr-agents` (agent runtime) and `atomr-infer` (LLM providers, including
local) so that agents can programmatically build out an ontology of entities
and records.

Decisions locked in with the user:

- **Canonical data model:** labeled property graph (LPG). An RDF/OWL adapter
  ships alongside as a non-canonical projection, so the crate interoperates
  with W3C tooling without forcing RDF semantics on day-to-day authoring.
- **Domain scope:** generic ontology framework first; W3C Org Ontology + schema.org `Organization` ship as the reference worked example, not as hard-coded core types.
- **Provider strategy:** consume `atomr-infer` providers generically through
  `atomr-agents::InferenceClient` / `LocalRunnerClient`. Users select provider
  (OpenAI, Anthropic, Gemini, vLLM, Candle, ONNX Runtime, TensorRT, Mistral.rs, LiteLLM-proxied Ollama, etc.) via `RuntimeConfig` at construction time.

Naming follows the industry standard for ontology engineering and ontology
learning: terms like `Iri`, `Namespace`, `Vocabulary`, `Ontology`, `Class`,
`Individual`, `ObjectProperty`, `DataProperty`, `Triple`, `Axiom`, `Provenance`
(PROV-O aligned) on the RDF side; `Node`, `Edge`, `Label`, `Property`,
`Schema`, `NodeType`, `EdgeType`, `Cardinality`, `Record` on the LPG side.

## Repo skeleton (matches atomr-X house style)

Top-level files, copying the pattern shared by `atomr-accel`, `atomr-dledger`, `atomr-infer`, `atomr-physical`, `atomr-story`, `atomr-worlds`:

```
atomr-ontology/
├── Cargo.toml                 # cargo workspace, inherits style from /home/cognect/source/atomr/Cargo.toml
├── Cargo.lock                 # committed (per sibling convention)
├── rust-toolchain.toml        # channel = "stable", components = ["rustfmt","clippy"]
├── rustfmt.toml               # edition = "2021", max_width = 110, use_small_heuristics = "Max"
├── deny.toml                  # cargo-deny: advisories, licenses (Apache-2.0/MIT), bans, sources
├── .gitignore                 # /target, Cargo.lock under crates/, .claude/worktrees/, __pycache__, maturin artifacts
├── .cargo/config.toml         # alias xtask = "run --package xtask --"
├── LICENSE                    # Apache-2.0 (header matches atomr)
├── README.md                  # what / why / minimal example / workspace tier table / docs links
├── CONTRIBUTING.md            # mirrors atomr/CONTRIBUTING.md
├── CHANGELOG.md               # keepachangelog format, starts at 0.1.0
├── SECURITY.md                # mirrors atomr-agents
├── docs/
│   ├── architecture.md        # tier diagram, lifecycle of an auto-built ontology
│   ├── data-model.md          # LPG canonical types + RDF/OWL adapter
│   ├── agents.md              # the 7-step ontology-learning pipeline below
│   ├── providers.md           # how to point at any atomr-infer runtime
│   └── naming.md              # mapping from atomr-ontology vocabulary to RDF/OWL/PROV-O/schema.org
├── examples/
│   ├── org_ontology_demo/     # build a W3C Org Ontology graph with a mock runner
│   └── auto_extract_from_text/# end-to-end extraction against a real configured provider
├── xtask/                     # parity, audit, verify subcommands (copy atomr/xtask shape)
├── .github/workflows/
│   ├── ci.yml                 # fmt → clippy → test → doc, RUSTFLAGS="-D warnings"
│   ├── docs.yml
│   └── release.yml
└── crates/                    # tiered, see below
```

## Crate decomposition (workspace members)

Tiered the same way `atomr-worlds-core` / `atomr-dledger-types` / `atomr-story-core` do — pure-data Tier 1, behavior in Tier 2, umbrella facade at the top.

### Tier 1 — pure data, no I/O, no actors

- **`atomr-ontology-core`** — canonical labeled property graph types: `Iri`,
  `Namespace`, `Vocabulary`, `Ontology`, `Node`, `NodeId`, `Edge`, `EdgeId`,
  `Label`, `Property`, `PropertyValue`, `Datatype`, `Cardinality`,
  `Schema`, `NodeType`, `EdgeType`, `PropertyType`, `Record` (flat
  node-with-properties snapshot), `Axiom` (subclass/equivalent/disjoint/
  domain/range/functional/inverse), `OntologyError`. `serde` derives, content-addressed IDs in the style of `atomr-dledger-types::id`. `#![forbid(unsafe_code)]`.
- **`atomr-ontology-rdf`** — RDF/OWL adapter: `Class`, `Individual`,
  `ObjectProperty`, `DataProperty`, `Triple`, `Quad`, plus
  `to_rdf(&Ontology)` / `from_rdf(...)`. Serializers/parsers for Turtle,
  N-Triples, and JSON-LD behind feature flags. Round-trip semantics documented in `docs/naming.md`.
- **`atomr-ontology-provenance`** — PROV-O-aligned types: `Activity`,
  `ProvAgent` (disambiguated from `atomr_agents::Agent`), `ProvEntity`,
  `wasDerivedFrom`, `wasAttributedTo`, `wasGeneratedBy`. Every fact written
  to a store carries a `ProvenanceId` so we can answer "which agent proposed
  this, from what source, when, under which model run."

### Tier 2 — runtime: storage, extraction, induction, validation

- **`atomr-ontology-store`** — `OntologyStore` trait (async; `upsert_node`,
  `upsert_edge`, `match_pattern`, `traverse`, `snapshot`, `diff`,
  `commit_with_provenance`) + in-memory `MemStore` impl. Query interface
  inspired by openCypher pattern matching and SPARQL BGPs, but expressed as
  Rust builder types — no string DSL in v0.1.
- **`atomr-ontology-extract`** — agent-driven extraction primitives, each
  exposed both as a standalone `atomr_agents::Tool` and as a composable
  `atomr_agents::Callable`:
  - `TermExtractor` — surface terms from a document.
  - `EntityResolver` — link mentions to existing `Node`s or propose new ones.
  - `RelationExtractor` — propose object properties between resolved entities.
  - `RecordExtractor` — convert structured/semi-structured input into `Record`s.
- **`atomr-ontology-induce`** — higher-order ontology learning, composed of
  the Tier-2 extractors:
  - `TaxonomyInducer` — propose subclass axioms.
  - `ConceptFormer` — cluster terms into candidate `Class`es.
  - `AxiomMiner` — propose `Axiom` candidates (domain/range/functional/...).
- **`atomr-ontology-validate`** — SHACL-style shape validation +
  axiom-consistency checks over a `Schema` and the current store contents.
  Returns a structured `ValidationReport`.

### Tier 3 — testkit, reference example, umbrella

- **`atomr-ontology-testkit`** — test fixtures (toy corpora, golden
  ontologies), a `MockProvider` wrapping `atomr_infer::testkit::MockRunner`,
  assertion helpers (`assert_ontology_eq`, `assert_subclass_of`).
- **`atomr-ontology-org`** — reference vocabulary modeling organizations
  using W3C Org Ontology (`Organization`, `FormalOrganization`,
  `OrganizationalUnit`, `Membership`, `Role`, `Post`, `Site`) and
  schema.org `Organization`. This is a *worked example*, not a privileged
  built-in.
- **`atomr-ontology`** — umbrella facade. `pub use atomr_ontology_core as core;` etc., plus a `prelude` re-exporting the most common types. Feature flags gate every Tier-2/3 crate so downstream pulls only what it needs.

## Cargo workspace shape

`Cargo.toml` mirrors `/home/cognect/source/atomr/Cargo.toml`:

- `resolver = "2"`, `[workspace.package]` centralizes `version = "0.1.0"`,
  `edition = "2021"`, `rust-version = "1.78"`, `license = "Apache-2.0"`,
  `repository = "https://github.com/rustakka/atomr-ontology"`,
  `authors = ["atomr contributors"]`.
- `[workspace.dependencies]` pins externals (`tokio = { version = "1", features = ["full"] }`, `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `thiserror = "1"`, `anyhow = "1"`, `tracing = "0.1"`, `async-trait = "0.1"`, `criterion = "0.5"`).
- **Upstream atomr deps declared as path dependencies during co-development, with `version =` set so they also publish cleanly** — exactly how `atomr-dledger` and `atomr-worlds` declare upstream siblings:
  ```toml
  atomr            = { path = "../atomr",        version = "0.9" }
  atomr-agents     = { path = "../atomr-agents", version = "0.10", default-features = false }
  atomr-infer      = { path = "../atomr-infer",  version = "0.8",  default-features = false }
  ```
  Provider features are *re-exported*, not duplicated: `atomr-ontology`'s
  `provider-openai` feature simply enables `atomr-infer/openai`, and so on
  for `anthropic`, `gemini`, `litellm`, `vllm`, `candle`, `ort`, `tensorrt`,
  `mistralrs`, `cudarc`. This is the "generic providers" contract the user asked for.
- `[workspace.lints.clippy]` copies atomr's baseline (`todo = "deny"`, `unimplemented = "deny"`).
- `[profile.release]`: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`.

## Auto-ontology pipeline (the immediate focus)

A standard ontology-learning pipeline (Maedche & Staab style, modernized for
LLM agents), shipped as a default `atomr_agents::Workflow` so downstreams can
swap out any stage:

1. **Ingest** — load corpus via `atomr_agents::ingest` (files, URLs, structured rows).
2. **Extract terms** — `TermExtractor` agent emits candidate surface forms.
3. **Resolve entities** — `EntityResolver` matches mentions to existing `Node`s or proposes new ones; emits `Record` candidates.
4. **Form concepts** — `ConceptFormer` clusters synonymous terms into candidate `Class`es.
5. **Induce taxonomy** — `TaxonomyInducer` proposes subclass `Axiom`s.
6. **Extract relations** — `RelationExtractor` proposes `Edge`s + their `EdgeType`s.
7. **Validate & commit** — `atomr-ontology-validate` checks the proposed delta against the live `Schema`; `OntologyStore::commit_with_provenance` writes accepted facts with full PROV-O lineage back to the agent run.

The whole pipeline runs in an `atomr_agents::Harness` so iterative refinement
(re-running stages with the growing ontology as additional context) is the
default rather than a special case. Each agent receives an
`InferenceClient` constructed from whichever `atomr_infer::ModelRunner` the
user wired up — local Candle/vLLM/ONNX/TensorRT/Mistral.rs, or remote
OpenAI/Anthropic/Gemini, or anything reachable via LiteLLM (which is how
Ollama plugs in).

## Critical files to be created

- `Cargo.toml`
- `rust-toolchain.toml`
- `rustfmt.toml`
- `deny.toml`
- `.gitignore`
- `.cargo/config.toml`
- `LICENSE`
- `README.md`
- `CONTRIBUTING.md`
- `CHANGELOG.md`
- `SECURITY.md`
- `.github/workflows/{ci.yml,docs.yml,release.yml}`
- `docs/{architecture,data-model,agents,providers,naming}.md`
- `crates/atomr-ontology-core/{Cargo.toml,src/lib.rs,src/{iri,namespace,vocabulary,node,edge,schema,record,axiom,error}.rs}`
- `crates/atomr-ontology-rdf/{Cargo.toml,src/lib.rs,src/{triple,owl,turtle,ntriples,jsonld,adapter}.rs}`
- `crates/atomr-ontology-provenance/{Cargo.toml,src/lib.rs}`
- `crates/atomr-ontology-store/{Cargo.toml,src/{lib,trait,mem,query}.rs}`
- `crates/atomr-ontology-extract/{Cargo.toml,src/{lib,terms,entities,relations,records}.rs}`
- `crates/atomr-ontology-induce/{Cargo.toml,src/{lib,taxonomy,concepts,axioms}.rs}`
- `crates/atomr-ontology-validate/{Cargo.toml,src/{lib,shapes,consistency}.rs}`
- `crates/atomr-ontology-testkit/{Cargo.toml,src/lib.rs,fixtures/}`
- `crates/atomr-ontology-org/{Cargo.toml,src/lib.rs}`
- `crates/atomr-ontology/{Cargo.toml,src/lib.rs}` (umbrella + prelude)
- `examples/org_ontology_demo/{Cargo.toml,src/main.rs}`
- `examples/auto_extract_from_text/{Cargo.toml,src/main.rs}`
- `xtask/{Cargo.toml,src/main.rs}`

## Existing types / utilities to reuse (do not reinvent)

- `atomr_agents_core::{AgentId, RunId, AgentContext, Message, CallCtx}` — `../atomr-agents/crates/core/src/lib.rs`. Wire these through every extractor/inducer.
- `atomr_agents::{Agent, AgentRef, AgentBudgets, InferenceClient, LocalRunnerClient}` — `../atomr-agents/crates/agent/src/lib.rs`. Concrete agent type for every pipeline stage.
- `atomr_agents::Tool` + `ToolDescriptor` + `ToolSet` — `../atomr-agents/crates/tool/src/lib.rs`. Each extractor implements `Tool` so other agents can call it directly.
- `atomr_agents::Callable` + `Pipeline` — `../atomr-agents/crates/callable/src/lib.rs`. Use for `.then()`/`.fan_out()` composition between stages.
- `atomr_agents::{Workflow, Dag, Step}` — `../atomr-agents/crates/workflow/src/lib.rs`. The 7-step pipeline is a default `Dag`.
- `atomr_agents::{Harness, LoopStrategy, TerminationStrategy}` — `../atomr-agents/crates/harness/src/lib.rs`. Iterative refinement.
- `atomr_agents::{EventBus, Tracer}` — `../atomr-agents/crates/observability/src/lib.rs`. Every store mutation emits an event for the registered tracer.
- `atomr_agents::Registry` + `ArtifactRecord` + `ArtifactKind` — `../atomr-agents/crates/registry/src/lib.rs`. Version ontology snapshots as artifacts.
- `atomr_infer::ModelRunner`, `RuntimeConfig`, `RuntimeKind`, `infer_runtime` — `../atomr-infer/crates/inference-core/src/{runner.rs,runtime.rs,registry.rs}`. The provider abstraction; do not redeclare it.
- `atomr_infer::testkit::MockRunner` — `../atomr-infer/crates/inference-testkit/src/mock_runner.rs`. Backs `atomr-ontology-testkit::MockProvider`.
- `atomr_patterns::ddd::{Entity, ValueObject, AggregateRoot, Repository}` — `../atomr/crates/atomr-patterns/src/ddd/mod.rs`. The `OntologyStore` trait is a `Repository`; `Ontology` is an `AggregateRoot`.
- ID and content-addressing patterns from `atomr_dledger_types::id` — `../atomr-dledger/crates/atomr-dledger-types/src/id.rs`. Reuse for `NodeId`/`EdgeId`/`ProvenanceId`.
- Hierarchical/graph data patterns from `atomr_worlds_core::{addr, hierarchy, interaction}` — `../atomr-worlds/crates/atomr-worlds-core/src/`. Reference for traversal/addressing shapes.

## Verification

End-to-end checks the implementation must pass before the init is considered done:

1. `cargo build --workspace` — clean build with default features.
2. `cargo build --workspace --all-features` — every provider feature compiles.
3. `cargo test --workspace` — Tier-1 unit tests + testkit-backed integration tests in `atomr-ontology-extract` and `atomr-ontology-store` pass.
4. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` — match the lint bar enforced by the rest of the ecosystem.
5. `cargo doc --workspace --no-deps` — rustdoc builds with no broken intra-doc links.
6. `cargo run -p org_ontology_demo` — emits a W3C Org Ontology graph from a hand-written seed using `MockProvider`; asserts node/edge counts and a few subclass axioms.
7. `cargo run -p auto_extract_from_text -- --provider <openai|anthropic|litellm|...> --model <...> --corpus docs/sample-corpus/` — runs the full 7-stage pipeline against a real provider, writes the resulting ontology to `out/ontology.{ttl,jsonld}` plus a PROV-O run trace; smoke-test asserts the trace contains every stage and that the validator reports no consistency errors.
8. `cargo run -p xtask -- parity` and `cargo run -p xtask -- verify` — match the parity/verify gates the rest of the atomr ecosystem uses.
