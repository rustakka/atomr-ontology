//! Builder-style query patterns.
//!
//! The shape mirrors openCypher path expressions (`(a:Org)-[:memberOf]->(b)`)
//! and SPARQL basic graph patterns. A [`NodePattern`] matches a
//! single node; a [`TraversalPlan`] is a sequence of edge hops
//! starting from a seed pattern.
//!
//! Beyond the v0.1 surface, this module adds:
//!
//! - **Variable-length paths**: `EdgePattern::any().repeat(1..=3)` to
//!   match transitive closures of an edge label (`-[:subClassOf*1..3]->`).
//! - **Alternation / negation**: `NodePattern::any().or(...)` and
//!   `.not(...)` to compose disjunctive or negative constraints.
//! - **Result projection**: [`TraversalPlan::return_columns`] picks
//!   which bindings to emit (defaulting to all bound variables).
//! - **Order / limit**: [`TraversalPlan::limit`] truncates rows;
//!   [`TraversalPlan::order_by`] sorts by a binding name.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

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
    /// Alternative patterns — at least one of the alternatives, when
    /// non-empty, must also match the node (OR semantics over the
    /// alternatives, AND with the base pattern).
    #[allow(clippy::vec_box)] // intentional: alternatives are clone-cheap structs.
    pub or: Vec<Box<NodePattern>>,
    /// Negative patterns — none of these patterns may match the node.
    pub not: Vec<Box<NodePattern>>,
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

    /// Add an OR-branch: a candidate node must satisfy the base pattern
    /// AND (when alternatives are present) match at least one alternative.
    pub fn or(mut self, alt: NodePattern) -> Self {
        self.or.push(Box::new(alt));
        self
    }

    /// Add a NOT-branch: candidates matching this pattern are excluded.
    pub fn not(mut self, neg: NodePattern) -> Self {
        self.not.push(Box::new(neg));
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
    /// Variable-length repetition. `None` ⇒ exactly one hop;
    /// `Some(min..=max)` ⇒ match between `min` and `max` consecutive
    /// hops of the same edge pattern (Kleene-star-like).
    pub repeat: Option<RangeInclusive<usize>>,
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

    /// Apply a variable-length repetition (`-[:edge*min..=max]->`).
    pub fn repeat(mut self, range: RangeInclusive<usize>) -> Self {
        self.repeat = Some(range);
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

/// Sort direction for [`TraversalPlan::order_by`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SortOrder {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

/// Multi-step traversal: seed pattern, then a chain of hops.
#[derive(Clone, Debug)]
pub struct TraversalPlan {
    /// Starting node pattern.
    pub seed: NodePattern,
    /// Hops to follow from the seed binding.
    pub steps: Vec<TraversalStep>,
    /// If set, only emit these binding names in [`MatchRow`]; the
    /// executor may strip other bindings from the row.
    pub return_columns: Vec<String>,
    /// Bindings to sort by, in priority order.
    pub order: Vec<(String, SortOrder)>,
    /// Skip this many rows after ordering.
    pub skip: usize,
    /// Cap the row count after ordering and skipping.
    pub limit: Option<usize>,
}

impl TraversalPlan {
    /// Plan starting at `seed`.
    pub fn from(seed: NodePattern) -> Self {
        Self {
            seed,
            steps: Vec::new(),
            return_columns: Vec::new(),
            order: Vec::new(),
            skip: 0,
            limit: None,
        }
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

    /// Restrict the result row to these binding names.
    pub fn return_(mut self, columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.return_columns = columns.into_iter().map(Into::into).collect();
        self
    }

    /// Add an ascending sort key.
    pub fn order_by(mut self, binding: impl Into<String>) -> Self {
        self.order.push((binding.into(), SortOrder::Ascending));
        self
    }

    /// Add a descending sort key.
    pub fn order_by_desc(mut self, binding: impl Into<String>) -> Self {
        self.order.push((binding.into(), SortOrder::Descending));
        self
    }

    /// Skip `n` rows after ordering.
    pub fn skip(mut self, n: usize) -> Self {
        self.skip = n;
        self
    }

    /// Limit the number of rows.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
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
