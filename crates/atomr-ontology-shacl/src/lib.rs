//! Compile [`Schema`](atomr_ontology_core::Schema) + Axiom candidates
//! to SHACL shapes (Turtle output), and read SHACL shapes back into a
//! `Schema` for validation.

#![forbid(unsafe_code)]

pub mod compile;
pub mod ns;
pub mod parse;

pub use compile::{to_shacl_turtle, ShaclCompileError};
pub use parse::{from_shacl_turtle, ShaclParseError};
