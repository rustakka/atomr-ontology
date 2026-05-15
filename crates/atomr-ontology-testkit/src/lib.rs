//! Testing helpers for `atomr-ontology`.

#![forbid(unsafe_code)]

pub mod assertions;
pub mod fixtures;
pub mod mock;

pub use assertions::{assert_axiom_present, assert_subclass_of};
pub use fixtures::{toy_corpus, toy_org_ontology};
pub use mock::MockBackend;
