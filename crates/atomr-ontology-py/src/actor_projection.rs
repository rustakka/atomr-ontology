//! PyO3 wrappers for `atomr-ontology-actor-projection`.
//!
//! Exposes just enough surface for a Python user to:
//!   1. Build an in-memory actor source.
//!   2. Pick an ingest mode + projection shape + strategies.
//!   3. Run the projector against an `OntologyStore`.
//!
//! Async methods return Python coroutines via `pyo3-async-runtimes`.

use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use atomr_ontology_actor_projection::{
    ingest::{
        IngestMode as RustIngestMode, PollingIngest as RustPollingIngest,
        ReplayIngest as RustReplayIngest,
    },
    projection::{
        EventStreamProjection as RustEventStreamProjection,
        FlatProjection as RustFlatProjection,
        HierarchicalProjection as RustHierarchicalProjection, ProjectionStrategy as RustProjectionStrategy,
        SnapshotDiffProjection as RustSnapshotDiffProjection,
    },
    source::{
        ActorId, ActorPersistenceSource as RustActorPersistenceSource, Cursor,
        InMemoryActorPersistenceSource, JournalEvent, JournalEventKind, SerializedState,
        SupervisionPath,
    },
    strategy::{ConflictResolution, IriMintingStrategy, SchemaStrategy},
    vocab as rust_vocab, ProjectorBuilder,
};
use atomr_ontology_core::Iri;
use atomr_ontology_persist::{MemCheckpointer as RustMemCheckpointer, PersistentStore as RustPersistentStore};
use atomr_ontology_store::r#trait::OntologyStore;

/// Wrapped [`InMemoryActorPersistenceSource`].
#[pyclass(module = "atomr_ontology._atomr_ontology.actor_projection", name = "InMemoryActorSource")]
#[derive(Clone)]
pub struct PyInMemoryActorSource {
    inner: Arc<InMemoryActorPersistenceSource>,
}

#[pymethods]
impl PyInMemoryActorSource {
    #[new]
    fn new(label: String) -> Self {
        Self { inner: Arc::new(InMemoryActorPersistenceSource::new(label)) }
    }

    /// Append a supervision-tree path, e.g. `"/workflow/foo/run/1/step/a"`.
    fn push_path(&self, path: &str) {
        self.inner.push_path(SupervisionPath::parse(path));
    }

    /// Append a journal event.
    #[pyo3(signature = (actor, kind, payload=None, path=None))]
    fn push_event(
        &self,
        actor: &str,
        kind: &str,
        payload: Option<&Bound<'_, PyAny>>,
        path: Option<&str>,
    ) -> PyResult<()> {
        let kind = parse_kind(kind);
        let mut event = JournalEvent::new(Cursor::beginning(), actor.to_owned(), kind);
        if let Some(p) = path {
            event = event.with_path(SupervisionPath::parse(p));
        }
        if let Some(payload) = payload {
            let json = pythonize_to_json(payload)?;
            event = event.with_payload(json);
        }
        self.inner.push_event(event);
        Ok(())
    }

    /// Set the latest state for an actor.
    fn put_state(&self, actor: &str, payload: &Bound<'_, PyAny>) -> PyResult<()> {
        let json = pythonize_to_json(payload)?;
        self.inner.put_state(SerializedState::new(actor.to_owned(), json));
        Ok(())
    }

    /// Current event count.
    fn event_count(&self) -> usize {
        self.inner.event_count()
    }
}

fn parse_kind(s: &str) -> JournalEventKind {
    match s {
        "created" => JournalEventKind::Created,
        "state_changed" => JournalEventKind::StateChanged,
        "completed" => JournalEventKind::Completed,
        "terminated" => JournalEventKind::Terminated,
        other => JournalEventKind::Custom(other.to_owned()),
    }
}

/// Convert a Python object into `serde_json::Value` by routing through
/// the `json` stdlib module. Avoids a separate pythonize dep.
fn pythonize_to_json(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    let py = value.py();
    let json_mod = py.import_bound("json")?;
    let s: String = json_mod.call_method1("dumps", (value,))?.extract()?;
    serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Wrapped projector (post-build).
#[pyclass(module = "atomr_ontology._atomr_ontology.actor_projection", name = "Projector")]
pub struct PyProjector {
    inner: Option<atomr_ontology_actor_projection::Projector>,
}

#[pymethods]
impl PyProjector {
    /// Run the projector to completion. Returns a dict with batch/node/edge counts.
    fn run<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let projector = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("projector already consumed"))?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let report = projector
                .run()
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Python::with_gil(|py| {
                let dict = PyDict::new_bound(py);
                dict.set_item("batches", report.batches)?;
                dict.set_item("nodes_written", report.nodes_written)?;
                dict.set_item("edges_written", report.edges_written)?;
                dict.set_item("activities_recorded", report.activities_recorded)?;
                Ok(dict.unbind())
            })
        })
    }
}

/// Builder for [`PyProjector`]. Mirrors the Rust fluent surface.
#[pyclass(module = "atomr_ontology._atomr_ontology.actor_projection", name = "ProjectorBuilder")]
pub struct PyProjectorBuilder {
    source: Option<Arc<dyn RustActorPersistenceSource>>,
    ingest: Vec<Arc<dyn RustIngestMode>>,
    projection: Option<Arc<dyn RustProjectionStrategy>>,
    iri: Option<IriMintingStrategy>,
    conflict: Option<ConflictResolution>,
    schema: Option<SchemaStrategy>,
    store: Option<Arc<dyn OntologyStore>>,
}

#[pymethods]
impl PyProjectorBuilder {
    #[new]
    fn new() -> Self {
        Self {
            source: None,
            ingest: Vec::new(),
            projection: None,
            iri: None,
            conflict: None,
            schema: None,
            store: None,
        }
    }

    fn source(&mut self, src: &PyInMemoryActorSource) -> PyResult<()> {
        self.source = Some(src.inner.clone());
        Ok(())
    }

    /// Add a Replay ingest mode.
    fn with_replay(&mut self) {
        self.ingest.push(Arc::new(RustReplayIngest::once()));
    }

    /// Add a Polling ingest mode (`interval_ms` milliseconds between ticks).
    fn with_polling(&mut self, interval_ms: u64) {
        self.ingest.push(Arc::new(RustPollingIngest::every(std::time::Duration::from_millis(
            interval_ms,
        ))));
    }

    /// Set the projection to the named built-in: `"hierarchical"`,
    /// `"event_stream"`, `"snapshot_diff"`, or `"flat"`.
    fn projection(&mut self, name: &str) -> PyResult<()> {
        let projection: Arc<dyn RustProjectionStrategy> = match name {
            "hierarchical" => Arc::new(RustHierarchicalProjection::new()),
            "event_stream" => Arc::new(RustEventStreamProjection::new()),
            "snapshot_diff" => Arc::new(RustSnapshotDiffProjection::new()),
            "flat" => Arc::new(RustFlatProjection::new()),
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown projection: {other:?} (expected hierarchical|event_stream|snapshot_diff|flat)"
                )))
            }
        };
        self.projection = Some(projection);
        Ok(())
    }

    /// Set the IRI minting strategy. Choices: `"path_based"` (requires
    /// `base`), `"content_addressed"`, `"uuid"`.
    #[pyo3(signature = (kind, base=None))]
    fn iri(&mut self, kind: &str, base: Option<&str>) -> PyResult<()> {
        let strategy = match kind {
            "path_based" => {
                let base = base.ok_or_else(|| {
                    PyValueError::new_err("path_based requires `base` IRI argument")
                })?;
                IriMintingStrategy::PathBased {
                    base: Iri::new(base.to_owned()).map_err(|e| PyValueError::new_err(e.to_string()))?,
                }
            }
            "content_addressed" => IriMintingStrategy::ContentAddressed,
            "uuid" => IriMintingStrategy::Uuid,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown iri strategy: {other:?}"
                )))
            }
        };
        self.iri = Some(strategy);
        Ok(())
    }

    /// Set the conflict resolution: `"last_write_wins"`, `"merge"`,
    /// `"skip_existing"`.
    fn conflict(&mut self, kind: &str) -> PyResult<()> {
        let c = match kind {
            "last_write_wins" => ConflictResolution::LastWriteWins,
            "merge" => ConflictResolution::Merge,
            "skip_existing" => ConflictResolution::SkipExisting,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown conflict resolution: {other:?}"
                )))
            }
        };
        self.conflict = Some(c);
        Ok(())
    }

    /// Set the schema strategy: `"induced"` (default), `"hybrid"`
    /// (uses the default actor vocabulary), `"fixed"` (uses the
    /// default actor vocabulary and rejects unknown types).
    fn schema(&mut self, kind: &str) -> PyResult<()> {
        let s = match kind {
            "induced" => SchemaStrategy::InducedSchema,
            "hybrid" => SchemaStrategy::Hybrid(rust_vocab::actor_schema()),
            "fixed" => SchemaStrategy::FixedSchema(rust_vocab::actor_schema()),
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown schema strategy: {other:?}"
                )))
            }
        };
        self.schema = Some(s);
        Ok(())
    }

    /// Attach the destination store. Currently accepts only an
    /// [`MemCheckpointer`]-backed [`PersistentStore`] (the common case
    /// from Python).
    fn store_from_memory<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // We cannot stash a mutable `&mut self` across the await — so
        // create the store in a future, and the caller wires it via
        // `.set_store(store)` after `await`.
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let store = RustPersistentStore::new(RustMemCheckpointer::new())
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            let arc: Arc<dyn OntologyStore> = Arc::new(store);
            Ok(PyOntologyStoreHandle { inner: arc })
        })
    }

    /// Attach the prepared store handle.
    fn set_store(&mut self, handle: &PyOntologyStoreHandle) {
        self.store = Some(handle.inner.clone());
    }

    /// Validate and finalize.
    fn build(&mut self) -> PyResult<PyProjector> {
        let source = self
            .source
            .clone()
            .ok_or_else(|| PyValueError::new_err("source required"))?;
        if self.ingest.is_empty() {
            return Err(PyValueError::new_err("at least one ingest mode required"));
        }
        let projection = self
            .projection
            .clone()
            .ok_or_else(|| PyValueError::new_err("projection required"))?;
        let store = self
            .store
            .clone()
            .ok_or_else(|| PyValueError::new_err("store required"))?;

        let mut builder = ProjectorBuilder::new()
            .source(source)
            .projection(projection)
            .store(store);
        for mode in self.ingest.drain(..) {
            builder = builder.with_ingest(mode);
        }
        if let Some(i) = self.iri.clone() {
            builder = builder.iri(i);
        }
        if let Some(c) = self.conflict {
            builder = builder.conflict(c);
        }
        if let Some(s) = self.schema.clone() {
            builder = builder.schema(s);
        }
        let projector = builder
            .build()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyProjector { inner: Some(projector) })
    }
}

/// Opaque handle to an `OntologyStore` so Python can pass one between
/// builder and projector without exposing the full surface.
#[pyclass(module = "atomr_ontology._atomr_ontology.actor_projection", name = "OntologyStoreHandle")]
#[derive(Clone)]
pub struct PyOntologyStoreHandle {
    pub(crate) inner: Arc<dyn OntologyStore>,
}

#[pymethods]
impl PyOntologyStoreHandle {
    /// Async: snapshot node count.
    fn node_count<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let snap = store
                .snapshot()
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(snap.node_count())
        })
    }

    /// Async: snapshot edge count.
    fn edge_count<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let snap = store
                .snapshot()
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(snap.edge_count())
        })
    }

    /// Async: number of provenance activities.
    fn activity_count<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let store = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let log = store
                .provenance()
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(log.activities.len())
        })
    }
}

// Silence dead-code on the type alias.
#[allow(dead_code)]
type _Aid = ActorId;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInMemoryActorSource>()?;
    m.add_class::<PyOntologyStoreHandle>()?;
    m.add_class::<PyProjector>()?;
    m.add_class::<PyProjectorBuilder>()?;
    Ok(())
}
