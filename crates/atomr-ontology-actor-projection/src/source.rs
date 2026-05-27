//! SPI for actor-system persistence sources.
//!
//! An [`ActorPersistenceSource`] yields three shapes of data:
//!
//! 1. **Supervision-tree paths** — the static topology of named actors.
//! 2. **Journal events** — a chronological stream of actor lifecycle
//!    transitions and state mutations.
//! 3. **Serialized state** — the latest opaque state blob for an actor.
//!
//! All three are optional in practice; sources whose underlying system
//! does not distinguish them can return empty vectors for the shapes
//! they do not support.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::SourceError;

/// Opaque actor identity within a source.
///
/// Sources are free to use whatever native form is convenient (path
/// string, UUID, integer key). The projector treats `ActorId` as an
/// opaque key for joining paths, events, and state blobs.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActorId(pub String);

impl ActorId {
    /// Wrap a string as an `ActorId`.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S: Into<String>> From<S> for ActorId {
    fn from(value: S) -> Self {
        Self(value.into())
    }
}

/// A supervision-tree path identifying an actor.
///
/// Segments encode the hierarchy from the root downward. The last
/// segment is conventionally the actor's local name; intermediate
/// segments are supervisors (`workflow`, `run`, `step`, ...).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SupervisionPath {
    /// Hierarchical segments, root first.
    pub segments: Vec<String>,
    /// Actor identity at the leaf.
    pub actor: ActorId,
    /// Optional free-form attributes (kind, role, ...).
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl SupervisionPath {
    /// Construct a path from segments and an actor id.
    pub fn new(segments: Vec<String>, actor: impl Into<ActorId>) -> Self {
        Self { segments, actor: actor.into(), attributes: BTreeMap::new() }
    }

    /// Parse a `/a/b/c` slash-separated path. An empty leading segment
    /// (caused by a leading `/`) is stripped.
    pub fn parse(path: &str) -> Self {
        let mut segs: Vec<String> = path.split('/').map(str::to_owned).collect();
        if segs.first().is_some_and(String::is_empty) {
            segs.remove(0);
        }
        let actor = segs.last().cloned().unwrap_or_default();
        Self { segments: segs, actor: ActorId::new(actor), attributes: BTreeMap::new() }
    }

    /// Attach an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Render the path back as a `/`-joined string with a leading
    /// slash.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.segments.iter().map(|s| s.len() + 1).sum());
        for seg in &self.segments {
            out.push('/');
            out.push_str(seg);
        }
        if out.is_empty() {
            out.push('/');
        }
        out
    }

    /// Iterator over `(depth, segment)` pairs from the root.
    pub fn iter_segments(&self) -> impl Iterator<Item = (usize, &str)> {
        self.segments.iter().enumerate().map(|(i, s)| (i, s.as_str()))
    }

    /// Return the parent of this path, if any (the prefix with the
    /// last segment removed).
    pub fn parent(&self) -> Option<SupervisionPath> {
        if self.segments.len() < 2 {
            return None;
        }
        let parent_segments = self.segments[..self.segments.len() - 1].to_vec();
        let parent_actor = parent_segments.last().cloned().unwrap_or_default();
        Some(Self { segments: parent_segments, actor: ActorId::new(parent_actor), attributes: BTreeMap::new() })
    }
}

/// Classification of a journal entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "name")]
pub enum JournalEventKind {
    /// Actor came into existence (spawned/started).
    Created,
    /// Actor's state changed in place.
    StateChanged,
    /// Actor finished a unit of work (step completed).
    Completed,
    /// Actor was terminated / unregistered.
    Terminated,
    /// Any other named event.
    Custom(String),
}

/// A single entry from an actor system's journal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JournalEvent {
    /// Cursor position immediately *after* this event.
    pub cursor: Cursor,
    /// Actor the event pertains to.
    pub actor: ActorId,
    /// Optional supervision path of the actor at event time.
    pub path: Option<SupervisionPath>,
    /// Event classification.
    pub kind: JournalEventKind,
    /// Wall-clock timestamp.
    pub at: DateTime<Utc>,
    /// Event-specific payload.
    pub payload: serde_json::Value,
}

impl JournalEvent {
    /// Convenience constructor for an event at a given cursor / actor /
    /// kind, defaulting `at` to `Utc::now()` and payload to `Null`.
    pub fn new(cursor: Cursor, actor: impl Into<ActorId>, kind: JournalEventKind) -> Self {
        Self {
            cursor,
            actor: actor.into(),
            path: None,
            kind,
            at: Utc::now(),
            payload: serde_json::Value::Null,
        }
    }

    /// Attach a supervision path.
    pub fn with_path(mut self, path: SupervisionPath) -> Self {
        self.path = Some(path);
        self
    }

    /// Attach a payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Override the timestamp.
    pub fn at(mut self, at: DateTime<Utc>) -> Self {
        self.at = at;
        self
    }
}

/// Opaque resumption position within a source's journal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cursor {
    /// Monotonic version counter. Sources without a native counter may
    /// use the event sequence number.
    pub version: u64,
    /// Optional opaque token (e.g. a database row id, an offset).
    pub token: Option<String>,
}

impl Cursor {
    /// Cursor at version 0 with no token. Use this as `since` to ask
    /// for the entire journal.
    pub fn beginning() -> Self {
        Self::default()
    }

    /// Cursor at a specific version.
    pub fn at(version: u64) -> Self {
        Self { version, token: None }
    }
}

/// A latest serialized-state blob for one actor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializedState {
    /// Actor identity.
    pub actor: ActorId,
    /// Opaque payload (typically JSON, but free-form).
    pub payload: serde_json::Value,
    /// Optional content digest.
    pub digest: Option<String>,
}

impl SerializedState {
    /// Construct a state record.
    pub fn new(actor: impl Into<ActorId>, payload: serde_json::Value) -> Self {
        Self { actor: actor.into(), payload, digest: None }
    }

    /// Attach a digest.
    pub fn with_digest(mut self, digest: impl Into<String>) -> Self {
        self.digest = Some(digest.into());
        self
    }
}

/// SPI for an actor-system persistence backend.
///
/// Implementations are `Send + Sync` so they can be shared behind
/// `Arc<dyn ActorPersistenceSource>`.
#[async_trait]
pub trait ActorPersistenceSource: Send + Sync {
    /// Stable, human-readable label (used in tracing spans).
    fn label(&self) -> &str;

    /// Enumerate all known supervision-tree paths.
    async fn paths(&self) -> Result<Vec<SupervisionPath>, SourceError>;

    /// Fetch journal events strictly after `since`.
    async fn journal(&self, since: &Cursor) -> Result<Vec<JournalEvent>, SourceError>;

    /// Fetch the latest serialized state for `actor`, if known.
    async fn state(&self, actor: &ActorId) -> Result<Option<SerializedState>, SourceError>;

    /// Current high-water-mark cursor for the source.
    async fn cursor(&self) -> Result<Cursor, SourceError>;
}

/// Hand-built in-memory [`ActorPersistenceSource`] used in tests and
/// examples.
///
/// The source is mutable; tests can call [`push_path`](Self::push_path),
/// [`push_event`](Self::push_event), and [`put_state`](Self::put_state)
/// to drive the projector with synthetic data.
#[derive(Clone)]
pub struct InMemoryActorPersistenceSource {
    label: String,
    inner: Arc<RwLock<InMemoryState>>,
}

#[derive(Default)]
struct InMemoryState {
    paths: Vec<SupervisionPath>,
    journal: Vec<JournalEvent>,
    states: BTreeMap<ActorId, SerializedState>,
    cursor: Cursor,
}

impl InMemoryActorPersistenceSource {
    /// Empty source with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), inner: Arc::new(RwLock::new(InMemoryState::default())) }
    }

    /// Append a supervision-tree path.
    pub fn push_path(&self, path: SupervisionPath) {
        self.inner.write().paths.push(path);
    }

    /// Append a journal event and bump the cursor.
    pub fn push_event(&self, mut event: JournalEvent) {
        let mut guard = self.inner.write();
        guard.cursor.version = guard.cursor.version.saturating_add(1);
        event.cursor = guard.cursor.clone();
        guard.journal.push(event);
    }

    /// Set the latest serialized state for an actor.
    pub fn put_state(&self, state: SerializedState) {
        self.inner.write().states.insert(state.actor.clone(), state);
    }

    /// Current event count.
    pub fn event_count(&self) -> usize {
        self.inner.read().journal.len()
    }
}

#[async_trait]
impl ActorPersistenceSource for InMemoryActorPersistenceSource {
    fn label(&self) -> &str {
        &self.label
    }

    async fn paths(&self) -> Result<Vec<SupervisionPath>, SourceError> {
        Ok(self.inner.read().paths.clone())
    }

    async fn journal(&self, since: &Cursor) -> Result<Vec<JournalEvent>, SourceError> {
        let guard = self.inner.read();
        Ok(guard.journal.iter().filter(|e| e.cursor.version > since.version).cloned().collect())
    }

    async fn state(&self, actor: &ActorId) -> Result<Option<SerializedState>, SourceError> {
        Ok(self.inner.read().states.get(actor).cloned())
    }

    async fn cursor(&self) -> Result<Cursor, SourceError> {
        Ok(self.inner.read().cursor.clone())
    }
}

#[cfg(feature = "snapshot-source")]
mod snapshot_source;
#[cfg(feature = "snapshot-source")]
pub use snapshot_source::SnapshotActorPersistenceSource;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_render_paths_round_trip() {
        let p = SupervisionPath::parse("/workflow/foo/run/1/step/2");
        assert_eq!(p.segments, vec!["workflow", "foo", "run", "1", "step", "2"]);
        assert_eq!(p.actor.as_str(), "2");
        assert_eq!(p.render(), "/workflow/foo/run/1/step/2");
    }

    #[test]
    fn parent_drops_last_segment() {
        let p = SupervisionPath::parse("/workflow/foo/run/1");
        let parent = p.parent().unwrap();
        assert_eq!(parent.segments, vec!["workflow", "foo", "run"]);
        assert_eq!(parent.actor.as_str(), "run");
    }

    #[test]
    fn root_has_no_parent() {
        let p = SupervisionPath::parse("/root");
        assert!(p.parent().is_none());
    }

    #[tokio::test]
    async fn in_memory_source_round_trip() {
        let src = InMemoryActorPersistenceSource::new("test");
        src.push_path(SupervisionPath::parse("/workflow/foo/run/1"));
        src.push_event(JournalEvent::new(Cursor::beginning(), "1", JournalEventKind::Created));
        src.push_event(JournalEvent::new(Cursor::beginning(), "1", JournalEventKind::Completed));
        src.put_state(SerializedState::new("1", serde_json::json!({"phase": "done"})));

        assert_eq!(src.paths().await.unwrap().len(), 1);
        let events = src.journal(&Cursor::beginning()).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(src.cursor().await.unwrap().version, 2);
        let st = src.state(&ActorId::new("1")).await.unwrap().unwrap();
        assert_eq!(st.actor.as_str(), "1");
    }

    #[tokio::test]
    async fn journal_filter_skips_known_versions() {
        let src = InMemoryActorPersistenceSource::new("test");
        for _ in 0..5 {
            src.push_event(JournalEvent::new(Cursor::beginning(), "x", JournalEventKind::StateChanged));
        }
        let tail = src.journal(&Cursor::at(3)).await.unwrap();
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].cursor.version, 4);
    }
}
