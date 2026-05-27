//! PyO3 wrappers for the agentic surface in
//! `atomr-ontology-extract::agentic` + the agentic inducers in
//! `atomr-ontology-induce`.
//!
//! Compiled only when the `agents` cargo feature is on. Provides:
//!
//! - `AgenticAgent` — Python handle wrapping a Rust `AgenticAgent`.
//!   Also satisfies the narrow `Backend` contract so it can be passed
//!   to any extractor.
//! - `AgenticSession`, `AgenticOutcome`, `ToolSpec`, `ToolCallRecord`,
//!   `TurnRecord` — value types.
//! - `AgenticTaxonomyInducer`, `AgenticAxiomMiner` — multi-turn
//!   inducers that hand a `ToolSpec` palette to the agent.
//! - `default_store_tools(store)` — convenience helper that builds the
//!   bundled `OntologyStore` introspection tools.
//!
//! See `docs/providers.md` for the canonical `AgenticAgent →
//! atomr_agents::Agent → atomr_infer::Provider` layering.

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use atomr_ontology::extract::agentic::{
    AgenticAgent, AgenticDriver, AgenticOutcome, AgenticSession, StopCondition, ToolCallRecord,
    ToolSpec, TurnRecord,
};
use atomr_ontology::extract::backend::{Backend, BackendError, Prompt};
use atomr_ontology::extract::store_tools::default_store_tools;
use atomr_ontology::induce::{AgenticAxiomMiner, AgenticTaxonomyInducer};

use crate::errors::backend_err;
use crate::extract::{PyBackend, PyPrompt};
use crate::induce::{PyAxiomProposal, PySubclassProposal};
use crate::provenance::PyActivity;
use crate::store::PyMemStore;

// ============================================================================
// ToolSpec
// ============================================================================

/// A tool the agent can invoke during a session.
///
/// Construct via the [`ToolSpec.from_python`] classmethod, passing a
/// Python `async def handler(args: dict) -> dict` callback.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "ToolSpec")]
#[derive(Clone)]
pub struct PyToolSpec {
    pub inner: ToolSpec,
}

#[pymethods]
impl PyToolSpec {
    /// Build a tool from name + description + JSON schema + an
    /// `async def handler(args: dict) -> dict` Python callback.
    #[classmethod]
    #[pyo3(signature = (name, description, json_schema, handler))]
    fn from_python(
        _cls: &Bound<'_, pyo3::types::PyType>,
        name: String,
        description: String,
        json_schema: &Bound<'_, PyAny>,
        handler: PyObject,
    ) -> PyResult<Self> {
        let schema_value = py_any_to_json(json_schema)?;
        let handler_arc: Arc<PyObject> = Arc::new(handler);
        let inner = ToolSpec::new(
            name,
            description,
            schema_value,
            move |args: serde_json::Value| -> BoxFuture<'static, Result<serde_json::Value, BackendError>> {
                let handler = handler_arc.clone();
                Box::pin(async move {
                    let awaitable = Python::with_gil(|py| -> PyResult<PyObject> {
                        let py_args = json_to_py(py, &args)?;
                        let coro = handler.call1(py, (py_args,))?;
                        Ok(coro)
                    })
                    .map_err(|e| BackendError::Other(format!("tool callback invoke: {e}")))?;
                    let result_obj = Python::with_gil(|py| {
                        pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
                    })
                    .map_err(|e| BackendError::Other(format!("tool callback into_future: {e}")))?
                    .await
                    .map_err(|e| BackendError::Other(format!("tool callback error: {e}")))?;
                    Python::with_gil(|py| py_any_to_json(result_obj.bind(py)))
                        .map_err(|e| BackendError::Other(format!("tool result to JSON: {e}")))
                })
            },
        );
        Ok(Self { inner })
    }

    /// Tool name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Tool description.
    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }

    /// JSON schema (as a Python dict via `json.loads`).
    fn json_schema<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let s = serde_json::to_string(&self.inner.json_schema)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let json = py.import_bound("json")?;
        json.call_method1("loads", (s,))
    }

    fn __repr__(&self) -> String {
        format!("ToolSpec(name={:?})", self.inner.name)
    }
}

// ============================================================================
// StopCondition
// ============================================================================

/// Stop condition for an agent session.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "StopCondition", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyStopCondition {
    pub inner: StopCondition,
}

#[pymethods]
impl PyStopCondition {
    /// `NoMoreToolCalls` — stop when the agent emits a turn with no
    /// tool calls.
    #[classmethod]
    fn no_more_tool_calls(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self { inner: StopCondition::NoMoreToolCalls }
    }

    /// `FirstJsonMatching(name)` — stop when the final-text response
    /// parses as JSON matching the named schema.
    #[classmethod]
    fn first_json_matching(_cls: &Bound<'_, pyo3::types::PyType>, name: String) -> Self {
        Self { inner: StopCondition::FirstJsonMatching(name) }
    }

    /// `FixedTurns(n)` — stop after exactly N turns regardless.
    #[classmethod]
    fn fixed_turns(_cls: &Bound<'_, pyo3::types::PyType>, n: u32) -> Self {
        Self { inner: StopCondition::FixedTurns(n) }
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

// ============================================================================
// AgenticSession
// ============================================================================

/// Description of one agent session.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "AgenticSession")]
#[derive(Clone)]
pub struct PyAgenticSession {
    pub inner: AgenticSession,
}

#[pymethods]
impl PyAgenticSession {
    #[new]
    fn new(seed_user: String) -> Self {
        Self { inner: AgenticSession::new(seed_user) }
    }

    fn with_system(mut slf: PyRefMut<'_, Self>, system: String) -> PyRefMut<'_, Self> {
        slf.inner.system = Some(system);
        slf
    }

    fn with_tool(mut slf: PyRefMut<'_, Self>, tool: PyToolSpec) -> PyRefMut<'_, Self> {
        slf.inner.tools.push(tool.inner);
        slf
    }

    fn with_tools(mut slf: PyRefMut<'_, Self>, tools: Vec<PyToolSpec>) -> PyRefMut<'_, Self> {
        slf.inner.tools = tools.into_iter().map(|t| t.inner).collect();
        slf
    }

    fn with_max_turns(mut slf: PyRefMut<'_, Self>, n: u32) -> PyRefMut<'_, Self> {
        slf.inner.max_turns = n;
        slf
    }

    fn with_stop_on(mut slf: PyRefMut<'_, Self>, stop: PyStopCondition) -> PyRefMut<'_, Self> {
        slf.inner.stop_on = stop.inner;
        slf
    }

    #[getter]
    fn seed_user(&self) -> &str {
        &self.inner.seed_user
    }

    #[getter]
    fn system(&self) -> Option<&str> {
        self.inner.system.as_deref()
    }

    #[getter]
    fn max_turns(&self) -> u32 {
        self.inner.max_turns
    }
}

// ============================================================================
// AgenticOutcome / TurnRecord / ToolCallRecord
// ============================================================================

/// A single turn in an agent session.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "TurnRecord")]
#[derive(Clone)]
pub struct PyTurnRecord {
    pub inner: TurnRecord,
}

#[pymethods]
impl PyTurnRecord {
    #[getter]
    fn role(&self) -> &str {
        &self.inner.role
    }
    #[getter]
    fn text(&self) -> &str {
        &self.inner.text
    }
    fn __repr__(&self) -> String {
        format!("TurnRecord(role={:?}, len={})", self.inner.role, self.inner.text.len())
    }
}

/// A record of one tool call made during a session.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "ToolCallRecord")]
#[derive(Clone)]
pub struct PyToolCallRecord {
    pub inner: ToolCallRecord,
}

#[pymethods]
impl PyToolCallRecord {
    #[getter]
    fn tool(&self) -> &str {
        &self.inner.tool
    }
    fn arguments<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        json_to_py(py, &self.inner.arguments)
    }
    fn result<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        json_to_py(py, &self.inner.result)
    }
    fn __repr__(&self) -> String {
        format!("ToolCallRecord(tool={:?})", self.inner.tool)
    }
}

/// Result of running an agent session.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "AgenticOutcome")]
#[derive(Clone)]
pub struct PyAgenticOutcome {
    pub inner: AgenticOutcome,
}

#[pymethods]
impl PyAgenticOutcome {
    #[getter]
    fn final_text(&self) -> &str {
        &self.inner.final_text
    }
    #[getter]
    fn turns(&self) -> Vec<PyTurnRecord> {
        self.inner.turns.iter().cloned().map(|t| PyTurnRecord { inner: t }).collect()
    }
    #[getter]
    fn tool_invocations(&self) -> Vec<PyToolCallRecord> {
        self.inner
            .tool_invocations
            .iter()
            .cloned()
            .map(|t| PyToolCallRecord { inner: t })
            .collect()
    }
    fn __repr__(&self) -> String {
        format!(
            "AgenticOutcome(turns={}, tools_called={})",
            self.inner.turns.len(),
            self.inner.tool_invocations.len(),
        )
    }
}

// ============================================================================
// AgenticAgent
// ============================================================================

/// A Python handle around a Rust `AgenticAgent`.
///
/// Construct via `AgenticAgent.from_python(label, driver)` where
/// `driver` is a Python object with `run_session(session)` and
/// `complete_one(prompt)` `async def` methods (each returning a
/// coroutine).
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "AgenticAgent")]
#[derive(Clone)]
pub struct PyAgenticAgent {
    pub inner: Arc<AgenticAgent>,
}

impl PyAgenticAgent {
    pub fn inner_arc(&self) -> Arc<AgenticAgent> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyAgenticAgent {
    /// Build an `AgenticAgent` from a Python driver object.
    #[classmethod]
    fn from_python(
        _cls: &Bound<'_, pyo3::types::PyType>,
        label: String,
        driver: PyObject,
    ) -> Self {
        let driver_arc: Arc<dyn AgenticDriver> = Arc::new(PythonAgenticDriver { inner: driver });
        Self { inner: Arc::new(AgenticAgent::new(label, driver_arc)) }
    }

    /// Backend label.
    #[getter]
    fn label(&self) -> String {
        Backend::label(self.inner.as_ref()).to_string()
    }

    /// Run a session (awaitable resolving to an `AgenticOutcome`).
    fn run<'py>(
        &self,
        py: Python<'py>,
        session: PyAgenticSession,
    ) -> PyResult<Bound<'py, PyAny>> {
        let agent = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let outcome = agent.run(session.inner).await.map_err(backend_err)?;
            Ok(PyAgenticOutcome { inner: outcome })
        })
    }

    /// `AgenticAgent` also satisfies the narrow `Backend` contract —
    /// convert into a `Backend` handle that any extractor accepts.
    fn as_backend(&self) -> PyBackend {
        PyBackend { inner: self.inner.clone() as Arc<dyn Backend> }
    }

    fn __repr__(&self) -> String {
        format!("AgenticAgent(label={:?})", Backend::label(self.inner.as_ref()))
    }
}

/// `AgenticDriver` bridge over a Python object exposing `run_session`
/// and `complete_one` async methods.
struct PythonAgenticDriver {
    inner: PyObject,
}

#[async_trait]
impl AgenticDriver for PythonAgenticDriver {
    async fn run_session(&self, session: AgenticSession) -> Result<AgenticOutcome, BackendError> {
        let awaitable = Python::with_gil(|py| -> PyResult<PyObject> {
            let py_session = PyAgenticSession { inner: session }.into_py(py);
            self.inner.call_method1(py, "run_session", (py_session,))
        })
        .map_err(|e| BackendError::Other(format!("driver.run_session call: {e}")))?;
        let outcome_obj = Python::with_gil(|py| {
            pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
        })
        .map_err(|e| BackendError::Other(format!("driver.run_session into_future: {e}")))?
        .await
        .map_err(|e| BackendError::Other(format!("driver.run_session error: {e}")))?;
        Python::with_gil(|py| {
            let py_outcome: PyAgenticOutcome = outcome_obj.extract(py).map_err(|e| {
                BackendError::Other(format!("driver.run_session result not AgenticOutcome: {e}"))
            })?;
            Ok::<_, BackendError>(py_outcome.inner)
        })
    }

    async fn complete_one(&self, prompt: Prompt) -> Result<String, BackendError> {
        let awaitable = Python::with_gil(|py| -> PyResult<PyObject> {
            let py_prompt = PyPrompt { inner: prompt }.into_py(py);
            self.inner.call_method1(py, "complete_one", (py_prompt,))
        })
        .map_err(|e| BackendError::Other(format!("driver.complete_one call: {e}")))?;
        let text_obj = Python::with_gil(|py| {
            pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
        })
        .map_err(|e| BackendError::Other(format!("driver.complete_one into_future: {e}")))?
        .await
        .map_err(|e| BackendError::Other(format!("driver.complete_one error: {e}")))?;
        Python::with_gil(|py| {
            text_obj
                .extract::<String>(py)
                .map_err(|e| BackendError::Other(format!("driver.complete_one not str: {e}")))
        })
    }
}

// ============================================================================
// Default OntologyStore tools
// ============================================================================

/// Build the bundled tool palette over a live `MemStore`. Returns a
/// list of `ToolSpec`. Mirrors
/// [`atomr_ontology::extract::store_tools::default_store_tools`].
#[pyfunction]
fn default_store_tools_py(store: &PyMemStore) -> Vec<PyToolSpec> {
    let store_arc = store.inner_arc();
    default_store_tools(store_arc).into_iter().map(|t| PyToolSpec { inner: t }).collect()
}

// ============================================================================
// AgenticTaxonomyInducer
// ============================================================================

/// Multi-turn taxonomy inducer driven by an `AgenticAgent`.
#[pyclass(
    module = "atomr_ontology._atomr_ontology.agents",
    name = "AgenticTaxonomyInducer"
)]
#[derive(Clone)]
pub struct PyAgenticTaxonomyInducer {
    inner: AgenticTaxonomyInducer,
}

#[pymethods]
impl PyAgenticTaxonomyInducer {
    #[new]
    #[pyo3(signature = (agent, tools=Vec::new()))]
    fn new(agent: PyAgenticAgent, tools: Vec<PyToolSpec>) -> Self {
        let tool_vec: Vec<ToolSpec> = tools.into_iter().map(|t| t.inner).collect();
        Self { inner: AgenticTaxonomyInducer::new(agent.inner_arc(), tool_vec) }
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    fn with_max_turns(slf: PyRef<'_, Self>, n: u32) -> Self {
        Self { inner: slf.inner.clone().with_max_turns(n) }
    }

    /// Returns a coroutine resolving to `(proposals, activity)`.
    fn induce<'py>(
        &self,
        py: Python<'py>,
        candidate_classes: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inducer = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (props, activity) = inducer.induce(&candidate_classes).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for p in props {
                    list.append(PySubclassProposal::from(p).into_py(py))?;
                }
                Ok::<PyObject, PyErr>(
                    (list.unbind(), PyActivity::from(activity).into_py(py)).into_py(py),
                )
            })
        })
    }
}

// ============================================================================
// AgenticAxiomMiner
// ============================================================================

/// Multi-turn axiom miner driven by an `AgenticAgent`.
#[pyclass(module = "atomr_ontology._atomr_ontology.agents", name = "AgenticAxiomMiner")]
#[derive(Clone)]
pub struct PyAgenticAxiomMiner {
    inner: AgenticAxiomMiner,
}

#[pymethods]
impl PyAgenticAxiomMiner {
    #[new]
    #[pyo3(signature = (agent, tools=Vec::new()))]
    fn new(agent: PyAgenticAgent, tools: Vec<PyToolSpec>) -> Self {
        let tool_vec: Vec<ToolSpec> = tools.into_iter().map(|t| t.inner).collect();
        Self { inner: AgenticAxiomMiner::new(agent.inner_arc(), tool_vec) }
    }

    fn with_system_prompt(slf: PyRef<'_, Self>, prompt: String) -> Self {
        Self { inner: slf.inner.clone().with_system_prompt(prompt) }
    }

    fn with_max_turns(slf: PyRef<'_, Self>, n: u32) -> Self {
        Self { inner: slf.inner.clone().with_max_turns(n) }
    }

    /// Returns a coroutine resolving to `(proposals, activity)`.
    fn mine<'py>(&self, py: Python<'py>, context: String) -> PyResult<Bound<'py, PyAny>> {
        let miner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (ps, activity) = miner.mine(&context).await.map_err(backend_err)?;
            Python::with_gil(|py| {
                let list = PyList::empty_bound(py);
                for p in ps {
                    list.append(PyAxiomProposal::from(p).into_py(py))?;
                }
                Ok::<PyObject, PyErr>(
                    (list.unbind(), PyActivity::from(activity).into_py(py)).into_py(py),
                )
            })
        })
    }
}

// ============================================================================
// JSON / Python conversion helpers
// ============================================================================

fn py_any_to_json(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    let py = value.py();
    let json = py.import_bound("json")?;
    let s: String = json.call_method1("dumps", (value,))?.extract()?;
    serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn json_to_py<'py>(py: Python<'py>, value: &serde_json::Value) -> PyResult<Bound<'py, PyAny>> {
    let s = serde_json::to_string(value).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let json = py.import_bound("json")?;
    json.call_method1("loads", (s,))
}

// ============================================================================
// Module registration
// ============================================================================

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyToolSpec>()?;
    m.add_class::<PyStopCondition>()?;
    m.add_class::<PyAgenticSession>()?;
    m.add_class::<PyTurnRecord>()?;
    m.add_class::<PyToolCallRecord>()?;
    m.add_class::<PyAgenticOutcome>()?;
    m.add_class::<PyAgenticAgent>()?;
    m.add_class::<PyAgenticTaxonomyInducer>()?;
    m.add_class::<PyAgenticAxiomMiner>()?;
    m.add_function(wrap_pyfunction!(default_store_tools_py, m)?)?;
    Ok(())
}

// Reserved so the `induce` module's `PyDict` import stays warning-free
// when this file is the only consumer.
#[allow(dead_code)]
fn _unused_pydict(_: &Bound<'_, PyDict>) {}
