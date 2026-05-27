//! Mermaid `graph LR` renderer.

use atomr_ontology_core::Ontology;
use atomr_ontology_provenance::ProvenanceLog;

/// Render an ontology as a Mermaid graph.
pub fn render_ontology_mermaid(ontology: &Ontology) -> String {
    let mut out = String::new();
    out.push_str("graph LR\n");
    for node in ontology.nodes.values() {
        let id = mermaid_id(&node.id.to_string());
        let name = node
            .properties
            .get("name")
            .map(|v| format!("{v:?}"))
            .unwrap_or_else(|| node.types.first().cloned().unwrap_or_default());
        out.push_str(&format!("  {id}[\"{}\"]\n", escape(&name)));
    }
    for edge in ontology.edges.values() {
        let src = mermaid_id(&edge.source.to_string());
        let dst = mermaid_id(&edge.target.to_string());
        out.push_str(&format!("  {src} -- \"{}\" --> {dst}\n", escape(&edge.label)));
    }
    out
}

/// Render a provenance log as a Mermaid graph.
pub fn render_provenance_mermaid(log: &ProvenanceLog) -> String {
    let mut out = String::new();
    out.push_str("graph TD\n");
    for (id, act) in &log.activities {
        let nid = mermaid_id(&id.to_string());
        out.push_str(&format!("  {nid}([\"{}\"])\n", escape(&act.label)));
    }
    for d in &log.derivations {
        let from = mermaid_id(&d.source.to_string());
        let to = mermaid_id(&d.derived.to_string());
        out.push_str(&format!("  {to} -- wasDerivedFrom --> {from}\n"));
    }
    out
}

fn mermaid_id(raw: &str) -> String {
    let mut s = String::with_capacity(raw.len() + 1);
    s.push('n');
    for c in raw.chars().take(20) {
        if c.is_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    s
}

fn escape(s: &str) -> String {
    s.replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{Edge, Node, Ontology};

    #[test]
    fn renders_ontology() {
        let mut o = Ontology::new();
        o.declare_node_type("Org");
        let a = o.upsert_node(Node::new("Org").with_property("name", "Acme"));
        let b = o.upsert_node(Node::new("Org"));
        o.upsert_edge(Edge::between(a, "subOrgOf", b));
        let m = render_ontology_mermaid(&o);
        assert!(m.starts_with("graph LR"));
        assert!(m.contains("subOrgOf"));
    }
}
