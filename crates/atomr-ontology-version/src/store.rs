//! [`VersionedStore`] — Git-style branchable history over
//! [`Ontology`](atomr_ontology_core::Ontology) snapshots.
//!
//! The store owns a commit DAG keyed by [`CommitId`] and a flat
//! `name -> CommitId` table for branches. It exposes operations
//! familiar from Git: `commit`, `branch`, `checkout`, `merge`, `log`,
//! plus `as_of` for time-travel reads.
//!
//! The store is intentionally in-memory; persistence can layer on top
//! by serializing the `commits` and `branches` maps.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use atomr_ontology_core::Ontology;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::commit::{compute_id, Commit, CommitId, MergeStrategy};

/// Default branch name created by [`VersionedStore::init`].
pub const DEFAULT_BRANCH: &str = "main";

/// Errors raised by [`VersionedStore`] operations.
#[derive(Debug, Error)]
pub enum VersionError {
    /// The requested branch is not known to the store.
    #[error("unknown branch: {0}")]
    UnknownBranch(String),
    /// The requested commit is not known to the store.
    #[error("unknown commit: {0}")]
    UnknownCommit(CommitId),
    /// A branch with that name already exists; refused to overwrite.
    #[error("branch already exists: {0}")]
    BranchExists(String),
    /// Merge invoked but the other branch has no commits.
    #[error("nothing to merge: branch {0} has no commits")]
    NothingToMerge(String),
    /// The current branch has no head yet (no commits made).
    #[error("current branch {0} has no head; commit first")]
    EmptyCurrentBranch(String),
    /// Caller tried to merge a branch into itself.
    #[error("cannot merge branch {0} into itself")]
    SelfMerge(String),
}

/// Branchable, time-travelable history over ontology snapshots.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionedStore {
    /// All known commits, keyed by id. `BTreeMap` gives deterministic
    /// iteration for reproducible serialization.
    pub commits: BTreeMap<CommitId, Commit>,
    /// `branch name -> current head`.
    pub branches: HashMap<String, CommitId>,
    /// Name of the currently checked-out branch.
    pub current: String,
}

impl Default for VersionedStore {
    fn default() -> Self {
        Self::init()
    }
}

impl VersionedStore {
    /// Empty repository with `main` as the default current branch and
    /// no commits yet.
    pub fn init() -> Self {
        Self {
            commits: BTreeMap::new(),
            branches: HashMap::new(),
            current: DEFAULT_BRANCH.to_owned(),
        }
    }

    /// Name of the current branch.
    pub fn current_branch(&self) -> &str {
        &self.current
    }

    /// Return all known branch names in arbitrary order.
    pub fn branch_names(&self) -> Vec<String> {
        self.branches.keys().cloned().collect()
    }

    /// Append a commit to the current branch and advance its head.
    ///
    /// Returns the new commit's id. The parent is the current
    /// branch's head (or `None` if this is the first commit).
    pub fn commit(
        &mut self,
        message: String,
        author: String,
        snapshot: Ontology,
    ) -> CommitId {
        let parent = self.branches.get(&self.current).copied();
        let commit = Commit::new(
            parent,
            None,
            message,
            author,
            Utc::now(),
            snapshot,
            None,
        );
        let id = commit.id;
        self.commits.insert(id, commit);
        self.branches.insert(self.current.clone(), id);
        id
    }

    /// Fork the current head to a new branch named `name`.
    ///
    /// The new branch points at the same commit as the current branch.
    /// The current branch is *not* changed — call [`checkout`] to
    /// switch.
    pub fn branch(&mut self, name: String) -> Result<(), VersionError> {
        if self.branches.contains_key(&name) {
            return Err(VersionError::BranchExists(name));
        }
        let head = self
            .branches
            .get(&self.current)
            .copied()
            .ok_or_else(|| VersionError::EmptyCurrentBranch(self.current.clone()))?;
        self.branches.insert(name, head);
        Ok(())
    }

    /// Switch the current branch.
    pub fn checkout(&mut self, name: &str) -> Result<(), VersionError> {
        if !self.branches.contains_key(name) {
            return Err(VersionError::UnknownBranch(name.to_owned()));
        }
        self.current = name.to_owned();
        Ok(())
    }

    /// Merge `other` into the current branch using `strategy`.
    ///
    /// Produces a merge commit whose `parent` is the current head and
    /// `second_parent` is the other branch's head. Returns the new
    /// merge commit's id.
    pub fn merge(
        &mut self,
        other: &str,
        strategy: MergeStrategy,
    ) -> Result<CommitId, VersionError> {
        if other == self.current {
            return Err(VersionError::SelfMerge(other.to_owned()));
        }

        let other_head = self
            .branches
            .get(other)
            .copied()
            .ok_or_else(|| VersionError::UnknownBranch(other.to_owned()))?;
        let current_head = self
            .branches
            .get(&self.current)
            .copied()
            .ok_or_else(|| VersionError::EmptyCurrentBranch(self.current.clone()))?;

        // Borrow snapshots out into owned clones so we can mutate the
        // store afterwards without holding overlapping borrows.
        let ours = self
            .commits
            .get(&current_head)
            .ok_or(VersionError::UnknownCommit(current_head))?
            .snapshot
            .clone();
        let theirs_commit = self
            .commits
            .get(&other_head)
            .ok_or(VersionError::UnknownCommit(other_head))?;
        let theirs = theirs_commit.snapshot.clone();

        // Detect "nothing to merge": other branch is an ancestor of
        // (or equal to) the current head — no new history would be
        // introduced. We treat the equal-head case as nothing-to-merge
        // because the snapshots are byte-identical.
        if current_head == other_head {
            return Err(VersionError::NothingToMerge(other.to_owned()));
        }

        let merged = merge_snapshots(ours, theirs, strategy);

        let message = format!("merge branch '{}' into '{}'", other, self.current);
        let mut commit = Commit::new(
            Some(current_head),
            Some(other_head),
            message,
            "merge".to_owned(),
            Utc::now(),
            merged,
            None,
        );

        // Mix the second parent into the id so distinct merges aren't
        // collapsed by content addressing. `Commit::new` only hashes
        // (parent, message, snapshot); we re-derive here with the
        // second parent appended to the message conceptually, but
        // keep the user-visible `message` unchanged.
        let id = compute_id_with_second_parent(
            commit.parent.as_ref(),
            commit.second_parent.as_ref(),
            &commit.message,
            &commit.snapshot,
        );
        commit.id = id;

        self.commits.insert(id, commit);
        self.branches.insert(self.current.clone(), id);
        Ok(id)
    }

    /// Borrow the commit at the current branch's head, if any.
    pub fn head(&self) -> Option<&Commit> {
        let id = self.branches.get(&self.current)?;
        self.commits.get(id)
    }

    /// Borrow the head commit id of the current branch, if any.
    pub fn head_id(&self) -> Option<CommitId> {
        self.branches.get(&self.current).copied()
    }

    /// Time-travel: borrow the snapshot recorded at `commit_id`.
    pub fn as_of(&self, commit_id: CommitId) -> Option<&Ontology> {
        self.commits.get(&commit_id).map(|c| &c.snapshot)
    }

    /// Borrow a commit by id.
    pub fn commit_at(&self, commit_id: CommitId) -> Option<&Commit> {
        self.commits.get(&commit_id)
    }

    /// Walk the ancestor DAG starting at the current head in
    /// breadth-first order (current head first). Each commit appears
    /// at most once even if reachable via both parents.
    pub fn log(&self) -> Vec<&Commit> {
        let mut out = Vec::new();
        let Some(start) = self.branches.get(&self.current).copied() else {
            return out;
        };
        let mut seen: HashSet<CommitId> = HashSet::new();
        let mut queue: VecDeque<CommitId> = VecDeque::new();
        queue.push_back(start);
        seen.insert(start);
        while let Some(id) = queue.pop_front() {
            let Some(commit) = self.commits.get(&id) else {
                continue;
            };
            out.push(commit);
            if let Some(p) = commit.parent {
                if seen.insert(p) {
                    queue.push_back(p);
                }
            }
            if let Some(p) = commit.second_parent {
                if seen.insert(p) {
                    queue.push_back(p);
                }
            }
        }
        out
    }
}

/// Same hash construction as [`compute_id`] but with the second parent
/// folded in, so two distinct merge commits that happen to share
/// `(parent, message, snapshot)` still get distinct ids.
fn compute_id_with_second_parent(
    parent: Option<&CommitId>,
    second_parent: Option<&CommitId>,
    message: &str,
    snapshot: &Ontology,
) -> CommitId {
    // We delegate the heavy lifting to `compute_id` over a
    // synthetic message that embeds the second parent. This keeps a
    // single canonical hashing routine without exposing a second
    // public hashing function.
    let synthetic = match second_parent {
        Some(p) => format!("{}\0merge-with:{}", message, p),
        None => message.to_owned(),
    };
    compute_id(parent, &synthetic, snapshot)
}

/// Combine two snapshots per [`MergeStrategy`].
///
/// `Union` performs a per-key union of `nodes`, `edges`, and `axioms`;
/// on collision the current ("ours") side wins. Schema, vocabulary,
/// and IRI also fall through to "ours" on conflict because they aren't
/// safely set-mergeable in general.
fn merge_snapshots(mut ours: Ontology, theirs: Ontology, strategy: MergeStrategy) -> Ontology {
    match strategy {
        MergeStrategy::Ours => ours,
        MergeStrategy::Theirs => theirs,
        MergeStrategy::Union => {
            for (id, node) in theirs.nodes {
                ours.nodes.entry(id).or_insert(node);
            }
            for (id, edge) in theirs.edges {
                ours.edges.entry(id).or_insert(edge);
            }
            for (id, axiom) in theirs.axioms {
                ours.axioms.entry(id).or_insert(axiom);
            }
            // Merge schema declarations and vocabulary bindings;
            // collisions keep "ours" so we never silently overwrite a
            // typed declaration with a different shape from the other
            // side.
            for (name, nt) in theirs.schema.node_types {
                ours.schema.node_types.entry(name).or_insert(nt);
            }
            for (name, et) in theirs.schema.edge_types {
                ours.schema.edge_types.entry(name).or_insert(et);
            }
            ours
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{Edge, Iri, Node};

    fn make_ontology_with_org(name: &str) -> Ontology {
        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        let iri = Iri::new(format!("https://example.org/{}", name)).unwrap();
        let _ = o.upsert_node(Node::from_iri(iri, "Organization"));
        o
    }

    #[test]
    fn init_and_first_commit() {
        let mut store = VersionedStore::init();
        assert_eq!(store.current_branch(), "main");
        assert!(store.head().is_none());

        let snap = make_ontology_with_org("Acme");
        let id = store.commit("init".to_owned(), "alice".to_owned(), snap);

        let head = store.head().expect("head after first commit");
        assert_eq!(head.id, id);
        assert_eq!(head.parent, None);
        assert_eq!(head.author, "alice");
        assert_eq!(head.snapshot.node_count(), 1);
    }

    #[test]
    fn second_commit_parents_to_first() {
        let mut store = VersionedStore::init();
        let first = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());
        let snap = make_ontology_with_org("Acme");
        let second = store.commit("add-acme".to_owned(), "alice".to_owned(), snap);
        let head = store.head().unwrap();
        assert_eq!(head.id, second);
        assert_eq!(head.parent, Some(first));
    }

    #[test]
    fn branch_and_checkout() {
        let mut store = VersionedStore::init();
        let _root = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());

        store.branch("feature".to_owned()).unwrap();
        assert!(store.branches.contains_key("feature"));
        assert_eq!(store.current_branch(), "main");

        store.checkout("feature").unwrap();
        assert_eq!(store.current_branch(), "feature");

        // Duplicate branch is rejected.
        let dup = store.branch("feature".to_owned()).unwrap_err();
        assert!(matches!(dup, VersionError::BranchExists(_)));

        // Unknown branch checkout is rejected.
        let bad = store.checkout("nope").unwrap_err();
        assert!(matches!(bad, VersionError::UnknownBranch(_)));
    }

    #[test]
    fn branch_without_commit_is_rejected() {
        let mut store = VersionedStore::init();
        let err = store.branch("feature".to_owned()).unwrap_err();
        assert!(matches!(err, VersionError::EmptyCurrentBranch(_)));
    }

    #[test]
    fn merge_union_combines_nodes_from_both_branches() {
        let mut store = VersionedStore::init();
        // Root commit on main.
        let _root = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());

        // Fork "feature" off main, then diverge.
        store.branch("feature".to_owned()).unwrap();

        // On main, add Acme.
        let main_snap = make_ontology_with_org("Acme");
        let acme_id = *main_snap.nodes.keys().next().unwrap();
        let _ = store.commit("add-acme".to_owned(), "alice".to_owned(), main_snap);

        // Switch to feature, add Globex.
        store.checkout("feature").unwrap();
        let globex_snap = make_ontology_with_org("Globex");
        let globex_id = globex_snap.nodes.keys().next().copied().unwrap();
        let _ = store.commit("add-globex".to_owned(), "bob".to_owned(), globex_snap);

        // Switch back to main and merge feature in.
        store.checkout("main").unwrap();
        let merge_id = store.merge("feature", MergeStrategy::Union).unwrap();

        let merged = store.commit_at(merge_id).unwrap();
        assert!(merged.second_parent.is_some(), "merge commit has second parent");

        let snap = &merged.snapshot;
        assert!(snap.nodes.contains_key(&acme_id), "Acme survives merge");
        assert!(snap.nodes.contains_key(&globex_id), "Globex appears via union");
        assert_eq!(snap.node_count(), 2);
    }

    #[test]
    fn merge_ours_keeps_current() {
        let mut store = VersionedStore::init();
        let _ = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());
        store.branch("feature".to_owned()).unwrap();

        let main_snap = make_ontology_with_org("Acme");
        let _ = store.commit("add-acme".to_owned(), "alice".to_owned(), main_snap);

        store.checkout("feature").unwrap();
        let feat_snap = make_ontology_with_org("Globex");
        let _ = store.commit("add-globex".to_owned(), "bob".to_owned(), feat_snap);

        store.checkout("main").unwrap();
        let merge_id = store.merge("feature", MergeStrategy::Ours).unwrap();
        let merged = store.commit_at(merge_id).unwrap();
        assert_eq!(merged.snapshot.node_count(), 1, "ours discards feature side");
    }

    #[test]
    fn merge_theirs_takes_other() {
        let mut store = VersionedStore::init();
        let _ = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());
        store.branch("feature".to_owned()).unwrap();

        let main_snap = make_ontology_with_org("Acme");
        let _ = store.commit("add-acme".to_owned(), "alice".to_owned(), main_snap);

        store.checkout("feature").unwrap();
        let mut feat_snap = make_ontology_with_org("Globex");
        let _ = feat_snap.upsert_node(Node::from_iri(
            Iri::new("https://example.org/Initech").unwrap(),
            "Organization",
        ));
        let _ = store.commit("add-feat".to_owned(), "bob".to_owned(), feat_snap);

        store.checkout("main").unwrap();
        let merge_id = store.merge("feature", MergeStrategy::Theirs).unwrap();
        let merged = store.commit_at(merge_id).unwrap();
        assert_eq!(merged.snapshot.node_count(), 2, "theirs adopts feature snapshot");
    }

    #[test]
    fn merge_unknown_branch_errors() {
        let mut store = VersionedStore::init();
        let _ = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());
        let err = store.merge("nope", MergeStrategy::Union).unwrap_err();
        assert!(matches!(err, VersionError::UnknownBranch(_)));
    }

    #[test]
    fn merge_self_errors() {
        let mut store = VersionedStore::init();
        let _ = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());
        let err = store.merge("main", MergeStrategy::Union).unwrap_err();
        assert!(matches!(err, VersionError::SelfMerge(_)));
    }

    #[test]
    fn as_of_returns_historical_snapshot() {
        let mut store = VersionedStore::init();
        let snap_a = make_ontology_with_org("Acme");
        let id_a = store.commit("a".to_owned(), "alice".to_owned(), snap_a);

        let mut snap_b = make_ontology_with_org("Acme");
        let _ = snap_b.upsert_edge(Edge::between(
            *snap_b.nodes.keys().next().unwrap(),
            "selfRef",
            *snap_b.nodes.keys().next().unwrap(),
        ));
        let id_b = store.commit("b".to_owned(), "alice".to_owned(), snap_b);

        let hist_a = store.as_of(id_a).expect("snapshot a");
        assert_eq!(hist_a.edge_count(), 0);

        let hist_b = store.as_of(id_b).expect("snapshot b");
        assert_eq!(hist_b.edge_count(), 1);

        // Unknown commit returns None rather than panicking.
        assert!(store.as_of(CommitId::from_bytes([0u8; 32])).is_none());
    }

    #[test]
    fn log_walks_parents_from_current_head() {
        let mut store = VersionedStore::init();
        let a = store.commit("a".to_owned(), "alice".to_owned(), Ontology::new());
        let b = store.commit("b".to_owned(), "alice".to_owned(), Ontology::new());
        let c = store.commit("c".to_owned(), "alice".to_owned(), Ontology::new());

        let log = store.log();
        let ids: Vec<CommitId> = log.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![c, b, a]);
    }

    #[test]
    fn log_after_merge_includes_both_parents() {
        let mut store = VersionedStore::init();
        let root = store.commit("init".to_owned(), "alice".to_owned(), Ontology::new());
        store.branch("feature".to_owned()).unwrap();

        let main_snap = make_ontology_with_org("Acme");
        let main_tip = store.commit("add-acme".to_owned(), "alice".to_owned(), main_snap);

        store.checkout("feature").unwrap();
        let feat_snap = make_ontology_with_org("Globex");
        let feat_tip = store.commit("add-globex".to_owned(), "bob".to_owned(), feat_snap);

        store.checkout("main").unwrap();
        let merge_id = store.merge("feature", MergeStrategy::Union).unwrap();

        let log_ids: HashSet<CommitId> = store.log().into_iter().map(|c| c.id).collect();
        for expected in [root, main_tip, feat_tip, merge_id] {
            assert!(log_ids.contains(&expected), "log missing {}", expected);
        }
    }

    #[test]
    fn commit_id_is_content_addressed() {
        let snap = make_ontology_with_org("Acme");
        let mut s1 = VersionedStore::init();
        let mut s2 = VersionedStore::init();
        let a = s1.commit("init".to_owned(), "alice".to_owned(), snap.clone());
        let b = s2.commit("init".to_owned(), "bob".to_owned(), snap);
        // Different authors don't change the id — only (parent,
        // message, snapshot) feed into it.
        assert_eq!(a, b);
    }
}
