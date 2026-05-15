//! Domain-specific assertions.

use atomr_ontology_core::{axiom::AxiomKind, Ontology};

/// Panic when no axiom matching the predicate is present.
pub fn assert_axiom_present<F: Fn(&AxiomKind) -> bool>(ontology: &Ontology, predicate: F, msg: &str) {
    let found = ontology.axioms.values().any(|a| predicate(&a.kind));
    assert!(found, "{msg}");
}

/// Panic unless `sub` is a (transitive) subclass of `sup`.
pub fn assert_subclass_of(ontology: &Ontology, sub: &str, sup: &str) {
    let chain = ontology.schema.supertypes_of(sub);
    if chain.iter().any(|n| n == &sup) {
        return;
    }
    let axiom_chain: Vec<(String, String)> = ontology
        .axioms
        .values()
        .filter_map(|a| match &a.kind {
            AxiomKind::SubClassOf { sub, sup } => Some((sub.clone(), sup.clone())),
            _ => None,
        })
        .collect();
    if reachable(sub, sup, &axiom_chain) {
        return;
    }
    panic!("expected `{sub}` to be a subclass of `{sup}` but no path was found in schema or axioms");
}

fn reachable(start: &str, target: &str, edges: &[(String, String)]) -> bool {
    let mut stack = vec![start.to_string()];
    let mut seen = std::collections::HashSet::new();
    while let Some(node) = stack.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        for (s, t) in edges {
            if s == &node {
                if t == target {
                    return true;
                }
                stack.push(t.clone());
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::toy_org_ontology;

    #[test]
    fn subclass_via_schema() {
        let o = toy_org_ontology();
        assert_subclass_of(&o, "FormalOrganization", "Organization");
    }
}
