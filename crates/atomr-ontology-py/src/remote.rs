//! PyO3 wrappers for `atomr-ontology-remote` — HTTP client + server.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use atomr_ontology_remote::RemoteClient as RustRemoteClient;

/// Thin client implementing `OntologyStore` over HTTP/JSON.
///
/// Only smoke-test methods are exposed for now; full async parity will
/// follow as users adopt the remote store in Python.
#[pyclass(module = "atomr_ontology._atomr_ontology.remote", name = "RemoteClient")]
#[derive(Clone)]
pub struct PyRemoteClient {
    inner: RustRemoteClient,
}

#[pymethods]
impl PyRemoteClient {
    /// Build a client pointing at the given base URL (e.g. `http://localhost:8080`).
    #[new]
    fn new(base_url: String) -> PyResult<Self> {
        let inner = RustRemoteClient::new(base_url).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Borrow the configured base URL.
    #[getter]
    fn base_url(&self) -> &str {
        self.inner.base_url()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRemoteClient>()?;
    Ok(())
}
