//! `Ontology` — the aggregate root that owns a `Schema`, a vocabulary,
//! a set of nodes and edges, and an axiom set.
//!
//! `Ontology` is intentionally lightweight: it is an in-memory
//! aggregate suitable for tests and small ontologies. Production
//! workflows should keep their canonical state in an
//! [`OntologyStore`](https://docs.rs/atomr-ontology-store) and treat
//! `Ontology` as a snapshot type.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::axiom::Axiom;
use crate::edge::Edge;
use crate::error::OntologyError;
use crate::id::{EdgeId, NodeId};
use crate::iri::Iri;
use crate::namespace::Vocabulary;
use crate::node::Node;
use crate::schema::Schema;

/// In-memory snapshot of an ontology.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Ontology {
    /// Canonical IRI for this ontology version.
    pub iri: Option<Iri>,
    /// Bound namespaces.
    pub vocabulary: Vocabulary,
    /// Declared types.
    pub schema: Schema,
    /// Nodes keyed by id.
    pub nodes: BTreeMap<NodeId, Node>,
    /// Edges keyed by id.
    pub edges: BTreeMap<EdgeId, Edge>,
    /// Axioms keyed by id.
    pub axioms: BTreeMap<crate::axiom::AxiomId, Axiom>,
}

impl Ontology {
    /// Empty ontology.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty ontology with a canonical IRI label.
    pub fn with_iri(iri: impl Into<String>) -> Result<Self, OntologyError> {
        let iri = Iri::new(iri.into())?;
        Ok(Self { iri: Some(iri), ..Self::default() })
    }

    /// Declare a node type and return its name for chaining.
    pub fn declare_node_type(&mut self, name: impl Into<String>) -> String {
        let name = name.into();
        if !self.schema.node_types.contains_key(&name) {
            self.schema.declare_node_type(crate::schema::NodeType::new(name.clone()));
        }
        name
    }

    /// Declare an edge type and return its name for chaining.
    pub fn declare_edge_type(&mut self, name: impl Into<String>) -> String {
        let name = name.into();
        if !self.schema.edge_types.contains_key(&name) {
            self.schema.declare_edge_type(crate::schema::EdgeType::new(name.clone()));
        }
        name
    }

    /// Insert or replace a node. Returns its id.
    pub fn upsert_node(&mut self, node: Node) -> NodeId {
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    /// Insert or replace an edge. Returns its id.
    pub fn upsert_edge(&mut self, edge: Edge) -> EdgeId {
        let id = edge.id;
        self.edges.insert(id, edge);
        id
    }

    /// Insert or replace an axiom. Returns its id.
    pub fn upsert_axiom(&mut self, axiom: Axiom) -> crate::axiom::AxiomId {
        let id = axiom.id;
        self.axioms.insert(id, axiom);
        id
    }

    /// Borrow a node.
    pub fn node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Borrow an edge.
    pub fn edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Count of stored nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Count of stored edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Iterate the outbound edges of a node.
    pub fn outbound<'a>(&'a self, node: &'a NodeId) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges.values().filter(move |e| &e.source == node)
    }

    /// Iterate the inbound edges of a node.
    pub fn inbound<'a>(&'a self, node: &'a NodeId) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges.values().filter(move |e| &e.target == node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;

    #[test]
    fn upsert_node_idempotent_by_id() {
        let mut o = Ontology::new();
        let iri = Iri::new("https://example.org/Acme").unwrap();
        let id = o.upsert_node(Node::from_iri(iri.clone(), "Organization"));
        let id_again = o.upsert_node(Node::from_iri(iri, "Organization").with_property("name", "Acme"));
        assert_eq!(id, id_again);
        assert_eq!(o.node_count(), 1);
        assert!(o.node(&id).unwrap().property("name").is_some());
    }

    #[test]
    fn outbound_and_inbound_iterators() {
        let mut o = Ontology::new();
        let a = o.upsert_node(Node::new("Org"));
        let b = o.upsert_node(Node::new("Org"));
        let _ = o.upsert_edge(Edge::between(a, "memberOf", b));
        assert_eq!(o.outbound(&a).count(), 1);
        assert_eq!(o.inbound(&b).count(), 1);
        assert_eq!(o.outbound(&b).count(), 0);
    }
}
