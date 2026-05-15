//! Axiom-consistency checks (disjoint classes, functional properties, …).

use std::collections::{HashMap, HashSet};

use atomr_ontology_core::{axiom::AxiomKind, Ontology};

use crate::report::{ValidationFinding, ValidationReport};

/// Verify that no node violates an active disjointness or functional axiom.
pub fn check_consistency(ontology: &Ontology) -> ValidationReport {
    let mut report = ValidationReport::default();

    // Disjointness: collect pairs.
    let mut disjoint_pairs: Vec<(String, String)> = Vec::new();
    let mut functional: HashSet<String> = HashSet::new();
    let mut subclass_of: HashMap<String, Vec<String>> = HashMap::new();

    for ax in ontology.axioms.values() {
        match &ax.kind {
            AxiomKind::DisjointWith { left, right } => {
                disjoint_pairs.push((left.clone(), right.clone()));
            }
            AxiomKind::Functional { property } => {
                functional.insert(property.clone());
            }
            AxiomKind::SubClassOf { sub, sup } => {
                subclass_of.entry(sub.clone()).or_default().push(sup.clone());
            }
            _ => {}
        }
    }

    // Node check: no node holds both sides of a disjoint pair.
    for node in ontology.nodes.values() {
        let types: HashSet<&str> = node.types.iter().map(|s| s.as_str()).collect();
        for (a, b) in &disjoint_pairs {
            if types.contains(a.as_str()) && types.contains(b.as_str()) {
                report.push(
                    ValidationFinding::error(
                        "axiom.disjoint",
                        format!("node {} holds both disjoint types `{}` and `{}`", node.id, a, b),
                    )
                    .focus(node.id.to_string()),
                );
            }
        }
    }

    // Functional check: edge with functional label appears at most once per source.
    let mut by_source: HashMap<(String, _), usize> = HashMap::new();
    for edge in ontology.edges.values() {
        if functional.contains(&edge.label) {
            let counter = by_source.entry((edge.label.clone(), edge.source)).or_insert(0);
            *counter += 1;
            if *counter > 1 {
                report.push(
                    ValidationFinding::error(
                        "axiom.functional",
                        format!(
                            "functional edge `{}` has multiple targets from source {}",
                            edge.label, edge.source
                        ),
                    )
                    .focus(edge.id.to_string()),
                );
            }
        }
    }

    // Subclass cycle detection.
    for start in subclass_of.keys() {
        if has_cycle(start, &subclass_of) {
            report.push(ValidationFinding::error(
                "axiom.subclass.cycle",
                format!("subclass relation forms a cycle starting at `{start}`"),
            ));
        }
    }

    report
}

fn has_cycle(start: &str, edges: &HashMap<String, Vec<String>>) -> bool {
    let mut stack = vec![start.to_string()];
    let mut seen: HashSet<String> = HashSet::new();
    while let Some(node) = stack.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        if let Some(ups) = edges.get(&node) {
            for up in ups {
                if up == start {
                    return true;
                }
                stack.push(up.clone());
            }
        }
    }
    false
}
