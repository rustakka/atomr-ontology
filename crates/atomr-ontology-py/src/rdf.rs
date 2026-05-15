//! PyO3 wrappers for `atomr-ontology-rdf`.

use pyo3::prelude::*;
use pyo3::types::PyType;

use atomr_ontology::core::Iri;
use atomr_ontology::rdf::{
    from_rdf as rust_from_rdf, jsonld, ntriples, to_rdf as rust_to_rdf, turtle, Class, DataProperty,
    Individual, Object, ObjectProperty, Quad, Subject, Triple,
};

use crate::core::{PyIri, PyOntology};
use crate::errors::adapter_err;

// ============================================================================
// Subject + Object enums
// ============================================================================

/// The subject of a triple — IRI or blank node.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "Subject", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PySubject {
    pub inner: Subject,
}

#[pymethods]
impl PySubject {
    #[classmethod]
    fn iri(_cls: &Bound<'_, PyType>, value: PyIri) -> Self {
        PySubject { inner: Subject::Iri(value.inner) }
    }
    #[classmethod]
    fn blank(_cls: &Bound<'_, PyType>, label: &str) -> Self {
        PySubject { inner: Subject::Blank(label.to_string()) }
    }
    #[classmethod]
    fn blank_n(_cls: &Bound<'_, PyType>, n: u64) -> Self {
        PySubject { inner: Subject::blank_n(n) }
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            Subject::Iri(_) => "iri",
            Subject::Blank(_) => "blank",
        }
    }

    #[getter]
    fn value<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match &self.inner {
            Subject::Iri(i) => PyIri::from(i.clone()).into_py(py).into_bound(py),
            Subject::Blank(s) => s.clone().into_py(py).into_bound(py),
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Subject::Iri(i) => format!("Subject.iri({:?})", i.as_str()),
            Subject::Blank(s) => format!("Subject.blank({:?})", s),
        }
    }
}

impl From<Subject> for PySubject {
    fn from(inner: Subject) -> Self {
        PySubject { inner }
    }
}

/// The object of a triple — IRI, blank node, or typed literal.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "Object", eq, frozen)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyObject_ {
    pub inner: Object,
}

#[pymethods]
impl PyObject_ {
    #[classmethod]
    fn iri(_cls: &Bound<'_, PyType>, value: PyIri) -> Self {
        Self { inner: Object::Iri(value.inner) }
    }
    #[classmethod]
    fn blank(_cls: &Bound<'_, PyType>, label: &str) -> Self {
        Self { inner: Object::Blank(label.to_string()) }
    }
    #[classmethod]
    #[pyo3(signature = (lexical, datatype, language=None))]
    fn literal(
        _cls: &Bound<'_, PyType>,
        lexical: String,
        datatype: PyIri,
        language: Option<String>,
    ) -> Self {
        Self {
            inner: Object::Literal { lexical, datatype: datatype.inner, language },
        }
    }
    #[classmethod]
    fn xsd_string(_cls: &Bound<'_, PyType>, value: String) -> Self {
        Self { inner: Object::xsd_string(value) }
    }
    #[classmethod]
    fn xsd_integer(_cls: &Bound<'_, PyType>, value: i64) -> Self {
        Self { inner: Object::xsd_integer(value) }
    }
    #[classmethod]
    fn xsd_double(_cls: &Bound<'_, PyType>, value: f64) -> Self {
        Self { inner: Object::xsd_double(value) }
    }
    #[classmethod]
    fn xsd_boolean(_cls: &Bound<'_, PyType>, value: bool) -> Self {
        Self { inner: Object::xsd_boolean(value) }
    }
    #[classmethod]
    fn xsd_date_time(_cls: &Bound<'_, PyType>, value: chrono::DateTime<chrono::Utc>) -> Self {
        Self { inner: Object::xsd_date_time(value) }
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            Object::Iri(_) => "iri",
            Object::Blank(_) => "blank",
            Object::Literal { .. } => "literal",
        }
    }

    #[getter]
    fn lexical(&self) -> Option<&str> {
        match &self.inner {
            Object::Literal { lexical, .. } => Some(lexical),
            _ => None,
        }
    }

    #[getter]
    fn datatype(&self) -> Option<PyIri> {
        match &self.inner {
            Object::Literal { datatype, .. } => Some(PyIri::from(datatype.clone())),
            _ => None,
        }
    }

    #[getter]
    fn language(&self) -> Option<&str> {
        match &self.inner {
            Object::Literal { language, .. } => language.as_deref(),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("Object({:?})", self.inner)
    }
}

impl From<Object> for PyObject_ {
    fn from(inner: Object) -> Self {
        PyObject_ { inner }
    }
}

// ============================================================================
// Triple / Quad
// ============================================================================

/// `<subject> <predicate> <object>` RDF triple.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "Triple", eq, frozen)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyTriple {
    pub inner: Triple,
}

#[pymethods]
impl PyTriple {
    #[new]
    fn new(subject: PySubject, predicate: PyIri, object: PyObject_) -> Self {
        PyTriple { inner: Triple::new(subject.inner, predicate.inner, object.inner) }
    }
    #[getter]
    fn subject(&self) -> PySubject {
        PySubject::from(self.inner.subject.clone())
    }
    #[getter]
    fn predicate(&self) -> PyIri {
        PyIri::from(self.inner.predicate.clone())
    }
    #[getter]
    fn object(&self) -> PyObject_ {
        PyObject_::from(self.inner.object.clone())
    }
    fn __repr__(&self) -> String {
        format!("Triple({:?})", self.inner)
    }
}

impl From<Triple> for PyTriple {
    fn from(inner: Triple) -> Self {
        PyTriple { inner }
    }
}

/// RDF quad — triple plus a named-graph IRI.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "Quad", eq, frozen)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyQuad {
    pub inner: Quad,
}

#[pymethods]
impl PyQuad {
    #[new]
    fn new(subject: PySubject, predicate: PyIri, object: PyObject_, graph: PyIri) -> Self {
        PyQuad {
            inner: Quad {
                subject: subject.inner,
                predicate: predicate.inner,
                object: object.inner,
                graph: graph.inner,
            },
        }
    }
    #[getter]
    fn subject(&self) -> PySubject {
        PySubject::from(self.inner.subject.clone())
    }
    #[getter]
    fn predicate(&self) -> PyIri {
        PyIri::from(self.inner.predicate.clone())
    }
    #[getter]
    fn object(&self) -> PyObject_ {
        PyObject_::from(self.inner.object.clone())
    }
    #[getter]
    fn graph(&self) -> PyIri {
        PyIri::from(self.inner.graph.clone())
    }
}

// ============================================================================
// OWL vocabulary surface
// ============================================================================

/// OWL `Class` view over an LPG `NodeType`.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "Class", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyClass {
    pub inner: Class,
}

#[pymethods]
impl PyClass {
    #[new]
    fn new(name: String, iri: PyIri, super_classes: Vec<PyIri>) -> Self {
        Self {
            inner: Class {
                name,
                iri: iri.inner,
                super_classes: super_classes.into_iter().map(|i| i.inner).collect(),
            },
        }
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn iri(&self) -> PyIri {
        PyIri::from(self.inner.iri.clone())
    }
    #[getter]
    fn super_classes(&self) -> Vec<PyIri> {
        self.inner.super_classes.iter().cloned().map(PyIri::from).collect()
    }
}

/// OWL `Individual` view over an LPG `Node`.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "Individual", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyIndividual {
    pub inner: Individual,
}

#[pymethods]
impl PyIndividual {
    #[new]
    fn new(iri: PyIri, types: Vec<PyIri>) -> Self {
        Self {
            inner: Individual { iri: iri.inner, types: types.into_iter().map(|i| i.inner).collect() },
        }
    }
    #[getter]
    fn iri(&self) -> PyIri {
        PyIri::from(self.inner.iri.clone())
    }
    #[getter]
    fn types(&self) -> Vec<PyIri> {
        self.inner.types.iter().cloned().map(PyIri::from).collect()
    }
}

/// OWL `ObjectProperty` view.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "ObjectProperty", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyObjectProperty {
    pub inner: ObjectProperty,
}

#[pymethods]
impl PyObjectProperty {
    #[new]
    #[pyo3(signature = (name, iri, domain, range, functional=false, inverse_of=None))]
    fn new(
        name: String,
        iri: PyIri,
        domain: Vec<PyIri>,
        range: Vec<PyIri>,
        functional: bool,
        inverse_of: Option<PyIri>,
    ) -> Self {
        Self {
            inner: ObjectProperty {
                name,
                iri: iri.inner,
                domain: domain.into_iter().map(|i| i.inner).collect(),
                range: range.into_iter().map(|i| i.inner).collect(),
                functional,
                inverse_of: inverse_of.map(|i| i.inner),
            },
        }
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn iri(&self) -> PyIri {
        PyIri::from(self.inner.iri.clone())
    }
    #[getter]
    fn domain(&self) -> Vec<PyIri> {
        self.inner.domain.iter().cloned().map(PyIri::from).collect()
    }
    #[getter]
    fn range(&self) -> Vec<PyIri> {
        self.inner.range.iter().cloned().map(PyIri::from).collect()
    }
    #[getter]
    fn functional(&self) -> bool {
        self.inner.functional
    }
    #[getter]
    fn inverse_of(&self) -> Option<PyIri> {
        self.inner.inverse_of.clone().map(PyIri::from)
    }
}

/// OWL `DataProperty` view.
#[pyclass(module = "atomr_ontology._atomr_ontology.rdf", name = "DataProperty", eq, hash, frozen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyDataProperty {
    pub inner: DataProperty,
}

#[pymethods]
impl PyDataProperty {
    #[new]
    fn new(name: String, iri: PyIri, domain: Vec<PyIri>, range_xsd: PyIri) -> Self {
        Self {
            inner: DataProperty {
                name,
                iri: iri.inner,
                domain: domain.into_iter().map(|i| i.inner).collect(),
                range_xsd: range_xsd.inner,
            },
        }
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn iri(&self) -> PyIri {
        PyIri::from(self.inner.iri.clone())
    }
    #[getter]
    fn domain(&self) -> Vec<PyIri> {
        self.inner.domain.iter().cloned().map(PyIri::from).collect()
    }
    #[getter]
    fn range_xsd(&self) -> PyIri {
        PyIri::from(self.inner.range_xsd.clone())
    }
}

// ============================================================================
// Adapter functions + writers
// ============================================================================

/// Project an `Ontology` to a deterministic sequence of triples.
#[pyfunction]
pub fn to_rdf(ontology: &PyOntology) -> Vec<PyTriple> {
    rust_to_rdf(&ontology.inner).into_iter().map(PyTriple::from).collect()
}

/// Reconstruct an `Ontology` from a triple stream. Partial: in v0.1
/// only T-Box assertions and IRI-named instances are recognized.
#[pyfunction]
pub fn from_rdf(triples: Vec<PyTriple>) -> PyResult<PyOntology> {
    let raw: Vec<_> = triples.into_iter().map(|t| t.inner).collect();
    rust_from_rdf(&raw).map(PyOntology::from).map_err(adapter_err)
}

/// Emit Turtle.
#[pyfunction]
pub fn turtle_write(ontology: &PyOntology) -> String {
    turtle::write(&ontology.inner)
}

/// Emit N-Triples.
#[pyfunction]
pub fn ntriples_write(ontology: &PyOntology) -> String {
    ntriples::write(&ontology.inner)
}

/// Emit JSON-LD.
#[pyfunction]
pub fn jsonld_write(ontology: &PyOntology) -> String {
    jsonld::write(&ontology.inner)
}

// Helper: construct an Iri without raising — used for shorter examples.
#[pyfunction]
fn iri_unchecked(value: &str) -> PyIri {
    PyIri::from(Iri::from_unchecked(value.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySubject>()?;
    m.add_class::<PyObject_>()?;
    m.add_class::<PyTriple>()?;
    m.add_class::<PyQuad>()?;
    m.add_class::<PyClass>()?;
    m.add_class::<PyIndividual>()?;
    m.add_class::<PyObjectProperty>()?;
    m.add_class::<PyDataProperty>()?;
    m.add_function(wrap_pyfunction!(to_rdf, m)?)?;
    m.add_function(wrap_pyfunction!(from_rdf, m)?)?;
    m.add_function(wrap_pyfunction!(turtle_write, m)?)?;
    m.add_function(wrap_pyfunction!(ntriples_write, m)?)?;
    m.add_function(wrap_pyfunction!(jsonld_write, m)?)?;
    m.add_function(wrap_pyfunction!(iri_unchecked, m)?)?;
    Ok(())
}
