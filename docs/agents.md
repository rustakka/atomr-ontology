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

Each extractor consumes an `Arc<dyn Backend>`. The trait is defined
in [`crates/atomr-ontology-extract/src/backend.rs`](../crates/atomr-ontology-extract/src/backend.rs)
and centers on one async method:

```rust
async fn complete(&self, prompt: Prompt) -> Result<String, BackendError>;
```

`Prompt` carries a system message, a user message, and an advisory
`max_tokens`. The trait stays deliberately smaller than
`atomr_agents::Agent` or `atomr_infer::Provider` so the workspace is
decoupled from the upstream generics machinery.

The **recommended `Backend` implementation for production agentic
workflows** is `AgentBackend` (or its multi-turn counterpart
`AgenticAgent`) wrapping an `atomr_agents::Agent` whose inference path
goes through an `atomr_infer::Provider`. See
[`providers.md`](providers.md#provider-selection) for the full
decision tree.

All `Backend` implementations:

- `AgentBackend` / `AgenticAgent` (from
  `atomr-ontology::agents_integration`, feature `agents`) —
  **RECOMMENDED**. Wraps an `atomr_agents::Agent` driver. Use
  `AgentBackend` for single-turn drop-in compatibility with the
  existing extractors; use `AgenticAgent` for tool-using, multi-turn,
  planning workflows (`AgenticTaxonomyInducer`, `AgenticAxiomMiner` —
  see the [Agentic induction](#agentic-induction) section). The
  recommended composition is `agents-with-openai` /
  `agents-with-anthropic` / `agents-with-litellm` so the agent loop's
  inference is provided by `atomr-infer` underneath.
- `InferBackend` (from `atomr-ontology::infer_integration`, feature
  `infer`) — direct `atomr_infer::ModelRunner` wrap, no agent loop.
  Use when you don't need tools / planning. See
  [`providers.md`](providers.md).
- `MockBackend` (from `atomr-ontology-testkit`) — replays a queue
  of pre-scripted responses; the default in CI.
- `HttpDriver` (from `atomr-ontology::http_driver`, feature
  `http-driver`) — **DEPRECATED** (removed in 0.4). Direct REST to
  OpenAI / Anthropic / LiteLLM. Migrate to the matching `provider-*`
  feature; see
  [`providers.md`](providers.md#http-driver) for the deprecation
  notice and migration recipe.

### Batching, streaming, caching

The trait carries three default implementations that any backend
can override:

```rust
async fn batch_complete(&self, prompts: Vec<Prompt>)
    -> Vec<Result<String, BackendError>>;

async fn stream_complete(&self, prompt: Prompt)
    -> Result<ChunkStream, BackendError>;
```

- **`batch_complete`** — concurrent fan-out via
  `futures::future::join_all`. Native batch APIs (vLLM, batched
  ONNX) should override to call their batch endpoint directly.
- **`stream_complete`** — yields `StreamChunk { text, done }`
  values. The default wraps `complete` and emits a single terminal
  chunk; HTTP-SSE / WebSocket drivers should override to yield
  incremental tokens. Returns `ChunkStream =
  Pin<Box<dyn Stream<Item = Result<StreamChunk, BackendError>> + Send>>`.

Add caching transparently by wrapping any backend:

```rust
use atomr_ontology::extract::backend::{CachePolicy, CachedBackend, lru_cached, content_cached};

let cached = CachedBackend::new(my_backend, CachePolicy::Lru(256));
// or:
let cached = lru_cached(my_backend, 256);
let cached = content_cached(my_backend);   // unbounded content-addressed cache
```

`CachePolicy` is `None`, `ContentAddressed` (unbounded; keyed by
prompt hash), or `Lru(n)`. `CachedBackend` short-circuits
`complete` on cache hits; `batch_complete` and `stream_complete`
fall through uncached so latency-sensitive callers can decide
whether to cache stream chunks themselves.

## Composing pipelines

`atomr-ontology-extract::pipeline` ships a lightweight
`Callable<I, O>` trait and a two-stage `Pipeline` combinator. Use
them when you want type-safe composition without pulling in
`atomr-agents`:

```rust
use atomr_ontology::extract::pipeline::{Callable, Pipeline};
let pipeline: Arc<dyn Callable<&str, Vec<RelationCandidate>>> = /* ... */;
```

For real workflows you almost always want the full agents harness —
iterative refinement loops, tool-mediated store introspection,
error recovery, budgeted execution. The recommended path is:

```rust
use std::sync::Arc;
use atomr_ontology::agents_integration::{AgenticAgent, AgenticDriver};
let agent = Arc::new(AgenticAgent::new("anthropic", my_driver));

// Drop-in for the single-turn extractors via the Backend impl.
let term_extractor = TermExtractor::new(agent.clone());

// Multi-turn / tool-using inducers — see "Agentic induction" below.
use atomr_ontology::induce::AgenticAxiomMiner;
let miner = AgenticAxiomMiner::new(agent, tools);
```

The harness owns scheduling and termination; the extractors stay
backend-agnostic.

## Agentic induction

Two inducers in `atomr-ontology-induce` exercise the full agent
surface for workflows that benefit from multi-turn refinement +
tool-mediated store introspection:

| Type | Builds on | Use when |
| --- | --- | --- |
| `AgenticTaxonomyInducer` | `AgenticAgent` + `Vec<ToolSpec>` | The agent should look up existing classes / supertypes before proposing a `sub :> sup` (e.g. to avoid cycles or duplicate edges). |
| `AgenticAxiomMiner` | `AgenticAgent` + `Vec<ToolSpec>` | The agent should validate each axiom family (domain/range/functional/inverse-of/…) against the live schema before emitting it. |

Both take a tool palette — the bundled
`atomr_ontology::extract::store_tools::default_store_tools(store)`
returns `class_exists`, `list_classes`, `list_edge_types`,
`count_instances`, `subclasses_of`, `supertypes_of`, and
`properties_of` over a live `OntologyStore`. Pass an empty `Vec` if
your agent doesn't need store introspection.

```rust
use std::sync::Arc;
use atomr_ontology::agents_integration::AgenticAgent;
use atomr_ontology::extract::store_tools::default_store_tools;
use atomr_ontology::induce::AgenticAxiomMiner;

let agent = Arc::new(AgenticAgent::new("anthropic", my_driver));
let tools = default_store_tools(store.clone());
let miner = AgenticAxiomMiner::new(agent, tools);
let (proposals, activity) = miner.mine("context...").await?;
// activity carries `tool_calls` and `turns` counts from the session.
```

The existing one-shot inducers (`TaxonomyInducer`, `ConceptFormer`,
`AxiomMiner`) remain unchanged — pick them for small workloads where
a single LLM call is enough, and where you want the `MockBackend`
test path to stay simple.

## Activity tagging

Every extractor emits an `Activity` whose label matches the
stage. The standard labels are `term-extraction`,
`entity-resolution`, `concept-formation`, `taxonomy-induction`,
`relation-extraction`, `axiom-mining`, `record-extraction`, plus
`auto-extract.commit` for the commit step. Use those labels
when filtering the provenance log.
