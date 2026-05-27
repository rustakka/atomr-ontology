//! Persistent [`OntologyStore`](atomr_ontology_store::OntologyStore)
//! backed by pluggable [`Checkpointer`] providers.
//!
//! The design mirrors `atomr-agents-state::Checkpointer`: a
//! single-method `Checkpointer` trait that can save and load a
//! serialized snapshot, with provider impls plugged in behind cargo
//! features. The [`PersistentStore`] wraps any `Checkpointer` and
//! implements `OntologyStore` by buffering mutations in memory and
//! flushing on commit.
//!
//! Two checkpointers ship in this crate:
//!
//! - [`MemCheckpointer`] — Arc<Mutex<Option<Snapshot>>> for tests.
//! - [`FileCheckpointer`] — JSON file on disk (feature `file`,
//!   default-off; useful for single-process workflows).
//!
//! Additional providers (SQLite, Postgres, S3, …) plug in via the
//! same trait and re-export through optional features.

#![forbid(unsafe_code)]

pub mod checkpointer;
pub mod store;
pub mod wire;

#[cfg(feature = "file")]
pub mod file;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use checkpointer::{Checkpointer, CheckpointerError, MemCheckpointer, Snapshot};
pub use store::PersistentStore;

#[cfg(feature = "file")]
pub use file::FileCheckpointer;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteCheckpointer;
