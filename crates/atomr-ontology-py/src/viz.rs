//! PyO3 wrappers for `atomr-ontology-viz` — DOT / Mermaid renderers.

use pyo3::prelude::*;

use atomr_ontology_viz::{
    render_ontology_dot as rust_render_ontology_dot,
    render_ontology_mermaid as rust_render_ontology_mermaid,
    render_provenance_dot as rust_render_provenance_dot,
    render_provenance_mermaid as rust_render_provenance_mermaid,
};

use crate::core::PyOntology;
use crate::provenance::PyProvenanceLog;

/// Render an ontology as a GraphViz DOT document.
#[pyfunction]
pub fn render_ontology_dot(ontology: &PyOntology) -> String {
    rust_render_ontology_dot(&ontology.inner)
}

/// Render an ontology as a Mermaid `graph LR` document.
#[pyfunction]
pub fn render_ontology_mermaid(ontology: &PyOntology) -> String {
    rust_render_ontology_mermaid(&ontology.inner)
}

/// Render a provenance log as a GraphViz DOT document.
#[pyfunction]
pub fn render_provenance_dot(log: &PyProvenanceLog) -> String {
    rust_render_provenance_dot(&log.inner)
}

/// Render a provenance log as a Mermaid `graph TD` document.
#[pyfunction]
pub fn render_provenance_mermaid(log: &PyProvenanceLog) -> String {
    rust_render_provenance_mermaid(&log.inner)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(render_ontology_dot, m)?)?;
    m.add_function(wrap_pyfunction!(render_ontology_mermaid, m)?)?;
    m.add_function(wrap_pyfunction!(render_provenance_dot, m)?)?;
    m.add_function(wrap_pyfunction!(render_provenance_mermaid, m)?)?;
    Ok(())
}
