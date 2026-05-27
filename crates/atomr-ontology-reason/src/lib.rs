//! Forward-chaining reasoner over OWL 2 RL / EL axiom fragments.
//!
//! Materializes derived `SubClassOf`, `EquivalentClass`, transitive
//! closures, and inverse-of triples into the ontology with
//! `wasDerivedFrom` provenance attached.

#![forbid(unsafe_code)]

pub mod engine;
pub mod rules;

pub use engine::{Reasoner, ReasonerError, ReasoningReport};
pub use rules::{Rule, RuleSet};
