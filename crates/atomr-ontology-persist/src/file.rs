//! [`FileCheckpointer`] — JSON-on-disk persistence.
//!
//! Each `save` rewrites the configured file with a pretty-printed
//! JSON-encoded [`Snapshot`]. `load` reads it back. This is a simple
//! single-process checkpointer suitable for desktop pipelines, demos,
//! and tests that need real I/O. Concurrent writers across processes
//! are not supported.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use crate::checkpointer::{Checkpointer, CheckpointerError, Snapshot};

/// JSON-file [`Checkpointer`].
#[derive(Clone, Debug)]
pub struct FileCheckpointer {
    /// Destination file. Parent directories must already exist (or
    /// the first `save` will create the file but fail if a directory
    /// in the path is missing — `tokio::fs::write` does not recurse).
    path: PathBuf,
    label: String,
}

impl FileCheckpointer {
    /// Create a checkpointer that reads from and writes to `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let label = format!("file:{}", path.display());
        Self { path, label }
    }

    /// Borrow the destination path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl Checkpointer for FileCheckpointer {
    async fn save(&self, snapshot: Snapshot) -> Result<(), CheckpointerError> {
        let bytes = serde_json::to_vec_pretty(&snapshot)
            .map_err(|e| CheckpointerError::Serialize(e.to_string()))?;
        // Ensure the parent directory exists so callers can point at
        // a fresh subdirectory without having to mkdir-p themselves.
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| CheckpointerError::Io(e.to_string()))?;
            }
        }
        fs::write(&self.path, bytes)
            .await
            .map_err(|e| CheckpointerError::Io(e.to_string()))?;
        Ok(())
    }

    async fn load(&self) -> Result<Option<Snapshot>, CheckpointerError> {
        match fs::read(&self.path).await {
            Ok(bytes) => {
                let snap: Snapshot = serde_json::from_slice(&bytes)
                    .map_err(|e| CheckpointerError::Serialize(e.to_string()))?;
                Ok(Some(snap))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CheckpointerError::Io(e.to_string())),
        }
    }

    fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpointer::Snapshot;
    use crate::store::PersistentStore;
    use atomr_ontology_core::{Node, Ontology};
    use atomr_ontology_provenance::{Activity, ProvenanceLog};
    use atomr_ontology_store::r#trait::{OntologyDelta, OntologyStore};
    use tempfile::tempdir;

    #[tokio::test]
    async fn missing_file_loads_as_none() {
        let dir = tempdir().unwrap();
        let cp = FileCheckpointer::new(dir.path().join("snap.json"));
        assert!(cp.load().await.unwrap().is_none());
        assert!(cp.label().starts_with("file:"));
    }

    #[tokio::test]
    async fn file_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snap.json");
        let cp = FileCheckpointer::new(&path);

        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        o.upsert_node(Node::new("Organization").with_property("name", "Acme"));
        let snap = Snapshot::new(o, ProvenanceLog::new(), 42);
        cp.save(snap).await.unwrap();

        // File should now exist and be valid JSON.
        let raw = std::fs::read_to_string(&path).unwrap();
        let _v: serde_json::Value = serde_json::from_str(&raw).unwrap();

        let loaded = cp.load().await.unwrap().unwrap();
        assert_eq!(loaded.version, 42);
        assert_eq!(loaded.ontology.node_count(), 1);
    }

    #[tokio::test]
    async fn creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/dir/snap.json");
        let cp = FileCheckpointer::new(&path);
        cp.save(Snapshot::new(Ontology::new(), ProvenanceLog::new(), 1))
            .await
            .unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn persistent_store_with_file_checkpointer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ontology.json");

        let cp = FileCheckpointer::new(&path);
        let store = PersistentStore::new(cp).await.unwrap();
        let delta = OntologyDelta::new()
            .with_node(Node::new("Organization").with_property("name", "Acme"));
        let pid = store
            .commit_with_provenance(delta, Activity::started("seed"))
            .await
            .unwrap();
        assert_eq!(store.version(), 1);
        assert!(path.exists());

        // Re-open via a fresh checkpointer + store.
        let cp2 = FileCheckpointer::new(&path);
        let store2 = PersistentStore::new(cp2).await.unwrap();
        assert_eq!(store2.version(), 1);
        assert_eq!(store2.snapshot().await.unwrap().node_count(), 1);
        let log = store2.provenance().await.unwrap();
        assert!(log.activities.contains_key(&pid));
    }
}
