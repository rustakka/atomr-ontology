//! PyO3 wrappers for `atomr-ontology-org`.

use pyo3::prelude::*;

use atomr_ontology::org::{
    build_reference_vocabulary as rust_build_reference_vocabulary, reference_ontology as rust_reference_ontology,
    FOAF_NS, ORG_NS, SCHEMA_NS,
};

use crate::core::PyOntology;

/// Build a fresh reference ontology containing the W3C Org Ontology
/// node/edge types plus the schema.org bridge axioms.
#[pyfunction]
pub fn reference_ontology() -> PyOntology {
    PyOntology::from(rust_reference_ontology())
}

/// Mutate `ontology` in place to add the reference vocabulary. Idempotent.
#[pyfunction]
pub fn build_reference_vocabulary(ontology: &mut PyOntology) {
    rust_build_reference_vocabulary(&mut ontology.inner);
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("ORG_NS", ORG_NS)?;
    m.add("FOAF_NS", FOAF_NS)?;
    m.add("SCHEMA_NS", SCHEMA_NS)?;
    m.add_function(wrap_pyfunction!(reference_ontology, m)?)?;
    m.add_function(wrap_pyfunction!(build_reference_vocabulary, m)?)?;
    Ok(())
}
