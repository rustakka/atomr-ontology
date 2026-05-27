//! Git-style branchable, time-travelable ontologies.
//!
//! Each commit is a content-addressed snapshot; branches are named
//! references into the commit DAG; merges resolve a 3-way diff.

#![forbid(unsafe_code)]

pub mod branch;
pub mod commit;
pub mod store;

pub use branch::{Branch, BranchRef};
pub use commit::{Commit, CommitId, MergeStrategy};
pub use store::{VersionError, VersionedStore};
