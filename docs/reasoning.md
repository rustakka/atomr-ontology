# Reasoning

`atomr-ontology-reason` is a forward-chaining reasoner over an OWL 2
RL / EL fragment. It closes an `Ontology` under a fixed set of
inference rules, attaches PROV-O lineage to every derived axiom, and
either returns the derivations or merges them back in place. The
implementation is sound, deliberately incomplete (only the rule
subset below), and idempotent on already-closed inputs.

## When to reach for this

- You have authored a partial T-Box and want the transitive /
  symmetric / inverse-of consequences materialized before validation
  or RDF export.
- You want a deductive closure baked into the store so downstream
  pattern queries see e.g. derived `subClassOf` hops without
  re-running the reasoner.
- You need the cheap closure step in an auto-ontology pipeline,
  before the SHACL-style checks in `atomr-ontology-validate`.

Reach for a full description-logic reasoner (HermiT, ELK, …) when
you need consistency checking under disjointness, complex class
expressions, or the full OWL 2 DL semantics — those are out of
scope here.

## Concepts

- `Reasoner` — owns a `RuleSet`, the `AgentRef` credited with each
  derivation, and the iteration cap (`with_max_iterations`,
  defaulting to `DEFAULT_MAX_ITERATIONS = 100`).
- `RuleSet` — an ordered bundle of `Rule` variants. `RuleSet::standard()`
  is the v0.1 default and bundles all seven rules.
- `Rule` — a single closed-form inference shape; one variant per
  derivation pattern. The variants are listed below.
- `ReasoningReport` — output of `Reasoner::run`. Carries
  `derived_axioms`, `derived_edges`, the `iterations` count, and
  the `Activity` that authored the run. `is_clean()` is `true` iff
  no new facts were produced.
- `ReasonerError::Cycle { limit }` — the fixed-point loop did not
  converge within the iteration cap. Treat as a pathological
  ontology or a rule-set bug; do not retry blindly.

### The seven standard rules

| Variant | Premise | Conclusion |
| --- | --- | --- |
| `SubClassTransitivity`   | `A ⊑ B ∧ B ⊑ C`                  | `A ⊑ C` |
| `EquivalentSymmetry`     | `A ≡ B`                          | `B ≡ A` |
| `EquivalentToSubClass`   | `A ≡ B`                          | `A ⊑ B ∧ B ⊑ A` |
| `InverseOfSymmetry`      | `InverseOf(P, Q)`                | `InverseOf(Q, P)` |
| `PropertySymmetry`       | `Symmetric(P) ∧ (a P b)`         | `(b P a)` |
| `PropertyTransitivity`   | `Transitive(P) ∧ (a P b) ∧ (b P c)` | `(a P c)` |
| `InverseOfMaterializes`  | `InverseOf(P, Q) ∧ (a P b)`      | `(b Q a)` |

The first four rules add `Axiom`s; the last three add `Edge`s.
Rules are applied in declaration order at every iteration, and the
engine reaches a fixed point when no rule produces a not-already-
present fact.

### `run` versus `materialize`

- `Reasoner::run(&ontology)` is non-destructive: it clones the
  ontology internally, closes the clone, and returns a
  `ReasoningReport` whose `derived_axioms` and `derived_edges` are
  exactly the newly inferred facts. Pick this when you want to
  inspect, validate, or selectively persist the derivations.
- `Reasoner::materialize(&mut ontology)` runs `run` and then merges
  every derived axiom and edge back into the supplied ontology
  in place. Pick this when you want the closure committed.

Both return the same `ReasoningReport`; in both cases the
underlying activity ends at the moment the fixed point is reached.

### Provenance stamping

Every derived `Axiom` is stamped with `Axiom::with_provenance(activity.id)`
before being inserted into the working ontology. The activity is
the `Activity::started("reasoning")` returned in the report, and
the link reads as a `wasDerivedFrom` edge in the PROV-O surface.
Derived edges currently have no provenance field of their own;
consumers attribute them via the reported activity.

### Cycle handling

`SubClassTransitivity` and `PropertyTransitivity` skip self-loops
when they detect that the candidate conclusion would be reflexive.
On top of that, the reasoner caps the outer fixed-point loop at
`with_max_iterations(...)` (default `100`). Hitting the cap raises
`ReasonerError::Cycle { limit }` rather than spinning forever.

## Rust example

```rust
use atomr_ontology::core::{axiom::{Axiom, AxiomKind}, Edge, Node, Ontology};
use atomr_ontology_reason::{Reasoner, RuleSet};
use atomr_ontology_testkit::fixtures::toy_org_ontology;

let mut ontology: Ontology = toy_org_ontology();

// Author a transitive property plus a two-hop chain that should
// close to a third (a, ancestorOf, c) edge.
ontology.upsert_axiom(Axiom::new(AxiomKind::Transitive {
    property: "ancestorOf".into(),
}));
let a = ontology.upsert_node(Node::new("Person"));
let b = ontology.upsert_node(Node::new("Person"));
let c = ontology.upsert_node(Node::new("Person"));
ontology.upsert_edge(Edge::between(a, "ancestorOf", b));
ontology.upsert_edge(Edge::between(b, "ancestorOf", c));

let reasoner = Reasoner::with_rules(RuleSet::standard())
    .with_max_iterations(50);

// Non-destructive: inspect derivations before committing.
let report = reasoner.run(&ontology).expect("converges");
assert!(report.derived_edges.iter().any(|e|
    e.label == "ancestorOf" && e.source == a && e.target == c));
for axiom in &report.derived_axioms {
    assert_eq!(axiom.provenance, Some(report.activity.id));
}

// Or merge back in place; second pass is a no-op.
let _ = Reasoner::new().materialize(&mut ontology).expect("closure");
assert!(Reasoner::new().run(&ontology).unwrap().is_clean());
```

## Python example

```python
import asyncio
from atomr_ontology.core import Axiom, AxiomKind, Edge, Node
from atomr_ontology.reason import Reasoner, RuleSet
from atomr_ontology.testkit import toy_org_ontology

ontology = toy_org_ontology()
ontology.upsert_axiom(Axiom(AxiomKind.transitive("ancestorOf")))
a = ontology.upsert_node(Node("Person"))
b = ontology.upsert_node(Node("Person"))
c = ontology.upsert_node(Node("Person"))
ontology.upsert_edge(Edge.between(a, "ancestorOf", b))
ontology.upsert_edge(Edge.between(b, "ancestorOf", c))

reasoner = Reasoner.with_max_iterations(50)
assert len(RuleSet.standard()) == 7

# (derived_axioms, derived_edges, iterations, activity)
n_axioms, n_edges, iters, activity = reasoner.run(ontology)
assert n_edges >= 1  # at least (a, ancestorOf, c)

# Merge derived facts back in place.
activity = reasoner.materialize(ontology)
n2_axioms, n2_edges, _, _ = reasoner.run(ontology)
assert n2_axioms == 0 and n2_edges == 0  # idempotent
```

## Reference

| File | Contents |
| --- | --- |
| `crates/atomr-ontology-reason/src/lib.rs`    | Crate root; re-exports `Reasoner`, `ReasonerError`, `ReasoningReport`, `Rule`, `RuleSet`. |
| `crates/atomr-ontology-reason/src/rules.rs`  | `Rule` variants, `RuleSet::standard()`, the seven rule bodies. |
| `crates/atomr-ontology-reason/src/engine.rs` | `Reasoner`, fixed-point loop, provenance stamping, `DEFAULT_MAX_ITERATIONS`. |
| `crates/atomr-ontology-py/src/reason.rs`     | PyO3 wrapper exposing `Reasoner` and `RuleSet` to Python. |
| `crates/atomr-ontology-py/python/atomr_ontology/reason.pyi` | Python type stubs. |

## Cross-links

- [`data-model.md`](data-model.md) — `AxiomKind` variants the rules
  reason over and the provenance surface attached to derived axioms.
- [`architecture.md`](architecture.md) — where reasoning slots into
  the lifecycle (after extraction / induction, before validate /
  commit).
- [`query.md`](query.md) — once the closure is materialized,
  `TraversalPlan` and the Cypher / SPARQL DSL read the derived
  facts without re-running the reasoner.
