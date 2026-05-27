//! Shared projection helper: walk a triple stream and populate an
//! [`Ontology`] using a per-vocabulary [`Mapping`] of known classes,
//! datatype properties, and object properties.
//!
//! Each importer (SKOS, FOAF, schema.org) declares a `Mapping`
//! tailored to its vocabulary; [`project`] then does the generic work
//! of:
//!
//! 1. declaring the [`NodeType`]s, [`EdgeType`]s, and
//!    [`PropertyType`]s up-front (so the T-Box is populated even when
//!    no individuals exist);
//! 2. walking the triple stream to materialize one [`Node`] per
//!    subject IRI carrying a recognized class, attaching recognized
//!    datatype-property values, and emitting [`Edge`]s for recognized
//!    object properties.
//!
//! Triples that don't match the vocabulary are silently skipped — by
//! design, since SKOS / FOAF / schema.org sources routinely include
//! `rdfs:label`, `dct:creator`, and other adjacencies that have no
//! place in the LPG projection here.

use std::collections::BTreeMap;

use atomr_ontology_core::schema::{Cardinality, EdgeType, NodeType, PropertyType};
use atomr_ontology_core::{Datatype, Edge, Iri, Node, NodeId, Ontology, PropertyValue};
use atomr_ontology_rdf::{Object, Subject, Triple};

/// IRI of `rdf:type`.
pub(crate) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Declaration of a recognized datatype property in the source vocabulary.
#[derive(Clone, Debug)]
pub(crate) struct DataPropertySpec {
    /// Predicate IRI used in the source.
    pub iri: &'static str,
    /// Local name used as the LPG property key.
    pub local: &'static str,
    /// XSD-aligned datatype.
    pub datatype: Datatype,
}

/// Declaration of a recognized object property in the source vocabulary.
#[derive(Clone, Debug)]
pub(crate) struct ObjectPropertySpec {
    /// Predicate IRI used in the source.
    pub iri: &'static str,
    /// LPG edge-type label.
    pub local: &'static str,
}

/// Declaration of a recognized class in the source vocabulary.
#[derive(Clone, Debug)]
pub(crate) struct ClassSpec {
    /// Class IRI used in the source.
    pub iri: &'static str,
    /// LPG node-type label.
    pub local: &'static str,
}

/// Per-vocabulary mapping driven by [`project`].
#[derive(Clone, Debug)]
pub(crate) struct Mapping {
    /// Recognized classes (`rdf:type` targets).
    pub classes: &'static [ClassSpec],
    /// Recognized datatype properties; each property is attached to
    /// every node type listed in `applies_to`.
    pub data_properties: &'static [(DataPropertySpec, &'static [&'static str])],
    /// Recognized object properties; each edge type is declared with
    /// the listed domain and range.
    pub object_properties: &'static [(ObjectPropertySpec, &'static [&'static str], &'static [&'static str])],
    /// Default node-type label used for subjects that appear as an
    /// edge target but never carry an `rdf:type`. None disables this
    /// fallback (in which case those subjects are skipped).
    pub default_object_class: Option<&'static str>,
}

/// Declare the T-Box ([`NodeType`]s, [`EdgeType`]s, [`PropertyType`]s)
/// for the given mapping. Returns an [`Ontology`] with an empty A-Box.
pub(crate) fn declare(m: &Mapping) -> Ontology {
    let mut o = Ontology::new();

    // Group declared properties by their owning node type so each
    // class lands with the union of its data properties.
    let mut props_by_class: BTreeMap<&str, Vec<PropertyType>> = BTreeMap::new();
    for (spec, applies_to) in m.data_properties {
        let pt = PropertyType {
            name: spec.local.to_string(),
            datatype: spec.datatype,
            cardinality: Cardinality::ANY,
            iri: Some(Iri::from_unchecked(spec.iri.to_string())),
            description: None,
        };
        for class in *applies_to {
            props_by_class.entry(class).or_default().push(pt.clone());
        }
    }

    for class in m.classes {
        let mut nt = NodeType::new(class.local).with_iri(Iri::from_unchecked(class.iri.to_string()));
        if let Some(props) = props_by_class.remove(class.local) {
            for p in props {
                nt = nt.with_property(p);
            }
        }
        o.schema.declare_node_type(nt);
    }

    // Any classes only referenced via `applies_to` but not in
    // `classes` should still be declared, since `props_by_class` may
    // hold leftovers.
    for (class, props) in props_by_class {
        let mut nt = NodeType::new(class);
        for p in props {
            nt = nt.with_property(p);
        }
        o.schema.declare_node_type(nt);
    }

    for (spec, domain, range) in m.object_properties {
        let mut et = EdgeType::new(spec.local).with_iri(Iri::from_unchecked(spec.iri.to_string()));
        for d in *domain {
            et = et.with_domain(*d);
        }
        for r in *range {
            et = et.with_range(*r);
        }
        o.schema.declare_edge_type(et);
    }

    o
}

/// Walk the triple stream and populate the A-Box of `ontology` using
/// the mapping.
pub(crate) fn project(ontology: &mut Ontology, mapping: &Mapping, triples: &[Triple]) {
    // Pass 1: pick up each subject's recognized type. A subject may
    // carry several types; we keep the first recognized label since
    // the schema is keyed by name. We accept either a true
    // `rdf:type` triple or — as a fallback for parsers that emit
    // JSON-LD `@type` as `<subject> <class> <class>` (the shape used
    // by `atomr_ontology_rdf::jsonld`) — any triple whose object IRI
    // is one of our recognized classes.
    let rdf_type = Iri::from_unchecked(RDF_TYPE);
    let mut subject_types: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in triples {
        let Subject::Iri(subj) = &t.subject else { continue };
        let Object::Iri(class_iri) = &t.object else { continue };
        let is_type_triple = t.predicate == rdf_type
            || mapping.classes.iter().any(|c| c.iri == class_iri.as_str());
        if !is_type_triple {
            continue;
        }
        if let Some(class) = mapping.classes.iter().find(|c| c.iri == class_iri.as_str()) {
            let entry = subject_types.entry(subj.as_str().to_string()).or_default();
            if !entry.iter().any(|t| t == class.local) {
                entry.push(class.local.to_string());
            }
        }
    }

    // Helper closures for upserts.
    let ensure_node = |o: &mut Ontology, iri_str: &str, fallback: Option<&str>| -> Option<NodeId> {
        let labels = subject_types.get(iri_str).cloned();
        let label = match labels.as_ref().and_then(|v| v.first().cloned()) {
            Some(l) => l,
            None => match fallback {
                Some(f) => f.to_string(),
                None => return None,
            },
        };
        let iri = Iri::from_unchecked(iri_str.to_string());
        let mut node = Node::from_iri(iri, label);
        // Attach any additional recognized labels.
        if let Some(all_labels) = labels {
            for extra in all_labels.into_iter().skip(1) {
                node = node.with_label(extra);
            }
        }
        // Merge with any existing node so we don't blow away earlier
        // property writes.
        if let Some(existing) = o.nodes.get(&node.id).cloned() {
            let mut merged = existing.clone();
            for label in &node.types {
                if !merged.types.iter().any(|t| t == label) {
                    merged.types.push(label.clone());
                }
            }
            Some(o.upsert_node(merged))
        } else {
            Some(o.upsert_node(node))
        }
    };

    // First pre-create every typed subject so edges have endpoints.
    let typed_subjects: Vec<String> = subject_types.keys().cloned().collect();
    for iri_str in &typed_subjects {
        let _ = ensure_node(ontology, iri_str, None);
    }

    // Pass 2: assign data properties and emit edges.
    for t in triples {
        if t.predicate == rdf_type {
            continue;
        }
        let Subject::Iri(subj_iri) = &t.subject else { continue };
        let pred = t.predicate.as_str();

        // Data property?
        if let Some((spec, _)) = mapping.data_properties.iter().find(|(s, _)| s.iri == pred) {
            let value = match &t.object {
                Object::Literal { lexical, .. } => Some(literal_value(spec.datatype, lexical)),
                // IRI-shaped values land here when the source uses
                // `<...>` (Turtle) or `{"@id": ...}` (JSON-LD) for a
                // property we declared as `Datatype::Iri` (e.g.
                // `foaf:mbox`, `schema:url`). Project them as
                // `PropertyValue::Iri` so downstream code sees the
                // intended typed value.
                Object::Iri(iri) if matches!(spec.datatype, Datatype::Iri) => {
                    Some(PropertyValue::Iri(iri.clone()))
                }
                _ => None,
            };
            let Some(value) = value else { continue };
            let Some(node_id) = ensure_node(ontology, subj_iri.as_str(), None) else { continue };
            if let Some(node) = ontology.nodes.get(&node_id).cloned() {
                let updated = node.with_property(spec.local, value);
                ontology.upsert_node(updated);
            }
            continue;
        }

        // Object property?
        if let Some((spec, _, _)) = mapping.object_properties.iter().find(|(s, _, _)| s.iri == pred) {
            let Object::Iri(target_iri) = &t.object else { continue };
            let Some(src_id) = ensure_node(ontology, subj_iri.as_str(), None) else { continue };
            let Some(tgt_id) = ensure_node(ontology, target_iri.as_str(), mapping.default_object_class) else {
                continue;
            };
            let _ = ontology.upsert_edge(Edge::between(src_id, spec.local, tgt_id));
            continue;
        }
    }
}

/// Convert a literal's lexical form into the
/// [`PropertyValue`](atomr_ontology_core::PropertyValue) appropriate
/// for the declared datatype, falling back to `String` if parsing
/// fails (so importers stay resilient against dirty input).
fn literal_value(datatype: Datatype, lexical: &str) -> PropertyValue {
    match datatype {
        Datatype::String => PropertyValue::String(lexical.to_string()),
        Datatype::Integer => lexical
            .parse::<i64>()
            .map(PropertyValue::Integer)
            .unwrap_or_else(|_| PropertyValue::String(lexical.to_string())),
        Datatype::Float => lexical
            .parse::<f64>()
            .map(PropertyValue::Float)
            .unwrap_or_else(|_| PropertyValue::String(lexical.to_string())),
        Datatype::Bool => match lexical {
            "true" | "1" => PropertyValue::Bool(true),
            "false" | "0" => PropertyValue::Bool(false),
            _ => PropertyValue::String(lexical.to_string()),
        },
        Datatype::Iri => match Iri::new(lexical) {
            Ok(iri) => PropertyValue::Iri(iri),
            Err(_) => PropertyValue::String(lexical.to_string()),
        },
        // DateTime / Bytes / Json fall through as plain strings —
        // good enough for the import surface; downstream code can
        // re-interpret if needed.
        _ => PropertyValue::String(lexical.to_string()),
    }
}
