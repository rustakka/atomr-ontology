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

`Prompt` carries a system message, a user message, and an
advisory `max_tokens`. Implementations of `Backend` are:

- `MockBackend` (from `atomr-ontology-testkit`) — replays a queue
  of pre-scripted responses; the default in CI.
- `HttpDriver` (from `atomr-ontology::http_driver`, feature
  `http-driver`) — direct HTTP to OpenAI Chat Completions,
  Anthropic Messages, or any OpenAI-compatible / LiteLLM endpoint.
  Reads `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `LITELLM_API_KEY`
  from the environment.
- `InferBackend` (from `atomr-ontology::infer_integration`,
  feature `infer`) — wraps a driver that owns an
  `atomr_infer::ModelRunner` (vLLM, TensorRT, ONNX, Candle,
  Cudarc, Mistral.rs, OpenAI, Anthropic, Gemini, LiteLLM, etc.).
  See [`providers.md`](providers.md).
- `AgentBackend` (from `atomr-ontology::agents_integration`) —
  wraps an opaque `atomr_agents::Agent` driver, useful when
  extraction should run inside the agents framework's harness /
  workflow runtime.

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
