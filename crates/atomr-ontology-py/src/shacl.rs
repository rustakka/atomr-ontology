//! PyO3 wrappers for `atomr-ontology-shacl`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use atomr_ontology_shacl::{
    from_shacl_turtle as rust_from_shacl_turtle, to_shacl_turtle as rust_to_shacl_turtle,
};

use crate::core::PySchema;

/// Compile a `Schema` to SHACL Turtle.
#[pyfunction]
pub fn to_shacl_turtle(schema: &PySchema) -> PyResult<String> {
    rust_to_shacl_turtle(&schema.inner).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Parse a SHACL Turtle document back into a `Schema`.
#[pyfunction]
pub fn from_shacl_turtle(input: &str) -> PyResult<PySchema> {
    rust_from_shacl_turtle(input)
        .map(|s| PySchema { inner: s })
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_shacl_turtle, m)?)?;
    m.add_function(wrap_pyfunction!(from_shacl_turtle, m)?)?;
    Ok(())
}
