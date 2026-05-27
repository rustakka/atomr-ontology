//! Forward-chaining rule definitions.
//!
//! Each [`Rule`] is a single OWL 2 RL / EL inference shape that the
//! [`Reasoner`](crate::engine::Reasoner) repeatedly applies until a
//! fixed point is reached. Rules either derive new
//! [`Axiom`](atomr_ontology_core::Axiom)s (T-Box / schema-level facts)
//! or new [`Edge`](atomr_ontology_core::Edge)s (A-Box / instance-level
//! facts) — never both at once.
//!
//! The shapes implemented here are intentionally a minimal subset that
//! covers the rules the validator and the LPG materializer rely on:
//!
//! - `SubClassTransitivity` — `A ⊑ B ∧ B ⊑ C ⟹ A ⊑ C`
//! - `EquivalentSymmetry`   — `A ≡ B ⟹ B ≡ A`
//! - `EquivalentToSubClass` — `A ≡ B ⟹ A ⊑ B ∧ B ⊑ A`
//! - `InverseOfSymmetry`    — `InverseOf(P, Q) ⟹ InverseOf(Q, P)`
//! - `PropertySymmetry`     — `Symmetric(P) ∧ (a P b) ⟹ (b P a)`
//! - `PropertyTransitivity` — `Transitive(P) ∧ (a P b) ∧ (b P c) ⟹ (a P c)`
//! - `InverseOfMaterializes`— `InverseOf(P, Q) ∧ (a P b) ⟹ (b Q a)`

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use atomr_ontology_core::{
    axiom::{Axiom, AxiomKind},
    Edge, Ontology,
};

/// The output of a single rule application against an ontology.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleOutput {
    /// Axioms produced by this rule pass.
    pub axioms: Vec<Axiom>,
    /// Edges produced by this rule pass.
    pub edges: Vec<Edge>,
}

impl RuleOutput {
    /// Empty output.
    pub fn empty() -> Self {
        Self::default()
    }

    /// `true` when the rule produced neither axioms nor edges.
    pub fn is_empty(&self) -> bool {
        self.axioms.is_empty() && self.edges.is_empty()
    }
}

/// A single forward-chaining inference rule.
///
/// Each variant is a closed-form shape; the engine matches on the
/// variant rather than dispatching through a trait object so we can
/// keep the rule set serializable and inspectable from Python / RDF.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum Rule {
    /// `A ⊑ B ∧ B ⊑ C ⟹ A ⊑ C`
    SubClassTransitivity,
    /// `A ≡ B ⟹ B ≡ A`
    EquivalentSymmetry,
    /// `A ≡ B ⟹ A ⊑ B ∧ B ⊑ A`
    EquivalentToSubClass,
    /// `InverseOf(P, Q) ⟹ InverseOf(Q, P)`
    InverseOfSymmetry,
    /// `Symmetric(P) ∧ (a P b) ⟹ (b P a)`
    PropertySymmetry,
    /// `Transitive(P) ∧ (a P b) ∧ (b P c) ⟹ (a P c)`
    PropertyTransitivity,
    /// `InverseOf(P, Q) ∧ (a P b) ⟹ (b Q a)`
    InverseOfMaterializes,
}

impl Rule {
    /// Apply this rule once to the supplied ontology, returning the
    /// derivations the rule would add. The output is *not* deduped
    /// against already-known facts; the engine handles that.
    pub fn apply(self, ontology: &Ontology) -> RuleOutput {
        match self {
            Rule::SubClassTransitivity => subclass_transitivity(ontology),
            Rule::EquivalentSymmetry => equivalent_symmetry(ontology),
            Rule::EquivalentToSubClass => equivalent_to_subclass(ontology),
            Rule::InverseOfSymmetry => inverse_of_symmetry(ontology),
            Rule::PropertySymmetry => property_symmetry(ontology),
            Rule::PropertyTransitivity => property_transitivity(ontology),
            Rule::InverseOfMaterializes => inverse_of_materializes(ontology),
        }
    }
}

/// An ordered bundle of [`Rule`]s applied together by a reasoner pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleSet {
    /// The active rules. Order is irrelevant for soundness but
    /// affects which rule is credited as "first to derive" a given
    /// fact (rules are applied in declaration order).
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Build an empty rule set.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Build a rule set from an explicit list.
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// The standard OWL 2 RL / EL subset shipped with v0.1.
    pub fn standard() -> Self {
        Self {
            rules: vec![
                Rule::SubClassTransitivity,
                Rule::EquivalentSymmetry,
                Rule::EquivalentToSubClass,
                Rule::InverseOfSymmetry,
                Rule::PropertySymmetry,
                Rule::PropertyTransitivity,
                Rule::InverseOfMaterializes,
            ],
        }
    }

    /// Append a rule.
    pub fn with(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Iterate the active rules in declaration order.
    pub fn iter(&self) -> std::slice::Iter<'_, Rule> {
        self.rules.iter()
    }

    /// `true` if the rule set has no rules — application is a no-op.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::standard()
    }
}

// ---------------------------------------------------------------------
// Rule bodies
// ---------------------------------------------------------------------

fn subclass_transitivity(ontology: &Ontology) -> RuleOutput {
    // Index: sub -> [sup]. The relation is asymmetric in general (a
    // cycle is an authoring error caught by the validator), so we
    // walk every direct edge and pair it with every successor.
    let mut by_sub: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for ax in ontology.axioms.values() {
        if let AxiomKind::SubClassOf { sub, sup } = &ax.kind {
            by_sub.entry(sub.as_str()).or_default().insert(sup.as_str());
        }
    }

    let mut out = RuleOutput::empty();
    for (sub, sups) in &by_sub {
        for sup in sups {
            // Walk one extra hop: B ⊑ C for each B in sups.
            if let Some(grandsups) = by_sub.get(sup) {
                for grand in grandsups {
                    if grand == sub {
                        // Cycle detected; skip — engine treats this
                        // as authored data, not a derivation target.
                        continue;
                    }
                    if sups.contains(grand) {
                        // Already a direct fact; don't re-emit.
                        continue;
                    }
                    out.axioms.push(Axiom::new(AxiomKind::SubClassOf {
                        sub: (*sub).to_string(),
                        sup: (*grand).to_string(),
                    }));
                }
            }
        }
    }
    out
}

fn equivalent_symmetry(ontology: &Ontology) -> RuleOutput {
    let mut existing: BTreeSet<(&str, &str)> = BTreeSet::new();
    for ax in ontology.axioms.values() {
        if let AxiomKind::EquivalentClass { left, right } = &ax.kind {
            existing.insert((left.as_str(), right.as_str()));
        }
    }
    let mut out = RuleOutput::empty();
    for (left, right) in &existing {
        if !existing.contains(&(*right, *left)) {
            out.axioms.push(Axiom::new(AxiomKind::EquivalentClass {
                left: (*right).to_string(),
                right: (*left).to_string(),
            }));
        }
    }
    out
}

fn equivalent_to_subclass(ontology: &Ontology) -> RuleOutput {
    let mut equivalents: Vec<(&str, &str)> = Vec::new();
    let mut subclasses: BTreeSet<(&str, &str)> = BTreeSet::new();
    for ax in ontology.axioms.values() {
        match &ax.kind {
            AxiomKind::EquivalentClass { left, right } => {
                equivalents.push((left.as_str(), right.as_str()));
            }
            AxiomKind::SubClassOf { sub, sup } => {
                subclasses.insert((sub.as_str(), sup.as_str()));
            }
            _ => {}
        }
    }
    let mut out = RuleOutput::empty();
    for (left, right) in equivalents {
        if !subclasses.contains(&(left, right)) {
            out.axioms.push(Axiom::new(AxiomKind::SubClassOf {
                sub: left.to_string(),
                sup: right.to_string(),
            }));
        }
        if !subclasses.contains(&(right, left)) {
            out.axioms.push(Axiom::new(AxiomKind::SubClassOf {
                sub: right.to_string(),
                sup: left.to_string(),
            }));
        }
    }
    out
}

fn inverse_of_symmetry(ontology: &Ontology) -> RuleOutput {
    let mut existing: BTreeSet<(&str, &str)> = BTreeSet::new();
    for ax in ontology.axioms.values() {
        if let AxiomKind::InverseOf { left, right } = &ax.kind {
            existing.insert((left.as_str(), right.as_str()));
        }
    }
    let mut out = RuleOutput::empty();
    for (left, right) in &existing {
        if !existing.contains(&(*right, *left)) {
            out.axioms.push(Axiom::new(AxiomKind::InverseOf {
                left: (*right).to_string(),
                right: (*left).to_string(),
            }));
        }
    }
    out
}

fn property_symmetry(ontology: &Ontology) -> RuleOutput {
    let mut symmetric: BTreeSet<&str> = BTreeSet::new();
    for ax in ontology.axioms.values() {
        if let AxiomKind::Symmetric { property } = &ax.kind {
            symmetric.insert(property.as_str());
        }
    }
    if symmetric.is_empty() {
        return RuleOutput::empty();
    }
    // Existing (label, source, target) tuples to dedupe against.
    let mut existing: BTreeSet<(&str, _, _)> = BTreeSet::new();
    for e in ontology.edges.values() {
        existing.insert((e.label.as_str(), e.source, e.target));
    }
    let mut out = RuleOutput::empty();
    for e in ontology.edges.values() {
        if !symmetric.contains(e.label.as_str()) {
            continue;
        }
        let needle = (e.label.as_str(), e.target, e.source);
        if existing.contains(&needle) {
            continue;
        }
        out.edges.push(Edge::between(e.target, e.label.clone(), e.source));
    }
    out
}

fn property_transitivity(ontology: &Ontology) -> RuleOutput {
    let mut transitive: BTreeSet<&str> = BTreeSet::new();
    for ax in ontology.axioms.values() {
        if let AxiomKind::Transitive { property } = &ax.kind {
            transitive.insert(property.as_str());
        }
    }
    if transitive.is_empty() {
        return RuleOutput::empty();
    }

    // Bucket edges by label so we only join compatible properties,
    // then index by source within each label for fast successor lookup.
    let mut by_label_then_source: BTreeMap<&str, BTreeMap<_, Vec<_>>> = BTreeMap::new();
    let mut existing: BTreeSet<(&str, _, _)> = BTreeSet::new();
    for e in ontology.edges.values() {
        if transitive.contains(e.label.as_str()) {
            by_label_then_source
                .entry(e.label.as_str())
                .or_default()
                .entry(e.source)
                .or_default()
                .push(e.target);
        }
        existing.insert((e.label.as_str(), e.source, e.target));
    }

    let mut out = RuleOutput::empty();
    for (label, by_source) in &by_label_then_source {
        for (a, bs) in by_source {
            for b in bs {
                if let Some(cs) = by_source.get(b) {
                    for c in cs {
                        if a == c {
                            // Reflexive closure isn't in the rule's
                            // body — skip self-loops produced by data.
                            continue;
                        }
                        if existing.contains(&(*label, *a, *c)) {
                            continue;
                        }
                        out.edges.push(Edge::between(*a, (*label).to_string(), *c));
                    }
                }
            }
        }
    }
    out
}

fn inverse_of_materializes(ontology: &Ontology) -> RuleOutput {
    // Collect both directions of declared inverse pairs so that we can
    // materialize triples in either rotation without depending on
    // `InverseOfSymmetry` having already fired.
    let mut inverse_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for ax in ontology.axioms.values() {
        if let AxiomKind::InverseOf { left, right } = &ax.kind {
            inverse_of.entry(left.as_str()).or_default().insert(right.as_str());
            inverse_of.entry(right.as_str()).or_default().insert(left.as_str());
        }
    }
    if inverse_of.is_empty() {
        return RuleOutput::empty();
    }
    let mut existing: BTreeSet<(&str, _, _)> = BTreeSet::new();
    for e in ontology.edges.values() {
        existing.insert((e.label.as_str(), e.source, e.target));
    }

    let mut out = RuleOutput::empty();
    for e in ontology.edges.values() {
        let Some(inverses) = inverse_of.get(e.label.as_str()) else { continue };
        for inv in inverses {
            if existing.contains(&(*inv, e.target, e.source)) {
                continue;
            }
            out.edges.push(Edge::between(e.target, (*inv).to_string(), e.source));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{Edge, Node};

    fn axiom(kind: AxiomKind) -> Axiom {
        Axiom::new(kind)
    }

    #[test]
    fn standard_includes_all_rules() {
        let rs = RuleSet::standard();
        assert_eq!(rs.rules.len(), 7);
        assert!(rs.rules.contains(&Rule::SubClassTransitivity));
        assert!(rs.rules.contains(&Rule::PropertyTransitivity));
    }

    #[test]
    fn subclass_transitivity_emits_skip_hop() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "C".into() }));
        let out = Rule::SubClassTransitivity.apply(&o);
        assert_eq!(out.axioms.len(), 1);
        match &out.axioms[0].kind {
            AxiomKind::SubClassOf { sub, sup } => {
                assert_eq!(sub, "A");
                assert_eq!(sup, "C");
            }
            other => panic!("unexpected kind: {other:?}"),
        }
    }

    #[test]
    fn subclass_transitivity_does_not_loop_on_cycle() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "A".into() }));
        // Applying once is well-defined and finite — no panic, no infinite loop.
        let out = Rule::SubClassTransitivity.apply(&o);
        // Neither A ⊑ A nor B ⊑ B is emitted (cycles are skipped).
        for a in &out.axioms {
            if let AxiomKind::SubClassOf { sub, sup } = &a.kind {
                assert_ne!(sub, sup, "rule must not emit reflexive cycles");
            }
        }
    }

    #[test]
    fn equivalent_symmetry_round_trips() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::EquivalentClass {
            left: "Org".into(),
            right: "Organization".into(),
        }));
        let out = Rule::EquivalentSymmetry.apply(&o);
        assert_eq!(out.axioms.len(), 1);
        match &out.axioms[0].kind {
            AxiomKind::EquivalentClass { left, right } => {
                assert_eq!(left, "Organization");
                assert_eq!(right, "Org");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn equivalent_to_subclass_yields_two_subclass_facts() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::EquivalentClass {
            left: "Org".into(),
            right: "Organization".into(),
        }));
        let out = Rule::EquivalentToSubClass.apply(&o);
        assert_eq!(out.axioms.len(), 2);
    }

    #[test]
    fn property_symmetry_materializes_reverse_edge() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::Symmetric { property: "knows".into() }));
        let a = o.upsert_node(Node::new("Person"));
        let b = o.upsert_node(Node::new("Person"));
        o.upsert_edge(Edge::between(a, "knows", b));
        let out = Rule::PropertySymmetry.apply(&o);
        assert_eq!(out.edges.len(), 1);
        assert_eq!(out.edges[0].source, b);
        assert_eq!(out.edges[0].target, a);
    }

    #[test]
    fn transitive_property_chains() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::Transitive { property: "ancestorOf".into() }));
        let a = o.upsert_node(Node::new("Person"));
        let b = o.upsert_node(Node::new("Person"));
        let c = o.upsert_node(Node::new("Person"));
        o.upsert_edge(Edge::between(a, "ancestorOf", b));
        o.upsert_edge(Edge::between(b, "ancestorOf", c));
        let out = Rule::PropertyTransitivity.apply(&o);
        assert_eq!(out.edges.len(), 1);
        assert_eq!(out.edges[0].source, a);
        assert_eq!(out.edges[0].target, c);
        assert_eq!(out.edges[0].label, "ancestorOf");
    }

    #[test]
    fn inverse_of_symmetry_round_trips() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::InverseOf {
            left: "memberOf".into(),
            right: "hasMember".into(),
        }));
        let out = Rule::InverseOfSymmetry.apply(&o);
        assert_eq!(out.axioms.len(), 1);
        match &out.axioms[0].kind {
            AxiomKind::InverseOf { left, right } => {
                assert_eq!(left, "hasMember");
                assert_eq!(right, "memberOf");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn inverse_of_materializes_both_directions() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::InverseOf {
            left: "memberOf".into(),
            right: "hasMember".into(),
        }));
        let alice = o.upsert_node(Node::new("Person"));
        let acme = o.upsert_node(Node::new("Organization"));
        o.upsert_edge(Edge::between(alice, "memberOf", acme));
        let out = Rule::InverseOfMaterializes.apply(&o);
        assert_eq!(out.edges.len(), 1);
        assert_eq!(out.edges[0].label, "hasMember");
        assert_eq!(out.edges[0].source, acme);
        assert_eq!(out.edges[0].target, alice);
    }

    #[test]
    fn rules_are_idempotent_on_a_closed_ontology() {
        let mut o = Ontology::new();
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "B".into(), sup: "C".into() }));
        o.upsert_axiom(axiom(AxiomKind::SubClassOf { sub: "A".into(), sup: "C".into() }));
        let out = Rule::SubClassTransitivity.apply(&o);
        assert!(out.is_empty(), "closed ontology must yield no new facts");
    }
}
