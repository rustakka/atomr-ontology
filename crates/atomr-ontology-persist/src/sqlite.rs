//! [`SqliteCheckpointer`] — append-only SQLite-backed persistence.
//!
//! Snapshots are written as JSON blobs into a `snapshots(id, json,
//! version)` table. `save` inserts a new row; `load` returns the row
//! with the highest `version`. We do not pool connections — each call
//! opens a fresh `rusqlite::Connection` inside
//! `tokio::task::spawn_blocking`. SQLite is plenty fast for the
//! cadence we expect here (commit-grained snapshots), and skipping
//! `r2d2` keeps the dependency surface minimal.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rusqlite::{params, Connection};
use tokio::task;

use crate::checkpointer::{Checkpointer, CheckpointerError, Snapshot};

/// SQLite-backed [`Checkpointer`].
#[derive(Clone, Debug)]
pub struct SqliteCheckpointer {
    path: PathBuf,
    label: String,
}

impl SqliteCheckpointer {
    /// Create a checkpointer that reads/writes the SQLite database at
    /// `path`. The database file and schema are created lazily on the
    /// first `save` or `load` call.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let label = format!("sqlite:{}", path.display());
        Self { path, label }
    }

    /// Borrow the destination path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Open the SQLite handle and ensure the schema exists.
    fn open_and_init(path: &Path) -> Result<Connection, CheckpointerError> {
        // Parent directory might not exist on first use; create it so
        // `Connection::open` doesn't fail with ErrorCode::CannotOpen.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CheckpointerError::Io(e.to_string()))?;
            }
        }
        let conn = Connection::open(path).map_err(|e| CheckpointerError::Io(e.to_string()))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots ( \
                id INTEGER PRIMARY KEY, \
                json TEXT NOT NULL, \
                version INTEGER NOT NULL \
            )",
            [],
        )
        .map_err(|e| CheckpointerError::Other(e.to_string()))?;
        Ok(conn)
    }
}

#[async_trait]
impl Checkpointer for SqliteCheckpointer {
    async fn save(&self, snapshot: Snapshot) -> Result<(), CheckpointerError> {
        let path = self.path.clone();
        let json = serde_json::to_string(&snapshot)
            .map_err(|e| CheckpointerError::Serialize(e.to_string()))?;
        let version = snapshot.version as i64;
        task::spawn_blocking(move || {
            let conn = Self::open_and_init(&path)?;
            conn.execute(
                "INSERT INTO snapshots (json, version) VALUES (?1, ?2)",
                params![json, version],
            )
            .map_err(|e| CheckpointerError::Other(e.to_string()))?;
            Ok::<(), CheckpointerError>(())
        })
        .await
        .map_err(|e| CheckpointerError::Other(format!("join error: {e}")))??;
        Ok(())
    }

    async fn load(&self) -> Result<Option<Snapshot>, CheckpointerError> {
        let path = self.path.clone();
        let json: Option<String> = task::spawn_blocking(move || {
            let conn = Self::open_and_init(&path)?;
            // Order by version (primary key) then id (insertion order)
            // so a re-saved snapshot at the same version still returns
            // the most recent row.
            let mut stmt = conn
                .prepare(
                    "SELECT json FROM snapshots ORDER BY version DESC, id DESC LIMIT 1",
                )
                .map_err(|e| CheckpointerError::Other(e.to_string()))?;
            let mut rows = stmt
                .query([])
                .map_err(|e| CheckpointerError::Other(e.to_string()))?;
            match rows.next().map_err(|e| CheckpointerError::Other(e.to_string()))? {
                Some(row) => {
                    let s: String = row
                        .get(0)
                        .map_err(|e| CheckpointerError::Other(e.to_string()))?;
                    Ok::<Option<String>, CheckpointerError>(Some(s))
                }
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| CheckpointerError::Other(format!("join error: {e}")))??;

        match json {
            Some(s) => {
                let snap: Snapshot = serde_json::from_str(&s)
                    .map_err(|e| CheckpointerError::Serialize(e.to_string()))?;
                Ok(Some(snap))
            }
            None => Ok(None),
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
    async fn empty_db_loads_none() {
        let dir = tempdir().unwrap();
        let cp = SqliteCheckpointer::new(dir.path().join("snap.sqlite"));
        assert!(cp.load().await.unwrap().is_none());
        assert!(cp.label().starts_with("sqlite:"));
    }

    #[tokio::test]
    async fn sqlite_round_trip() {
        let dir = tempdir().unwrap();
        let cp = SqliteCheckpointer::new(dir.path().join("snap.sqlite"));

        let mut o = Ontology::new();
        o.declare_node_type("Organization");
        o.upsert_node(Node::new("Organization").with_property("name", "Acme"));
        let snap = Snapshot::new(o, ProvenanceLog::new(), 9);
        cp.save(snap).await.unwrap();

        let loaded = cp.load().await.unwrap().unwrap();
        assert_eq!(loaded.version, 9);
        assert_eq!(loaded.ontology.node_count(), 1);
    }

    #[tokio::test]
    async fn highest_version_wins() {
        let dir = tempdir().unwrap();
        let cp = SqliteCheckpointer::new(dir.path().join("snap.sqlite"));
        cp.save(Snapshot::new(Ontology::new(), ProvenanceLog::new(), 1))
            .await
            .unwrap();
        cp.save(Snapshot::new(Ontology::new(), ProvenanceLog::new(), 5))
            .await
            .unwrap();
        cp.save(Snapshot::new(Ontology::new(), ProvenanceLog::new(), 3))
            .await
            .unwrap();
        let loaded = cp.load().await.unwrap().unwrap();
        assert_eq!(loaded.version, 5);
    }

    #[tokio::test]
    async fn persistent_store_with_sqlite_checkpointer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ontology.sqlite");

        let cp = SqliteCheckpointer::new(&path);
        let store = PersistentStore::new(cp).await.unwrap();
        let delta = OntologyDelta::new()
            .with_node(Node::new("Organization").with_property("name", "Acme"));
        let pid = store
            .commit_with_provenance(delta, Activity::started("seed"))
            .await
            .unwrap();
        assert_eq!(store.version(), 1);

        let cp2 = SqliteCheckpointer::new(&path);
        let store2 = PersistentStore::new(cp2).await.unwrap();
        assert_eq!(store2.version(), 1);
        assert_eq!(store2.snapshot().await.unwrap().node_count(), 1);
        let log = store2.provenance().await.unwrap();
        assert!(log.activities.contains_key(&pid));
    }
}
