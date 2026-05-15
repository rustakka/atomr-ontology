//! PyO3 wrappers for `atomr-ontology-provenance`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyType};

use atomr_ontology::provenance::{
    Activity, AgentKind, AgentRef, ProvEntity, ProvenanceLog, Used, WasAttributedTo,
    WasDerivedFrom, WasGeneratedBy,
};

use crate::core::PyProvenanceId;

// ============================================================================
// AgentKind / AgentRef
// ============================================================================

/// PROV-O agent kind.
#[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = "AgentKind", eq, hash, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyAgentKind {
    Person,
    Software,
    Organization,
}

#[pymethods]
impl PyAgentKind {
    fn __repr__(&self) -> String {
        format!("AgentKind.{:?}", self)
    }
}

impl From<AgentKind> for PyAgentKind {
    fn from(k: AgentKind) -> Self {
        match k {
            AgentKind::Person => PyAgentKind::Person,
            AgentKind::Software => PyAgentKind::Software,
            AgentKind::Organization => PyAgentKind::Organization,
        }
    }
}
impl From<PyAgentKind> for AgentKind {
    fn from(k: PyAgentKind) -> Self {
        match k {
            PyAgentKind::Person => AgentKind::Person,
            PyAgentKind::Software => AgentKind::Software,
            PyAgentKind::Organization => AgentKind::Organization,
        }
    }
}

/// Reference to a PROV-O agent.
#[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = "AgentRef", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyAgentRef {
    pub inner: AgentRef,
}

#[pymethods]
impl PyAgentRef {
    #[new]
    fn new(id: &str, kind: PyAgentKind, label: &str) -> Self {
        PyAgentRef {
            inner: AgentRef { id: id.to_string(), kind: kind.into(), label: label.to_string() },
        }
    }

    #[classmethod]
    fn software(_cls: &Bound<'_, PyType>, id: &str, label: &str) -> Self {
        PyAgentRef { inner: AgentRef::software(id.to_string(), label.to_string()) }
    }

    #[classmethod]
    fn person(_cls: &Bound<'_, PyType>, id: &str, label: &str) -> Self {
        PyAgentRef { inner: AgentRef::person(id.to_string(), label.to_string()) }
    }

    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }
    #[getter]
    fn kind(&self) -> PyAgentKind {
        PyAgentKind::from(self.inner.kind)
    }
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    fn __repr__(&self) -> String {
        format!("AgentRef(id={:?}, kind={:?}, label={:?})", self.inner.id, self.inner.kind, self.inner.label)
    }
}

impl From<AgentRef> for PyAgentRef {
    fn from(inner: AgentRef) -> Self {
        PyAgentRef { inner }
    }
}

// ============================================================================
// Activity
// ============================================================================

/// A PROV-O activity.
#[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = "Activity")]
#[derive(Clone)]
pub struct PyActivity {
    pub inner: Activity,
}

#[pymethods]
impl PyActivity {
    /// Start a fresh activity with the given label.
    #[classmethod]
    fn started(_cls: &Bound<'_, PyType>, label: &str) -> Self {
        PyActivity { inner: Activity::started(label.to_string()) }
    }

    /// Mark the activity finished (`ended_at = now`).
    fn finish(slf: PyRef<'_, Self>) -> Self {
        PyActivity { inner: slf.inner.clone().finish() }
    }

    fn by(mut slf: PyRefMut<'_, Self>, agent: PyAgentRef) -> PyRefMut<'_, Self> {
        slf.inner.agent = Some(agent.inner);
        slf
    }

    /// Attach an attribute. `value` is converted to JSON via Python's
    /// ``json.dumps``.
    fn with_attribute<'py>(
        mut slf: PyRefMut<'py, Self>,
        key: String,
        value: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let json = slf.py().import_bound("json")?;
        let s: String = json.call_method1("dumps", (value,))?.extract()?;
        let v: serde_json::Value =
            serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))?;
        slf.inner.attributes.insert(key.to_string(), v);
        Ok(slf)
    }

    #[getter]
    fn id(&self) -> PyProvenanceId {
        PyProvenanceId::from(self.inner.id)
    }
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }
    #[getter]
    fn started_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.inner.started_at
    }
    #[getter]
    fn ended_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.inner.ended_at
    }
    #[getter]
    fn agent(&self) -> Option<PyAgentRef> {
        self.inner.agent.clone().map(PyAgentRef::from)
    }
    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        attributes_to_py(py, &self.inner.attributes)
    }

    fn __repr__(&self) -> String {
        format!("Activity(id={}, label={:?})", self.inner.id, self.inner.label)
    }
}

impl From<Activity> for PyActivity {
    fn from(inner: Activity) -> Self {
        PyActivity { inner }
    }
}

fn attributes_to_py<'py>(
    py: Python<'py>,
    a: &std::collections::BTreeMap<String, serde_json::Value>,
) -> PyResult<Bound<'py, PyDict>> {
    let json = py.import_bound("json")?;
    let d = PyDict::new_bound(py);
    for (k, v) in a {
        let s = serde_json::to_string(v).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let value = json.call_method1("loads", (s,))?;
        d.set_item(k, value)?;
    }
    Ok(d)
}

// ============================================================================
// ProvEntity
// ============================================================================

/// A PROV-O entity (snapshot of data used or produced).
#[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = "ProvEntity")]
#[derive(Clone)]
pub struct PyProvEntity {
    pub inner: ProvEntity,
}

#[pymethods]
impl PyProvEntity {
    #[new]
    #[pyo3(signature = (label, digest=None))]
    fn new(label: &str, digest: Option<String>) -> Self {
        PyProvEntity { inner: ProvEntity::new(label.to_string(), digest) }
    }

    #[getter]
    fn id(&self) -> PyProvenanceId {
        PyProvenanceId::from(self.inner.id)
    }
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }
    #[getter]
    fn digest(&self) -> Option<&str> {
        self.inner.digest.as_deref()
    }
    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        attributes_to_py(py, &self.inner.attributes)
    }

    fn __repr__(&self) -> String {
        format!("ProvEntity(id={}, label={:?})", self.inner.id, self.inner.label)
    }
}

impl From<ProvEntity> for PyProvEntity {
    fn from(inner: ProvEntity) -> Self {
        PyProvEntity { inner }
    }
}

// ============================================================================
// Lineage edge rows
// ============================================================================

macro_rules! lineage_pair {
    ($PyTy:ident, $Inner:ty, $name:literal, $a:ident, $b:ident, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = $name, frozen, eq, hash)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $PyTy {
            pub inner: $Inner,
        }
        #[pymethods]
        impl $PyTy {
            #[getter]
            fn $a(&self) -> PyProvenanceId {
                PyProvenanceId::from(self.inner.$a)
            }
            #[getter]
            fn $b(&self) -> PyProvenanceId {
                PyProvenanceId::from(self.inner.$b)
            }
        }
        impl From<$Inner> for $PyTy {
            fn from(inner: $Inner) -> Self {
                $PyTy { inner }
            }
        }
    };
}

lineage_pair!(
    PyWasGeneratedBy,
    WasGeneratedBy,
    "WasGeneratedBy",
    entity,
    activity,
    "`prov:wasGeneratedBy` lineage edge."
);
lineage_pair!(
    PyWasDerivedFrom,
    WasDerivedFrom,
    "WasDerivedFrom",
    derived,
    source,
    "`prov:wasDerivedFrom` lineage edge."
);
lineage_pair!(PyUsed, Used, "Used", activity, entity, "`prov:used` lineage edge.");

/// `prov:wasAttributedTo` lineage row.
#[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = "WasAttributedTo", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyWasAttributedTo {
    pub inner: WasAttributedTo,
}

#[pymethods]
impl PyWasAttributedTo {
    #[getter]
    fn entity(&self) -> PyProvenanceId {
        PyProvenanceId::from(self.inner.entity)
    }
    #[getter]
    fn agent(&self) -> PyAgentRef {
        PyAgentRef::from(self.inner.agent.clone())
    }
}

impl From<WasAttributedTo> for PyWasAttributedTo {
    fn from(inner: WasAttributedTo) -> Self {
        PyWasAttributedTo { inner }
    }
}

// ============================================================================
// ProvenanceLog
// ============================================================================

/// In-memory provenance ledger.
#[pyclass(module = "atomr_ontology._atomr_ontology.provenance", name = "ProvenanceLog")]
#[derive(Clone, Default)]
pub struct PyProvenanceLog {
    pub inner: ProvenanceLog,
}

#[pymethods]
impl PyProvenanceLog {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    fn record_activity(&mut self, activity: PyActivity) -> PyProvenanceId {
        PyProvenanceId::from(self.inner.record_activity(activity.inner))
    }

    fn record_entity(&mut self, entity: PyProvEntity) -> PyProvenanceId {
        PyProvenanceId::from(self.inner.record_entity(entity.inner))
    }

    fn generated(&mut self, entity: PyProvenanceId, activity: PyProvenanceId) {
        self.inner.generated(entity.inner, activity.inner);
    }

    fn derived(&mut self, derived: PyProvenanceId, source: PyProvenanceId) {
        self.inner.derived(derived.inner, source.inner);
    }

    fn attributed(&mut self, entity: PyProvenanceId, agent: PyAgentRef) {
        self.inner.attributed(entity.inner, agent.inner);
    }

    fn used(&mut self, activity: PyProvenanceId, entity: PyProvenanceId) {
        self.inner.used(activity.inner, entity.inner);
    }

    fn activity(&self, id: PyProvenanceId) -> Option<PyActivity> {
        self.inner.activities.get(&id.inner).cloned().map(PyActivity::from)
    }

    fn entity(&self, id: PyProvenanceId) -> Option<PyProvEntity> {
        self.inner.entities.get(&id.inner).cloned().map(PyProvEntity::from)
    }

    #[getter]
    fn activities<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty_bound(py);
        for a in self.inner.activities.values() {
            out.append(PyActivity::from(a.clone()).into_py(py))?;
        }
        Ok(out)
    }
    #[getter]
    fn entities<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty_bound(py);
        for e in self.inner.entities.values() {
            out.append(PyProvEntity::from(e.clone()).into_py(py))?;
        }
        Ok(out)
    }
    #[getter]
    fn generations(&self) -> Vec<PyWasGeneratedBy> {
        self.inner.generations.iter().copied().map(PyWasGeneratedBy::from).collect()
    }
    #[getter]
    fn derivations(&self) -> Vec<PyWasDerivedFrom> {
        self.inner.derivations.iter().copied().map(PyWasDerivedFrom::from).collect()
    }
    #[getter]
    fn attributions(&self) -> Vec<PyWasAttributedTo> {
        self.inner.attributions.iter().cloned().map(PyWasAttributedTo::from).collect()
    }
    #[getter]
    fn uses(&self) -> Vec<PyUsed> {
        self.inner.uses.iter().copied().map(PyUsed::from).collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "ProvenanceLog(activities={}, entities={}, generations={}, derivations={}, uses={})",
            self.inner.activities.len(),
            self.inner.entities.len(),
            self.inner.generations.len(),
            self.inner.derivations.len(),
            self.inner.uses.len(),
        )
    }
}

impl From<ProvenanceLog> for PyProvenanceLog {
    fn from(inner: ProvenanceLog) -> Self {
        PyProvenanceLog { inner }
    }
}

// ============================================================================
// Module registration
// ============================================================================

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAgentKind>()?;
    m.add_class::<PyAgentRef>()?;
    m.add_class::<PyActivity>()?;
    m.add_class::<PyProvEntity>()?;
    m.add_class::<PyWasGeneratedBy>()?;
    m.add_class::<PyWasDerivedFrom>()?;
    m.add_class::<PyWasAttributedTo>()?;
    m.add_class::<PyUsed>()?;
    m.add_class::<PyProvenanceLog>()?;
    m.add_class::<PyProvenanceId>()?;
    // `ProvAgent` is an alias for `AgentRef` upstream — mirror that.
    m.add("ProvAgent", m.py().get_type_bound::<PyAgentRef>())?;
    Ok(())
}
