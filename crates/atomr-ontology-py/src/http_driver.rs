//! PyO3 wrapper for the HTTP-based [`atomr_ontology::http_driver::HttpDriver`].
//!
//! Compiled only when the `http-driver` cargo feature is on. Exposes a
//! `HttpDriver(provider: str, model: str)` constructor that produces a
//! `Backend` handle usable by all extractors.

use std::sync::Arc;

use pyo3::prelude::*;

use atomr_ontology::extract::backend::Backend;
use atomr_ontology::http_driver::HttpDriver;

use crate::errors::backend_err;
use crate::extract::PyBackend;

/// HTTP-based [`Backend`] for OpenAI / Anthropic / LiteLLM endpoints.
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
    #[new]
    fn new(provider: &str, model: &str) -> PyResult<Self> {
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
