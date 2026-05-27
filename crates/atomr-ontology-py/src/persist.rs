//! PyO3 wrappers for `atomr-ontology-persist`.

use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use atomr_ontology_persist::{
    Checkpointer, FileCheckpointer as RustFileCheckpointer, MemCheckpointer as RustMemCheckpointer,
    PersistentStore as RustPersistentStore,
};

/// In-memory checkpointer (for tests / ephemeral workflows).
#[pyclass(module = "atomr_ontology._atomr_ontology.persist", name = "MemCheckpointer")]
#[derive(Clone)]
pub struct PyMemCheckpointer {
    inner: RustMemCheckpointer,
}

#[pymethods]
impl PyMemCheckpointer {
    #[new]
    fn new() -> Self {
        Self { inner: RustMemCheckpointer::new() }
    }

    #[getter]
    fn label(&self) -> String {
        self.inner.label().to_string()
    }
}

/// JSON-file checkpointer.
#[pyclass(module = "atomr_ontology._atomr_ontology.persist", name = "FileCheckpointer")]
#[derive(Clone)]
pub struct PyFileCheckpointer {
    inner: RustFileCheckpointer,
}

#[pymethods]
impl PyFileCheckpointer {
    #[new]
    fn new(path: PathBuf) -> Self {
        Self { inner: RustFileCheckpointer::new(path) }
    }

    #[getter]
    fn label(&self) -> String {
        self.inner.label().to_string()
    }
}

/// Persistent OntologyStore backed by a pluggable checkpointer.
///
/// Construct via one of the `from_*` classmethods. Mutating operations
/// (upsert/commit) return Python coroutines.
#[pyclass(module = "atomr_ontology._atomr_ontology.persist", name = "PersistentStore")]
#[derive(Clone)]
pub struct PyPersistentStore {
    // Boxed trait object — concrete checkpointer chosen at construction time.
    inner_mem: Option<Arc<RustPersistentStore<RustMemCheckpointer>>>,
    inner_file: Option<Arc<RustPersistentStore<RustFileCheckpointer>>>,
}

#[pymethods]
impl PyPersistentStore {
    /// Build a persistent store on top of an in-memory checkpointer.
    #[classmethod]
    fn from_memory<'py>(
        _cls: &Bound<'_, pyo3::types::PyType>,
        py: Python<'py>,
        checkpointer: PyMemCheckpointer,
    ) -> PyResult<Bound<'py, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let store = RustPersistentStore::new(checkpointer.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(PyPersistentStore { inner_mem: Some(Arc::new(store)), inner_file: None })
        })
    }

    /// Build a persistent store on top of a file checkpointer.
    #[classmethod]
    fn from_file<'py>(
        _cls: &Bound<'_, pyo3::types::PyType>,
        py: Python<'py>,
        checkpointer: PyFileCheckpointer,
    ) -> PyResult<Bound<'py, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let store = RustPersistentStore::new(checkpointer.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(PyPersistentStore { inner_mem: None, inner_file: Some(Arc::new(store)) })
        })
    }

    /// Current snapshot version (monotonic).
    fn version(&self) -> u64 {
        if let Some(s) = &self.inner_mem {
            s.version()
        } else if let Some(s) = &self.inner_file {
            s.version()
        } else {
            0
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMemCheckpointer>()?;
    m.add_class::<PyFileCheckpointer>()?;
    m.add_class::<PyPersistentStore>()?;
    Ok(())
}
