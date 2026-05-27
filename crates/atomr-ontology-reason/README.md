# atomr-ontology-reason

Forward-chaining OWL 2 RL / EL reasoner for the
[`atomr-ontology`](https://github.com/rustakka/atomr-ontology)
workspace. Materializes derived `SubClassOf` /
`EquivalentClass` / `InverseOf` / transitive / symmetric /
property consequences with `wasDerivedFrom` provenance.

## Features

None — no feature flags. The default `RuleSet::standard()` ships
seven rules; extend with `RuleSet::empty().with(rule)` to opt
into a custom set.

## Example

```rust
use atomr_ontology_reason::Reasoner;

let report = Reasoner::new().materialize(&mut ontology)?;
println!("derived {} axioms in {} iterations",
    report.derived_axioms.len(), report.iterations);
```

Use `Reasoner::run(&ontology)` for a non-mutating run that
returns the derived facts without touching the input.

## Full guide

[`docs/reasoning.md`](../../docs/reasoning.md).
