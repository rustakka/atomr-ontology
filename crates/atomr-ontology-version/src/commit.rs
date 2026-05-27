//! Content-addressed commits over [`Ontology`] snapshots.
//!
//! A [`Commit`] is the unit of versioning: it carries a snapshot of the
//! ontology, links to its parent (and a `second_parent` for merge
//! commits), and is keyed by a [`CommitId`] derived from a canonical
//! Blake3 hash of its parent linkage plus the serialized snapshot.
//!
//! Two commits with identical parent, message, and snapshot produce the
//! same [`CommitId`] — content addressing is intentional so that
//! reproducible pipelines yield reproducible histories.

use core::fmt;
use core::str::FromStr;

use atomr_ontology_core::Ontology;
use atomr_ontology_provenance::ProvenanceId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors raised by [`CommitId`] parsing.
#[derive(Debug, Error)]
pub enum CommitIdError {
    /// The string was not valid hex or had the wrong length.
    #[error("invalid commit id: {0}")]
    Invalid(String),
}

/// Content-addressed identifier for a [`Commit`].
///
/// Internally a 32-byte Blake3 digest; rendered as lowercase hex.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommitId(#[serde(with = "serde_bytes_array")] pub [u8; 32]);

impl CommitId {
    /// Wrap raw bytes as a commit id.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying 32 bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Move the 32 bytes out of the wrapper.
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Convenience constructor from a [`blake3::Hash`].
    pub fn from_hash(hash: blake3::Hash) -> Self {
        Self(*hash.as_bytes())
    }
}

impl fmt::Debug for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommitId({})", hex::encode(self.0))
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl FromStr for CommitId {
    type Err = CommitIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = hex::decode(s).map_err(|e| CommitIdError::Invalid(e.to_string()))?;
        if raw.len() != 32 {
            return Err(CommitIdError::Invalid(format!("expected 32 bytes, got {}", raw.len())));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&raw);
        Ok(Self(bytes))
    }
}

impl From<[u8; 32]> for CommitId {
    fn from(b: [u8; 32]) -> Self {
        Self(b)
    }
}

impl From<blake3::Hash> for CommitId {
    fn from(h: blake3::Hash) -> Self {
        Self::from_hash(h)
    }
}

impl AsRef<[u8]> for CommitId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// A single revision in the commit DAG.
///
/// A commit owns its snapshot outright: time-travel queries return a
/// borrow of the snapshot in place rather than reconstructing it from
/// deltas. This keeps the in-memory model simple at the cost of
/// memory; persistent stores can specialize as needed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commit {
    /// Content-addressed id.
    pub id: CommitId,
    /// First parent (None for the root commit on a branch).
    pub parent: Option<CommitId>,
    /// Second parent for merge commits.
    pub second_parent: Option<CommitId>,
    /// Human-readable commit message.
    pub message: String,
    /// Free-form author identifier.
    pub author: String,
    /// When the commit was created.
    pub timestamp: DateTime<Utc>,
    /// Full ontology snapshot at this commit.
    pub snapshot: Ontology,
    /// Optional PROV-O activity that produced this commit.
    pub activity: Option<ProvenanceId>,
}

impl Commit {
    /// Build a commit, computing its content-addressed id from
    /// `(parent, message, snapshot)`.
    ///
    /// `second_parent`, `author`, `timestamp`, and `activity` are
    /// recorded on the commit but intentionally **not** mixed into the
    /// id: the goal is that two commits with the same parent, message,
    /// and snapshot share an id even if produced by different authors
    /// or merge strategies. (Merge commits include their second parent
    /// only via the message convention applied by the store.)
    pub fn new(
        parent: Option<CommitId>,
        second_parent: Option<CommitId>,
        message: String,
        author: String,
        timestamp: DateTime<Utc>,
        snapshot: Ontology,
        activity: Option<ProvenanceId>,
    ) -> Self {
        let id = compute_id(parent.as_ref(), &message, &snapshot);
        Self { id, parent, second_parent, message, author, timestamp, snapshot, activity }
    }
}

/// Compute the canonical [`CommitId`] for a (parent, message, snapshot)
/// tuple.
///
/// The hash input is the concatenation of:
/// 1. the parent's 32 bytes (or 32 zero bytes if absent),
/// 2. the UTF-8 message bytes terminated by a NUL separator,
/// 3. the JSON serialization of the snapshot (BTreeMap-backed, so
///    field order is stable across runs).
///
/// Blake3 is invoked with a domain-separated key to avoid cross-type
/// collisions with the `atomr-ontology-core` id space.
pub fn compute_id(
    parent: Option<&CommitId>,
    message: &str,
    snapshot: &Ontology,
) -> CommitId {
    let mut hasher = blake3::Hasher::new_derive_key("atomr-ontology-version/CommitId/v1");
    match parent {
        Some(p) => hasher.update(p.as_bytes()),
        None => hasher.update(&[0u8; 32]),
    };
    hasher.update(message.as_bytes());
    hasher.update(&[0u8]);
    // BTreeMaps in `Ontology` give a stable iteration order, so JSON
    // serialization is deterministic. We tolerate a serialization
    // failure (impossible for the well-typed fields here) by mixing in
    // a sentinel — the only realistic cause would be a non-finite
    // float, which we still want to address rather than panic on.
    match serde_json::to_vec(snapshot) {
        Ok(bytes) => hasher.update(&bytes),
        Err(_) => hasher.update(b"<unserializable-snapshot>"),
    };
    let out = hasher.finalize();
    CommitId::from_hash(out)
}

/// How to combine two ontology snapshots when merging branches.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Keep the current branch's snapshot; discard the other side.
    Ours,
    /// Adopt the other branch's snapshot; discard the current side.
    Theirs,
    /// Set-union of nodes, edges, and axioms; on key collisions the
    /// current side wins (matching Git's "ours" tie-break, but applied
    /// per-key rather than per-snapshot).
    Union,
}

pub(crate) mod serde_bytes_array {
    use serde::{Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        serde_bytes::Bytes::new(bytes).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let buf: &[u8] = serde_bytes::deserialize(d)?;
        if buf.len() != 32 {
            return Err(serde::de::Error::invalid_length(buf.len(), &"32 bytes"));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(buf);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_id_is_deterministic() {
        let snap = Ontology::new();
        let a = compute_id(None, "init", &snap);
        let b = compute_id(None, "init", &snap);
        assert_eq!(a, b);
    }

    #[test]
    fn commit_id_changes_with_parent() {
        let snap = Ontology::new();
        let root = compute_id(None, "init", &snap);
        let child = compute_id(Some(&root), "init", &snap);
        assert_ne!(root, child);
    }

    #[test]
    fn commit_id_changes_with_message() {
        let snap = Ontology::new();
        let a = compute_id(None, "msg-a", &snap);
        let b = compute_id(None, "msg-b", &snap);
        assert_ne!(a, b);
    }

    #[test]
    fn commit_id_round_trips_through_hex() {
        let id = compute_id(None, "init", &Ontology::new());
        let parsed: CommitId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }
}
