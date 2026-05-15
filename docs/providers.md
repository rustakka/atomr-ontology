# Providers

`atomr-ontology` does not hard-code any LLM provider. Instead it
consumes the provider abstraction from
[`atomr-infer`](https://github.com/rustakka/atomr-infer) through a
narrow [`Backend`](agents.md#backend-contract) adapter.

## Feature flags

Enable a provider by turning on the matching cargo feature on the
umbrella crate:

| Feature | Enables |
| --- | --- |
| `provider-openai` | `atomr-infer/openai` (OpenAI / Azure OpenAI) |
| `provider-anthropic` | `atomr-infer/anthropic` |
| `provider-gemini` | `atomr-infer/gemini` |
| `provider-litellm` | `atomr-infer/litellm` (Ollama via LiteLLM proxy) |
| `provider-vllm` | `atomr-infer/vllm` |
| `provider-candle` | `atomr-infer/candle` |
| `provider-ort` | `atomr-infer/ort` |
| `provider-tensorrt` | `atomr-infer/tensorrt` |
| `provider-mistralrs` | `atomr-infer/mistralrs` |
| `provider-cudarc` | `atomr-infer/cudarc` |

Each flag pulls in `atomr-infer` and the matching runtime crate.
Multiple flags can be combined; the choice between local and
remote is made at construction time by passing the appropriate
`RuntimeConfig` to your driver implementation.

## Wiring a driver

The `InferBackend` in `atomr-ontology::infer_integration` expects
an `Arc<dyn InferDriver>`. A minimal driver wraps an
`atomr_infer::ModelRunner` behind a mutex and dispatches a single
streaming completion per `complete` call. The umbrella crate
ships the trait; the implementation lives in user code so that
each application can choose its own batching / streaming /
caching policy.

```rust
use std::sync::Arc;
use atomr_ontology::infer_integration::{InferBackend, InferDriver};

let driver: Arc<dyn InferDriver> = make_my_driver(config);
let backend = Arc::new(InferBackend::new(driver));
let extractor = TermExtractor::new(backend);
```

For local-only deployments, the recommended path is
`atomr_infer/candle` or `atomr_infer/vllm` (the latter through
the Python bridge). For remote, `provider-openai`,
`provider-anthropic`, and `provider-gemini` are the smallest
on-ramp. LiteLLM (`provider-litellm`) is the recommended way to
reach Ollama from Rust.

## Switching providers at runtime

`RuntimeConfig` is `serde::Deserialize`, so wiring it to a TOML
or environment-driven configuration is straightforward:

```rust
let config: atomr_infer::RuntimeConfig = serde_json::from_str(&body)?;
let runner = build_runner_from_config(config)?;   // user code
let backend = Arc::new(InferBackend::new(Arc::new(MyDriver::new(runner))));
```

The runner choice does not change the extractor surface — the
same `TermExtractor` / `EntityResolver` / `RelationExtractor`
works across every provider.

## CI / hermetic testing

For tests and the default `auto_extract_from_text --provider mock`
example, use `atomr-ontology-testkit::MockBackend` directly. It
replays a queue of scripted JSON responses without touching the
network. The accompanying `MockRunner` from
`atomr-infer-testkit` can be used at the runner layer if you need
to test driver code without a real provider.
