//! Extraction primitives — the agent-driven stages of the auto-ontology
//! pipeline.
//!
//! Each extractor consumes a [`Backend`] (typically an LLM wrapped
//! through `atomr_infer::ModelRunner` or a `MockBackend` from
//! [`atomr-ontology-testkit`](https://docs.rs/atomr-ontology-testkit))
//! and emits typed candidates that downstream stages can resolve
//! against a live [`OntologyStore`](https://docs.rs/atomr-ontology-store).
//!
//! The extractors implement [`Callable`] so they can be composed
//! with `.then()` into a [`Pipeline`]. They also produce structured
//! [`Activity`](atomr_ontology_provenance::Activity) records so a
//! pipeline run is fully introspectable.

#![forbid(unsafe_code)]

pub mod backend;
pub mod entities;
pub mod pipeline;
pub mod records;
pub mod relations;
pub mod terms;

pub use backend::{Backend, BackendError, Prompt};
pub use entities::{EntityCandidate, EntityResolver};
pub use pipeline::{Callable, ExtractStage, Pipeline};
pub use records::RecordExtractor;
pub use relations::{RelationCandidate, RelationExtractor};
pub use terms::{TermCandidate, TermExtractor};
