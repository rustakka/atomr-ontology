//! Flat, denormalized snapshot of a node plus its outbound edges.
//!
//! `Record`s are the natural product of [`RecordExtractor`][rec] —
//! they collapse a structured source row (CSV line, JSON document,
//! database row) into a single addressable object that subsequent
//! stages can resolve into the canonical [`Ontology`](crate::ontology::Ontology).
//!
//! [rec]: https://docs.rs/atomr-ontology-extract

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{NodeId, RecordId};
use crate::iri::Iri;
use crate::node::PropertyValue;

/// A flat record. The primary node's `iri`, label, and property bag
/// are held inline; outbound edges are listed by target IRI plus the
/// edge label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Stable identity.
    pub id: RecordId,
    /// Optional canonical IRI of the primary node.
    pub iri: Option<Iri>,
    /// Subject node id (set after the record is committed).
    pub subject: Option<NodeId>,
    /// Primary node type label.
    pub type_name: String,
    /// Property bag.
    pub properties: BTreeMap<String, PropertyValue>,
    /// Outbound relations as `(edge_label, target_iri)` pairs.
    pub outbound: Vec<(String, Iri)>,
    /// Free-form source citation (file path, row id, URL...).
    pub source: Option<String>,
}

impl Record {
    /// Empty record for `type_name`.
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            id: RecordId::new_random(),
            iri: None,
            subject: None,
            type_name: type_name.into(),
            properties: BTreeMap::new(),
            outbound: Vec::new(),
            source: None,
        }
    }

    /// Set the canonical IRI; the record's id is recomputed as a
    /// content-address of the IRI so equal IRIs deduplicate.
    pub fn with_iri(mut self, iri: Iri) -> Self {
        self.id = RecordId::content_address(iri.as_str().as_bytes());
        self.iri = Some(iri);
        self
    }

    /// Attach a property.
    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<PropertyValue>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// Add an outbound edge.
    pub fn with_outbound(mut self, label: impl Into<String>, target: Iri) -> Self {
        self.outbound.push((label.into(), target));
        self
    }

    /// Attach a source citation.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iri_makes_record_content_addressed() {
        let iri = Iri::new("https://example.org/Acme").unwrap();
        let a = Record::new("Organization").with_iri(iri.clone());
        let b = Record::new("Organization").with_iri(iri);
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn outbound_appended() {
        let target = Iri::new("https://example.org/Bob").unwrap();
        let r = Record::new("Organization").with_outbound("hasMember", target.clone());
        assert_eq!(r.outbound, vec![("hasMember".into(), target)]);
    }
}
