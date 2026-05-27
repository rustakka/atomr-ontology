//! PyO3 wrapper for the deprecated [`atomr_ontology::http_driver::HttpDriver`].
//!
//! Compiled only when the `http-driver` cargo feature is on. Constructing
//! a `HttpDriver` from Python emits a `DeprecationWarning` pointing at the
//! canonical replacement (`provider-*` extras + `InferBackend` /
//! `AgentBackend`).

#![allow(deprecated)]

use std::sync::Arc;

use pyo3::exceptions::PyDeprecationWarning;
use pyo3::prelude::*;

use atomr_ontology::extract::backend::Backend;
use atomr_ontology::http_driver::HttpDriver;

use crate::errors::backend_err;
use crate::extract::PyBackend;

/// HTTP-based [`Backend`] for OpenAI / Anthropic / LiteLLM endpoints.
///
/// **Deprecated.** Prefer the `[openai]` / `[anthropic]` / `[litellm]`
/// pip extras (which install the matching `atomr-infer` provider) plus
/// `atomr_ontology.infer.InferBackend`, or — for the recommended
/// agentic layering — the `[agents-with-anthropic]` extra plus
/// `atomr_ontology.agents.AgenticAgent`. See `docs/providers.md`.
#[pyclass(
    module = "atomr_ontology._atomr_ontology.http_driver",
    name = "HttpDriver"
)]
#[derive(Clone)]
pub struct PyHttpDriver {
    inner: Arc<HttpDriver>,
}

#[pymethods]
impl PyHttpDriver {
    /// Build a driver for the named provider + model.
    ///
    /// Recognized providers: `openai`, `anthropic`, `litellm`,
    /// `openai-compatible`. API keys are read from environment
    /// variables (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`,
    /// `LITELLM_API_KEY`).
    ///
    /// Emits a `DeprecationWarning` on construction; the underlying
    /// driver still works through the deprecation window.
    #[new]
    fn new(py: Python<'_>, provider: &str, model: &str) -> PyResult<Self> {
        PyErr::warn_bound(
            py,
            &py.get_type_bound::<PyDeprecationWarning>(),
            "atomr_ontology.http_driver.HttpDriver is deprecated and will be removed in 0.4. \
             Install the matching provider extra ([openai], [anthropic], [litellm], …) and use \
             atomr_ontology.infer.InferBackend, or for the recommended agentic layering use \
             atomr_ontology.agents.AgenticAgent. See docs/providers.md for the migration guide.",
            1,
        )?;
        let d = HttpDriver::from_provider(provider, model).map_err(backend_err)?;
        Ok(Self { inner: Arc::new(d) })
    }

    /// Return a generic `Backend` handle that extractors accept.
    fn as_backend(&self) -> PyBackend {
        PyBackend { inner: self.inner.clone() as Arc<dyn Backend> }
    }

    #[getter]
    fn label(&self) -> String {
        Backend::label(self.inner.as_ref()).to_string()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHttpDriver>()?;
    Ok(())
}
