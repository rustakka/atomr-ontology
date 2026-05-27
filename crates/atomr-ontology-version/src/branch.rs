//! Named pointers into the commit DAG.
//!
//! A [`Branch`] is just a `(name, head)` pair: the head is a
//! [`CommitId`] that the branch currently points at. Moving a branch
//! is a single-pointer write; forking is the same operation against a
//! fresh name. The full set of branches lives inside
//! [`VersionedStore`](crate::store::VersionedStore); this module
//! provides the value types and a tiny in-memory registry useful for
//! tests and embeddings.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::commit::CommitId;

/// A named pointer into the commit DAG.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    /// Branch name (e.g. `"main"`, `"feature/x"`).
    pub name: String,
    /// Current head commit.
    pub head: CommitId,
}

impl Branch {
    /// Build a branch with the given name and head commit.
    pub fn new(name: impl Into<String>, head: CommitId) -> Self {
        Self { name: name.into(), head }
    }
}

/// Lightweight reference to a branch by name. Kept as a distinct type
/// so APIs that consume "branch names" don't accidentally accept any
/// `String`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BranchRef(pub String);

impl BranchRef {
    /// Wrap a name as a branch reference.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Borrow the underlying name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BranchRef {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for BranchRef {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Simple in-memory branch registry.
///
/// Used by tests and lightweight callers that want branch management
/// without the full [`VersionedStore`](crate::store::VersionedStore).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BranchRegistry {
    branches: HashMap<String, CommitId>,
}

impl BranchRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a branch pointing at `head`. Returns `false` if the
    /// branch already exists (and leaves the existing entry alone).
    pub fn create(&mut self, name: impl Into<String>, head: CommitId) -> bool {
        let name = name.into();
        if self.branches.contains_key(&name) {
            return false;
        }
        self.branches.insert(name, head);
        true
    }

    /// Move an existing branch to a new head. Returns `false` if the
    /// branch is unknown.
    pub fn move_to(&mut self, name: &str, new_head: CommitId) -> bool {
        if let Some(slot) = self.branches.get_mut(name) {
            *slot = new_head;
            true
        } else {
            false
        }
    }

    /// Resolve a branch name to its current head.
    pub fn head(&self, name: &str) -> Option<CommitId> {
        self.branches.get(name).copied()
    }

    /// List all `(name, head)` pairs in arbitrary order.
    pub fn list(&self) -> Vec<Branch> {
        self.branches.iter().map(|(name, head)| Branch::new(name.clone(), *head)).collect()
    }

    /// True when the registry knows about `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.branches.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cid(byte: u8) -> CommitId {
        CommitId::from_bytes([byte; 32])
    }

    #[test]
    fn create_and_resolve() {
        let mut r = BranchRegistry::new();
        assert!(r.create("main", cid(1)));
        assert_eq!(r.head("main"), Some(cid(1)));
        assert!(!r.create("main", cid(2)), "duplicate create should fail");
        assert_eq!(r.head("main"), Some(cid(1)));
    }

    #[test]
    fn move_to_updates_head() {
        let mut r = BranchRegistry::new();
        r.create("main", cid(1));
        assert!(r.move_to("main", cid(2)));
        assert_eq!(r.head("main"), Some(cid(2)));
        assert!(!r.move_to("missing", cid(3)));
    }

    #[test]
    fn list_returns_all_branches() {
        let mut r = BranchRegistry::new();
        r.create("main", cid(1));
        r.create("feature", cid(2));
        let mut names: Vec<_> = r.list().into_iter().map(|b| b.name).collect();
        names.sort();
        assert_eq!(names, vec!["feature".to_string(), "main".to_string()]);
    }
}
