//! Python bindings for the `atomr-ontology` Rust workspace.
//!
//! See the top-level Python `atomr_ontology` package for the user-
//! facing API; this crate is the compiled extension module that
//! backs it. Submodules below mirror the Rust crates one-for-one.

#![forbid(unsafe_code)]
#![allow(clippy::useless_conversion)]
// pyo3 0.22's `create_exception!` macro references a `gil-refs` feature
// flag from a transitional pyo3 release; we don't control that and our
// crate doesn't declare the flag, so silence the lint to keep
// `-D warnings` green for actual issues in our code.
#![allow(unexpected_cfgs)]

use pyo3::prelude::*;

mod core;
mod errors;
mod extract;
mod induce;
mod org;
mod provenance;
mod rdf;
mod store;
mod testkit;
mod validate;

#[cfg(feature = "infer")]
mod infer;

/// Build the top-level `_atomr_ontology` Python module.
#[pymodule]
fn _atomr_ontology(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    errors::register(py, m)?;

    let core = PyModule::new_bound(py, "core")?;
    core::register(&core)?;
    m.add_submodule(&core)?;

    let provenance = PyModule::new_bound(py, "provenance")?;
    provenance::register(&provenance)?;
    m.add_submodule(&provenance)?;

    let store = PyModule::new_bound(py, "store")?;
    store::register(&store)?;
    m.add_submodule(&store)?;

    let extract = PyModule::new_bound(py, "extract")?;
    extract::register(&extract)?;
    m.add_submodule(&extract)?;

    let induce = PyModule::new_bound(py, "induce")?;
    induce::register(&induce)?;
    m.add_submodule(&induce)?;

    let validate = PyModule::new_bound(py, "validate")?;
    validate::register(&validate)?;
    m.add_submodule(&validate)?;

    let rdf = PyModule::new_bound(py, "rdf")?;
    rdf::register(&rdf)?;
    m.add_submodule(&rdf)?;

    let org = PyModule::new_bound(py, "org")?;
    org::register(&org)?;
    m.add_submodule(&org)?;

    let testkit = PyModule::new_bound(py, "testkit")?;
    testkit::register(&testkit)?;
    m.add_submodule(&testkit)?;

    #[cfg(feature = "infer")]
    {
        let infer = PyModule::new_bound(py, "infer")?;
        infer::register(&infer)?;
        m.add_submodule(&infer)?;
    }

    // Make `from atomr_ontology._atomr_ontology import core` work
    // even when the submodule was registered via add_submodule.
    let sys = py.import_bound("sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item("atomr_ontology._atomr_ontology.core", m.getattr("core")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.provenance", m.getattr("provenance")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.store", m.getattr("store")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.extract", m.getattr("extract")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.induce", m.getattr("induce")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.validate", m.getattr("validate")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.rdf", m.getattr("rdf")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.org", m.getattr("org")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.testkit", m.getattr("testkit")?)?;
    #[cfg(feature = "infer")]
    modules.set_item("atomr_ontology._atomr_ontology.infer", m.getattr("infer")?)?;

    Ok(())
}
