//! PyO3 wrappers for `atomr-ontology-validate`.

use pyo3::prelude::*;
use pyo3::types::PyType;

use atomr_ontology::validate::{
    check_consistency as rust_check_consistency, check_shapes as rust_check_shapes,
    validate as rust_validate, Severity, ValidationFinding, ValidationReport,
};

use crate::core::PyOntology;

/// Severity classification for a finding.
#[pyclass(module = "atomr_ontology._atomr_ontology.validate", name = "Severity", eq, hash, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PySeverity {
    Info,
    Warning,
    Error,
}

#[pymethods]
impl PySeverity {
    fn __repr__(&self) -> String {
        format!("Severity.{:?}", self)
    }
}

impl From<Severity> for PySeverity {
    fn from(s: Severity) -> Self {
        match s {
            Severity::Info => PySeverity::Info,
            Severity::Warning => PySeverity::Warning,
            Severity::Error => PySeverity::Error,
        }
    }
}

/// One validation finding.
#[pyclass(module = "atomr_ontology._atomr_ontology.validate", name = "ValidationFinding")]
#[derive(Clone)]
pub struct PyValidationFinding {
    pub inner: ValidationFinding,
}

#[pymethods]
impl PyValidationFinding {
    #[classmethod]
    fn error(_cls: &Bound<'_, PyType>, code: &str, message: &str) -> Self {
        PyValidationFinding { inner: ValidationFinding::error(code.to_string(), message.to_string()) }
    }
    #[classmethod]
    fn warning(_cls: &Bound<'_, PyType>, code: &str, message: &str) -> Self {
        PyValidationFinding { inner: ValidationFinding::warning(code.to_string(), message.to_string()) }
    }

    fn with_focus(mut slf: PyRefMut<'_, Self>, focus: String) -> PyRefMut<'_, Self> {
        slf.inner.focus = Some(focus.to_string());
        slf
    }

    #[getter]
    fn severity(&self) -> PySeverity {
        PySeverity::from(self.inner.severity)
    }
    #[getter]
    fn code(&self) -> &str {
        &self.inner.code
    }
    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }
    #[getter]
    fn focus(&self) -> Option<&str> {
        self.inner.focus.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationFinding(severity={:?}, code={:?}, message={:?}, focus={:?})",
            self.inner.severity, self.inner.code, self.inner.message, self.inner.focus,
        )
    }
}

impl From<ValidationFinding> for PyValidationFinding {
    fn from(inner: ValidationFinding) -> Self {
        Self { inner }
    }
}

/// Aggregate validation report.
#[pyclass(module = "atomr_ontology._atomr_ontology.validate", name = "ValidationReport")]
#[derive(Clone, Default)]
pub struct PyValidationReport {
    pub inner: ValidationReport,
}

#[pymethods]
impl PyValidationReport {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, finding: PyValidationFinding) {
        self.inner.push(finding.inner);
    }

    fn extend(&mut self, other: PyValidationReport) {
        self.inner.extend(other.inner);
    }

    fn is_clean(&self) -> bool {
        self.inner.is_clean()
    }

    #[getter]
    fn findings(&self) -> Vec<PyValidationFinding> {
        self.inner.findings.iter().cloned().map(PyValidationFinding::from).collect()
    }

    fn errors(&self) -> Vec<PyValidationFinding> {
        self.inner.errors().cloned().map(PyValidationFinding::from).collect()
    }

    fn __repr__(&self) -> String {
        format!("ValidationReport(findings={})", self.inner.findings.len())
    }
}

impl From<ValidationReport> for PyValidationReport {
    fn from(inner: ValidationReport) -> Self {
        Self { inner }
    }
}

/// Run every check and return the aggregate report.
#[pyfunction]
pub fn validate(ontology: &PyOntology) -> PyValidationReport {
    PyValidationReport::from(rust_validate(&ontology.inner))
}

/// SHACL-style shape checks only.
#[pyfunction]
pub fn check_shapes(ontology: &PyOntology) -> PyValidationReport {
    PyValidationReport::from(rust_check_shapes(&ontology.inner))
}

/// Axiom-consistency checks only.
#[pyfunction]
pub fn check_consistency(ontology: &PyOntology) -> PyValidationReport {
    PyValidationReport::from(rust_check_consistency(&ontology.inner))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySeverity>()?;
    m.add_class::<PyValidationFinding>()?;
    m.add_class::<PyValidationReport>()?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(check_shapes, m)?)?;
    m.add_function(wrap_pyfunction!(check_consistency, m)?)?;
    Ok(())
}
