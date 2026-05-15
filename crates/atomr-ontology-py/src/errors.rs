//! Exception hierarchy + Rust-error → Python-exception converters.
//!
//! Every Rust error in the workspace funnels through a subclass of
//! [`AtomrOntologyError`]:
//!
//! ```text
//!   AtomrOntologyError(Exception)
//!     ├── IriError
//!     ├── OntologyError
//!     ├── StoreError
//!     ├── BackendError
//!     ├── AdapterError
//!     └── ValidationError
//! ```

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::PyErr;

use atomr_ontology::core::IriError as RustIriError;
use atomr_ontology::core::OntologyError as RustOntologyError;
use atomr_ontology::extract::BackendError as RustBackendError;
use atomr_ontology::rdf::AdapterError as RustAdapterError;
use atomr_ontology::store::StoreError as RustStoreError;

create_exception!(_atomr_ontology, AtomrOntologyError, PyException);
create_exception!(_atomr_ontology, IriError, AtomrOntologyError);
create_exception!(_atomr_ontology, OntologyError, AtomrOntologyError);
create_exception!(_atomr_ontology, StoreError, AtomrOntologyError);
create_exception!(_atomr_ontology, BackendError, AtomrOntologyError);
create_exception!(_atomr_ontology, AdapterError, AtomrOntologyError);
create_exception!(_atomr_ontology, ValidationError, AtomrOntologyError);

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("AtomrOntologyError", py.get_type_bound::<AtomrOntologyError>())?;
    m.add("IriError", py.get_type_bound::<IriError>())?;
    m.add("OntologyError", py.get_type_bound::<OntologyError>())?;
    m.add("StoreError", py.get_type_bound::<StoreError>())?;
    m.add("BackendError", py.get_type_bound::<BackendError>())?;
    m.add("AdapterError", py.get_type_bound::<AdapterError>())?;
    m.add("ValidationError", py.get_type_bound::<ValidationError>())?;
    Ok(())
}

pub fn iri_err(e: RustIriError) -> PyErr {
    IriError::new_err(e.to_string())
}

pub fn ontology_err(e: RustOntologyError) -> PyErr {
    OntologyError::new_err(e.to_string())
}

pub fn store_err(e: RustStoreError) -> PyErr {
    StoreError::new_err(e.to_string())
}

pub fn backend_err(e: RustBackendError) -> PyErr {
    BackendError::new_err(e.to_string())
}

pub fn adapter_err(e: RustAdapterError) -> PyErr {
    AdapterError::new_err(e.to_string())
}

#[allow(dead_code)]
pub fn validation_err(msg: impl Into<String>) -> PyErr {
    ValidationError::new_err(msg.into())
}
