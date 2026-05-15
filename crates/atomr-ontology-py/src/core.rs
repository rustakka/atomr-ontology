//! Python wrappers for the Tier-1 `atomr-ontology-core` data types.
//!
//! Every Rust type here is wrapped as a `#[pyclass]` carrying its
//! `Clone`-able Rust value by composition. We expose ergonomic
//! constructors plus typed getters/setters; the Rust builder
//! patterns (`with_*`) are mirrored so users can chain calls if they
//! prefer.

use std::collections::BTreeMap;

use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyType};

use atomr_ontology::core::{
    Axiom, AxiomId, AxiomKind, Cardinality, Datatype, Edge, EdgeId, EdgeType, Iri, IriError, Namespace,
    Node, NodeId, NodeType, Ontology, Property, PropertyType, PropertyValue, Record, RecordId, Schema,
    Vocabulary,
};
use atomr_ontology::provenance::ProvenanceId;

use crate::errors::{iri_err, ontology_err};

// ============================================================================
// Iri
// ============================================================================

/// A validated Internationalized Resource Identifier (RFC 3987).
///
/// Constructed via `Iri(value)`; raises `IriError` on empty or
/// whitespace-containing strings.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Iri", frozen, eq, hash)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyIri {
    pub inner: Iri,
}

#[pymethods]
impl PyIri {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        Iri::new(value.to_string()).map(|i| PyIri { inner: i }).map_err(iri_err)
    }

    /// Construct without validation. Use sparingly; the caller is
    /// responsible for guaranteeing well-formedness.
    #[classmethod]
    fn from_unchecked(_cls: &Bound<'_, PyType>, value: &str) -> Self {
        PyIri { inner: Iri::from_unchecked(value.to_string()) }
    }

    /// The underlying string.
    #[getter]
    fn value(&self) -> &str {
        self.inner.as_str()
    }

    fn __str__(&self) -> &str {
        self.inner.as_str()
    }

    fn __repr__(&self) -> String {
        format!("Iri({:?})", self.inner.as_str())
    }
}

impl From<Iri> for PyIri {
    fn from(inner: Iri) -> Self {
        PyIri { inner }
    }
}

// ============================================================================
// IDs — NodeId, EdgeId, RecordId, ProvenanceId, AxiomId
// ============================================================================

macro_rules! py_id {
    ($PyTy:ident, $Inner:path, $name:literal, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(module = "atomr_ontology._atomr_ontology.core", name = $name, frozen, eq, hash)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $PyTy {
            pub inner: $Inner,
        }

        #[pymethods]
        impl $PyTy {
            /// Generate a fresh random id.
            #[classmethod]
            fn new_random(_cls: &Bound<'_, PyType>) -> Self {
                $PyTy { inner: <$Inner>::new_random() }
            }

            /// Content-addressed id derived deterministically from the
            /// supplied bytes. Identical input → identical id.
            #[classmethod]
            fn content_address(_cls: &Bound<'_, PyType>, input: &[u8]) -> Self {
                $PyTy { inner: <$Inner>::content_address(input) }
            }

            /// Parse a 64-character hex string.
            #[classmethod]
            fn from_hex(_cls: &Bound<'_, PyType>, value: &str) -> PyResult<Self> {
                value
                    .parse::<$Inner>()
                    .map(|i| $PyTy { inner: i })
                    .map_err(|e| PyValueError::new_err(e.to_string()))
            }

            /// Wrap a raw 32-byte sequence without validation.
            #[classmethod]
            fn from_bytes(_cls: &Bound<'_, PyType>, value: &[u8]) -> PyResult<Self> {
                if value.len() != 32 {
                    return Err(PyValueError::new_err(format!(
                        "expected 32 bytes, got {}",
                        value.len()
                    )));
                }
                let mut buf = [0u8; 32];
                buf.copy_from_slice(value);
                Ok($PyTy { inner: <$Inner>::from_bytes(buf) })
            }

            /// Return the underlying 32 raw bytes.
            fn as_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
                PyBytes::new_bound(py, self.inner.as_bytes())
            }

            /// Lower-case hex encoding.
            fn hex(&self) -> String {
                self.inner.to_string()
            }

            fn __str__(&self) -> String {
                self.inner.to_string()
            }

            fn __repr__(&self) -> String {
                format!("{}({})", $name, self.inner)
            }
        }

        impl From<$Inner> for $PyTy {
            fn from(inner: $Inner) -> Self {
                $PyTy { inner }
            }
        }
    };
}

py_id!(PyNodeId, NodeId, "NodeId", "Opaque 32-byte node identifier.");
py_id!(PyEdgeId, EdgeId, "EdgeId", "Opaque 32-byte edge identifier.");
py_id!(PyRecordId, RecordId, "RecordId", "Opaque 32-byte record identifier.");

// AxiomId is structurally similar but uses a different constructor path
// (no `new_random` / `content_address`); wrap by hand for parity with the
// Rust crate, exposing only its display + serde behaviour.
/// Opaque 32-byte content-addressed axiom identifier.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "AxiomId", frozen, eq, hash)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyAxiomId {
    pub inner: AxiomId,
}

#[pymethods]
impl PyAxiomId {
    /// Wrap a raw 32-byte sequence without validation.
    #[classmethod]
    fn from_bytes(_cls: &Bound<'_, PyType>, value: &[u8]) -> PyResult<Self> {
        if value.len() != 32 {
            return Err(PyValueError::new_err(format!(
                "expected 32 bytes, got {}",
                value.len()
            )));
        }
        let mut buf = [0u8; 32];
        buf.copy_from_slice(value);
        Ok(PyAxiomId { inner: AxiomId(buf) })
    }

    fn as_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.inner.0)
    }

    fn hex(&self) -> String {
        self.inner.to_string()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("AxiomId({})", self.inner)
    }
}

impl From<AxiomId> for PyAxiomId {
    fn from(inner: AxiomId) -> Self {
        PyAxiomId { inner }
    }
}

// ProvenanceId is in the provenance crate (re-exported from core).
/// Opaque 32-byte provenance identifier (PROV-O activity / entity).
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "ProvenanceId", frozen, eq, hash)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyProvenanceId {
    pub inner: ProvenanceId,
}

#[pymethods]
impl PyProvenanceId {
    #[classmethod]
    fn new_random(_cls: &Bound<'_, PyType>) -> Self {
        PyProvenanceId { inner: ProvenanceId::new_random() }
    }

    #[classmethod]
    fn content_address(_cls: &Bound<'_, PyType>, input: &[u8]) -> Self {
        PyProvenanceId { inner: ProvenanceId::content_address(input) }
    }

    #[classmethod]
    fn from_hex(_cls: &Bound<'_, PyType>, value: &str) -> PyResult<Self> {
        value
            .parse::<ProvenanceId>()
            .map(|i| PyProvenanceId { inner: i })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[classmethod]
    fn from_bytes(_cls: &Bound<'_, PyType>, value: &[u8]) -> PyResult<Self> {
        if value.len() != 32 {
            return Err(PyValueError::new_err(format!(
                "expected 32 bytes, got {}",
                value.len()
            )));
        }
        let mut buf = [0u8; 32];
        buf.copy_from_slice(value);
        Ok(PyProvenanceId { inner: ProvenanceId::from_bytes(buf) })
    }

    fn as_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, self.inner.as_bytes())
    }

    fn hex(&self) -> String {
        self.inner.to_string()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("ProvenanceId({})", self.inner)
    }
}

impl From<ProvenanceId> for PyProvenanceId {
    fn from(inner: ProvenanceId) -> Self {
        PyProvenanceId { inner }
    }
}

// ============================================================================
// Namespace + Vocabulary
// ============================================================================

/// A prefix-bound namespace, e.g. `rdf:` → `http://www.w3.org/1999/02/22-rdf-syntax-ns#`.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Namespace")]
#[derive(Clone)]
pub struct PyNamespace {
    pub inner: Namespace,
}

#[pymethods]
impl PyNamespace {
    #[new]
    fn new(prefix: &str, base: PyIri) -> Self {
        PyNamespace { inner: Namespace::new(prefix.to_string(), base.inner) }
    }

    #[getter]
    fn prefix(&self) -> &str {
        &self.inner.prefix
    }

    #[getter]
    fn base(&self) -> PyIri {
        PyIri::from(self.inner.base.clone())
    }

    /// Resolve a local name into its expanded IRI.
    fn expand(&self, local_name: &str) -> PyIri {
        PyIri::from(self.inner.expand(local_name))
    }

    fn __repr__(&self) -> String {
        format!("Namespace({:?}, {:?})", self.inner.prefix, self.inner.base.as_str())
    }
}

/// A collection of [`Namespace`]s keyed by prefix.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Vocabulary")]
#[derive(Clone, Default)]
pub struct PyVocabulary {
    pub inner: Vocabulary,
}

#[pymethods]
impl PyVocabulary {
    #[new]
    fn new() -> Self {
        PyVocabulary { inner: Vocabulary::new() }
    }

    /// The seven standard W3C / schema.org bindings.
    #[classmethod]
    fn with_standard_bindings(_cls: &Bound<'_, PyType>) -> Self {
        PyVocabulary { inner: Vocabulary::with_standard_bindings() }
    }

    /// Bind a prefix to a base IRI, replacing any existing binding.
    fn bind(mut slf: PyRefMut<'_, Self>, prefix: String, base: PyIri) -> PyRefMut<'_, Self> {
        slf.inner.bind(prefix.to_string(), base.inner);
        slf
    }

    /// Look up the base IRI for a prefix.
    fn base(&self, prefix: &str) -> Option<PyIri> {
        self.inner.base(prefix).cloned().map(PyIri::from)
    }

    /// Expand a CURIE of the form `prefix:local`.
    fn expand_curie(&self, curie: &str) -> Option<PyIri> {
        self.inner.expand_curie(curie).map(PyIri::from)
    }

    /// All bindings as a list of `Namespace` rows, sorted by prefix.
    fn items(&self) -> Vec<PyNamespace> {
        self.inner.iter().map(|n| PyNamespace { inner: n }).collect()
    }

    fn __repr__(&self) -> String {
        let count = self.inner.iter().count();
        format!("Vocabulary(bindings={count})")
    }
}

// ============================================================================
// PropertyValue (tagged union)
// ============================================================================

/// A typed property value attached to a Node or Edge.
///
/// Construct via the typed classmethod constructors (`PropertyValue.string(...)`,
/// `PropertyValue.integer(...)`, …). The `kind` and `value` attributes
/// expose the tagged-union shape; `to_python()` returns a plain Python
/// value where possible.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "PropertyValue", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyPropertyValue {
    pub inner: PropertyValue,
}

#[pymethods]
impl PyPropertyValue {
    #[classmethod]
    fn string(_cls: &Bound<'_, PyType>, value: String) -> Self {
        Self { inner: PropertyValue::String(value) }
    }
    #[classmethod]
    fn integer(_cls: &Bound<'_, PyType>, value: i64) -> Self {
        Self { inner: PropertyValue::Integer(value) }
    }
    #[classmethod]
    fn float(_cls: &Bound<'_, PyType>, value: f64) -> Self {
        Self { inner: PropertyValue::Float(value) }
    }
    #[classmethod]
    fn boolean(_cls: &Bound<'_, PyType>, value: bool) -> Self {
        Self { inner: PropertyValue::Bool(value) }
    }
    #[classmethod]
    fn datetime(
        _cls: &Bound<'_, PyType>,
        value: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self { inner: PropertyValue::DateTime(value) }
    }
    #[classmethod]
    fn iri(_cls: &Bound<'_, PyType>, value: PyIri) -> Self {
        Self { inner: PropertyValue::Iri(value.inner) }
    }
    #[classmethod]
    fn bytes(_cls: &Bound<'_, PyType>, value: &[u8]) -> Self {
        Self { inner: PropertyValue::Bytes(value.to_vec()) }
    }
    #[classmethod]
    fn json(_cls: &Bound<'_, PyType>, value: &str) -> PyResult<Self> {
        let v: serde_json::Value =
            serde_json::from_str(value).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner: PropertyValue::Json(v) })
    }
    #[classmethod]
    fn null(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: PropertyValue::Null }
    }

    /// Build the most appropriate variant from a plain Python value
    /// (str, int, float, bool, bytes, None, Iri).
    #[classmethod]
    fn from_python(_cls: &Bound<'_, PyType>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        py_to_property_value(value)
    }

    /// Tag of the variant: `"string" | "integer" | "float" | "bool" |`
    /// `"date_time" | "iri" | "bytes" | "json" | "null"`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            PropertyValue::String(_) => "string",
            PropertyValue::Integer(_) => "integer",
            PropertyValue::Float(_) => "float",
            PropertyValue::Bool(_) => "bool",
            PropertyValue::DateTime(_) => "date_time",
            PropertyValue::Iri(_) => "iri",
            PropertyValue::Bytes(_) => "bytes",
            PropertyValue::Json(_) => "json",
            PropertyValue::Null => "null",
        }
    }

    /// The value as the most natural Python type.
    fn to_python(&self, py: Python<'_>) -> PyResult<PyObject> {
        property_value_to_py(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        format!("PropertyValue({:?})", self.inner)
    }
}

impl From<PropertyValue> for PyPropertyValue {
    fn from(inner: PropertyValue) -> Self {
        Self { inner }
    }
}

pub fn py_to_property_value(value: &Bound<'_, PyAny>) -> PyResult<PyPropertyValue> {
    if value.is_none() {
        return Ok(PyPropertyValue { inner: PropertyValue::Null });
    }
    if let Ok(existing) = value.extract::<PyPropertyValue>() {
        return Ok(existing);
    }
    if let Ok(iri) = value.extract::<PyIri>() {
        return Ok(PyPropertyValue { inner: PropertyValue::Iri(iri.inner) });
    }
    if let Ok(b) = value.extract::<bool>() {
        return Ok(PyPropertyValue { inner: PropertyValue::Bool(b) });
    }
    if let Ok(i) = value.extract::<i64>() {
        return Ok(PyPropertyValue { inner: PropertyValue::Integer(i) });
    }
    if let Ok(f) = value.extract::<f64>() {
        return Ok(PyPropertyValue { inner: PropertyValue::Float(f) });
    }
    if let Ok(s) = value.extract::<String>() {
        return Ok(PyPropertyValue { inner: PropertyValue::String(s) });
    }
    if let Ok(bytes) = value.extract::<&[u8]>() {
        return Ok(PyPropertyValue { inner: PropertyValue::Bytes(bytes.to_vec()) });
    }
    if let Ok(dt) = value.extract::<chrono::DateTime<chrono::Utc>>() {
        return Ok(PyPropertyValue { inner: PropertyValue::DateTime(dt) });
    }
    Err(PyTypeError::new_err(format!(
        "cannot convert {} into PropertyValue",
        value.get_type().name()?,
    )))
}

pub fn property_value_to_py(py: Python<'_>, v: &PropertyValue) -> PyResult<PyObject> {
    Ok(match v {
        PropertyValue::String(s) => s.into_py(py),
        PropertyValue::Integer(i) => i.into_py(py),
        PropertyValue::Float(f) => f.into_py(py),
        PropertyValue::Bool(b) => b.into_py(py),
        PropertyValue::DateTime(dt) => dt.into_py(py),
        PropertyValue::Iri(iri) => PyIri::from(iri.clone()).into_py(py),
        PropertyValue::Bytes(b) => PyBytes::new_bound(py, b).into_py(py),
        PropertyValue::Json(j) => {
            let s = serde_json::to_string(j).map_err(|e| PyValueError::new_err(e.to_string()))?;
            let json = py.import_bound("json")?;
            json.call_method1("loads", (s,))?.into_py(py)
        }
        PropertyValue::Null => py.None(),
    })
}

fn property_map_from_py(map: Option<&Bound<'_, PyDict>>) -> PyResult<BTreeMap<String, PropertyValue>> {
    let mut out = BTreeMap::new();
    if let Some(d) = map {
        for (k, v) in d.iter() {
            let key: String = k.extract()?;
            let value = py_to_property_value(&v)?.inner;
            out.insert(key, value);
        }
    }
    Ok(out)
}

fn property_map_to_py<'py>(
    py: Python<'py>,
    m: &BTreeMap<String, PropertyValue>,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new_bound(py);
    for (k, v) in m {
        d.set_item(k, property_value_to_py(py, v)?)?;
    }
    Ok(d)
}

// ============================================================================
// Property
// ============================================================================

/// A property attached to a Node or Edge.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Property")]
#[derive(Clone)]
pub struct PyProperty {
    pub inner: Property,
}

#[pymethods]
impl PyProperty {
    #[new]
    fn new(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let pv = py_to_property_value(value)?;
        Ok(PyProperty { inner: Property::new(name.to_string(), pv.inner) })
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn value(&self) -> PyPropertyValue {
        PyPropertyValue { inner: self.inner.value.clone() }
    }

    fn __repr__(&self) -> String {
        format!("Property({:?}, {:?})", self.inner.name, self.inner.value)
    }
}

// ============================================================================
// Node
// ============================================================================

/// A labeled-property-graph node.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Node")]
#[derive(Clone)]
pub struct PyNode {
    pub inner: Node,
}

#[pymethods]
impl PyNode {
    /// Build a node. When `iri` is given, the node id is content-
    /// addressed from the IRI; otherwise it is random.
    #[new]
    #[pyo3(signature = (type_name, iri=None, properties=None))]
    fn new(
        type_name: &str,
        iri: Option<PyIri>,
        properties: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut node = match iri {
            Some(iri) => Node::from_iri(iri.inner, type_name.to_string()),
            None => Node::new(type_name.to_string()),
        };
        let props = property_map_from_py(properties)?;
        for (k, v) in props {
            node = node.with_property(k, v);
        }
        Ok(PyNode { inner: node })
    }

    #[classmethod]
    fn from_iri(_cls: &Bound<'_, PyType>, iri: PyIri, type_name: &str) -> Self {
        PyNode { inner: Node::from_iri(iri.inner, type_name.to_string()) }
    }

    fn with_label(mut slf: PyRefMut<'_, Self>, type_name: String) -> PyRefMut<'_, Self> {
        slf.inner.types.push(type_name.to_string());
        slf
    }

    fn with_iri(mut slf: PyRefMut<'_, Self>, iri: PyIri) -> PyRefMut<'_, Self> {
        slf.inner.iri = Some(iri.inner);
        slf
    }

    fn with_property<'py>(
        mut slf: PyRefMut<'py, Self>,
        name: String,
        value: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pv = py_to_property_value(&value)?;
        slf.inner.properties.insert(name.to_string(), pv.inner);
        Ok(slf)
    }

    fn property(&self, name: &str) -> Option<PyPropertyValue> {
        self.inner.properties.get(name).cloned().map(|v| PyPropertyValue { inner: v })
    }

    fn has_type(&self, type_name: &str) -> bool {
        self.inner.has_type(type_name)
    }

    #[getter]
    fn id(&self) -> PyNodeId {
        PyNodeId::from(self.inner.id)
    }

    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }

    #[getter]
    fn types(&self) -> Vec<String> {
        self.inner.types.clone()
    }

    #[getter]
    fn properties<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        property_map_to_py(py, &self.inner.properties)
    }

    fn __repr__(&self) -> String {
        format!("Node(id={}, types={:?})", self.inner.id, self.inner.types)
    }
}

impl From<Node> for PyNode {
    fn from(inner: Node) -> Self {
        PyNode { inner }
    }
}

// ============================================================================
// Edge
// ============================================================================

/// A directed labeled-property-graph edge.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Edge")]
#[derive(Clone)]
pub struct PyEdge {
    pub inner: Edge,
}

#[pymethods]
impl PyEdge {
    /// Construct an edge between two nodes; the edge id is content-
    /// addressed over `(source, label, target)` so duplicate triples
    /// share an id.
    #[new]
    #[pyo3(signature = (source, label, target, properties=None))]
    fn new(
        source: PyNodeId,
        label: &str,
        target: PyNodeId,
        properties: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut edge = Edge::between(source.inner, label.to_string(), target.inner);
        let props = property_map_from_py(properties)?;
        for (k, v) in props {
            edge = edge.with_property(k, v);
        }
        Ok(PyEdge { inner: edge })
    }

    #[classmethod]
    fn between(_cls: &Bound<'_, PyType>, source: PyNodeId, label: &str, target: PyNodeId) -> Self {
        PyEdge { inner: Edge::between(source.inner, label.to_string(), target.inner) }
    }

    fn with_property<'py>(
        mut slf: PyRefMut<'py, Self>,
        name: String,
        value: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pv = py_to_property_value(&value)?;
        slf.inner.properties.insert(name.to_string(), pv.inner);
        Ok(slf)
    }

    fn property(&self, name: &str) -> Option<PyPropertyValue> {
        self.inner.properties.get(name).cloned().map(|v| PyPropertyValue { inner: v })
    }

    #[getter]
    fn id(&self) -> PyEdgeId {
        PyEdgeId::from(self.inner.id)
    }
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }
    #[getter]
    fn source(&self) -> PyNodeId {
        PyNodeId::from(self.inner.source)
    }
    #[getter]
    fn target(&self) -> PyNodeId {
        PyNodeId::from(self.inner.target)
    }
    #[getter]
    fn properties<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        property_map_to_py(py, &self.inner.properties)
    }

    fn __repr__(&self) -> String {
        format!("Edge(id={}, label={:?})", self.inner.id, self.inner.label)
    }
}

impl From<Edge> for PyEdge {
    fn from(inner: Edge) -> Self {
        PyEdge { inner }
    }
}

// ============================================================================
// Datatype + Cardinality + PropertyType + NodeType + EdgeType + Schema
// ============================================================================

/// Datatype of a property value, mapped to XSD on the RDF side.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Datatype", eq, hash, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyDatatype {
    String,
    Integer,
    Float,
    Bool,
    DateTime,
    Iri,
    Bytes,
    Json,
}

#[pymethods]
impl PyDatatype {
    fn __repr__(&self) -> String {
        format!("Datatype.{:?}", self)
    }
}

impl From<Datatype> for PyDatatype {
    fn from(d: Datatype) -> Self {
        match d {
            Datatype::String => PyDatatype::String,
            Datatype::Integer => PyDatatype::Integer,
            Datatype::Float => PyDatatype::Float,
            Datatype::Bool => PyDatatype::Bool,
            Datatype::DateTime => PyDatatype::DateTime,
            Datatype::Iri => PyDatatype::Iri,
            Datatype::Bytes => PyDatatype::Bytes,
            Datatype::Json => PyDatatype::Json,
        }
    }
}

impl From<PyDatatype> for Datatype {
    fn from(d: PyDatatype) -> Self {
        match d {
            PyDatatype::String => Datatype::String,
            PyDatatype::Integer => Datatype::Integer,
            PyDatatype::Float => Datatype::Float,
            PyDatatype::Bool => Datatype::Bool,
            PyDatatype::DateTime => Datatype::DateTime,
            PyDatatype::Iri => Datatype::Iri,
            PyDatatype::Bytes => Datatype::Bytes,
            PyDatatype::Json => Datatype::Json,
        }
    }
}

/// `Cardinality { min, max }` — `max=None` means unbounded.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Cardinality", eq, hash, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCardinality {
    pub inner: Cardinality,
}

#[pymethods]
impl PyCardinality {
    #[new]
    #[pyo3(signature = (min=0, max=None))]
    fn new(min: u32, max: Option<u32>) -> Self {
        PyCardinality { inner: Cardinality::new(min, max) }
    }

    #[classattr]
    const ANY: PyCardinality = PyCardinality { inner: Cardinality::ANY };
    #[classattr]
    const ONE: PyCardinality = PyCardinality { inner: Cardinality::ONE };
    #[classattr]
    const OPTIONAL: PyCardinality = PyCardinality { inner: Cardinality::OPTIONAL };
    #[classattr]
    const AT_LEAST_ONE: PyCardinality = PyCardinality { inner: Cardinality::AT_LEAST_ONE };

    #[getter]
    fn min(&self) -> u32 {
        self.inner.min
    }
    #[getter]
    fn max(&self) -> Option<u32> {
        self.inner.max
    }

    /// True when `count` lies within the bound.
    fn contains(&self, count: u32) -> bool {
        self.inner.contains(count)
    }

    fn __repr__(&self) -> String {
        format!("Cardinality(min={}, max={:?})", self.inner.min, self.inner.max)
    }
}

/// Declared shape of a property — datatype + cardinality.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "PropertyType")]
#[derive(Clone)]
pub struct PyPropertyType {
    pub inner: PropertyType,
}

#[pymethods]
impl PyPropertyType {
    #[new]
    #[pyo3(signature = (name, datatype, cardinality=PyCardinality { inner: Cardinality::ANY }, iri=None, description=None))]
    fn new(
        name: &str,
        datatype: PyDatatype,
        cardinality: PyCardinality,
        iri: Option<PyIri>,
        description: Option<String>,
    ) -> Self {
        PyPropertyType {
            inner: PropertyType {
                name: name.to_string(),
                datatype: datatype.into(),
                cardinality: cardinality.inner,
                iri: iri.map(|i| i.inner),
                description,
            },
        }
    }

    #[classmethod]
    fn required_string(_cls: &Bound<'_, PyType>, name: &str) -> Self {
        PyPropertyType { inner: PropertyType::required_string(name.to_string()) }
    }

    #[classmethod]
    fn optional(_cls: &Bound<'_, PyType>, name: &str, datatype: PyDatatype) -> Self {
        PyPropertyType { inner: PropertyType::optional(name.to_string(), datatype.into()) }
    }

    fn with_iri(mut slf: PyRefMut<'_, Self>, iri: PyIri) -> PyRefMut<'_, Self> {
        slf.inner.iri = Some(iri.inner);
        slf
    }

    fn with_description(mut slf: PyRefMut<'_, Self>, text: String) -> PyRefMut<'_, Self> {
        slf.inner.description = Some(text.to_string());
        slf
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn datatype(&self) -> PyDatatype {
        PyDatatype::from(self.inner.datatype)
    }
    #[getter]
    fn cardinality(&self) -> PyCardinality {
        PyCardinality { inner: self.inner.cardinality }
    }
    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("PropertyType(name={:?}, datatype={:?})", self.inner.name, self.inner.datatype)
    }
}

impl From<PropertyType> for PyPropertyType {
    fn from(inner: PropertyType) -> Self {
        PyPropertyType { inner }
    }
}

/// Declared node type (LPG analogue of `owl:Class`).
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "NodeType")]
#[derive(Clone)]
pub struct PyNodeType {
    pub inner: NodeType,
}

#[pymethods]
impl PyNodeType {
    #[new]
    fn new(name: &str) -> Self {
        PyNodeType { inner: NodeType::new(name.to_string()) }
    }

    fn with_supertype(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.inner.supertypes.push(name.to_string());
        slf
    }
    fn with_property(mut slf: PyRefMut<'_, Self>, prop: PyPropertyType) -> PyRefMut<'_, Self> {
        slf.inner.properties.push(prop.inner);
        slf
    }
    fn with_iri(mut slf: PyRefMut<'_, Self>, iri: PyIri) -> PyRefMut<'_, Self> {
        slf.inner.iri = Some(iri.inner);
        slf
    }
    fn with_description(mut slf: PyRefMut<'_, Self>, text: String) -> PyRefMut<'_, Self> {
        slf.inner.description = Some(text.to_string());
        slf
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }
    #[getter]
    fn supertypes(&self) -> Vec<String> {
        self.inner.supertypes.clone()
    }
    #[getter]
    fn properties(&self) -> Vec<PyPropertyType> {
        self.inner.properties.iter().cloned().map(PyPropertyType::from).collect()
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("NodeType({:?})", self.inner.name)
    }
}

impl From<NodeType> for PyNodeType {
    fn from(inner: NodeType) -> Self {
        PyNodeType { inner }
    }
}

/// Declared edge type (LPG analogue of `owl:ObjectProperty`).
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "EdgeType")]
#[derive(Clone)]
pub struct PyEdgeType {
    pub inner: EdgeType,
}

#[pymethods]
impl PyEdgeType {
    #[new]
    fn new(name: &str) -> Self {
        PyEdgeType { inner: EdgeType::new(name.to_string()) }
    }

    fn with_domain(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.inner.domain.push(name.to_string());
        slf
    }
    fn with_range(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.inner.range.push(name.to_string());
        slf
    }
    fn with_iri(mut slf: PyRefMut<'_, Self>, iri: PyIri) -> PyRefMut<'_, Self> {
        slf.inner.iri = Some(iri.inner);
        slf
    }
    fn functional(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.inner.functional = true;
        slf
    }
    fn with_inverse(mut slf: PyRefMut<'_, Self>, label: String) -> PyRefMut<'_, Self> {
        slf.inner.inverse_of = Some(label.to_string());
        slf
    }
    fn with_description(mut slf: PyRefMut<'_, Self>, text: String) -> PyRefMut<'_, Self> {
        slf.inner.description = Some(text.to_string());
        slf
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }
    #[getter]
    fn domain(&self) -> Vec<String> {
        self.inner.domain.clone()
    }
    #[getter]
    fn range(&self) -> Vec<String> {
        self.inner.range.clone()
    }
    #[getter]
    fn is_functional(&self) -> bool {
        self.inner.functional
    }
    #[getter]
    fn inverse_of(&self) -> Option<&str> {
        self.inner.inverse_of.as_deref()
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }
    #[getter]
    fn properties(&self) -> Vec<PyPropertyType> {
        self.inner.properties.iter().cloned().map(PyPropertyType::from).collect()
    }
    #[getter]
    fn cardinality(&self) -> PyCardinality {
        PyCardinality { inner: self.inner.cardinality }
    }

    fn __repr__(&self) -> String {
        format!("EdgeType({:?})", self.inner.name)
    }
}

impl From<EdgeType> for PyEdgeType {
    fn from(inner: EdgeType) -> Self {
        PyEdgeType { inner }
    }
}

/// Top-level schema — declared node and edge types.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Schema")]
#[derive(Clone, Default)]
pub struct PySchema {
    pub inner: Schema,
}

#[pymethods]
impl PySchema {
    #[new]
    fn new() -> Self {
        PySchema { inner: Schema::new() }
    }

    fn declare_node_type(mut slf: PyRefMut<'_, Self>, ty: PyNodeType) -> PyRefMut<'_, Self> {
        slf.inner.declare_node_type(ty.inner);
        slf
    }

    fn declare_edge_type(mut slf: PyRefMut<'_, Self>, ty: PyEdgeType) -> PyRefMut<'_, Self> {
        slf.inner.declare_edge_type(ty.inner);
        slf
    }

    fn node_type(&self, name: &str) -> Option<PyNodeType> {
        self.inner.node_type(name).cloned().map(PyNodeType::from)
    }

    fn edge_type(&self, name: &str) -> Option<PyEdgeType> {
        self.inner.edge_type(name).cloned().map(PyEdgeType::from)
    }

    fn supertypes_of(&self, name: &str) -> Vec<String> {
        self.inner.supertypes_of(name).into_iter().map(str::to_string).collect()
    }

    #[getter]
    fn node_types(&self) -> Vec<PyNodeType> {
        self.inner.node_types.values().cloned().map(PyNodeType::from).collect()
    }

    #[getter]
    fn edge_types(&self) -> Vec<PyEdgeType> {
        self.inner.edge_types.values().cloned().map(PyEdgeType::from).collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Schema(node_types={}, edge_types={})",
            self.inner.node_types.len(),
            self.inner.edge_types.len()
        )
    }
}

impl From<Schema> for PySchema {
    fn from(inner: Schema) -> Self {
        PySchema { inner }
    }
}

// ============================================================================
// AxiomKind + Axiom
// ============================================================================

/// One of the ten supported axiom shapes. Construct via the typed
/// classmethods on `AxiomKind`.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "AxiomKind", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyAxiomKind {
    pub inner: AxiomKind,
}

#[pymethods]
impl PyAxiomKind {
    #[classmethod]
    fn sub_class_of(_cls: &Bound<'_, PyType>, sub: String, sup: String) -> Self {
        Self { inner: AxiomKind::SubClassOf { sub, sup } }
    }
    #[classmethod]
    fn equivalent_class(_cls: &Bound<'_, PyType>, left: String, right: String) -> Self {
        Self { inner: AxiomKind::EquivalentClass { left, right } }
    }
    #[classmethod]
    fn disjoint_with(_cls: &Bound<'_, PyType>, left: String, right: String) -> Self {
        Self { inner: AxiomKind::DisjointWith { left, right } }
    }
    #[classmethod]
    fn domain(_cls: &Bound<'_, PyType>, property: String, class: String) -> Self {
        Self { inner: AxiomKind::Domain { property, class } }
    }
    #[classmethod]
    fn range(_cls: &Bound<'_, PyType>, property: String, class: String) -> Self {
        Self { inner: AxiomKind::Range { property, class } }
    }
    #[classmethod]
    fn functional(_cls: &Bound<'_, PyType>, property: String) -> Self {
        Self { inner: AxiomKind::Functional { property } }
    }
    #[classmethod]
    fn inverse_functional(_cls: &Bound<'_, PyType>, property: String) -> Self {
        Self { inner: AxiomKind::InverseFunctional { property } }
    }
    #[classmethod]
    fn inverse_of(_cls: &Bound<'_, PyType>, left: String, right: String) -> Self {
        Self { inner: AxiomKind::InverseOf { left, right } }
    }
    #[classmethod]
    fn symmetric(_cls: &Bound<'_, PyType>, property: String) -> Self {
        Self { inner: AxiomKind::Symmetric { property } }
    }
    #[classmethod]
    fn transitive(_cls: &Bound<'_, PyType>, property: String) -> Self {
        Self { inner: AxiomKind::Transitive { property } }
    }

    /// Snake-case tag of the kind, matching the JSON serde form.
    #[getter]
    fn tag(&self) -> &'static str {
        match self.inner {
            AxiomKind::SubClassOf { .. } => "sub_class_of",
            AxiomKind::EquivalentClass { .. } => "equivalent_class",
            AxiomKind::DisjointWith { .. } => "disjoint_with",
            AxiomKind::Domain { .. } => "domain",
            AxiomKind::Range { .. } => "range",
            AxiomKind::Functional { .. } => "functional",
            AxiomKind::InverseFunctional { .. } => "inverse_functional",
            AxiomKind::InverseOf { .. } => "inverse_of",
            AxiomKind::Symmetric { .. } => "symmetric",
            AxiomKind::Transitive { .. } => "transitive",
        }
    }

    /// Operands as a dict. The keys are the natural names from the
    /// Rust variant (`sub`, `sup`, `left`, `right`, `property`, `class`).
    fn operands<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new_bound(py);
        match &self.inner {
            AxiomKind::SubClassOf { sub, sup } => {
                d.set_item("sub", sub)?;
                d.set_item("sup", sup)?;
            }
            AxiomKind::EquivalentClass { left, right }
            | AxiomKind::DisjointWith { left, right }
            | AxiomKind::InverseOf { left, right } => {
                d.set_item("left", left)?;
                d.set_item("right", right)?;
            }
            AxiomKind::Domain { property, class } | AxiomKind::Range { property, class } => {
                d.set_item("property", property)?;
                d.set_item("class", class)?;
            }
            AxiomKind::Functional { property }
            | AxiomKind::InverseFunctional { property }
            | AxiomKind::Symmetric { property }
            | AxiomKind::Transitive { property } => {
                d.set_item("property", property)?;
            }
        }
        Ok(d)
    }

    fn __repr__(&self) -> String {
        format!("AxiomKind::{:?}", self.inner)
    }
}

impl From<AxiomKind> for PyAxiomKind {
    fn from(inner: AxiomKind) -> Self {
        Self { inner }
    }
}

/// An axiom plus optional provenance.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Axiom")]
#[derive(Clone)]
pub struct PyAxiom {
    pub inner: Axiom,
}

#[pymethods]
impl PyAxiom {
    #[new]
    fn new(kind: PyAxiomKind) -> Self {
        PyAxiom { inner: Axiom::new(kind.inner) }
    }

    fn with_provenance(mut slf: PyRefMut<'_, Self>, prov: PyProvenanceId) -> PyRefMut<'_, Self> {
        slf.inner.provenance = Some(prov.inner);
        slf
    }

    #[getter]
    fn id(&self) -> PyAxiomId {
        PyAxiomId::from(self.inner.id)
    }
    #[getter]
    fn kind(&self) -> PyAxiomKind {
        PyAxiomKind::from(self.inner.kind.clone())
    }
    #[getter]
    fn provenance(&self) -> Option<PyProvenanceId> {
        self.inner.provenance.map(PyProvenanceId::from)
    }

    fn __repr__(&self) -> String {
        format!("Axiom(id={}, kind={:?})", self.inner.id, self.inner.kind)
    }
}

impl From<Axiom> for PyAxiom {
    fn from(inner: Axiom) -> Self {
        PyAxiom { inner }
    }
}

// ============================================================================
// Record
// ============================================================================

/// A flat, denormalized snapshot of a node plus its outbound edges —
/// the natural product of a `RecordExtractor`.
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Record")]
#[derive(Clone)]
pub struct PyRecord {
    pub inner: Record,
}

#[pymethods]
impl PyRecord {
    #[new]
    fn new(type_name: &str) -> Self {
        PyRecord { inner: Record::new(type_name.to_string()) }
    }

    fn with_iri(mut slf: PyRefMut<'_, Self>, iri: PyIri) -> PyRefMut<'_, Self> {
        let r = std::mem::replace(&mut slf.inner, Record::new(""));
        slf.inner = r.with_iri(iri.inner);
        slf
    }

    fn with_property<'py>(
        mut slf: PyRefMut<'py, Self>,
        name: String,
        value: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pv = py_to_property_value(&value)?;
        slf.inner.properties.insert(name.to_string(), pv.inner);
        Ok(slf)
    }

    fn with_outbound(mut slf: PyRefMut<'_, Self>, label: String, target: PyIri) -> PyRefMut<'_, Self> {
        slf.inner.outbound.push((label.to_string(), target.inner));
        slf
    }

    fn with_source(mut slf: PyRefMut<'_, Self>, source: String) -> PyRefMut<'_, Self> {
        slf.inner.source = Some(source.to_string());
        slf
    }

    #[getter]
    fn id(&self) -> PyRecordId {
        PyRecordId::from(self.inner.id)
    }
    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }
    #[getter]
    fn subject(&self) -> Option<PyNodeId> {
        self.inner.subject.map(PyNodeId::from)
    }
    #[getter]
    fn type_name(&self) -> &str {
        &self.inner.type_name
    }
    #[getter]
    fn properties<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        property_map_to_py(py, &self.inner.properties)
    }
    #[getter]
    fn outbound(&self) -> Vec<(String, PyIri)> {
        self.inner
            .outbound
            .iter()
            .map(|(l, t)| (l.clone(), PyIri::from(t.clone())))
            .collect()
    }
    #[getter]
    fn source(&self) -> Option<&str> {
        self.inner.source.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("Record(id={}, type={:?})", self.inner.id, self.inner.type_name)
    }
}

impl From<Record> for PyRecord {
    fn from(inner: Record) -> Self {
        PyRecord { inner }
    }
}

// ============================================================================
// Ontology
// ============================================================================

/// In-memory ontology snapshot (the aggregate root).
#[pyclass(module = "atomr_ontology._atomr_ontology.core", name = "Ontology")]
#[derive(Clone, Default)]
pub struct PyOntology {
    pub inner: Ontology,
}

#[pymethods]
impl PyOntology {
    #[new]
    fn new() -> Self {
        PyOntology { inner: Ontology::new() }
    }

    /// Build with a canonical ontology IRI. Raises `IriError` on bad input.
    #[classmethod]
    fn with_iri(_cls: &Bound<'_, PyType>, iri: &str) -> PyResult<Self> {
        Ontology::with_iri(iri.to_string()).map(|o| PyOntology { inner: o }).map_err(ontology_err)
    }

    fn declare_node_type(&mut self, name: &str) -> String {
        self.inner.declare_node_type(name.to_string())
    }

    fn declare_edge_type(&mut self, name: &str) -> String {
        self.inner.declare_edge_type(name.to_string())
    }

    fn upsert_node(&mut self, node: PyNode) -> PyNodeId {
        PyNodeId::from(self.inner.upsert_node(node.inner))
    }

    fn upsert_edge(&mut self, edge: PyEdge) -> PyEdgeId {
        PyEdgeId::from(self.inner.upsert_edge(edge.inner))
    }

    fn upsert_axiom(&mut self, axiom: PyAxiom) -> PyAxiomId {
        PyAxiomId::from(self.inner.upsert_axiom(axiom.inner))
    }

    fn node(&self, id: PyNodeId) -> Option<PyNode> {
        self.inner.node(&id.inner).cloned().map(PyNode::from)
    }

    fn edge(&self, id: PyEdgeId) -> Option<PyEdge> {
        self.inner.edge(&id.inner).cloned().map(PyEdge::from)
    }

    fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    fn outbound(&self, node: PyNodeId) -> Vec<PyEdge> {
        self.inner.outbound(&node.inner).cloned().map(PyEdge::from).collect()
    }

    fn inbound(&self, node: PyNodeId) -> Vec<PyEdge> {
        self.inner.inbound(&node.inner).cloned().map(PyEdge::from).collect()
    }

    fn nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty_bound(py);
        for n in self.inner.nodes.values() {
            out.append(PyNode::from(n.clone()).into_py(py))?;
        }
        Ok(out)
    }

    fn edges<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty_bound(py);
        for e in self.inner.edges.values() {
            out.append(PyEdge::from(e.clone()).into_py(py))?;
        }
        Ok(out)
    }

    fn axioms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty_bound(py);
        for a in self.inner.axioms.values() {
            out.append(PyAxiom::from(a.clone()).into_py(py))?;
        }
        Ok(out)
    }

    #[getter]
    fn iri(&self) -> Option<PyIri> {
        self.inner.iri.clone().map(PyIri::from)
    }

    #[getter]
    fn schema(&self) -> PySchema {
        PySchema::from(self.inner.schema.clone())
    }

    #[getter]
    fn vocabulary(&self) -> PyVocabulary {
        PyVocabulary { inner: self.inner.vocabulary.clone() }
    }

    fn set_vocabulary(&mut self, vocab: PyVocabulary) {
        self.inner.vocabulary = vocab.inner;
    }

    fn set_schema(&mut self, schema: PySchema) {
        self.inner.schema = schema.inner;
    }

    /// Round-trip via serde-JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[classmethod]
    fn from_json(_cls: &Bound<'_, PyType>, value: &str) -> PyResult<Self> {
        let inner: Ontology =
            serde_json::from_str(value).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(PyOntology { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "Ontology(node_types={}, edge_types={}, nodes={}, edges={}, axioms={})",
            self.inner.schema.node_types.len(),
            self.inner.schema.edge_types.len(),
            self.inner.nodes.len(),
            self.inner.edges.len(),
            self.inner.axioms.len(),
        )
    }
}

impl From<Ontology> for PyOntology {
    fn from(inner: Ontology) -> Self {
        PyOntology { inner }
    }
}

// ============================================================================
// Re-exports useful to other PyO3 modules
// ============================================================================

/// Map an `IriError` to a Python error.
#[allow(dead_code)]
pub fn iri_error(e: IriError) -> PyErr {
    iri_err(e)
}

#[allow(dead_code)]
fn _unused_property_key_lookup() -> PyErr {
    PyKeyError::new_err("unused")
}

// ============================================================================
// Module registration
// ============================================================================

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIri>()?;
    m.add_class::<PyNodeId>()?;
    m.add_class::<PyEdgeId>()?;
    m.add_class::<PyRecordId>()?;
    m.add_class::<PyAxiomId>()?;
    m.add_class::<PyProvenanceId>()?;
    m.add_class::<PyNamespace>()?;
    m.add_class::<PyVocabulary>()?;
    m.add_class::<PyPropertyValue>()?;
    m.add_class::<PyProperty>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyEdge>()?;
    m.add_class::<PyDatatype>()?;
    m.add_class::<PyCardinality>()?;
    m.add_class::<PyPropertyType>()?;
    m.add_class::<PyNodeType>()?;
    m.add_class::<PyEdgeType>()?;
    m.add_class::<PySchema>()?;
    m.add_class::<PyAxiomKind>()?;
    m.add_class::<PyAxiom>()?;
    m.add_class::<PyRecord>()?;
    m.add_class::<PyOntology>()?;
    Ok(())
}
