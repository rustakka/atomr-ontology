//! Private JSON-friendly wire representation for [`Snapshot`](crate::Snapshot).
//!
//! The shared crates use `serde_bytes`-backed 32-byte ID newtypes for
//! [`NodeId`](atomr_ontology_core::NodeId), [`EdgeId`], and
//! [`ProvenanceId`]. Those wire shapes do not JSON-round-trip cleanly
//! (the deserializer expects a borrowed `&[u8]`, which JSON cannot
//! produce). On top of that, `Ontology` and `ProvenanceLog` both keep
//! their contents in `BTreeMap<Id, _>`, and JSON disallows non-string
//! map keys.
//!
//! This module defines a sibling set of wire types whose ids are
//! plain lower-case hex strings and whose collections are vectors.
//! Round-tripping a [`Snapshot`](crate::Snapshot) through
//! `SnapshotWire::from(&snap)` → JSON → `SnapshotWire::into_snapshot`
//! reproduces every id and structural relationship in the original.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use atomr_ontology_core::{
    Axiom, AxiomId, AxiomKind, Edge, EdgeId, Iri, Node, NodeId, Ontology, PropertyValue,
    ProvenanceId, Schema, Vocabulary,
};
use atomr_ontology_provenance::{
    Activity, AgentRef, ProvEntity, ProvenanceLog, Used, WasAttributedTo, WasDerivedFrom,
    WasGeneratedBy,
};

use crate::checkpointer::Snapshot;

/// Encode any 32-byte id as a lower-case hex string.
fn encode_id<I: AsRef<[u8]>>(id: I) -> String {
    hex::encode(id.as_ref())
}

/// Decode a hex string into a `[u8; 32]`. Used to rebuild ids.
fn decode_id(s: &str) -> Result<[u8; 32], String> {
    let raw = hex::decode(s).map_err(|e| format!("invalid hex id: {e}"))?;
    if raw.len() != 32 {
        return Err(format!("expected 32-byte id, got {} bytes", raw.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SnapshotWire {
    ontology: OntologyWire,
    provenance: ProvenanceLogWire,
    version: u64,
}

impl SnapshotWire {
    pub(crate) fn into_snapshot(self) -> Result<Snapshot, String> {
        Ok(Snapshot {
            ontology: self.ontology.into_ontology()?,
            provenance: self.provenance.into_log()?,
            version: self.version,
        })
    }
}

impl From<&Snapshot> for SnapshotWire {
    fn from(s: &Snapshot) -> Self {
        Self {
            ontology: OntologyWire::from(&s.ontology),
            provenance: ProvenanceLogWire::from(&s.provenance),
            version: s.version,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct OntologyWire {
    iri: Option<Iri>,
    /// `Vocabulary` is `BTreeMap<String, Iri>` and is JSON-safe as-is.
    vocabulary: Vocabulary,
    schema: Schema,
    nodes: Vec<NodeWire>,
    edges: Vec<EdgeWire>,
    axioms: Vec<AxiomWire>,
}

impl From<&Ontology> for OntologyWire {
    fn from(o: &Ontology) -> Self {
        Self {
            iri: o.iri.clone(),
            vocabulary: o.vocabulary.clone(),
            schema: o.schema.clone(),
            nodes: o.nodes.values().map(NodeWire::from).collect(),
            edges: o.edges.values().map(EdgeWire::from).collect(),
            axioms: o.axioms.values().map(AxiomWire::from).collect(),
        }
    }
}

impl OntologyWire {
    fn into_ontology(self) -> Result<Ontology, String> {
        let mut o = Ontology {
            iri: self.iri,
            vocabulary: self.vocabulary,
            schema: self.schema,
            ..Ontology::default()
        };
        for n in self.nodes {
            o.upsert_node(n.into_node()?);
        }
        for e in self.edges {
            o.upsert_edge(e.into_edge()?);
        }
        for a in self.axioms {
            o.upsert_axiom(a.into_axiom()?);
        }
        Ok(o)
    }
}

#[derive(Serialize, Deserialize)]
struct NodeWire {
    id: String,
    iri: Option<Iri>,
    types: Vec<String>,
    properties: BTreeMap<String, PropertyValue>,
}

impl NodeWire {
    fn from(n: &Node) -> Self {
        Self {
            id: encode_id(n.id.as_bytes()),
            iri: n.iri.clone(),
            types: n.types.clone(),
            properties: n.properties.clone(),
        }
    }

    fn into_node(self) -> Result<Node, String> {
        Ok(Node {
            id: NodeId::from_bytes(decode_id(&self.id)?),
            iri: self.iri,
            types: self.types,
            properties: self.properties,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct EdgeWire {
    id: String,
    label: String,
    source: String,
    target: String,
    properties: BTreeMap<String, PropertyValue>,
}

impl EdgeWire {
    fn from(e: &Edge) -> Self {
        Self {
            id: encode_id(e.id.as_bytes()),
            label: e.label.clone(),
            source: encode_id(e.source.as_bytes()),
            target: encode_id(e.target.as_bytes()),
            properties: e.properties.clone(),
        }
    }

    fn into_edge(self) -> Result<Edge, String> {
        Ok(Edge {
            id: EdgeId::from_bytes(decode_id(&self.id)?),
            label: self.label,
            source: NodeId::from_bytes(decode_id(&self.source)?),
            target: NodeId::from_bytes(decode_id(&self.target)?),
            properties: self.properties,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct AxiomWire {
    // AxiomId is `#[serde(transparent)]` over a plain `[u8; 32]`
    // (no `serde_bytes`), so it JSON-round-trips natively.
    id: AxiomId,
    kind: AxiomKind,
    /// Optional hex-encoded provenance pointer.
    provenance: Option<String>,
}

impl AxiomWire {
    fn from(a: &Axiom) -> Self {
        Self {
            id: a.id,
            kind: a.kind.clone(),
            provenance: a.provenance.map(|p| encode_id(p.as_bytes())),
        }
    }

    fn into_axiom(self) -> Result<Axiom, String> {
        let provenance = match self.provenance {
            Some(s) => Some(ProvenanceId::from_bytes(decode_id(&s)?)),
            None => None,
        };
        Ok(Axiom { id: self.id, kind: self.kind, provenance })
    }
}

#[derive(Serialize, Deserialize)]
struct ProvenanceLogWire {
    activities: Vec<ActivityWire>,
    entities: Vec<ProvEntityWire>,
    generations: Vec<WasGeneratedByWire>,
    derivations: Vec<WasDerivedFromWire>,
    attributions: Vec<WasAttributedTo>,
    uses: Vec<UsedWire>,
}

impl From<&ProvenanceLog> for ProvenanceLogWire {
    fn from(p: &ProvenanceLog) -> Self {
        Self {
            activities: p.activities.values().map(ActivityWire::from).collect(),
            entities: p.entities.values().map(ProvEntityWire::from).collect(),
            generations: p.generations.iter().map(WasGeneratedByWire::from).collect(),
            derivations: p.derivations.iter().map(WasDerivedFromWire::from).collect(),
            attributions: p.attributions.clone(),
            uses: p.uses.iter().map(UsedWire::from).collect(),
        }
    }
}

impl ProvenanceLogWire {
    fn into_log(self) -> Result<ProvenanceLog, String> {
        let mut log = ProvenanceLog::new();
        for a in self.activities {
            log.record_activity(a.into_activity()?);
        }
        for e in self.entities {
            log.record_entity(e.into_entity()?);
        }
        for g in self.generations {
            log.generated(
                ProvenanceId::from_bytes(decode_id(&g.entity)?),
                ProvenanceId::from_bytes(decode_id(&g.activity)?),
            );
        }
        for d in self.derivations {
            log.derived(
                ProvenanceId::from_bytes(decode_id(&d.derived)?),
                ProvenanceId::from_bytes(decode_id(&d.source)?),
            );
        }
        for u in self.uses {
            log.used(
                ProvenanceId::from_bytes(decode_id(&u.activity)?),
                ProvenanceId::from_bytes(decode_id(&u.entity)?),
            );
        }
        log.attributions = self.attributions;
        Ok(log)
    }
}

#[derive(Serialize, Deserialize)]
struct ActivityWire {
    id: String,
    label: String,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
    agent: Option<AgentRef>,
    attributes: BTreeMap<String, serde_json::Value>,
}

impl ActivityWire {
    fn from(a: &Activity) -> Self {
        Self {
            id: encode_id(a.id.as_bytes()),
            label: a.label.clone(),
            started_at: a.started_at,
            ended_at: a.ended_at,
            agent: a.agent.clone(),
            attributes: a.attributes.clone(),
        }
    }

    fn into_activity(self) -> Result<Activity, String> {
        Ok(Activity {
            id: ProvenanceId::from_bytes(decode_id(&self.id)?),
            label: self.label,
            started_at: self.started_at,
            ended_at: self.ended_at,
            agent: self.agent,
            attributes: self.attributes,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ProvEntityWire {
    id: String,
    label: String,
    digest: Option<String>,
    attributes: BTreeMap<String, serde_json::Value>,
}

impl ProvEntityWire {
    fn from(e: &ProvEntity) -> Self {
        Self {
            id: encode_id(e.id.as_bytes()),
            label: e.label.clone(),
            digest: e.digest.clone(),
            attributes: e.attributes.clone(),
        }
    }

    fn into_entity(self) -> Result<ProvEntity, String> {
        Ok(ProvEntity {
            id: ProvenanceId::from_bytes(decode_id(&self.id)?),
            label: self.label,
            digest: self.digest,
            attributes: self.attributes,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct WasGeneratedByWire {
    entity: String,
    activity: String,
}

impl WasGeneratedByWire {
    fn from(g: &WasGeneratedBy) -> Self {
        Self { entity: encode_id(g.entity.as_bytes()), activity: encode_id(g.activity.as_bytes()) }
    }
}

#[derive(Serialize, Deserialize)]
struct WasDerivedFromWire {
    derived: String,
    source: String,
}

impl WasDerivedFromWire {
    fn from(d: &WasDerivedFrom) -> Self {
        Self { derived: encode_id(d.derived.as_bytes()), source: encode_id(d.source.as_bytes()) }
    }
}

#[derive(Serialize, Deserialize)]
struct UsedWire {
    activity: String,
    entity: String,
}

impl UsedWire {
    fn from(u: &Used) -> Self {
        Self { activity: encode_id(u.activity.as_bytes()), entity: encode_id(u.entity.as_bytes()) }
    }
}

