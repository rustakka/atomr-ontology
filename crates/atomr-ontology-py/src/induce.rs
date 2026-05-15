//! PyO3 wrappers for `atomr-ontology-induce`.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use atomr_ontology::core::axiom::AxiomKind;
use atomr_ontology::core::NodeType;
use atomr_ontology::induce::axioms::AxiomProposal;
use atomr_ontology::induce::{
    AxiomMiner, ConceptCluster, ConceptFormer, SubclassProposal, TaxonomyInducer,
};

use crate::core::{PyAxiom, PyNodeType, PyProvenanceId};
use crate::errors::backend_err;
use crate::extract::{backend_from_py, PyTermCandidate};
use crate::provenance::PyActivity;

// ============================================================================
// SubclassProposal
// ============================================================================

/// A `(sub, sup)` subclass proposal.
#[pyclass(module = "atomr_ontology._atomr_ontology.induce", name = "SubclassProposal", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PySubclassProposal {
    pub inner: SubclassProposal,
}

#[pymethods]
impl PySubclassProposal {
    #[new]
    fn new(sub: String, sup: String, score: f32) -> Self {
        Self { inner: SubclassProposal { sub, sup, score } }
    }
    #[getter]
    fn sub(&self) -> &str {
        &self.inner.sub
    }
    #[getter]
    fn sup(&self) -> &str {
        &self.inner.sup
    }
    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }
    fn __repr__(&self) -> String {
        format!("SubclassProposal({:?} <: {:?}, score={})", self.inner.sub, self.inner.sup, self.inner.score)
    }
}

impl From<SubclassProposal> for PySubclassProposal {
    fn from(inner: SubclassProposal) -> Self {
        Self { inner }
    }
}

// ============================================================================
// ConceptCluster
// ============================================================================

/// A single cluster of synonymous surface forms.
#[pyclass(module = "atomr_ontology._atomr_ontology.induce", name = "ConceptCluster", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyConceptCluster {
    pub inner: ConceptCluster,
}

#[pymethods]
impl PyConceptCluster {
    #[new]
    #[pyo3(signature = (name, members, description=None, score=0.0))]
    fn new(name: String, members: Vec<String>, description: Option<String>, score: f32) -> Self {
        Self { inner: ConceptCluster { name, members, description, score } }
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn members(&self) -> Vec<String> {
        self.inner.members.clone()
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }
    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }

    /// Convert to a `NodeType`.
    fn into_node_type(slf: PyRef<'_, Self>) -> PyNodeType {
        PyNodeType::from(slf.inner.clone().into_node_type())
    }

    fn __repr__(&self) -> String {
        format!("ConceptCluster(name={:?}, members={:?})", self.inner.name, self.inner.members)
    }
}

impl From<ConceptCluster> for PyConceptCluster {
    fn from(inner: ConceptCluster) -> Self {
        Self { inner }
    }
}

// ============================================================================
// AxiomProposal
// ============================================================================

/// A raw axiom proposal — `kind` is one of the snake_case discriminants.
#[pyclass(module = "atomr_ontology._atomr_ontology.induce", name = "AxiomProposal", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAxiomProposal {
    pub inner: AxiomProposal,
}

#[pymethods]
impl PyAxiomProposal {
    /// Build from a Python dict like
    /// `{"kind": "sub_class_of", "sub": "A", "sup": "B", "score": 0.9}`.
    #[classmethod]
    fn from_dict(_cls: &Bound<'_, pyo3::types::PyType>, value: &Bound<'_, PyDict>) -> PyResult<Self> {
        let s: String = pyo3::Python::with_gil(|py| -> PyResult<String> {
            let json = py.import_bound("json")?;
            let s: String = json.call_method1("dumps", (value,))?.extract()?;
            Ok(s)
        })?;
        let inner: AxiomProposal =
            serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Tag of the variant.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            AxiomProposal::SubClassOf { .. } => "sub_class_of",
            AxiomProposal::EquivalentClass { .. } => "equivalent_class",
            AxiomProposal::DisjointWith { .. } => "disjoint_with",
            AxiomProposal::Domain { .. } => "domain",
            AxiomProposal::Range { .. } => "range",
            AxiomProposal::Functional { .. } => "functional",
            AxiomProposal::InverseFunctional { .. } => "inverse_functional",
            AxiomProposal::InverseOf { .. } => "inverse_of",
            AxiomProposal::Symmetric { .. } => "symmetric",
            AxiomProposal::Transitive { .. } => "transitive",
        }
    }

    /// Score in [0, 1].
    #[getter]
    fn score(&self) -> f32 {
        match self.inner {
            AxiomProposal::SubClassOf { score, .. }
            | AxiomProposal::EquivalentClass { score, .. }
            | AxiomProposal::DisjointWith { score, .. }
            | AxiomProposal::Domain { score, .. }
            | AxiomProposal::Range { score, .. }
            | AxiomProposal::Functional { score, .. }
            | AxiomProposal::InverseFunctional { score, .. }
            | AxiomProposal::InverseOf { score, .. }
            | AxiomProposal::Symmetric { score, .. }
            | AxiomProposal::Transitive { score, .. } => score,
        }
    }

    /// Operands as a dict.
    fn operands<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new_bound(py);
        match &self.inner {
            AxiomProposal::SubClassOf { sub, sup, .. } => {
                d.set_item("sub", sub)?;
                d.set_item("sup", sup)?;
            }
            AxiomProposal::EquivalentClass { left, right, .. }
            | AxiomProposal::DisjointWith { left, right, .. }
            | AxiomProposal::InverseOf { left, right, .. } => {
                d.set_item("left", left)?;
                d.set_item("right", right)?;
            }
            AxiomProposal::Domain { property, class, .. }
            | AxiomProposal::Range { property, class, .. } => {
                d.set_item("property", property)?;
                d.set_item("class", class)?;
            }
            AxiomProposal::Functional { property, .. }
            | AxiomProposal::InverseFunctional { property, .. }
            | AxiomProposal::Symmetric { property, .. }
            | AxiomProposal::Transitive { property, .. } => {
                d.set_item("property", property)?;
            }
        }
        Ok(d)
    }

    /// Promote to a canonical `Axiom`.
    fn into_axiom(slf: PyRef<'_, Self>) -> PyAxiom {
        PyAxiom::from(slf.inner.clone().into_axiom())
    }

    fn __repr__(&self) -> String {
        format!("AxiomProposal({:?})", self.inner)
    }
}

impl From<AxiomProposal> for PyAxiomProposal {
    fn from(inner: AxiomProposal) -> Self {
        Self { inner }
    }
}

// ============================================================================
// TaxonomyInducer
// ============================================================================

/// LLM-driven taxonomy inducer.
#[pyclass(module = "atomr_ontology._atomr_ontology.induce", name = "TaxonomyInducer")]
#[derive(Clone)]
pub struct PyTaxonomyInducer {
    inner: TaxonomyInducer,
}

#[pymethods]
impl PyTaxonomyInducer {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self { inner: TaxonomyInducer::new(backend_from_py(backend)?) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Returns a coroutine resolving to `(proposals, activity)`.
    fn induce<'py>(
        &self,
        py: Python<'py>,
        candidate_classes: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inducer = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (props, activity) = inducer.induce(&candidate_classes).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for p in props {
                    list.append(PySubclassProposal::from(p).into_py(py))?;
                }
                Ok::<PyObject, PyErr>((list.unbind(), PyActivity::from(activity).into_py(py)).into_py(py))
            })
        })
    }

    /// Promote proposals to `Axiom`s, optionally tagging each with a provenance id.
    #[staticmethod]
    #[pyo3(signature = (proposals, provenance=None))]
    fn into_axioms(proposals: Vec<PySubclassProposal>, provenance: Option<PyProvenanceId>) -> Vec<PyAxiom> {
        let raw: Vec<SubclassProposal> = proposals.into_iter().map(|p| p.inner).collect();
        let prov = provenance.map(|p| p.inner);
        TaxonomyInducer::into_axioms(&raw, prov).into_iter().map(PyAxiom::from).collect()
    }
}

// ============================================================================
// ConceptFormer
// ============================================================================

/// LLM-driven concept former.
#[pyclass(module = "atomr_ontology._atomr_ontology.induce", name = "ConceptFormer")]
#[derive(Clone)]
pub struct PyConceptFormer {
    inner: ConceptFormer,
}

#[pymethods]
impl PyConceptFormer {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self { inner: ConceptFormer::new(backend_from_py(backend)?) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Returns a coroutine resolving to `(clusters, activity)`.
    fn cluster<'py>(
        &self,
        py: Python<'py>,
        terms: Vec<PyTermCandidate>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let former = self.inner.clone();
        let raw: Vec<atomr_ontology::extract::TermCandidate> =
            terms.into_iter().map(|t| t.inner).collect();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (cs, activity) = former.cluster(&raw).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for c in cs {
                    list.append(PyConceptCluster::from(c).into_py(py))?;
                }
                Ok::<PyObject, PyErr>((list.unbind(), PyActivity::from(activity).into_py(py)).into_py(py))
            })
        })
    }
}

// ============================================================================
// AxiomMiner
// ============================================================================

/// LLM-driven axiom miner.
#[pyclass(module = "atomr_ontology._atomr_ontology.induce", name = "AxiomMiner")]
#[derive(Clone)]
pub struct PyAxiomMiner {
    inner: AxiomMiner,
}

#[pymethods]
impl PyAxiomMiner {
    #[new]
    fn new(backend: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self { inner: AxiomMiner::new(backend_from_py(backend)?) })
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    /// Returns a coroutine resolving to `(proposals, activity)`.
    fn mine<'py>(&self, py: Python<'py>, context: String) -> PyResult<Bound<'py, PyAny>> {
        let miner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (ps, activity) = miner.mine(&context).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for p in ps {
                    list.append(PyAxiomProposal::from(p).into_py(py))?;
                }
                Ok::<PyObject, PyErr>((list.unbind(), PyActivity::from(activity).into_py(py)).into_py(py))
            })
        })
    }
}

// Silence unused imports.
#[allow(dead_code)]
fn _unused(_: NodeType, _: AxiomKind) -> PyErr {
    PyTypeError::new_err("never")
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySubclassProposal>()?;
    m.add_class::<PyConceptCluster>()?;
    m.add_class::<PyAxiomProposal>()?;
    m.add_class::<PyTaxonomyInducer>()?;
    m.add_class::<PyConceptFormer>()?;
    m.add_class::<PyAxiomMiner>()?;
    Ok(())
}
