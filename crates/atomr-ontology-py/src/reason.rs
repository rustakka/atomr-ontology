//! PyO3 wrappers for `atomr-ontology-reason`.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use atomr_ontology_reason::{Reasoner as RustReasoner, RuleSet as RustRuleSet};

use crate::core::PyOntology;
use crate::provenance::PyActivity;

/// Forward-chaining OWL 2 RL/EL reasoner.
#[pyclass(module = "atomr_ontology._atomr_ontology.reason", name = "Reasoner")]
#[derive(Clone)]
pub struct PyReasoner {
    inner: RustReasoner,
}

#[pymethods]
impl PyReasoner {
    #[new]
    fn new() -> Self {
        Self { inner: RustReasoner::new() }
    }

    /// Build a reasoner with a custom maximum-iteration cap.
    #[classmethod]
    fn with_max_iterations(_cls: &Bound<'_, pyo3::types::PyType>, max: usize) -> Self {
        Self { inner: RustReasoner::new().with_max_iterations(max) }
    }

    /// Run the reasoner non-destructively: returns
    /// `(derived_axiom_count, derived_edge_count, iterations, activity)`.
    fn run<'py>(&self, py: Python<'py>, ontology: &PyOntology) -> PyResult<(usize, usize, usize, PyActivity)> {
        let report = self.inner.run(&ontology.inner).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let _ = py;
        Ok((
            report.derived_axioms.len(),
            report.derived_edges.len(),
            report.iterations,
            PyActivity { inner: report.activity },
        ))
    }

    /// Run the reasoner and merge derived facts back into the ontology.
    /// Returns the `Activity` describing the materialization.
    fn materialize(&self, ontology: &mut PyOntology) -> PyResult<PyActivity> {
        let report = self
            .inner
            .materialize(&mut ontology.inner)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyActivity { inner: report.activity })
    }
}

/// Set of forward-chaining rules.
#[pyclass(module = "atomr_ontology._atomr_ontology.reason", name = "RuleSet")]
#[derive(Clone)]
pub struct PyRuleSet {
    pub inner: RustRuleSet,
}

#[pymethods]
impl PyRuleSet {
    #[classmethod]
    fn standard(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self { inner: RustRuleSet::standard() }
    }

    #[classmethod]
    fn empty(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self { inner: RustRuleSet::empty() }
    }

    fn __len__(&self) -> usize {
        self.inner.iter().count()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyReasoner>()?;
    m.add_class::<PyRuleSet>()?;
    Ok(())
}
