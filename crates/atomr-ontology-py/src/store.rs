//! PyO3 wrappers for `atomr-ontology-store` — async OntologyStore.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use atomr_ontology::core::{Edge, EdgeId, Node, NodeId, Ontology};
use atomr_ontology::store::{
    EdgePattern, MatchRow, MemStore, NodePattern, OntologyDelta, OntologyStore, StoreDiff,
    TraversalPlan, TraversalStep,
};

use crate::core::{
    py_to_property_value, PyAxiom, PyEdge, PyEdgeId, PyNode, PyNodeId, PyOntology,
};
use crate::errors::store_err;
use crate::provenance::{PyActivity, PyProvenanceLog};
use crate::core::PyProvenanceId;

// ============================================================================
// Patterns
// ============================================================================

/// Single-node pattern.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "NodePattern")]
#[derive(Clone, Default)]
pub struct PyNodePattern {
    pub inner: NodePattern,
}

#[pymethods]
impl PyNodePattern {
    #[classmethod]
    fn any(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::default()
    }
    fn bind(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.inner.bind = Some(name.to_string());
        slf
    }
    fn typed(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.inner.types.push(name.to_string());
        slf
    }
    fn with_property<'py>(
        mut slf: PyRefMut<'py, Self>,
        name: String,
        value: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pv = py_to_property_value(&value)?;
        slf.inner.properties.insert(name.to_string(), pv.inner);
        Ok(slf)
    }
    fn with_id(mut slf: PyRefMut<'_, Self>, id: PyNodeId) -> PyRefMut<'_, Self> {
        slf.inner.id = Some(id.inner);
        slf
    }
}

/// Single-edge pattern.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "EdgePattern")]
#[derive(Clone, Default)]
pub struct PyEdgePattern {
    pub inner: EdgePattern,
}

#[pymethods]
impl PyEdgePattern {
    #[classmethod]
    fn any(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::default()
    }
    fn bind(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.inner.bind = Some(name.to_string());
        slf
    }
    fn labeled(mut slf: PyRefMut<'_, Self>, label: String) -> PyRefMut<'_, Self> {
        slf.inner.label = Some(label.to_string());
        slf
    }
    fn with_property<'py>(
        mut slf: PyRefMut<'py, Self>,
        name: String,
        value: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pv = py_to_property_value(&value)?;
        slf.inner.properties.insert(name.to_string(), pv.inner);
        Ok(slf)
    }
}

/// One step in a `TraversalPlan`.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "TraversalStep")]
#[derive(Clone)]
pub struct PyTraversalStep {
    pub inner: TraversalStep,
}

#[pymethods]
impl PyTraversalStep {
    #[classmethod]
    fn outbound(_cls: &Bound<'_, pyo3::types::PyType>, edge: PyEdgePattern, target: PyNodePattern) -> Self {
        Self { inner: TraversalStep::outbound(edge.inner, target.inner) }
    }
    #[classmethod]
    fn inbound(_cls: &Bound<'_, pyo3::types::PyType>, edge: PyEdgePattern, target: PyNodePattern) -> Self {
        Self { inner: TraversalStep::inbound(edge.inner, target.inner) }
    }
}

/// Multi-step traversal.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "TraversalPlan")]
#[derive(Clone)]
pub struct PyTraversalPlan {
    pub inner: TraversalPlan,
}

#[pymethods]
impl PyTraversalPlan {
    #[new]
    fn new(seed: PyNodePattern) -> Self {
        Self { inner: TraversalPlan::from(seed.inner) }
    }
    #[classmethod]
    fn from_seed(_cls: &Bound<'_, pyo3::types::PyType>, seed: PyNodePattern) -> Self {
        Self::new(seed)
    }
    fn outbound(mut slf: PyRefMut<'_, Self>, edge: PyEdgePattern, target: PyNodePattern) -> PyRefMut<'_, Self> {
        slf.inner.steps.push(TraversalStep::outbound(edge.inner, target.inner));
        slf
    }
    fn inbound(mut slf: PyRefMut<'_, Self>, edge: PyEdgePattern, target: PyNodePattern) -> PyRefMut<'_, Self> {
        slf.inner.steps.push(TraversalStep::inbound(edge.inner, target.inner));
        slf
    }
}

/// A pattern-match result row.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "MatchRow")]
#[derive(Clone, Default)]
pub struct PyMatchRow {
    pub inner: MatchRow,
}

#[pymethods]
impl PyMatchRow {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    #[getter]
    fn nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new_bound(py);
        for (k, v) in &self.inner.nodes {
            d.set_item(k, PyNodeId::from(*v).into_py(py))?;
        }
        Ok(d)
    }
    #[getter]
    fn edges<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new_bound(py);
        for (k, v) in &self.inner.edges {
            d.set_item(k, PyEdgeId::from(*v).into_py(py))?;
        }
        Ok(d)
    }

    fn __repr__(&self) -> String {
        format!("MatchRow(nodes={}, edges={})", self.inner.nodes.len(), self.inner.edges.len())
    }
}

impl From<MatchRow> for PyMatchRow {
    fn from(inner: MatchRow) -> Self {
        Self { inner }
    }
}

// ============================================================================
// OntologyDelta
// ============================================================================

/// A delta to be applied atomically with provenance.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "OntologyDelta")]
#[derive(Clone, Default)]
pub struct PyOntologyDelta {
    pub inner: OntologyDelta,
}

#[pymethods]
impl PyOntologyDelta {
    #[new]
    #[pyo3(signature = (nodes=None, edges=None, axioms=None))]
    fn new(
        nodes: Option<Vec<PyNode>>,
        edges: Option<Vec<PyEdge>>,
        axioms: Option<Vec<PyAxiom>>,
    ) -> Self {
        Self {
            inner: OntologyDelta {
                nodes: nodes.unwrap_or_default().into_iter().map(|n| n.inner).collect(),
                edges: edges.unwrap_or_default().into_iter().map(|e| e.inner).collect(),
                axioms: axioms.unwrap_or_default().into_iter().map(|a| a.inner).collect(),
            },
        }
    }

    fn with_node(mut slf: PyRefMut<'_, Self>, node: PyNode) -> PyRefMut<'_, Self> {
        slf.inner.nodes.push(node.inner);
        slf
    }
    fn with_edge(mut slf: PyRefMut<'_, Self>, edge: PyEdge) -> PyRefMut<'_, Self> {
        slf.inner.edges.push(edge.inner);
        slf
    }
    fn with_axiom(mut slf: PyRefMut<'_, Self>, axiom: PyAxiom) -> PyRefMut<'_, Self> {
        slf.inner.axioms.push(axiom.inner);
        slf
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[getter]
    fn nodes(&self) -> Vec<PyNode> {
        self.inner.nodes.iter().cloned().map(PyNode::from).collect()
    }
    #[getter]
    fn edges(&self) -> Vec<PyEdge> {
        self.inner.edges.iter().cloned().map(PyEdge::from).collect()
    }
    #[getter]
    fn axioms(&self) -> Vec<PyAxiom> {
        self.inner.axioms.iter().cloned().map(PyAxiom::from).collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "OntologyDelta(nodes={}, edges={}, axioms={})",
            self.inner.nodes.len(),
            self.inner.edges.len(),
            self.inner.axioms.len(),
        )
    }
}

// ============================================================================
// StoreDiff
// ============================================================================

/// Coarse diff between two ontology snapshots.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "StoreDiff")]
#[derive(Clone, Default)]
pub struct PyStoreDiff {
    pub inner: StoreDiff,
}

#[pymethods]
impl PyStoreDiff {
    #[getter]
    fn added_nodes(&self) -> Vec<PyNodeId> {
        self.inner.added_nodes.iter().copied().map(PyNodeId::from).collect()
    }
    #[getter]
    fn removed_nodes(&self) -> Vec<PyNodeId> {
        self.inner.removed_nodes.iter().copied().map(PyNodeId::from).collect()
    }
    #[getter]
    fn added_edges(&self) -> Vec<PyEdgeId> {
        self.inner.added_edges.iter().copied().map(PyEdgeId::from).collect()
    }
    #[getter]
    fn removed_edges(&self) -> Vec<PyEdgeId> {
        self.inner.removed_edges.iter().copied().map(PyEdgeId::from).collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "StoreDiff(added_nodes={}, removed_nodes={}, added_edges={}, removed_edges={})",
            self.inner.added_nodes.len(),
            self.inner.removed_nodes.len(),
            self.inner.added_edges.len(),
            self.inner.removed_edges.len(),
        )
    }
}

impl From<StoreDiff> for PyStoreDiff {
    fn from(inner: StoreDiff) -> Self {
        Self { inner }
    }
}

// ============================================================================
// MemStore
// ============================================================================

/// In-memory ontology store. Async methods return Python coroutines.
#[pyclass(module = "atomr_ontology._atomr_ontology.store", name = "MemStore")]
#[derive(Clone)]
pub struct PyMemStore {
    inner: MemStore,
}

impl PyMemStore {
    pub fn inner_arc(&self) -> Arc<dyn OntologyStore> {
        Arc::new(self.inner.clone())
    }
}

#[pymethods]
impl PyMemStore {
    #[new]
    fn new() -> Self {
        Self { inner: MemStore::new() }
    }

    /// Initialize from an existing ontology.
    #[classmethod]
    fn from_ontology(_cls: &Bound<'_, pyo3::types::PyType>, ontology: PyOntology) -> Self {
        Self { inner: MemStore::from_ontology(ontology.inner) }
    }

    /// Synchronous snapshot, useful when not in an async context.
    fn snapshot_blocking(&self) -> PyOntology {
        PyOntology::from(self.inner.snapshot_blocking())
    }

    fn upsert_node<'py>(&self, py: Python<'py>, node: PyNode) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let raw: Node = node.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store.upsert_node(raw).await.map(PyNodeId::from).map_err(store_err)
        })
    }

    fn upsert_edge<'py>(&self, py: Python<'py>, edge: PyEdge) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let raw: Edge = edge.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store.upsert_edge(raw).await.map(PyEdgeId::from).map_err(store_err)
        })
    }

    fn upsert_axiom<'py>(&self, py: Python<'py>, axiom: PyAxiom) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let raw = axiom.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store.upsert_axiom(raw).await.map_err(store_err)?;
            Ok(())
        })
    }

    fn node<'py>(&self, py: Python<'py>, id: PyNodeId) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let nid: NodeId = id.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let n = store.node(&nid).await.map_err(store_err)?;
            Ok(n.map(PyNode::from))
        })
    }

    fn edge<'py>(&self, py: Python<'py>, id: PyEdgeId) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let eid: EdgeId = id.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let e = store.edge(&eid).await.map_err(store_err)?;
            Ok(e.map(PyEdge::from))
        })
    }

    fn match_pattern<'py>(
        &self,
        py: Python<'py>,
        pattern: PyNodePattern,
    ) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let pat = pattern.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = store.match_pattern(&pat).await.map_err(store_err)?;
            Python::with_gil(|py| {
                let out = PyList::empty_bound(py);
                for r in rows {
                    out.append(PyMatchRow::from(r).into_py(py))?;
                }
                Ok(out.unbind())
            })
        })
    }

    fn traverse<'py>(
        &self,
        py: Python<'py>,
        plan: PyTraversalPlan,
    ) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let p = plan.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = store.traverse(&p).await.map_err(store_err)?;
            Python::with_gil(|py| {
                let out = PyList::empty_bound(py);
                for r in rows {
                    out.append(PyMatchRow::from(r).into_py(py))?;
                }
                Ok(out.unbind())
            })
        })
    }

    fn snapshot<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store.snapshot().await.map(PyOntology::from).map_err(store_err)
        })
    }

    fn diff<'py>(
        &self,
        py: Python<'py>,
        other: PyOntology,
    ) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let o: Ontology = other.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store.diff(&o).await.map(PyStoreDiff::from).map_err(store_err)
        })
    }

    fn commit_with_provenance<'py>(
        &self,
        py: Python<'py>,
        delta: PyOntologyDelta,
        activity: PyActivity,
    ) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        let d = delta.inner;
        let act = activity.inner;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store
                .commit_with_provenance(d, act)
                .await
                .map(PyProvenanceId::from)
                .map_err(store_err)
        })
    }

    fn provenance<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            store.provenance().await.map(PyProvenanceLog::from).map_err(store_err)
        })
    }

    fn __repr__(&self) -> String {
        "MemStore()".to_string()
    }
}

// ============================================================================
// Module registration
// ============================================================================

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyNodePattern>()?;
    m.add_class::<PyEdgePattern>()?;
    m.add_class::<PyTraversalStep>()?;
    m.add_class::<PyTraversalPlan>()?;
    m.add_class::<PyMatchRow>()?;
    m.add_class::<PyOntologyDelta>()?;
    m.add_class::<PyStoreDiff>()?;
    m.add_class::<PyMemStore>()?;
    Ok(())
}
