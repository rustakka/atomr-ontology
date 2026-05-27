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

mod actor_projection;
mod core;
mod embed;
mod errors;
mod extract;
mod import_;
mod induce;
mod org;
mod persist;
mod provenance;
mod query;
mod rdf;
mod reason;
mod remote;
mod shacl;
mod store;
mod testkit;
mod validate;
mod version;
mod viz;

#[cfg(feature = "infer")]
mod infer;

#[cfg(feature = "http-driver")]
mod http_driver;

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

    let viz = PyModule::new_bound(py, "viz")?;
    viz::register(&viz)?;
    m.add_submodule(&viz)?;

    let query = PyModule::new_bound(py, "query")?;
    query::register(&query)?;
    m.add_submodule(&query)?;

    let import_ = PyModule::new_bound(py, "import_")?;
    import_::register(&import_)?;
    m.add_submodule(&import_)?;

    let shacl = PyModule::new_bound(py, "shacl")?;
    shacl::register(&shacl)?;
    m.add_submodule(&shacl)?;

    let reason = PyModule::new_bound(py, "reason")?;
    reason::register(&reason)?;
    m.add_submodule(&reason)?;

    let embed = PyModule::new_bound(py, "embed")?;
    embed::register(&embed)?;
    m.add_submodule(&embed)?;

    let version = PyModule::new_bound(py, "version")?;
    version::register(&version)?;
    m.add_submodule(&version)?;

    let persist = PyModule::new_bound(py, "persist")?;
    persist::register(&persist)?;
    m.add_submodule(&persist)?;

    let remote = PyModule::new_bound(py, "remote")?;
    remote::register(&remote)?;
    m.add_submodule(&remote)?;

    let actor_projection = PyModule::new_bound(py, "actor_projection")?;
    actor_projection::register(&actor_projection)?;
    m.add_submodule(&actor_projection)?;

    #[cfg(feature = "infer")]
    {
        let infer = PyModule::new_bound(py, "infer")?;
        infer::register(&infer)?;
        m.add_submodule(&infer)?;
    }

    #[cfg(feature = "http-driver")]
    {
        let http_driver = PyModule::new_bound(py, "http_driver")?;
        http_driver::register(&http_driver)?;
        m.add_submodule(&http_driver)?;
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
    modules.set_item("atomr_ontology._atomr_ontology.viz", m.getattr("viz")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.query", m.getattr("query")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.import_", m.getattr("import_")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.shacl", m.getattr("shacl")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.reason", m.getattr("reason")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.embed", m.getattr("embed")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.version", m.getattr("version")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.persist", m.getattr("persist")?)?;
    modules.set_item("atomr_ontology._atomr_ontology.remote", m.getattr("remote")?)?;
    modules.set_item(
        "atomr_ontology._atomr_ontology.actor_projection",
        m.getattr("actor_projection")?,
    )?;
    #[cfg(feature = "infer")]
    modules.set_item("atomr_ontology._atomr_ontology.infer", m.getattr("infer")?)?;
    #[cfg(feature = "http-driver")]
    modules.set_item("atomr_ontology._atomr_ontology.http_driver", m.getattr("http_driver")?)?;

    Ok(())
}
