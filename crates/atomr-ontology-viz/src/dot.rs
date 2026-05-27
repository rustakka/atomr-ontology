//! GraphViz DOT renderer.

use atomr_ontology_core::Ontology;
use atomr_ontology_provenance::ProvenanceLog;

/// Render an ontology as a GraphViz DOT document.
///
/// Nodes are emitted with their type list as a label suffix; edges are
/// labeled by `EdgeType`.
pub fn render_ontology_dot(ontology: &Ontology) -> String {
    let mut out = String::new();
    out.push_str("digraph ontology {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box, style=rounded];\n");
    for node in ontology.nodes.values() {
        let id = node_dot_id(&node.id.to_string());
        let types = node.types.iter().cloned().collect::<Vec<_>>().join(",");
        let name = node.properties.get("name").map(|v| format!("{v:?}")).unwrap_or_default();
        let label = if name.is_empty() {
            escape(&types)
        } else {
            format!("{} | {}", escape(&name), escape(&types))
        };
        out.push_str(&format!("  {} [label=\"{}\"];\n", id, label));
    }
    for edge in ontology.edges.values() {
        let src = node_dot_id(&edge.source.to_string());
        let dst = node_dot_id(&edge.target.to_string());
        out.push_str(&format!("  {} -> {} [label=\"{}\"];\n", src, dst, escape(&edge.label)));
    }
    out.push_str("}\n");
    out
}

/// Render a provenance log as a GraphViz DOT document showing
/// activities + lineage edges.
pub fn render_provenance_dot(log: &ProvenanceLog) -> String {
    let mut out = String::new();
    out.push_str("digraph provenance {\n");
    out.push_str("  rankdir=TB;\n");
    out.push_str("  node [shape=ellipse, style=filled, fillcolor=lightgray];\n");
    for (id, act) in &log.activities {
        let nid = node_dot_id(&id.to_string());
        out.push_str(&format!("  {} [label=\"{}\"];\n", nid, escape(&act.label)));
    }
    for d in &log.derivations {
        let from = node_dot_id(&d.source.to_string());
        let to = node_dot_id(&d.derived.to_string());
        out.push_str(&format!("  {} -> {} [label=\"wasDerivedFrom\"];\n", to, from));
    }
    out.push_str("}\n");
    out
}

fn node_dot_id(raw: &str) -> String {
    // DOT identifiers should be safe ASCII; hash-style ids already are
    // alphanumeric, so we just prefix to keep them valid.
    let mut s = String::from("n_");
    for c in raw.chars() {
        if c.is_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    if s.len() > 60 {
        s.truncate(60);
    }
    s
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{Edge, Iri, Node, Ontology};

    #[test]
    fn renders_a_graph() {
        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        let a = o.upsert_node(Node::new("Organization").with_property("name", "Acme"));
        let b = o.upsert_node(Node::from_iri(Iri::from_unchecked("https://example.org/B"), "Organization"));
        o.upsert_edge(Edge::between(a, "partner", b));
        let dot = render_ontology_dot(&o);
        assert!(dot.contains("digraph ontology"));
        assert!(dot.contains("partner"));
    }
}
