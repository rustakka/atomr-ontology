//! HTTP-based [`Backend`] implementations for OpenAI / Anthropic /
//! LiteLLM / OpenAI-compatible proxies.
//!
//! # Deprecated — prefer `atomr-infer` providers
//!
//! As of v0.2 this module is **deprecated** and slated for removal in
//! v0.4. The canonical replacement is the `atomr-infer` provider matrix
//! wired through [`InferBackend`](crate::infer_integration::InferBackend)
//! or — for the recommended layering —
//! [`AgentBackend`](crate::agents_integration::AgentBackend) wrapping
//! an `atomr_agents::Agent` that itself dispatches to an `atomr-infer`
//! provider.
//!
//! Migration sketch (replace `http-driver` with `provider-openai`):
//!
//! ```toml
//! # Before
//! atomr-ontology = { version = "0.2", features = ["http-driver"] }
//!
//! # After (recommended)
//! atomr-ontology = { version = "0.2", features = ["agents-with-openai"] }
//! # Or, no agent loop:
//! atomr-ontology = { version = "0.2", features = ["provider-openai"] }
//! ```
//!
//! A worked migration example lives in
//! `examples/http_driver_migration.rs`. See `docs/providers.md` for the
//! full decision tree.
//!
//! ---
//!
//! The legacy behaviour: this driver speaks the chat-completions shape
//! that all three providers expose (OpenAI's Chat Completions API and
//! Anthropic's Messages API), reads API keys from the standard
//! environment variables (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`,
//! `LITELLM_API_KEY` / `OPENAI_API_KEY`), and returns the assistant
//! message text. It does not load weights, host models, or stream.
//!
//! Concrete provider mapping:
//!
//! | provider     | base URL                                  | env var               |
//! |--------------|-------------------------------------------|-----------------------|
//! | `openai`     | https://api.openai.com/v1                 | `OPENAI_API_KEY`      |
//! | `anthropic`  | https://api.anthropic.com/v1              | `ANTHROPIC_API_KEY`   |
//! | `litellm`    | $LITELLM_BASE_URL or http://localhost:4000 | `LITELLM_API_KEY`    |
//! | `openai-compatible` | $OPENAI_BASE_URL                   | `OPENAI_API_KEY`      |

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use atomr_ontology_extract::backend::{Backend, BackendError, Prompt};

/// Provider flavor — picks the wire shape used.
#[deprecated(
    since = "0.2.0",
    note = "http_driver is deprecated and will be removed in 0.4. Use the `provider-openai` / \
            `provider-anthropic` / `provider-litellm` features and construct an InferBackend via \
            `atomr_ontology::infer_integration`, or wire `AgentBackend` over an \
            `atomr_agents::Agent` for the recommended layering. See \
            examples/http_driver_migration.rs and docs/providers.md."
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flavor {
    /// OpenAI Chat Completions API.
    OpenAi,
    /// Anthropic Messages API.
    Anthropic,
    /// LiteLLM / OpenAI-compatible proxy.
    Compatible,
}

/// HTTP-based driver implementing [`Backend`].
#[deprecated(
    since = "0.2.0",
    note = "http_driver is deprecated and will be removed in 0.4. Use the `provider-openai` / \
            `provider-anthropic` / `provider-litellm` features and construct an InferBackend via \
            `atomr_ontology::infer_integration`, or wire `AgentBackend` over an \
            `atomr_agents::Agent` for the recommended layering. See \
            examples/http_driver_migration.rs and docs/providers.md."
)]
#[allow(deprecated)]
pub struct HttpDriver {
    flavor: Flavor,
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    label: String,
}

#[allow(deprecated)]
impl HttpDriver {
    /// Build a driver for the given provider name and model.
    ///
    /// Recognized provider names: `openai`, `anthropic`, `litellm`,
    /// `openai-compatible`. Other names produce an error.
    #[deprecated(
        since = "0.2.0",
        note = "http_driver is deprecated and will be removed in 0.4. Prefer \
                `InferBackend` over `atomr_infer` (feature `provider-openai`, \
                `provider-anthropic`, …) or `AgentBackend` for the recommended \
                agentic layering. See examples/http_driver_migration.rs."
    )]
    pub fn from_provider(provider: &str, model: &str) -> Result<Self, BackendError> {
        let flavor = match provider {
            "openai" => Flavor::OpenAi,
            "anthropic" => Flavor::Anthropic,
            "litellm" | "openai-compatible" => Flavor::Compatible,
            other => {
                return Err(BackendError::Other(format!(
                    "http-driver: unknown provider {other:?} (try openai/anthropic/litellm)",
                )))
            }
        };
        let (base_url, api_key) = match flavor {
            Flavor::OpenAi => (
                std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
                std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            ),
            Flavor::Anthropic => (
                std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string()),
                std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            ),
            Flavor::Compatible => (
                std::env::var("LITELLM_BASE_URL")
                    .or_else(|_| std::env::var("OPENAI_BASE_URL"))
                    .unwrap_or_else(|_| "http://localhost:4000".to_string()),
                std::env::var("LITELLM_API_KEY")
                    .or_else(|_| std::env::var("OPENAI_API_KEY"))
                    .unwrap_or_default(),
            ),
        };
        let client = Client::builder()
            .build()
            .map_err(|e| BackendError::Transport(format!("build reqwest client: {e}")))?;
        Ok(Self {
            flavor,
            client,
            base_url,
            api_key,
            model: model.to_string(),
            label: format!("http:{provider}"),
        })
    }

    async fn call_openai_chat(&self, prompt: &Prompt) -> Result<String, BackendError> {
        let mut messages = Vec::new();
        if let Some(sys) = &prompt.system {
            messages.push(ChatMessage { role: "system", content: sys.clone() });
        }
        messages.push(ChatMessage { role: "user", content: prompt.user.clone() });
        let body = ChatRequest {
            model: &self.model,
            messages: &messages,
            max_tokens: prompt.max_tokens.map(|n| n as usize),
            temperature: Some(0.0),
        };
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp: ChatResponse = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Transport(e.to_string()))?
            .error_for_status()
            .map_err(|e| BackendError::Transport(e.to_string()))?
            .json()
            .await
            .map_err(|e| BackendError::Parse(e.to_string()))?;
        Ok(resp.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default())
    }

    async fn call_anthropic_messages(&self, prompt: &Prompt) -> Result<String, BackendError> {
        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        let body = AnthropicRequest {
            model: &self.model,
            max_tokens: prompt.max_tokens.unwrap_or(4096) as usize,
            system: prompt.system.clone(),
            messages: vec![AnthropicMessage { role: "user", content: prompt.user.clone() }],
        };
        let resp: AnthropicResponse = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Transport(e.to_string()))?
            .error_for_status()
            .map_err(|e| BackendError::Transport(e.to_string()))?
            .json()
            .await
            .map_err(|e| BackendError::Parse(e.to_string()))?;
        Ok(resp.content.into_iter().filter_map(|c| c.text).collect::<Vec<_>>().join(""))
    }
}

#[allow(deprecated)]
#[async_trait]
impl Backend for HttpDriver {
    async fn complete(&self, prompt: Prompt) -> Result<String, BackendError> {
        match self.flavor {
            Flavor::OpenAi | Flavor::Compatible => self.call_openai_chat(&prompt).await,
            Flavor::Anthropic => self.call_anthropic_messages(&prompt).await,
        }
    }

    fn label(&self) -> &str {
        &self.label
    }
}

// --- OpenAI Chat Completions wire types ---

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage<'a>],
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

// --- Anthropic Messages wire types ---

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn unknown_provider_errors() {
        let err = HttpDriver::from_provider("nope", "gpt-x");
        assert!(err.is_err());
        let s = format!("{}", err.err().unwrap());
        assert!(s.contains("unknown provider"));
    }

    #[test]
    fn openai_label_set() {
        let d = HttpDriver::from_provider("openai", "gpt-4o-mini").expect("openai construct");
        assert_eq!(d.label(), "http:openai");
    }
}
