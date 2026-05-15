//! PyO3 wrappers for `atomr-ontology-extract` — async extractors.
//!
//! All `extract`/`resolve` methods return Python coroutines via
//! `pyo3_async_runtimes::tokio`. The shared tokio runtime is
//! initialized lazily by `pyo3-async-runtimes` on first use.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyType};

use atomr_ontology::extract::{
    pipeline::ExtractStage, Backend, EntityCandidate, EntityResolver, Prompt, RecordExtractor,
    RelationCandidate, RelationExtractor, TermCandidate, TermExtractor,
};

use crate::core::{PyEdge, PyIri, PyNode, PyNodeId, PyRecord};
use crate::errors::backend_err;
use crate::provenance::PyActivity;
use crate::testkit::PyMockBackend;

// ============================================================================
// Backend handle — Python-facing newtype wrapping any Rust Backend.
// ============================================================================

/// Opaque handle to an inference backend.
///
/// Users construct concrete backends via the testkit (`MockBackend`)
/// or the optional `infer` extra. Extractors accept this handle and
/// dispatch through the underlying Rust trait object.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "Backend")]
#[derive(Clone)]
pub struct PyBackend {
    pub inner: Arc<dyn Backend>,
}

#[pymethods]
impl PyBackend {
    /// Human-readable label (used in provenance / tracing).
    #[getter]
    fn label(&self) -> String {
        self.inner.label().to_string()
    }

    fn __repr__(&self) -> String {
        format!("Backend(label={:?})", self.inner.label())
    }
}

impl From<Arc<dyn Backend>> for PyBackend {
    fn from(inner: Arc<dyn Backend>) -> Self {
        Self { inner }
    }
}

/// Coerce any Python backend-shaped object into an `Arc<dyn Backend>`.
/// Currently supports: `Backend`, `MockBackend`, and `InferBackend`
/// when the `infer` feature is enabled.
pub fn backend_from_py(value: &Bound<'_, PyAny>) -> PyResult<Arc<dyn Backend>> {
    if let Ok(b) = value.extract::<PyBackend>() {
        return Ok(b.inner);
    }
    if let Ok(m) = value.extract::<PyMockBackend>() {
        return Ok(m.inner_arc());
    }
    #[cfg(feature = "infer")]
    {
        use crate::infer::PyInferBackend;
        if let Ok(i) = value.extract::<PyInferBackend>() {
            return Ok(i.inner_arc());
        }
    }
    Err(PyTypeError::new_err(format!(
        "expected a Backend (MockBackend / InferBackend / Backend handle), got {}",
        value.get_type().name()?,
    )))
}

// ============================================================================
// Prompt
// ============================================================================

/// A prompt sent to a backend.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "Prompt")]
#[derive(Clone)]
pub struct PyPrompt {
    pub inner: Prompt,
}

#[pymethods]
impl PyPrompt {
    #[new]
    #[pyo3(signature = (user, system=None, max_tokens=None))]
    fn new(user: String, system: Option<String>, max_tokens: Option<u32>) -> Self {
        let mut p = Prompt::user(user);
        if let Some(s) = system {
            p = p.with_system(s);
        }
        if let Some(n) = max_tokens {
            p = p.with_max_tokens(n);
        }
        PyPrompt { inner: p }
    }

    #[classmethod]
    fn user(_cls: &Bound<'_, PyType>, body: String) -> Self {
        PyPrompt { inner: Prompt::user(body) }
    }

    fn with_system(mut slf: PyRefMut<'_, Self>, body: String) -> PyRefMut<'_, Self> {
        slf.inner.system = Some(body);
        slf
    }

    fn with_max_tokens(mut slf: PyRefMut<'_, Self>, n: u32) -> PyRefMut<'_, Self> {
        slf.inner.max_tokens = Some(n);
        slf
    }

    #[getter]
    fn user_(&self) -> &str {
        &self.inner.user
    }
    #[getter]
    fn system(&self) -> Option<&str> {
        self.inner.system.as_deref()
    }
    #[getter]
    fn max_tokens(&self) -> Option<u32> {
        self.inner.max_tokens
    }

    fn __repr__(&self) -> String {
        format!("Prompt(user={:?}, system={:?})", self.inner.user, self.inner.system)
    }
}

impl From<Prompt> for PyPrompt {
    fn from(inner: Prompt) -> Self {
        PyPrompt { inner }
    }
}

// ============================================================================
// Candidate types
// ============================================================================

/// A surface-term candidate.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "TermCandidate", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyTermCandidate {
    pub inner: TermCandidate,
}

#[pymethods]
impl PyTermCandidate {
    #[new]
    #[pyo3(signature = (surface, score, category=None, context=None))]
    fn new(surface: String, score: f32, category: Option<String>, context: Option<String>) -> Self {
        let mut t = TermCandidate::new(surface, score);
        if let Some(c) = category {
            t = t.with_category(c);
        }
        if let Some(c) = context {
            t = t.with_context(c);
        }
        Self { inner: t }
    }

    #[getter]
    fn surface(&self) -> &str {
        &self.inner.surface
    }
    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }
    #[getter]
    fn category(&self) -> Option<&str> {
        self.inner.category.as_deref()
    }
    #[getter]
    fn context(&self) -> Option<&str> {
        self.inner.context.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("TermCandidate(surface={:?}, score={})", self.inner.surface, self.inner.score)
    }
}

impl From<TermCandidate> for PyTermCandidate {
    fn from(inner: TermCandidate) -> Self {
        Self { inner }
    }
}

/// A resolved entity candidate.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "EntityCandidate", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyEntityCandidate {
    pub inner: EntityCandidate,
}

#[pymethods]
impl PyEntityCandidate {
    #[new]
    #[pyo3(signature = (surface, type_name, score, iri=None, is_new=true))]
    fn new(
        surface: String,
        type_name: String,
        score: f32,
        iri: Option<PyIri>,
        is_new: bool,
    ) -> Self {
        Self {
            inner: EntityCandidate {
                surface,
                iri: iri.map(|i| i.inner),
                type_name,
                score,
                is_new,
            },
        }
    }

    #[getter]
    fn surface(&self) -> &str {
        &self.inner.surface
    }
    #[getter]
    fn type_name(&self) -> &str {
        &self.inner.type_name
    }
    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }
    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }
    #[getter]
    fn is_new(&self) -> bool {
        self.inner.is_new
    }

    fn __repr__(&self) -> String {
        format!(
            "EntityCandidate(surface={:?}, type={:?}, is_new={})",
            self.inner.surface, self.inner.type_name, self.inner.is_new,
        )
    }
}

impl From<EntityCandidate> for PyEntityCandidate {
    fn from(inner: EntityCandidate) -> Self {
        Self { inner }
    }
}

/// A relation proposal between two entity surfaces.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "RelationCandidate", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRelationCandidate {
    pub inner: RelationCandidate,
}

#[pymethods]
impl PyRelationCandidate {
    #[new]
    fn new(source: String, label: String, target: String, score: f32) -> Self {
        Self { inner: RelationCandidate { source, label, target, score } }
    }
    #[getter]
    fn source(&self) -> &str {
        &self.inner.source
    }
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }
    #[getter]
    fn target(&self) -> &str {
        &self.inner.target
    }
    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }
    fn __repr__(&self) -> String {
        format!(
            "RelationCandidate({:?} -[{}]-> {:?})",
            self.inner.source, self.inner.label, self.inner.target,
        )
    }
}

impl From<RelationCandidate> for PyRelationCandidate {
    fn from(inner: RelationCandidate) -> Self {
        Self { inner }
    }
}

// ============================================================================
// ExtractStage
// ============================================================================

/// Stable labels for the seven pipeline stages.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "ExtractStage", eq, hash, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyExtractStage {
    Ingest,
    Terms,
    Entities,
    Concepts,
    Taxonomy,
    Relations,
    Commit,
}

#[pymethods]
impl PyExtractStage {
    fn label(&self) -> &'static str {
        ExtractStage::from(*self).label()
    }
    fn __repr__(&self) -> String {
        format!("ExtractStage.{:?}", self)
    }
}

impl From<PyExtractStage> for ExtractStage {
    fn from(s: PyExtractStage) -> Self {
        match s {
            PyExtractStage::Ingest => ExtractStage::Ingest,
            PyExtractStage::Terms => ExtractStage::Terms,
            PyExtractStage::Entities => ExtractStage::Entities,
            PyExtractStage::Concepts => ExtractStage::Concepts,
            PyExtractStage::Taxonomy => ExtractStage::Taxonomy,
            PyExtractStage::Relations => ExtractStage::Relations,
            PyExtractStage::Commit => ExtractStage::Commit,
        }
    }
}

// ============================================================================
// TermExtractor
// ============================================================================

/// Surface-term extractor.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "TermExtractor")]
#[derive(Clone)]
pub struct PyTermExtractor {
    inner: TermExtractor,
}

#[pymethods]
impl PyTermExtractor {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = backend_from_py(backend)?;
        Ok(Self { inner: TermExtractor::new(b) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Extract terms from text. Returns a coroutine resolving to
    /// `(terms: list[TermCandidate], activity: Activity)`.
    fn extract<'py>(&self, py: Python<'py>, text: String) -> PyResult<Bound<'py, PyAny>> {
        let extractor = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (terms, activity) = extractor.extract(&text).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for t in terms {
                    list.append(PyTermCandidate::from(t).into_py(py))?;
                }
                let act = PyActivity::from(activity).into_py(py);
                Ok::<PyObject, PyErr>((list.unbind(), act).into_py(py))
            })
        })
    }
}

// ============================================================================
// EntityResolver
// ============================================================================

/// Entity resolver — links surface terms to canonical entities.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "EntityResolver")]
#[derive(Clone)]
pub struct PyEntityResolver {
    inner: EntityResolver,
}

#[pymethods]
impl PyEntityResolver {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self { inner: EntityResolver::new(backend_from_py(backend)?) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Attach an `OntologyStore` to bias against duplicates.
    fn with_store(
        slf: PyRef<'_, Self>,
        store: PyRef<'_, crate::store::PyMemStore>,
    ) -> Self {
        Self { inner: slf.inner.clone().with_store(store.inner_arc()) }
    }

    /// Resolve a batch of `TermCandidate`s. Returns a coroutine
    /// resolving to `(entities, activity)`.
    fn resolve<'py>(
        &self,
        py: Python<'py>,
        terms: Vec<PyTermCandidate>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let resolver = self.inner.clone();
        let raw: Vec<TermCandidate> = terms.into_iter().map(|t| t.inner).collect();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (ents, activity) = resolver.resolve(&raw).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for e in ents {
                    list.append(PyEntityCandidate::from(e).into_py(py))?;
                }
                let act = PyActivity::from(activity).into_py(py);
                Ok::<PyObject, PyErr>((list.unbind(), act).into_py(py))
            })
        })
    }

    /// Promote candidates to `Node`s. When `iri_required` is True, candidates
    /// without an IRI are dropped.
    #[staticmethod]
    fn into_nodes(candidates: Vec<PyEntityCandidate>, iri_required: bool) -> Vec<PyNode> {
        let raw: Vec<EntityCandidate> = candidates.into_iter().map(|c| c.inner).collect();
        EntityResolver::into_nodes(&raw, iri_required).into_iter().map(PyNode::from).collect()
    }
}

// ============================================================================
// RelationExtractor
// ============================================================================

/// Relation extractor.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "RelationExtractor")]
#[derive(Clone)]
pub struct PyRelationExtractor {
    inner: RelationExtractor,
}

#[pymethods]
impl PyRelationExtractor {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self { inner: RelationExtractor::new(backend_from_py(backend)?) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Returns a coroutine resolving to `(relations, activity)`.
    fn extract<'py>(
        &self,
        py: Python<'py>,
        text: String,
        entities: Vec<PyEntityCandidate>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let extractor = self.inner.clone();
        let raw: Vec<EntityCandidate> = entities.into_iter().map(|c| c.inner).collect();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (rels, activity) = extractor.extract(&text, &raw).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for r in rels {
                    list.append(PyRelationCandidate::from(r).into_py(py))?;
                }
                let act = PyActivity::from(activity).into_py(py);
                Ok::<PyObject, PyErr>((list.unbind(), act).into_py(py))
            })
        })
    }

    /// Convert relations to edges given a surface→NodeId map.
    #[staticmethod]
    fn into_edges(
        relations: Vec<PyRelationCandidate>,
        surface_to_id: &Bound<'_, PyDict>,
    ) -> PyResult<Vec<PyEdge>> {
        let mut map: HashMap<String, atomr_ontology::core::NodeId> = HashMap::new();
        for (k, v) in surface_to_id.iter() {
            let key: String = k.extract()?;
            let value: PyNodeId = v.extract()?;
            map.insert(key, value.inner);
        }
        let raw: Vec<RelationCandidate> = relations.into_iter().map(|r| r.inner).collect();
        Ok(RelationExtractor::into_edges(&raw, &map).into_iter().map(PyEdge::from).collect())
    }
}

// ============================================================================
// RecordExtractor
// ============================================================================

/// Record extractor — turns a structured row into a `Record`.
#[pyclass(module = "atomr_ontology._atomr_ontology.extract", name = "RecordExtractor")]
#[derive(Clone)]
pub struct PyRecordExtractor {
    inner: RecordExtractor,
}

#[pymethods]
impl PyRecordExtractor {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self { inner: RecordExtractor::new(backend_from_py(backend)?) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Returns a coroutine resolving to `(Record, Activity)`.
    fn extract<'py>(&self, py: Python<'py>, row: String) -> PyResult<Bound<'py, PyAny>> {
        let extractor = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (record, activity) = extractor.extract(&row).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let rec = PyRecord::from(record).into_py(py);
                let act = PyActivity::from(activity).into_py(py);
                Ok::<PyObject, PyErr>((rec, act).into_py(py))
            })
        })
    }
}

// ============================================================================
// Helper: serde-JSON parse helpers (sync, no backend) — useful for tests
// ============================================================================

#[pyfunction]
fn parse_terms(json: &str) -> PyResult<Vec<PyTermCandidate>> {
    atomr_ontology::extract::terms::parse_terms(json)
        .map(|v| v.into_iter().map(PyTermCandidate::from).collect())
        .map_err(PyValueError::new_err)
}

#[pyfunction]
fn parse_entities(json: &str) -> PyResult<Vec<PyEntityCandidate>> {
    atomr_ontology::extract::entities::parse_entities(json)
        .map(|v| v.into_iter().map(PyEntityCandidate::from).collect())
        .map_err(PyValueError::new_err)
}

#[pyfunction]
fn parse_relations(json: &str) -> PyResult<Vec<PyRelationCandidate>> {
    atomr_ontology::extract::relations::parse_relations(json)
        .map(|v| v.into_iter().map(PyRelationCandidate::from).collect())
        .map_err(PyValueError::new_err)
}

// ============================================================================
// Module registration
// ============================================================================

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBackend>()?;
    m.add_class::<PyPrompt>()?;
    m.add_class::<PyTermCandidate>()?;
    m.add_class::<PyEntityCandidate>()?;
    m.add_class::<PyRelationCandidate>()?;
    m.add_class::<PyExtractStage>()?;
    m.add_class::<PyTermExtractor>()?;
    m.add_class::<PyEntityResolver>()?;
    m.add_class::<PyRelationExtractor>()?;
    m.add_class::<PyRecordExtractor>()?;
    m.add_function(wrap_pyfunction!(parse_terms, m)?)?;
    m.add_function(wrap_pyfunction!(parse_entities, m)?)?;
    m.add_function(wrap_pyfunction!(parse_relations, m)?)?;
    Ok(())
}
