# Agents and the auto-ontology pipeline

The default auto-ontology pipeline is a seven-step workflow,
modeled after Maedche & Staab's ontology-learning layer cake and
modernized for LLM agents. Every step has a corresponding
`Activity` in the provenance log.

| # | Stage | Component | Inputs | Outputs |
| - | --- | --- | --- | --- |
| 1 | Ingest | (caller) | source documents | `Vec<String>` |
| 2 | Extract terms | `TermExtractor` | doc text | `Vec<TermCandidate>` |
| 3 | Resolve entities | `EntityResolver` | terms + store hint | `Vec<EntityCandidate>` |
| 4 | Form concepts (optional) | `ConceptFormer` | terms | `Vec<ConceptCluster>` |
| 5 | Induce taxonomy (optional) | `TaxonomyInducer` | class names | `Vec<SubclassProposal>` |
| 6 | Extract relations | `RelationExtractor` | doc + entities | `Vec<RelationCandidate>` |
| 7 | Validate & commit | `validate` + `commit_with_provenance` | delta | `ProvenanceId` |

Steps 4–5 are optional in the strict sense — the simpler
"resolve + relate + commit" path is enough when the schema is
already known. They become important when the schema itself is
being learned.

## Backend contract

Each extractor consumes an `Arc<dyn Backend>`. `Backend` is a
trait with one method:

```rust
async fn complete(&self, prompt: Prompt) -> Result<String, BackendError>;
```

`Prompt` carries a system message, a user message, and an
advisory `max_tokens`. Implementations of `Backend` are:

- `MockBackend` (from `atomr-ontology-testkit`) — replays a queue
  of pre-scripted responses; the default in CI.
- `InferBackend` (from `atomr-ontology::infer_integration`) —
  wraps a driver that owns an `atomr_infer::ModelRunner` (vLLM,
  TensorRT, ONNX, Candle, Cudarc, Mistral.rs, OpenAI, Anthropic,
  Gemini, LiteLLM, etc.). See [`providers.md`](providers.md).
- `AgentBackend` (from `atomr-ontology::agents_integration`) —
  wraps an opaque `atomr_agents::Agent` driver, useful when
  extraction should run inside the agents framework's harness /
  workflow runtime.

## Composing pipelines

`atomr-ontology-extract::pipeline` ships a lightweight
`Callable<I, O>` trait and a two-stage `Pipeline` combinator. Use
them when you want type-safe composition without pulling in
`atomr-agents`:

```rust
use atomr_ontology::extract::pipeline::{Callable, Pipeline};
let pipeline: Arc<dyn Callable<&str, Vec<RelationCandidate>>> = /* ... */;
```

When you do want the full agents harness — for iterative
refinement loops, error recovery, or budgeted execution — enable
the `agents` feature on the umbrella crate and lift the
extractors through `AgentBackend`. The harness owns scheduling
and termination; the extractors stay backend-agnostic.

## Activity tagging

Every extractor emits an `Activity` whose label matches the
stage. The standard labels are `term-extraction`,
`entity-resolution`, `concept-formation`, `taxonomy-induction`,
`relation-extraction`, `axiom-mining`, `record-extraction`, plus
`auto-extract.commit` for the commit step. Use those labels
when filtering the provenance log.
