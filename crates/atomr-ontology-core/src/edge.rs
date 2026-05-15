//! Labeled property graph edge — directed, typed, property-carrying.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{EdgeId, NodeId};
use crate::node::PropertyValue;

pub use crate::id::EdgeId as Id;

/// A directed edge between two [`Node`](crate::node::Node)s.
///
/// Edge identity is content-addressed over `(source, type, target)`
/// when constructed with [`Edge::between`], so the same triple
/// inserted twice produces the same id (and therefore the same row
/// in the store).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Stable identity.
    pub id: EdgeId,
    /// Edge type label — must reference an [`EdgeType`](crate::schema::EdgeType) in the active schema.
    pub label: String,
    /// Source node id.
    pub source: NodeId,
    /// Target node id.
    pub target: NodeId,
    /// Property bag.
    pub properties: BTreeMap<String, PropertyValue>,
}

impl Edge {
    /// Build an edge whose identity is determined by its endpoints
    /// and label, so identical edges deduplicate by id.
    pub fn between(source: NodeId, label: impl Into<String>, target: NodeId) -> Self {
        let label = label.into();
        let mut input = Vec::with_capacity(64 + label.len());
        input.extend_from_slice(source.as_bytes());
        input.push(0);
        input.extend_from_slice(label.as_bytes());
        input.push(0);
        input.extend_from_slice(target.as_bytes());
        Self { id: EdgeId::content_address(&input), label, source, target, properties: BTreeMap::new() }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_triples_share_id() {
        let s = NodeId::new_random();
        let t = NodeId::new_random();
        let a = Edge::between(s, "memberOf", t);
        let b = Edge::between(s, "memberOf", t);
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn different_labels_yield_different_ids() {
        let s = NodeId::new_random();
        let t = NodeId::new_random();
        let a = Edge::between(s, "memberOf", t);
        let b = Edge::between(s, "headOf", t);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn property_round_trip() {
        let s = NodeId::new_random();
        let t = NodeId::new_random();
        let e = Edge::between(s, "memberOf", t).with_property("since", 2020_i64);
        assert_eq!(e.property("since"), Some(&PropertyValue::Integer(2020)));
    }
}
