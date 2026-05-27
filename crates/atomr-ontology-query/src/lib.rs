//! openCypher / SPARQL subset parsers compiling to
//! [`atomr_ontology_store::TraversalPlan`].

#![forbid(unsafe_code)]

pub mod cypher;
pub mod sparql;

pub use cypher::{parse as parse_cypher, CypherError};
pub use sparql::{parse as parse_sparql, SparqlError};
