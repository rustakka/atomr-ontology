//! PyO3 wrappers for `atomr-ontology-testkit`.

use std::sync::Arc;

use pyo3::exceptions::PyAssertionError;
use pyo3::prelude::*;
use pyo3::types::PyType;

use atomr_ontology::core::axiom::AxiomKind;
use atomr_ontology::extract::backend::Backend;
use atomr_ontology::testkit::{
    assert_axiom_present as rust_assert_axiom_present, assert_subclass_of as rust_assert_subclass_of,
    toy_corpus as rust_toy_corpus, toy_org_ontology as rust_toy_org_ontology, MockBackend,
};

use crate::core::PyOntology;
use crate::extract::PyPrompt;

/// A deterministic [`Backend`] that replays a queue of pre-scripted
/// responses. Use this in tests and golden-output examples.
#[pyclass(module = "atomr_ontology._atomr_ontology.testkit", name = "MockBackend")]
#[derive(Clone)]
pub struct PyMockBackend {
    inner: MockBackend,
}

impl PyMockBackend {
    pub fn inner_arc(&self) -> Arc<dyn Backend> {
        Arc::new(self.inner.clone())
    }
}

#[pymethods]
impl PyMockBackend {
    #[new]
    fn new() -> Self {
        Self { inner: MockBackend::new() }
    }

    #[classmethod]
    fn with_label(_cls: &Bound<'_, PyType>, label: &str) -> Self {
        Self { inner: MockBackend::with_label(label.to_string()) }
    }

    /// Push a text response onto the queue. Returns self for chaining.
    fn enqueue(slf: PyRef<'_, Self>, response: &str) -> Self {
        slf.inner.enqueue(response.to_string());
        Self { inner: slf.inner.clone() }
    }

    /// Push a JSON-serializable Python object as the next response.
    fn enqueue_json(slf: PyRef<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let json = slf.py().import_bound("json")?;
        let s: String = json.call_method1("dumps", (value,))?.extract()?;
        slf.inner.enqueue(s);
        Ok(Self { inner: slf.inner.clone() })
    }

    /// Inspect prompts the mock has seen so far.
    fn captured(&self) -> Vec<PyPrompt> {
        self.inner.captured().into_iter().map(PyPrompt::from).collect()
    }

    /// Backend label.
    #[getter]
    fn label(&self) -> String {
        Backend::label(&self.inner).to_string()
    }

    fn __repr__(&self) -> String {
        format!("MockBackend(label={:?})", Backend::label(&self.inner))
    }
}

/// Three-sentence canonical organizational seed corpus.
#[pyfunction]
fn toy_corpus() -> Vec<&'static str> {
    rust_toy_corpus()
}

/// A small golden W3C-Org-style ontology (3 nodes, 2 edges).
#[pyfunction]
fn toy_org_ontology() -> PyOntology {
    PyOntology::from(rust_toy_org_ontology())
}

/// Raise `AssertionError` unless an axiom matching `tag` exists in
/// the ontology. `tag` is one of the snake-case axiom kinds.
#[pyfunction]
#[pyo3(signature = (ontology, tag, message=None))]
fn assert_axiom_present(ontology: &PyOntology, tag: &str, message: Option<&str>) -> PyResult<()> {
    let snapshot = ontology.inner.clone();
    let tag_owned = tag.to_string();
    let msg = message.unwrap_or("axiom not present");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rust_assert_axiom_present(
            &snapshot,
            |kind: &AxiomKind| axiom_kind_tag(kind) == tag_owned,
            msg,
        );
    }));
    result.map_err(|_| PyAssertionError::new_err(msg.to_string()))?;
    Ok(())
}

fn axiom_kind_tag(kind: &AxiomKind) -> &'static str {
    match kind {
        AxiomKind::SubClassOf { .. } => "sub_class_of",
        AxiomKind::EquivalentClass { .. } => "equivalent_class",
        AxiomKind::DisjointWith { .. } => "disjoint_with",
        AxiomKind::Domain { .. } => "domain",
        AxiomKind::Range { .. } => "range",
        AxiomKind::Functional { .. } => "functional",
        AxiomKind::InverseFunctional { .. } => "inverse_functional",
        AxiomKind::InverseOf { .. } => "inverse_of",
        AxiomKind::Symmetric { .. } => "symmetric",
        AxiomKind::Transitive { .. } => "transitive",
    }
}

/// Raise `AssertionError` unless `sub` is a transitive subclass of `sup`.
#[pyfunction]
fn assert_subclass_of(ontology: &PyOntology, sub: &str, sup: &str) -> PyResult<()> {
    // `rust_assert_subclass_of` panics; catch the panic to convert into
    // a Python AssertionError.
    let snapshot = ontology.inner.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rust_assert_subclass_of(&snapshot, sub, sup);
    }));
    result.map_err(|_| {
        PyAssertionError::new_err(format!("expected `{sub}` to be a subclass of `{sup}`"))
    })?;
    Ok(())
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMockBackend>()?;
    m.add_function(wrap_pyfunction!(toy_corpus, m)?)?;
    m.add_function(wrap_pyfunction!(toy_org_ontology, m)?)?;
    m.add_function(wrap_pyfunction!(assert_axiom_present, m)?)?;
    m.add_function(wrap_pyfunction!(assert_subclass_of, m)?)?;
    Ok(())
}
