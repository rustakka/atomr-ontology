# Providers

`atomr-ontology` doesn't hard-code any LLM provider — it depends only
on a narrow [`Backend`](agents.md#backend-contract) trait. The
**recommended layering** for agentic workflows is:

```text
agentic ontology workflow (TermExtractor, EntityResolver,
                           RelationExtractor, AxiomMiner, ...)
        │  takes Arc<dyn Backend>  (or Arc<AgenticAgent> for the
        ▼                            multi-turn surface)
   Backend trait                    (atomr-ontology-extract)
        │
        ▼
   AgentBackend / AgenticAgent      (atomr-ontology, recommended)
        │  drives
        ▼
   atomr_agents::Agent              (planning / tools / multi-turn)
        │  inference via
        ▼
   atomr_infer::Provider            (OpenAI, Anthropic, Gemini,
                                     LiteLLM, vLLM, Candle, ORT,
                                     TensorRT, Mistral.rs, ...)
        │
        ▼
   real model / endpoint
```

`atomr-agents` and `atomr-infer` are independent crates today —
`atomr-agents` does not transitively depend on `atomr-infer`. The
layering above is the **recommended wiring**: you instantiate an
`atomr_agents::Agent` whose inference path is backed by an
`atomr_infer::Provider`, then plug that into the umbrella's
`AgentBackend` (or its multi-turn counterpart, `AgenticAgent`). The
`agents-with-*` meta-features in the umbrella's `Cargo.toml` bundle
both feature surfaces in one shot so the recommended stack compiles
without manual feature juggling.

See also: [`agents.md`](agents.md) (Backend / Agentic contracts),
[`architecture.md`](architecture.md) (where this sits in the tier
diagram).

## Provider selection

```text
Need to drive an extractor?
├── In tests / CI / examples → MockBackend (atomr-ontology-testkit)
│
├── Want full agent loop (tools, planning, multi-turn refinement,
│   budgeted execution)?
│      → AgentBackend + atomr_agents::Agent + atomr_infer provider
│        (RECOMMENDED default for production agentic workflows)
│        — feature `agents-with-openai`, `agents-with-anthropic`,
│          `agents-with-litellm`, `agents-with-candle`, …
│
├── Want single-shot inference with atomr-infer's provider matrix,
│   no agent harness?
│      → InferBackend + atomr_infer provider directly
│        — feature `provider-openai` / `provider-anthropic` / …
│
└── Already have raw REST creds and want zero extra deps?
       → HttpDriver  (DEPRECATED — see "HTTP driver" section below;
         prefer InferBackend with the matching atomr-infer remote
         provider)
```

## Feature flags

| Feature | Enables |
| --- | --- |
| `agents` | `AgentBackend` (single-turn shim) + re-exports `AgenticAgent` / `AgenticSession` / `ToolSpec` from the extract crate. |
| `agents-with-openai` | `agents` + `provider-openai` (recommended for OpenAI). |
| `agents-with-anthropic` | `agents` + `provider-anthropic`. |
| `agents-with-gemini` | `agents` + `provider-gemini`. |
| `agents-with-litellm` | `agents` + `provider-litellm` (LiteLLM / Ollama). |
| `agents-with-vllm` | `agents` + `provider-vllm`. |
| `agents-with-candle` | `agents` + `provider-candle` (local). |
| `agents-with-infer` | `agents` + `infer` (only the `InferBackend` adapter, no provider). |
| `infer` | `InferBackend` (single-shot adapter over `atomr_infer::ModelRunner`), no specific provider. |
| `provider-openai` | `atomr-infer/openai` (OpenAI / Azure OpenAI). |
| `provider-anthropic` | `atomr-infer/anthropic`. |
| `provider-gemini` | `atomr-infer/gemini`. |
| `provider-litellm` | `atomr-infer/litellm` (Ollama via LiteLLM proxy). |
| `provider-vllm` | `atomr-infer/vllm`. |
| `provider-candle` | `atomr-infer/candle`. |
| `provider-ort` | `atomr-infer/ort`. |
| `provider-tensorrt` | `atomr-infer/tensorrt`. |
| `provider-mistralrs` | `atomr-infer/mistralrs`. |
| `provider-cudarc` | `atomr-infer/cudarc`. |
| `http-driver` | **DEPRECATED.** Direct REST `HttpDriver`; will be removed in 0.4. |

The `agents-with-*` meta-features do NOT enforce a Cargo dependency
between `atomr-agents` and `atomr-infer` — they only guarantee both
feature surfaces are compiled together so you can construct an
`AgenticDriver` that calls into an `InferBackend` underneath without
manual feature plumbing.

## Wiring the recommended stack

```rust
use std::sync::Arc;

use atomr_ontology::agents_integration::{AgenticAgent, AgenticDriver};
use atomr_ontology::extract::TermExtractor;

// 1. Build an `atomr_agents::Agent` whose inference path goes through
//    `atomr_infer::Provider::Anthropic` (your code / a thin adapter
//    crate; the exact shape depends on the atomr-agents API).
let driver: Arc<dyn AgenticDriver> = my_anthropic_agent_driver()?;
let agent = Arc::new(AgenticAgent::new("anthropic", driver));

// 2a. Drop into the narrow single-shot extractors — AgenticAgent
//     impls Backend, so this is a direct hand-off.
let extractor = TermExtractor::new(agent.clone());

// 2b. Multi-turn / tool-using path — hand the agent to one of the
//     agentic inducers (see docs/agents.md#agentic-induction).
use atomr_ontology::extract::store_tools::default_store_tools;
use atomr_ontology::induce::AgenticAxiomMiner;
let tools = default_store_tools(store_arc.clone());
let miner = AgenticAxiomMiner::new(agent, tools);
let (axioms, activity) = miner.mine(&context).await?;
```

The `AgenticDriver` you implement is the only piece that touches the
upstream `atomr_agents::Agent` directly — this keeps the umbrella
loosely coupled to `atomr-agents` version churn. A thin adapter crate
that translates `ToolSpec` / `AgenticSession` into the upstream
`Tool` / agent loop typically fits in ~100 lines.

## InferBackend (single-shot, no agent loop)

When you don't need tools / planning / multi-turn, lift an
`atomr_infer::ModelRunner` directly through `InferBackend`:

```rust
use std::sync::Arc;
use atomr_ontology::infer_integration::{InferBackend, InferDriver};

let driver: Arc<dyn InferDriver> = make_my_driver(config);
let backend = Arc::new(InferBackend::new(driver));
let extractor = TermExtractor::new(backend);
```

`RuntimeConfig` is `serde::Deserialize`, so wiring it to a TOML or
environment-driven configuration is straightforward.

For local-only deployments, `provider-candle` and `provider-vllm` are
the smallest on-ramps. For remote, `provider-openai`,
`provider-anthropic`, and `provider-gemini` are the smallest on-ramp;
`provider-litellm` is the recommended way to reach Ollama from Rust.

## HTTP driver

> **Deprecated.** `HttpDriver` and the `http-driver` Cargo / pip
> feature are deprecated as of v0.2 and will be removed in v0.4. The
> canonical replacement is `InferBackend` wrapping the matching
> `atomr-infer` remote provider (`provider-openai`,
> `provider-anthropic`, `provider-gemini`, `provider-litellm`).
>
> **Migration:** drop the `http-driver` feature, enable the matching
> `provider-*` feature, and replace `HttpDriver::from_provider(...)`
> with the recommended `AgentBackend` wiring above. Existing
> `HttpDriver` callers keep working through the deprecation window;
> new code should not adopt it. A worked migration example lives in
> [`crates/atomr-ontology/examples/http_driver_migration.rs`](../crates/atomr-ontology/examples/http_driver_migration.rs).

For OpenAI Chat Completions, Anthropic Messages, and any
OpenAI-compatible / LiteLLM-proxied endpoint, the umbrella ships a
lightweight reqwest-based alternative gated behind `http-driver`:

| Provider name | Default base URL | API-key env var |
| --- | --- | --- |
| `openai` | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| `anthropic` | `https://api.anthropic.com/v1` | `ANTHROPIC_API_KEY` |
| `litellm` / `openai-compatible` | `http://localhost:4000` | `LITELLM_API_KEY` or `OPENAI_API_KEY` |

Override the base URL per-provider via `OPENAI_BASE_URL`,
`ANTHROPIC_BASE_URL`, `LITELLM_BASE_URL`. Construction emits a
`#[deprecated]` warning in Rust and a `DeprecationWarning` in Python.

## CI / hermetic testing

For tests and the default `auto_extract_from_text --provider mock`
example, use `atomr-ontology-testkit::MockBackend` directly. It
replays a queue of scripted JSON responses without touching the
network. The accompanying `MockRunner` from `atomr-infer-testkit` can
be used at the runner layer if you need to test driver code without a
real provider.
