//! Axioms — RDFS / OWL-aligned assertions about classes and properties.

use serde::{Deserialize, Serialize};

use crate::id::ProvenanceId;

pub use crate::id::ProvenanceId as ProvId;

/// Stable identifier for an axiom assertion. Content-addressed over
/// its kind plus operands so equal axioms deduplicate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AxiomId(pub [u8; 32]);

impl AxiomId {
    /// Derive a content-address from the canonical serialization of an axiom kind.
    fn from_canonical(canonical: &str) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key("atomr-ontology-core/AxiomId/v1");
        hasher.update(canonical.as_bytes());
        let out = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(out.as_bytes());
        AxiomId(bytes)
    }
}

impl core::fmt::Display for AxiomId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// The set of axioms we model in v0.1. Mirrors RDFS subclass / OWL
/// property characteristics that the validator can actually check.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AxiomKind {
    /// `A rdfs:subClassOf B`.
    SubClassOf { sub: String, sup: String },
    /// `A owl:equivalentClass B`.
    EquivalentClass { left: String, right: String },
    /// `A owl:disjointWith B`.
    DisjointWith { left: String, right: String },
    /// `P rdfs:domain C`.
    Domain { property: String, class: String },
    /// `P rdfs:range C`.
    Range { property: String, class: String },
    /// `P rdf:type owl:FunctionalProperty`.
    Functional { property: String },
    /// `P rdf:type owl:InverseFunctionalProperty`.
    InverseFunctional { property: String },
    /// `P owl:inverseOf Q`.
    InverseOf { left: String, right: String },
    /// `P rdf:type owl:SymmetricProperty`.
    Symmetric { property: String },
    /// `P rdf:type owl:TransitiveProperty`.
    Transitive { property: String },
}

impl AxiomKind {
    fn canonical(&self) -> String {
        // Stable string encoding — we only need it to seed `AxiomId`,
        // so a minimal `{kind|operands}` form is enough.
        match self {
            AxiomKind::SubClassOf { sub, sup } => format!("SubClassOf|{}|{}", sub, sup),
            AxiomKind::EquivalentClass { left, right } => format!("EquivalentClass|{}|{}", left, right),
            AxiomKind::DisjointWith { left, right } => format!("DisjointWith|{}|{}", left, right),
            AxiomKind::Domain { property, class } => format!("Domain|{}|{}", property, class),
            AxiomKind::Range { property, class } => format!("Range|{}|{}", property, class),
            AxiomKind::Functional { property } => format!("Functional|{}", property),
            AxiomKind::InverseFunctional { property } => format!("InverseFunctional|{}", property),
            AxiomKind::InverseOf { left, right } => format!("InverseOf|{}|{}", left, right),
            AxiomKind::Symmetric { property } => format!("Symmetric|{}", property),
            AxiomKind::Transitive { property } => format!("Transitive|{}", property),
        }
    }
}

/// An axiom annotated with optional provenance.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Axiom {
    /// Stable identity (deterministic from the axiom body).
    pub id: AxiomId,
    /// The kind of axiom.
    pub kind: AxiomKind,
    /// Provenance pointer if the axiom was inferred or asserted by
    /// an identifiable activity.
    pub provenance: Option<ProvenanceId>,
}

impl Axiom {
    /// Build an axiom from its kind; the id is derived deterministically.
    pub fn new(kind: AxiomKind) -> Self {
        let id = AxiomId::from_canonical(&kind.canonical());
        Self { id, kind, provenance: None }
    }

    /// Attach provenance.
    pub fn with_provenance(mut self, prov: ProvenanceId) -> Self {
        self.provenance = Some(prov);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_axioms_share_id() {
        let a = Axiom::new(AxiomKind::SubClassOf {
            sub: "FormalOrganization".into(),
            sup: "Organization".into(),
        });
        let b = Axiom::new(AxiomKind::SubClassOf {
            sub: "FormalOrganization".into(),
            sup: "Organization".into(),
        });
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn different_axioms_differ() {
        let a = Axiom::new(AxiomKind::SubClassOf { sub: "A".into(), sup: "B".into() });
        let b = Axiom::new(AxiomKind::SubClassOf { sub: "B".into(), sup: "A".into() });
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn provenance_round_trip() {
        let p = ProvenanceId::new_random();
        let a = Axiom::new(AxiomKind::Functional { property: "homepage".into() }).with_provenance(p);
        assert_eq!(a.provenance, Some(p));
    }
}
