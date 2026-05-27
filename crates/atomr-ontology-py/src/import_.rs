//! PyO3 wrappers for `atomr-ontology-import`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use atomr_ontology_import::{
    import_foaf as rust_import_foaf, import_schema_org as rust_import_schema_org,
    import_skos as rust_import_skos,
};

use crate::core::PyOntology;
use crate::provenance::PyActivity;

/// Import SKOS Turtle into an `(Ontology, Activity)` pair.
#[pyfunction]
pub fn import_skos(turtle: &str) -> PyResult<(PyOntology, PyActivity)> {
    let (o, a) = rust_import_skos(turtle).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok((PyOntology { inner: o }, PyActivity { inner: a }))
}

/// Import FOAF Turtle into an `(Ontology, Activity)` pair.
#[pyfunction]
pub fn import_foaf(turtle: &str) -> PyResult<(PyOntology, PyActivity)> {
    let (o, a) = rust_import_foaf(turtle).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok((PyOntology { inner: o }, PyActivity { inner: a }))
}

/// Import schema.org JSON-LD into an `(Ontology, Activity)` pair.
#[pyfunction]
pub fn import_schema_org(jsonld: &str) -> PyResult<(PyOntology, PyActivity)> {
    let (o, a) =
        rust_import_schema_org(jsonld).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok((PyOntology { inner: o }, PyActivity { inner: a }))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(import_skos, m)?)?;
    m.add_function(wrap_pyfunction!(import_foaf, m)?)?;
    m.add_function(wrap_pyfunction!(import_schema_org, m)?)?;
    Ok(())
}
