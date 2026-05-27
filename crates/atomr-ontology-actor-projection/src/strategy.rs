//! Pure-functional strategy enums applied during projection.
//!
//! These mirror the [`CachePolicy`-style](https://docs.rs/atomr-ontology-extract)
//! enum-of-strategies pattern used elsewhere in the workspace: a small
//! enum captures the user's choice, and a method on the enum does the
//! work.

use std::collections::BTreeMap;

use atomr_ontology_core::{Iri, IriError, Node, NodeId, PropertyValue, Schema};

use crate::source::{JournalEvent, JournalEventKind, SupervisionPath};

/// How a projection should mint IRIs for actor and event nodes.
#[derive(Clone, Debug)]
pub enum IriMintingStrategy {
    /// Append the supervision path under a base IRI (`{base}{path}`).
    /// Deterministic; identical paths yield identical IRIs.
    PathBased {
        /// Base IRI (with trailing slash recommended).
        base: Iri,
    },
    /// Hash the path's canonical form with Blake3 to derive the node
    /// id, leaving IRI unset on the node.
    ContentAddressed,
    /// Random UUID-derived id, no IRI.
    Uuid,
}

impl IriMintingStrategy {
    /// Mint a node id + optional IRI for an actor identified by `path`.
    pub fn mint_actor(&self, path: &SupervisionPath) -> Result<(NodeId, Option<Iri>), IriError> {
        match self {
            IriMintingStrategy::PathBased { base } => {
                let rendered = format!("{}{}", trim_trailing_slash(base.as_str()), path.render());
                let iri = Iri::new(rendered)?;
                let id = NodeId::content_address(iri.as_str().as_bytes());
                Ok((id, Some(iri)))
            }
            IriMintingStrategy::ContentAddressed => {
                let id = NodeId::content_address(path.render().as_bytes());
                Ok((id, None))
            }
            IriMintingStrategy::Uuid => Ok((NodeId::new_random(), None)),
        }
    }

    /// Mint a node id + optional IRI for a journal event.
    pub fn mint_event(&self, event: &JournalEvent) -> Result<(NodeId, Option<Iri>), IriError> {
        match self {
            IriMintingStrategy::PathBased { base } => {
                let suffix = format!(
                    "/event/{}/{}",
                    event.cursor.version,
                    event.actor.as_str()
                );
                let rendered = format!("{}{}", trim_trailing_slash(base.as_str()), suffix);
                let iri = Iri::new(rendered)?;
                let id = NodeId::content_address(iri.as_str().as_bytes());
                Ok((id, Some(iri)))
            }
            IriMintingStrategy::ContentAddressed => {
                let key = format!(
                    "event:{}:{}:{}",
                    event.cursor.version,
                    event.actor.as_str(),
                    journal_event_kind_str(&event.kind)
                );
                let id = NodeId::content_address(key.as_bytes());
                Ok((id, None))
            }
            IriMintingStrategy::Uuid => Ok((NodeId::new_random(), None)),
        }
    }
}

fn trim_trailing_slash(s: &str) -> &str {
    s.strip_suffix('/').unwrap_or(s)
}

/// Human-readable name of a journal-event kind, used by event IRIs and
/// node properties.
pub fn journal_event_kind_str(kind: &JournalEventKind) -> &str {
    match kind {
        JournalEventKind::Created => "created",
        JournalEventKind::StateChanged => "state_changed",
        JournalEventKind::Completed => "completed",
        JournalEventKind::Terminated => "terminated",
        JournalEventKind::Custom(name) => name.as_str(),
    }
}

/// How an existing node should react to a new write at the same id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictResolution {
    /// New node fully replaces the old one. Default.
    LastWriteWins,
    /// Union of existing + new properties (new wins per key).
    Merge,
    /// No-op if a node with that id already exists.
    SkipExisting,
}

impl Default for ConflictResolution {
    fn default() -> Self {
        ConflictResolution::LastWriteWins
    }
}

impl ConflictResolution {
    /// Combine `incoming` against `existing` according to this policy.
    /// Returns `None` if the policy decides to skip the write entirely
    /// (`SkipExisting` when `existing` is `Some`).
    pub fn reconcile(&self, existing: Option<&Node>, incoming: Node) -> Option<Node> {
        match self {
            ConflictResolution::LastWriteWins => Some(incoming),
            ConflictResolution::SkipExisting => {
                if existing.is_some() {
                    None
                } else {
                    Some(incoming)
                }
            }
            ConflictResolution::Merge => match existing {
                None => Some(incoming),
                Some(prev) => {
                    let mut merged = prev.clone();
                    // Union labels (preserve order from existing, append unseen).
                    let seen: std::collections::BTreeSet<String> =
                        merged.types.iter().cloned().collect();
                    for ty in incoming.types {
                        if !seen.contains(&ty) {
                            merged.types.push(ty);
                        }
                    }
                    // Properties: incoming wins per-key.
                    let merged_properties: BTreeMap<String, PropertyValue> = merged
                        .properties
                        .into_iter()
                        .chain(incoming.properties)
                        .collect();
                    merged.properties = merged_properties;
                    if incoming.iri.is_some() {
                        merged.iri = incoming.iri;
                    }
                    Some(merged)
                }
            },
        }
    }
}

/// How the projector treats the destination schema.
#[derive(Clone, Debug)]
pub enum SchemaStrategy {
    /// Validate every node/edge against the supplied schema; reject if
    /// the type isn't declared.
    FixedSchema(Schema),
    /// Allow the destination store's schema to grow as new types are
    /// observed. Equivalent to "no validation."
    InducedSchema,
    /// Start from the supplied schema as a baseline but allow new
    /// types to be added at projection time.
    Hybrid(Schema),
}

impl Default for SchemaStrategy {
    fn default() -> Self {
        SchemaStrategy::InducedSchema
    }
}

impl SchemaStrategy {
    /// Return the baseline schema, if any.
    pub fn baseline(&self) -> Option<&Schema> {
        match self {
            SchemaStrategy::FixedSchema(s) | SchemaStrategy::Hybrid(s) => Some(s),
            SchemaStrategy::InducedSchema => None,
        }
    }

    /// `true` when unknown types must be rejected.
    pub fn is_strict(&self) -> bool {
        matches!(self, SchemaStrategy::FixedSchema(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ActorId;

    #[test]
    fn path_based_minting_deterministic() {
        let s = IriMintingStrategy::PathBased { base: Iri::new("https://atomr.dev/actor").unwrap() };
        let p = SupervisionPath::parse("/workflow/foo/run/1");
        let (a, _) = s.mint_actor(&p).unwrap();
        let (b, _) = s.mint_actor(&p).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn path_based_vs_content_addressed_differ() {
        let p = SupervisionPath::parse("/workflow/foo");
        let a = IriMintingStrategy::PathBased { base: Iri::new("https://atomr.dev/actor/").unwrap() }
            .mint_actor(&p)
            .unwrap()
            .0;
        let b = IriMintingStrategy::ContentAddressed.mint_actor(&p).unwrap().0;
        assert_ne!(a, b);
    }

    #[test]
    fn uuid_minting_unique_each_call() {
        let p = SupervisionPath::parse("/a");
        let a = IriMintingStrategy::Uuid.mint_actor(&p).unwrap().0;
        let b = IriMintingStrategy::Uuid.mint_actor(&p).unwrap().0;
        assert_ne!(a, b);
    }

    #[test]
    fn merge_combines_properties() {
        let existing = Node::new("Actor")
            .with_label("Workflow")
            .with_property("name", "old");
        let incoming = Node {
            id: existing.id,
            iri: None,
            types: vec!["Actor".into(), "Step".into()],
            properties: [("name".to_string(), PropertyValue::string("new"))].into_iter().collect(),
        };
        let merged = ConflictResolution::Merge.reconcile(Some(&existing), incoming).unwrap();
        assert!(merged.types.contains(&"Workflow".to_string()));
        assert!(merged.types.contains(&"Step".to_string()));
        assert_eq!(merged.property("name"), Some(&PropertyValue::String("new".into())));
    }

    #[test]
    fn skip_existing_no_ops_when_present() {
        let existing = Node::new("Actor");
        let incoming = Node { id: existing.id, ..Node::new("Actor") };
        assert!(ConflictResolution::SkipExisting.reconcile(Some(&existing), incoming).is_none());
    }

    #[test]
    fn skip_existing_writes_when_absent() {
        let incoming = Node::new("Actor");
        assert!(ConflictResolution::SkipExisting.reconcile(None, incoming).is_some());
    }

    #[test]
    fn event_minting_consistent() {
        let s = IriMintingStrategy::PathBased { base: Iri::new("https://atomr.dev/actor/").unwrap() };
        let ev = JournalEvent::new(
            crate::source::Cursor::at(7),
            ActorId::new("alpha"),
            JournalEventKind::Created,
        );
        let (a, _) = s.mint_event(&ev).unwrap();
        let (b, _) = s.mint_event(&ev).unwrap();
        assert_eq!(a, b);
    }
}
