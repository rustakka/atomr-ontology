//! Builder-style query patterns.
//!
//! The shape mirrors openCypher path expressions (`(a:Org)-[:memberOf]->(b)`)
//! and SPARQL basic graph patterns. A [`NodePattern`] matches a
//! single node; a [`TraversalPlan`] is a sequence of edge hops
//! starting from a seed pattern.

use std::collections::BTreeMap;

use atomr_ontology_core::{EdgeId, NodeId, PropertyValue};

/// Single-node pattern.
#[derive(Clone, Debug, Default)]
pub struct NodePattern {
    /// Bind the matched node id under this name in [`MatchRow`].
    pub bind: Option<String>,
    /// Type labels the node must carry.
    pub types: Vec<String>,
    /// Required property values (exact match).
    pub properties: BTreeMap<String, PropertyValue>,
    /// Optional id filter.
    pub id: Option<NodeId>,
}

impl NodePattern {
    /// Anonymous wildcard pattern.
    pub fn any() -> Self {
        Self::default()
    }

    /// Bind the matched id under the given variable name.
    pub fn bind(mut self, name: impl Into<String>) -> Self {
        self.bind = Some(name.into());
        self
    }

    /// Constrain by type label.
    pub fn typed(mut self, name: impl Into<String>) -> Self {
        self.types.push(name.into());
        self
    }

    /// Constrain by an exact property value.
    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<PropertyValue>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// Pin to a specific node id.
    pub fn with_id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Edge pattern — one hop in a [`TraversalPlan`].
#[derive(Clone, Debug, Default)]
pub struct EdgePattern {
    /// Bind the matched edge id under this name in [`MatchRow`].
    pub bind: Option<String>,
    /// Required edge label.
    pub label: Option<String>,
    /// Required property values (exact match).
    pub properties: BTreeMap<String, PropertyValue>,
}

impl EdgePattern {
    /// Anonymous edge pattern.
    pub fn any() -> Self {
        Self::default()
    }

    /// Bind the matched edge under a name.
    pub fn bind(mut self, name: impl Into<String>) -> Self {
        self.bind = Some(name.into());
        self
    }

    /// Constrain by edge label.
    pub fn labeled(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Constrain by an exact property value.
    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<PropertyValue>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }
}

/// One step in a [`TraversalPlan`].
#[derive(Clone, Debug)]
pub struct TraversalStep {
    /// The edge filter to apply at this step.
    pub edge: EdgePattern,
    /// The target node filter to apply at this step.
    pub target: NodePattern,
    /// Direction. `true` ⇒ follow outbound edges, `false` ⇒ inbound.
    pub outbound: bool,
}

impl TraversalStep {
    /// Outbound hop.
    pub fn outbound(edge: EdgePattern, target: NodePattern) -> Self {
        Self { edge, target, outbound: true }
    }

    /// Inbound hop.
    pub fn inbound(edge: EdgePattern, target: NodePattern) -> Self {
        Self { edge, target, outbound: false }
    }
}

/// Multi-step traversal: seed pattern, then a chain of hops.
#[derive(Clone, Debug)]
pub struct TraversalPlan {
    /// Starting node pattern.
    pub seed: NodePattern,
    /// Hops to follow from the seed binding.
    pub steps: Vec<TraversalStep>,
}

impl TraversalPlan {
    /// Plan starting at `seed`.
    pub fn from(seed: NodePattern) -> Self {
        Self { seed, steps: Vec::new() }
    }

    /// Append an outbound hop.
    pub fn outbound(mut self, edge: EdgePattern, target: NodePattern) -> Self {
        self.steps.push(TraversalStep::outbound(edge, target));
        self
    }

    /// Append an inbound hop.
    pub fn inbound(mut self, edge: EdgePattern, target: NodePattern) -> Self {
        self.steps.push(TraversalStep::inbound(edge, target));
        self
    }
}

/// A pattern-match result row.
#[derive(Clone, Debug, Default)]
pub struct MatchRow {
    /// Bindings from variable name → matched node id.
    pub nodes: BTreeMap<String, NodeId>,
    /// Bindings from variable name → matched edge id.
    pub edges: BTreeMap<String, EdgeId>,
}

impl MatchRow {
    /// Empty row.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a node under a name.
    pub fn bind_node(mut self, name: impl Into<String>, id: NodeId) -> Self {
        self.nodes.insert(name.into(), id);
        self
    }

    /// Bind an edge under a name.
    pub fn bind_edge(mut self, name: impl Into<String>, id: EdgeId) -> Self {
        self.edges.insert(name.into(), id);
        self
    }
}
