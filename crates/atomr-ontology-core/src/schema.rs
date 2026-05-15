//! Schema — declared shape of an ontology: node types, edge types,
//! property types, cardinalities.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::iri::Iri;

/// Datatype of a property value, mapped to XSD on the RDF side.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Datatype {
    /// `xsd:string`
    String,
    /// `xsd:integer` / `xsd:long`
    Integer,
    /// `xsd:double`
    Float,
    /// `xsd:boolean`
    Bool,
    /// `xsd:dateTime`
    DateTime,
    /// `xsd:anyURI`, used for IRI-valued properties.
    Iri,
    /// `xsd:base64Binary`
    Bytes,
    /// `rdf:JSON`
    Json,
}

/// Cardinality bounds; mirrors OWL `min`/`max`-cardinality and SHACL
/// `sh:minCount`/`sh:maxCount`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Cardinality {
    /// Minimum count, default 0.
    pub min: u32,
    /// Maximum count, `None` for unbounded.
    pub max: Option<u32>,
}

impl Cardinality {
    /// `[0..]` — optional, unbounded.
    pub const ANY: Cardinality = Cardinality { min: 0, max: None };
    /// `[1..1]` — exactly one.
    pub const ONE: Cardinality = Cardinality { min: 1, max: Some(1) };
    /// `[0..1]` — at most one.
    pub const OPTIONAL: Cardinality = Cardinality { min: 0, max: Some(1) };
    /// `[1..]` — at least one.
    pub const AT_LEAST_ONE: Cardinality = Cardinality { min: 1, max: None };

    /// Build an arbitrary cardinality range.
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }

    /// True when `count` lies within the bound.
    pub fn contains(&self, count: u32) -> bool {
        count >= self.min && self.max.map_or(true, |m| count <= m)
    }
}

impl Default for Cardinality {
    fn default() -> Self {
        Cardinality::ANY
    }
}

/// Declared shape of a property: its datatype and how many values
/// instances may carry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyType {
    /// Property name (scoped within a node or edge type).
    pub name: String,
    /// Underlying value datatype.
    pub datatype: Datatype,
    /// Cardinality bound.
    pub cardinality: Cardinality,
    /// Optional canonical IRI for RDF projection.
    pub iri: Option<Iri>,
    /// Human-readable description.
    pub description: Option<String>,
}

impl PropertyType {
    /// Required single-string property.
    pub fn required_string(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            datatype: Datatype::String,
            cardinality: Cardinality::ONE,
            iri: None,
            description: None,
        }
    }

    /// Optional property of any datatype.
    pub fn optional(name: impl Into<String>, datatype: Datatype) -> Self {
        Self { name: name.into(), datatype, cardinality: Cardinality::OPTIONAL, iri: None, description: None }
    }

    /// Attach a canonical IRI for RDF projection.
    pub fn with_iri(mut self, iri: Iri) -> Self {
        self.iri = Some(iri);
        self
    }

    /// Attach a human-readable description.
    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }
}

/// Declared node type — the labeled-property-graph analogue of an
/// OWL `Class`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeType {
    /// Type name (the label that [`Node`](crate::node::Node)s use).
    pub name: String,
    /// Canonical IRI for RDF projection (e.g., `org:Organization`).
    pub iri: Option<Iri>,
    /// Direct supertypes (subclass-of relations).
    pub supertypes: Vec<String>,
    /// Allowed properties on instances of this type.
    pub properties: Vec<PropertyType>,
    /// Human-readable description.
    pub description: Option<String>,
}

impl NodeType {
    /// Empty node type.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            iri: None,
            supertypes: Vec::new(),
            properties: Vec::new(),
            description: None,
        }
    }

    /// Add a supertype name.
    pub fn with_supertype(mut self, name: impl Into<String>) -> Self {
        self.supertypes.push(name.into());
        self
    }

    /// Attach a property declaration.
    pub fn with_property(mut self, prop: PropertyType) -> Self {
        self.properties.push(prop);
        self
    }

    /// Attach an IRI.
    pub fn with_iri(mut self, iri: Iri) -> Self {
        self.iri = Some(iri);
        self
    }

    /// Attach a description.
    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }
}

/// Declared edge type — the LPG analogue of an OWL `ObjectProperty`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeType {
    /// Edge label.
    pub name: String,
    /// Canonical IRI for RDF projection.
    pub iri: Option<Iri>,
    /// Allowed source node types (domain).
    pub domain: Vec<String>,
    /// Allowed target node types (range).
    pub range: Vec<String>,
    /// Allowed properties on edges of this type.
    pub properties: Vec<PropertyType>,
    /// Cardinality from a single source.
    pub cardinality: Cardinality,
    /// True if this edge is functional (at most one target per source).
    pub functional: bool,
    /// Optional inverse edge label.
    pub inverse_of: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
}

impl EdgeType {
    /// Empty edge type.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            iri: None,
            domain: Vec::new(),
            range: Vec::new(),
            properties: Vec::new(),
            cardinality: Cardinality::ANY,
            functional: false,
            inverse_of: None,
            description: None,
        }
    }

    /// Restrict the domain (source node types).
    pub fn with_domain(mut self, name: impl Into<String>) -> Self {
        self.domain.push(name.into());
        self
    }

    /// Restrict the range (target node types).
    pub fn with_range(mut self, name: impl Into<String>) -> Self {
        self.range.push(name.into());
        self
    }

    /// Attach an IRI.
    pub fn with_iri(mut self, iri: Iri) -> Self {
        self.iri = Some(iri);
        self
    }

    /// Mark functional.
    pub fn functional(mut self) -> Self {
        self.functional = true;
        self
    }

    /// Declare an inverse.
    pub fn with_inverse(mut self, inverse_label: impl Into<String>) -> Self {
        self.inverse_of = Some(inverse_label.into());
        self
    }

    /// Attach a description.
    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }
}

/// Top-level schema for an ontology.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Node type declarations, keyed by name.
    pub node_types: BTreeMap<String, NodeType>,
    /// Edge type declarations, keyed by name.
    pub edge_types: BTreeMap<String, EdgeType>,
}

impl Schema {
    /// Empty schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a node type.
    pub fn declare_node_type(&mut self, ty: NodeType) -> &mut Self {
        self.node_types.insert(ty.name.clone(), ty);
        self
    }

    /// Insert or replace an edge type.
    pub fn declare_edge_type(&mut self, ty: EdgeType) -> &mut Self {
        self.edge_types.insert(ty.name.clone(), ty);
        self
    }

    /// Look up a node type.
    pub fn node_type(&self, name: &str) -> Option<&NodeType> {
        self.node_types.get(name)
    }

    /// Look up an edge type.
    pub fn edge_type(&self, name: &str) -> Option<&EdgeType> {
        self.edge_types.get(name)
    }

    /// Walk the transitive supertype chain for a node type. The
    /// returned iterator yields `name` itself first, then its
    /// supertypes in declared order (depth-first).
    pub fn supertypes_of<'a>(&'a self, name: &'a str) -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut stack = vec![name];
        while let Some(n) = stack.pop() {
            if out.contains(&n) {
                continue;
            }
            out.push(n);
            if let Some(ty) = self.node_types.get(n) {
                for s in ty.supertypes.iter().rev() {
                    stack.push(s.as_str());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinality_bounds_work() {
        assert!(Cardinality::ONE.contains(1));
        assert!(!Cardinality::ONE.contains(0));
        assert!(!Cardinality::ONE.contains(2));
        assert!(Cardinality::AT_LEAST_ONE.contains(99));
    }

    #[test]
    fn supertype_walk() {
        let mut s = Schema::new();
        s.declare_node_type(NodeType::new("FormalOrganization").with_supertype("Organization"));
        s.declare_node_type(NodeType::new("Organization").with_supertype("Agent"));
        s.declare_node_type(NodeType::new("Agent"));
        let chain = s.supertypes_of("FormalOrganization");
        assert_eq!(chain, vec!["FormalOrganization", "Organization", "Agent"]);
    }
}
