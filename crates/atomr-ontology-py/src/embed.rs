//! PyO3 wrappers for `atomr-ontology-embed`.

use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use atomr_ontology_embed::{
    EmbeddingBackend, EmbeddingResolver as RustEmbeddingResolver, HashEmbedder as RustHashEmbedder,
    VectorIndex as RustVectorIndex, VectorRecord as RustVectorRecord,
};

use crate::core::PyOntology;

/// Deterministic hash-based embedder for testing.
#[pyclass(module = "atomr_ontology._atomr_ontology.embed", name = "HashEmbedder")]
#[derive(Clone)]
pub struct PyHashEmbedder {
    inner: Arc<RustHashEmbedder>,
}

#[pymethods]
impl PyHashEmbedder {
    #[new]
    fn new(dim: usize) -> Self {
        Self { inner: Arc::new(RustHashEmbedder::new(dim)) }
    }

    /// Dimensionality of produced vectors.
    #[getter]
    fn dimensions(&self) -> usize {
        EmbeddingBackend::dimensions(self.inner.as_ref())
    }

    /// Stable label.
    #[getter]
    fn label(&self) -> String {
        EmbeddingBackend::label(self.inner.as_ref()).to_string()
    }

    /// Embed a single string (synchronous — hash embedder doesn't await).
    fn embed<'py>(&self, py: Python<'py>, text: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            inner.embed(&text).await.map_err(|e| PyRuntimeError::new_err(e.to_string()))
        })
    }
}

/// In-memory vector index record.
#[pyclass(module = "atomr_ontology._atomr_ontology.embed", name = "VectorRecord")]
#[derive(Clone)]
pub struct PyVectorRecord {
    pub inner: RustVectorRecord,
}

#[pymethods]
impl PyVectorRecord {
    #[new]
    fn new(iri: String, vector: Vec<f32>) -> Self {
        Self { inner: RustVectorRecord::new(iri, vector) }
    }

    #[getter]
    fn iri(&self) -> &str {
        &self.inner.iri
    }

    #[getter]
    fn vector(&self) -> Vec<f32> {
        self.inner.vector.clone()
    }
}

/// Linear-scan vector index over node IRIs.
#[pyclass(module = "atomr_ontology._atomr_ontology.embed", name = "VectorIndex")]
pub struct PyVectorIndex {
    inner: RustVectorIndex,
}

#[pymethods]
impl PyVectorIndex {
    #[new]
    fn new() -> Self {
        Self { inner: RustVectorIndex::new() }
    }

    #[classmethod]
    fn with_dimensions(_cls: &Bound<'_, pyo3::types::PyType>, dim: usize) -> Self {
        Self { inner: RustVectorIndex::with_dimensions(dim) }
    }

    fn insert(&mut self, record: PyVectorRecord) -> PyResult<()> {
        self.inner.insert(record.inner).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn search(&self, query: Vec<f32>, top_k: usize) -> Vec<(String, f32)> {
        self.inner
            .search(&query, top_k)
            .into_iter()
            .map(|(rec, score)| (rec.iri.clone(), score))
            .collect()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn clear(&mut self) {
        self.inner.clear();
    }
}

/// Entity-resolution pre-filter combining an embedder + vector index.
#[pyclass(module = "atomr_ontology._atomr_ontology.embed", name = "EmbeddingResolver")]
pub struct PyEmbeddingResolver {
    inner: Arc<RustEmbeddingResolver>,
}

#[pymethods]
impl PyEmbeddingResolver {
    #[new]
    fn new(embedder: PyHashEmbedder) -> Self {
        Self { inner: Arc::new(RustEmbeddingResolver::new(embedder.inner)) }
    }

    /// Embed all named nodes from an ontology into the index.
    fn ingest_ontology<'py>(
        &self,
        py: Python<'py>,
        ontology: PyOntology,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            inner
                .ingest_ontology(&ontology.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))
        })
    }

    /// Propose top-k IRIs whose name is most similar to `surface`.
    fn propose<'py>(&self, py: Python<'py>, surface: String, top_k: usize) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            inner
                .propose(&surface, top_k)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))
        })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHashEmbedder>()?;
    m.add_class::<PyVectorRecord>()?;
    m.add_class::<PyVectorIndex>()?;
    m.add_class::<PyEmbeddingResolver>()?;
    Ok(())
}
