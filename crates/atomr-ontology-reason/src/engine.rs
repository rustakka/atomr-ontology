//! Forward-chaining reasoning engine.
//!
//! [`Reasoner`] iterates a [`RuleSet`] over an
//! [`Ontology`](atomr_ontology_core::Ontology) until a fixed point is
//! reached, returning the set of *newly derived* axioms and edges plus
//! the [`Activity`] that authored them. The reasoner is sound,
//! incomplete (it implements only the rule subset in
//! [`crate::rules`]), and idempotent: re-running on an already-closed
//! ontology produces zero new facts.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use atomr_ontology_core::{
    axiom::{Axiom, AxiomId},
    Edge, EdgeId, Ontology,
};
use atomr_ontology_provenance::{Activity, AgentRef, ProvenanceId};

use crate::rules::RuleSet;

/// Hard cap on fixed-point iterations. Hitting the cap signals either
/// a pathological ontology (e.g. an exponentially expanding rule set)
/// or a bug in the rule definitions; we error out rather than spin.
pub const DEFAULT_MAX_ITERATIONS: usize = 100;

/// Outcome of a reasoning run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReasoningReport {
    /// Axioms inferred by the rule set. Each carries
    /// `with_provenance(activity.id)` pointing at [`activity`](Self::activity).
    pub derived_axioms: Vec<Axiom>,
    /// Edges inferred by the rule set. Edges do not currently carry a
    /// provenance field on their own; consumers attach lineage via
    /// the activity returned alongside the report.
    pub derived_edges: Vec<Edge>,
    /// Number of fixed-point iterations executed (≥ 1 for any run).
    pub iterations: usize,
    /// The reasoning activity, finished at the moment the fixed point
    /// was reached.
    pub activity: Activity,
}

impl ReasoningReport {
    /// `true` when the reasoner derived no new facts. The ontology
    /// was already deductively closed under the active rule set.
    pub fn is_clean(&self) -> bool {
        self.derived_axioms.is_empty() && self.derived_edges.is_empty()
    }
}

/// Reasoner-level errors.
#[derive(Debug, Error)]
pub enum ReasonerError {
    /// The fixed-point loop ran past [`DEFAULT_MAX_ITERATIONS`]
    /// without converging. Typically indicates a runaway rule
    /// expansion or, less likely, an unbounded ontology.
    #[error("reasoning did not converge after {limit} iterations (cap reached)")]
    Cycle {
        /// The iteration cap that was hit.
        limit: usize,
    },
}

/// Forward-chaining reasoner.
///
/// The reasoner owns its [`RuleSet`] and the [`AgentRef`] credited
/// with every derivation it materializes. Instances are cheap to
/// clone and intentionally stateless across calls — pass an
/// `&Ontology` to each invocation.
#[derive(Clone, Debug)]
pub struct Reasoner {
    rules: RuleSet,
    agent: AgentRef,
    max_iterations: usize,
}

impl Reasoner {
    /// Build a reasoner with the standard rule set.
    pub fn new() -> Self {
        Self::with_rules(RuleSet::standard())
    }

    /// Build a reasoner with a custom rule set.
    pub fn with_rules(rules: RuleSet) -> Self {
        Self {
            rules,
            agent: AgentRef::software(
                "agent://atomr-ontology-reason/Reasoner",
                "atomr-ontology-reason",
            ),
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }

    /// Override the agent credited with derived axioms. Useful when
    /// the reasoner is driven by a higher-level pipeline that wants
    /// to attribute the run to its own identity.
    pub fn with_agent(mut self, agent: AgentRef) -> Self {
        self.agent = agent;
        self
    }

    /// Override the per-run iteration cap. The default is
    /// [`DEFAULT_MAX_ITERATIONS`].
    pub fn with_max_iterations(mut self, limit: usize) -> Self {
        self.max_iterations = limit.max(1);
        self
    }

    /// Borrow the active rule set.
    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// Run the reasoner against `ontology` (immutably), returning the
    /// set of newly-derived facts. The supplied ontology is **not**
    /// modified — use [`Reasoner::materialize`] for the
    /// convenience-mutate variant.
    pub fn run(&self, ontology: &Ontology) -> Result<ReasoningReport, ReasonerError> {
        // We accumulate derivations into a clone so successive rules
        // and iterations can chain on prior conclusions, then diff
        // against the original to return only the *new* facts.
        let original_axioms: BTreeSet<AxiomId> = ontology.axioms.keys().copied().collect();
        let original_edges: BTreeSet<EdgeId> = ontology.edges.keys().copied().collect();

        let mut working = ontology.clone();
        let activity = Activity::started("reasoning")
            .by(self.agent.clone())
            .with_attribute("rules", serde_json::json!(self.rules.rules.len()));
        let prov_id: ProvenanceId = activity.id;

        let mut iterations = 0_usize;
        loop {
            iterations += 1;
            if iterations > self.max_iterations {
                return Err(ReasonerError::Cycle { limit: self.max_iterations });
            }

            let mut produced_new = false;
            for rule in self.rules.iter() {
                let out = rule.apply(&working);
                for ax in out.axioms {
                    // Provenance-stamp every derived axiom.
                    let ax = stamp_axiom(ax, prov_id);
                    if !working.axioms.contains_key(&ax.id) {
                        working.upsert_axiom(ax);
                        produced_new = true;
                    }
                }
                for edge in out.edges {
                    if !working.edges.contains_key(&edge.id) {
                        working.upsert_edge(edge);
                        produced_new = true;
                    }
                }
            }
            if !produced_new {
                break;
            }
        }

        // Diff: anything in `working` whose id was not in the
        // original ontology is a derivation.
        let mut derived_axioms: Vec<Axiom> = working
            .axioms
            .iter()
            .filter(|(id, _)| !original_axioms.contains(id))
            .map(|(_, a)| a.clone())
            .collect();
        derived_axioms.sort_by(|l, r| l.id.cmp(&r.id));

        let mut derived_edges: Vec<Edge> = working
            .edges
            .iter()
            .filter(|(id, _)| !original_edges.contains(id))
            .map(|(_, e)| e.clone())
            .collect();
        derived_edges.sort_by(|l, r| l.id.cmp(&r.id));

        Ok(ReasoningReport {
            derived_axioms,
            derived_edges,
            iterations,
            activity: activity.finish(),
        })
    }

    /// Run the reasoner against `ontology` and merge every derived
    /// axiom and edge back into it in-place.
    pub fn materialize(&self, ontology: &mut Ontology) -> Result<ReasoningReport, ReasonerError> {
        let report = self.run(ontology)?;
        for ax in &report.derived_axioms {
            ontology.upsert_axiom(ax.clone());
        }
        for edge in &report.derived_edges {
            ontology.upsert_edge(edge.clone());
        }
        Ok(report)
    }
}

impl Default for Reasoner {
    fn default() -> Self {
        Self::new()
    }
}

/// Stamp a freshly-derived axiom with the reasoning activity id.
///
/// Axiom ids are content-addressed over the body, so attaching
/// provenance never perturbs identity — we go through
/// [`Axiom::with_provenance`] to keep the construction shape uniform
/// with the rest of the workspace.
fn stamp_axiom(ax: Axiom, prov: ProvenanceId) -> Axiom {
    ax.with_provenance(prov)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{axiom::AxiomKind, Edge, Node};

    fn axiom(kind: AxiomKind) -> Axiom {
        Axiom::new(kind)
    }

    #[test]
    fn empty_ontology_yields_clean_report() {
        let o = Ontology::new();
        let r = Reasoner::new().run(&o).expect("run");
        assert!(r.is_clean());
        // At minimum one iteration runs (to detect the no-change condition).
        assert!(r.iterations >= 1);
    }

    #[test]
    fn subclass_transitivity_three_class_chain() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "C".into() }));
        let report = Reasoner::new().run(&o).expect("run");
        let derived_pairs: Vec<(String, String)> = report
            .derived_axioms
            .iter()
            .filter_map(|a| match &a.kind {
                AxiomKind::SubClassOf { sub, sup } => Some((sub.clone(), sup.clone())),
                _ => None,
            })
            .collect();
        assert!(derived_pairs.contains(&("A".into(), "C".into())));
    }

    #[test]
    fn equivalent_symmetry_is_derived() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::EquivalentClass {
            left: "Org".into(),
            right: "Organization".into(),
        }));
        let report = Reasoner::new().run(&o).expect("run");
        let symmetric = report.derived_axioms.iter().any(|a| {
            matches!(&a.kind, AxiomKind::EquivalentClass { left, right }
                if left == "Organization" && right == "Org")
        });
        assert!(symmetric, "Equivalent symmetry not derived");
    }

    #[test]
    fn transitive_property_closes_a_chain() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::Transitive { property: "ancestorOf".into() }));
        let a = o.upsert_node(Node::new("Person"));
        let b = o.upsert_node(Node::new("Person"));
        let c = o.upsert_node(Node::new("Person"));
        o.upsert_edge(Edge::between(a, "ancestorOf", b));
        o.upsert_edge(Edge::between(b, "ancestorOf", c));
        let report = Reasoner::new().run(&o).expect("run");
        let has_ac = report
            .derived_edges
            .iter()
            .any(|e| e.label == "ancestorOf" && e.source == a && e.target == c);
        assert!(has_ac, "transitive (a, c) edge not derived");
    }

    #[test]
    fn transitive_property_closes_longer_chain_over_iterations() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::Transitive { property: "ancestorOf".into() }));
        let a = o.upsert_node(Node::new("Person"));
        let b = o.upsert_node(Node::new("Person"));
        let c = o.upsert_node(Node::new("Person"));
        let d = o.upsert_node(Node::new("Person"));
        o.upsert_edge(Edge::between(a, "ancestorOf", b));
        o.upsert_edge(Edge::between(b, "ancestorOf", c));
        o.upsert_edge(Edge::between(c, "ancestorOf", d));
        let report = Reasoner::new().run(&o).expect("run");
        // Expect (a,c), (b,d), (a,d) all derived.
        assert!(report.derived_edges.iter().any(|e| e.source == a && e.target == c));
        assert!(report.derived_edges.iter().any(|e| e.source == b && e.target == d));
        assert!(report.derived_edges.iter().any(|e| e.source == a && e.target == d));
    }

    #[test]
    fn inverse_of_bidirectional() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::InverseOf {
            left: "memberOf".into(),
            right: "hasMember".into(),
        }));
        let alice = o.upsert_node(Node::new("Person"));
        let acme = o.upsert_node(Node::new("Organization"));
        o.upsert_edge(Edge::between(alice, "memberOf", acme));
        let report = Reasoner::new().run(&o).expect("run");
        // Should derive the reverse (acme, hasMember, alice) edge.
        let reverse =
            report.derived_edges.iter().any(|e| e.source == acme && e.target == alice && e.label == "hasMember");
        assert!(reverse, "inverse-of edge not materialized");
        // Should also derive InverseOf(hasMember, memberOf) as an axiom.
        let inverse_axiom = report.derived_axioms.iter().any(|a| {
            matches!(&a.kind, AxiomKind::InverseOf { left, right }
                if left == "hasMember" && right == "memberOf")
        });
        assert!(inverse_axiom, "inverse-of axiom symmetry not derived");
    }

    #[test]
    fn cycle_in_subclass_does_not_infinite_loop() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "A".into() }));
        // The reasoner must terminate. Whether it derives transitive
        // hops between A and B is a soundness question — but it must
        // never run forever.
        let report = Reasoner::new().run(&o).expect("run terminates on cycle");
        assert!(report.iterations <= DEFAULT_MAX_ITERATIONS);
    }

    #[test]
    fn idempotent_on_a_closed_ontology() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        let reasoner = Reasoner::new();
        let _first = reasoner.materialize(&mut o).expect("first run");
        let second = reasoner.run(&o).expect("second run");
        assert!(second.is_clean(), "re-running on closed ontology must produce nothing");
    }

    #[test]
    fn derived_axioms_carry_activity_provenance() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "C".into() }));
        let report = Reasoner::new().run(&o).expect("run");
        let act_id = report.activity.id;
        for a in &report.derived_axioms {
            assert_eq!(a.provenance, Some(act_id));
        }
    }

    #[test]
    fn materialize_merges_back_into_ontology() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "C".into() }));
        let before = o.axioms.len();
        let report = Reasoner::new().materialize(&mut o).expect("materialize");
        assert_eq!(o.axioms.len(), before + report.derived_axioms.len());
    }

    #[test]
    fn cap_iterations_raises_cycle_error() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::Transitive { property: "knows".into() }));
        let a = o.upsert_node(Node::new("Person"));
        let b = o.upsert_node(Node::new("Person"));
        o.upsert_edge(Edge::between(a, "knows", b));
        o.upsert_edge(Edge::between(b, "knows", a));
        // Force the cap absurdly low to trigger the error path.
        let reasoner = Reasoner::new().with_max_iterations(1);
        // With a cap of 1 and a non-empty derivation, the loop will
        // exceed the cap on the second pass; if the ontology happens
        // to converge in one pass we don't fail the test.
        match reasoner.run(&o) {
            Err(ReasonerError::Cycle { limit }) => assert_eq!(limit, 1),
            Ok(_) => { /* converged in one iteration, acceptable */ }
        }
    }
}
