//! Labeled property graph node — entity carrying a set of labels and
//! a property bag.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::NodeId;
use crate::iri::Iri;

/// Typed property value attached to a [`Node`] or [`Edge`].
///
/// The variants cover the common JSON/RDF cross-section: text,
/// numbers, booleans, RFC 3339 timestamps, IRIs, opaque binary blobs,
/// and `null`. Composite values can be encoded as JSON-typed strings
/// via [`PropertyValue::Json`] when richer structure is needed
/// without leaving the LPG model.
///
/// [`Edge`]: crate::edge::Edge
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    /// UTF-8 text.
    String(String),
    /// 64-bit signed integer.
    Integer(i64),
    /// 64-bit floating point.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// RFC 3339 timestamp.
    DateTime(DateTime<Utc>),
    /// An IRI (kept distinct from `String` so RDF projection is lossless).
    Iri(Iri),
    /// Opaque binary content; encoded as base64 in JSON.
    Bytes(#[serde(with = "serde_bytes")] Vec<u8>),
    /// A nested JSON value for composite payloads.
    Json(serde_json::Value),
    /// Explicit null (distinct from "property absent").
    Null,
}

impl PropertyValue {
    /// Shorthand for `PropertyValue::String(value.into())`.
    pub fn string(value: impl Into<String>) -> Self {
        PropertyValue::String(value.into())
    }
}

impl From<&str> for PropertyValue {
    fn from(s: &str) -> Self {
        PropertyValue::String(s.to_owned())
    }
}

impl From<String> for PropertyValue {
    fn from(s: String) -> Self {
        PropertyValue::String(s)
    }
}

impl From<i64> for PropertyValue {
    fn from(v: i64) -> Self {
        PropertyValue::Integer(v)
    }
}

impl From<i32> for PropertyValue {
    fn from(v: i32) -> Self {
        PropertyValue::Integer(v as i64)
    }
}

impl From<f64> for PropertyValue {
    fn from(v: f64) -> Self {
        PropertyValue::Float(v)
    }
}

impl From<bool> for PropertyValue {
    fn from(v: bool) -> Self {
        PropertyValue::Bool(v)
    }
}

impl From<Iri> for PropertyValue {
    fn from(v: Iri) -> Self {
        PropertyValue::Iri(v)
    }
}

/// A property attached to a node or edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Property {
    /// Property name, scoped within the node's type.
    pub name: String,
    /// The value.
    pub value: PropertyValue,
}

impl Property {
    /// Construct a property pair.
    pub fn new(name: impl Into<String>, value: impl Into<PropertyValue>) -> Self {
        Self { name: name.into(), value: value.into() }
    }
}

/// A labeled property graph node.
///
/// Nodes carry one or more **labels** (the `types` field — string
/// keys into a [`Schema`](crate::schema::Schema)) and a property bag.
/// Identity is content-addressed via [`NodeId`] when an IRI is
/// supplied, and random otherwise.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Stable identity.
    pub id: NodeId,
    /// Optional canonical IRI for projection into RDF.
    pub iri: Option<Iri>,
    /// Type labels; must reference [`NodeType`](crate::schema::NodeType) names in the active schema.
    pub types: Vec<String>,
    /// Property bag, keyed by property name.
    pub properties: BTreeMap<String, PropertyValue>,
}

impl Node {
    /// Build a fresh node labeled with a single type. The node id is
    /// randomized; supply an IRI via [`Node::with_iri`] to derive a
    /// content-addressed id instead.
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            id: NodeId::new_random(),
            iri: None,
            types: vec![type_name.into()],
            properties: BTreeMap::new(),
        }
    }

    /// Build a node with a content-addressed id derived from its IRI.
    /// Two calls with the same IRI yield the same [`NodeId`].
    pub fn from_iri(iri: Iri, type_name: impl Into<String>) -> Self {
        let id = NodeId::content_address(iri.as_str().as_bytes());
        Self { id, iri: Some(iri), types: vec![type_name.into()], properties: BTreeMap::new() }
    }

    /// Attach an additional label to the node.
    pub fn with_label(mut self, type_name: impl Into<String>) -> Self {
        self.types.push(type_name.into());
        self
    }

    /// Set the node's IRI without rewriting its id. Used when the id
    /// is supplied externally (e.g. imported records).
    pub fn with_iri(mut self, iri: Iri) -> Self {
        self.iri = Some(iri);
        self
    }

    /// Set or replace a property.
    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<PropertyValue>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// Borrow a property value by name.
    pub fn property(&self, name: &str) -> Option<&PropertyValue> {
        self.properties.get(name)
    }

    /// True when the node carries the given label.
    pub fn has_type(&self, type_name: &str) -> bool {
        self.types.iter().any(|t| t == type_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_from_iri_is_content_addressed() {
        let iri = Iri::new("https://example.org/Acme").unwrap();
        let a = Node::from_iri(iri.clone(), "Organization");
        let b = Node::from_iri(iri, "Organization");
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn node_carries_multiple_labels() {
        let n = Node::new("Organization").with_label("FormalOrganization");
        assert!(n.has_type("Organization"));
        assert!(n.has_type("FormalOrganization"));
    }

    #[test]
    fn property_accessors() {
        let n = Node::new("Organization").with_property("name", "Acme").with_property("founded", 1995_i64);
        assert_eq!(n.property("name"), Some(&PropertyValue::String("Acme".into())));
        assert_eq!(n.property("founded"), Some(&PropertyValue::Integer(1995)));
    }
}
