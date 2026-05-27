//! PyO3 wrappers for `atomr-ontology-version`.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use atomr_ontology_version::{
    CommitId as RustCommitId, MergeStrategy as RustMergeStrategy, VersionedStore as RustVersionedStore,
};

use crate::core::PyOntology;

/// Content-addressed commit id (32 bytes).
#[pyclass(module = "atomr_ontology._atomr_ontology.version", name = "CommitId", frozen, eq, hash)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyCommitId {
    pub inner: RustCommitId,
}

#[pymethods]
impl PyCommitId {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }
    fn __repr__(&self) -> String {
        format!("CommitId({:?})", self.inner.to_string())
    }
}

/// Merge strategies.
#[pyclass(module = "atomr_ontology._atomr_ontology.version", name = "MergeStrategy", eq)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyMergeStrategy {
    Ours,
    Theirs,
    Union,
}

impl From<PyMergeStrategy> for RustMergeStrategy {
    fn from(v: PyMergeStrategy) -> Self {
        match v {
            PyMergeStrategy::Ours => RustMergeStrategy::Ours,
            PyMergeStrategy::Theirs => RustMergeStrategy::Theirs,
            PyMergeStrategy::Union => RustMergeStrategy::Union,
        }
    }
}

/// Git-style branchable ontology store.
#[pyclass(module = "atomr_ontology._atomr_ontology.version", name = "VersionedStore")]
pub struct PyVersionedStore {
    inner: RustVersionedStore,
}

#[pymethods]
impl PyVersionedStore {
    #[new]
    fn new() -> Self {
        Self { inner: RustVersionedStore::init() }
    }

    /// Append a commit to the current branch.
    fn commit(&mut self, message: String, author: String, snapshot: PyOntology) -> PyCommitId {
        let id = self.inner.commit(message, author, snapshot.inner);
        PyCommitId { inner: id }
    }

    /// Create a new branch off the current head.
    fn branch(&mut self, name: String) -> PyResult<()> {
        self.inner.branch(name).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Switch to a named branch.
    fn checkout(&mut self, name: &str) -> PyResult<()> {
        self.inner.checkout(name).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Merge another branch into the current one. Returns the merge commit id.
    fn merge(&mut self, other: &str, strategy: PyMergeStrategy) -> PyResult<PyCommitId> {
        let id = self
            .inner
            .merge(other, strategy.into())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyCommitId { inner: id })
    }

    /// Snapshot at a given commit.
    fn as_of(&self, id: PyCommitId) -> Option<PyOntology> {
        self.inner.as_of(id.inner).cloned().map(|o| PyOntology { inner: o })
    }

    /// List all branch names.
    fn branches(&self) -> Vec<String> {
        self.inner.branches.keys().cloned().collect()
    }

    /// Current branch name.
    fn current(&self) -> &str {
        self.inner.current_branch()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCommitId>()?;
    m.add_class::<PyMergeStrategy>()?;
    m.add_class::<PyVersionedStore>()?;
    Ok(())
}
