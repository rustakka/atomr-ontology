//! PyO3 wrappers for `atomr-ontology::infer_integration` — opt-in.
//!
//! Compiled only when at least one `provider-*` cargo feature is on.
//! Wraps the `InferBackend` adapter so the configured `atomr-infer`
//! runtime can drive any of the extractors.

use std::sync::Arc;

use pyo3::prelude::*;

use atomr_ontology::extract::backend::Backend;
use atomr_ontology::infer_integration::{InferBackend, InferDriver};

/// Inference backend wrapping an `atomr-infer` `ModelRunner`.
#[pyclass(module = "atomr_ontology._atomr_ontology.infer", name = "InferBackend")]
#[derive(Clone)]
pub struct PyInferBackend {
    inner: Arc<InferBackend>,
}

impl PyInferBackend {
    pub fn inner_arc(&self) -> Arc<dyn Backend> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyInferBackend {
    #[getter]
    fn label(&self) -> String {
        Backend::label(self.inner.as_ref()).to_string()
    }
}

/// Build an `InferBackend` from a Rust-side driver. Most users will
/// use the convenience builders in `atomr-infer` rather than this
/// raw constructor.
pub fn from_driver(driver: Arc<dyn InferDriver>) -> PyInferBackend {
    PyInferBackend { inner: Arc::new(InferBackend::new(driver)) }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInferBackend>()?;
    Ok(())
}
