//! PyO3 wrappers for `atomr-ontology-query` — Cypher/SPARQL DSL parsers.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use atomr_ontology_query::{parse_cypher as rust_parse_cypher, parse_sparql as rust_parse_sparql};

use crate::store::PyTraversalPlan;

/// Parse a Cypher subset query into a `TraversalPlan`.
#[pyfunction]
pub fn parse_cypher(query: &str) -> PyResult<PyTraversalPlan> {
    rust_parse_cypher(query)
        .map(|plan| PyTraversalPlan { inner: plan })
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Parse a SPARQL BGP subset query into a `TraversalPlan`.
#[pyfunction]
pub fn parse_sparql(query: &str) -> PyResult<PyTraversalPlan> {
    rust_parse_sparql(query)
        .map(|plan| PyTraversalPlan { inner: plan })
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_cypher, m)?)?;
    m.add_function(wrap_pyfunction!(parse_sparql, m)?)?;
    Ok(())
}
