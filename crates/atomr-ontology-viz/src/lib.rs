//! GraphViz DOT + Mermaid renderers for ontology and provenance graphs.

#![forbid(unsafe_code)]

pub mod dot;
pub mod mermaid;

pub use dot::{render_ontology_dot, render_provenance_dot};
pub use mermaid::{render_ontology_mermaid, render_provenance_mermaid};
