//! Flat denormalized projection — one node per (workflow, run) tuple
//! carrying all step state as nested JSON properties.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::json;

use atomr_ontology_core::{Iri, Node, NodeId, PropertyValue};
use atomr_ontology_store::r#trait::OntologyDelta;

use crate::batch::ActorBatch;
use crate::source::SupervisionPath;
use crate::strategy::IriMintingStrategy;
use crate::vocab;
use crate::ProjectionError;

use super::{ProjectionCtx, ProjectionKind, ProjectionStrategy};

/// One node per (workflow, run). All step events under that run get
/// aggregated into a `steps` JSON property; state blobs accumulate
/// under `states`.
///
/// Fast query at the cost of graph navigability.
#[derive(Clone, Debug, Default)]
pub struct FlatProjection {
    label: String,
}

impl FlatProjection {
    /// Construct with default label.
    pub fn new() -> Self {
        Self { label: "flat".into() }
    }

    /// Override the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[async_trait]
impl ProjectionStrategy for FlatProjection {
    fn label(&self) -> &str {
        if self.label.is_empty() {
            "flat"
        } else {
            &self.label
        }
    }

    fn kind(&self) -> ProjectionKind {
        ProjectionKind::Flat
    }

    async fn project(
        &self,
        batch: &ActorBatch,
        ctx: &ProjectionCtx,
    ) -> Result<OntologyDelta, ProjectionError> {
        // Aggregate paths by (workflow, run) prefix.
        let mut buckets: BTreeMap<(String, String), FlatBucket> = BTreeMap::new();

        for path in &batch.paths {
            if let Some(key) = workflow_run_key(path) {
                buckets.entry(key).or_default().add_path(path);
            }
        }
        for event in &batch.events {
            let key = event.path.as_ref().and_then(workflow_run_key);
            if let Some(key) = key {
                buckets
                    .entry(key)
                    .or_default()
                    .events
                    .push(json!({
                        "actor": event.actor.as_str(),
                        "kind": crate::strategy::journal_event_kind_str(&event.kind),
                        "cursor": event.cursor.version,
                        "at": event.at.to_rfc3339(),
                        "payload": event.payload,
                    }));
            }
        }
        for state in &batch.states {
            // Without a path, attach to a synthetic bucket keyed by actor.
            buckets
                .entry(("<unrouted>".into(), state.actor.as_str().to_owned()))
                .or_default()
                .states
                .push(json!({
                    "actor": state.actor.as_str(),
                    "payload": state.payload,
                    "digest": state.digest,
                }));
        }

        let mut delta = OntologyDelta::new();
        for ((wf, run), bucket) in buckets {
            let path = SupervisionPath::parse(&format!("/{}/{}", wf, run));
            // The flat projection synthesizes one IRI per bucket; for
            // Uuid strategy we still need stability so we fall back to
            // ContentAddressed for bucket nodes.
            let (id, iri) = match &ctx.iri {
                IriMintingStrategy::Uuid => (
                    NodeId::content_address(format!("flat:{}:{}", wf, run).as_bytes()),
                    None,
                ),
                IriMintingStrategy::ContentAddressed => (
                    NodeId::content_address(format!("flat:{}:{}", wf, run).as_bytes()),
                    None,
                ),
                IriMintingStrategy::PathBased { base } => {
                    let rendered = format!(
                        "{}{}",
                        base.as_str().strip_suffix('/').unwrap_or(base.as_str()),
                        path.render()
                    );
                    let iri = Iri::new(rendered)?;
                    (
                        NodeId::content_address(iri.as_str().as_bytes()),
                        Some(iri),
                    )
                }
            };

            let mut node = Node {
                id,
                iri,
                types: vec![vocab::NODE_ACTOR.into(), vocab::NODE_RUN.into()],
                properties: BTreeMap::new(),
            };
            node = node
                .with_property(vocab::PROP_PATH, path.render())
                .with_property("workflow", wf.clone())
                .with_property("run", run.clone())
                .with_property(vocab::PROP_SOURCE, ctx.source_label.clone())
                .with_property("steps", PropertyValue::Json(serde_json::Value::Array(bucket.events)))
                .with_property("states", PropertyValue::Json(serde_json::Value::Array(bucket.states)))
                .with_property(
                    "path_count",
                    bucket.path_count as i64,
                );
            delta.nodes.push(node);
        }

        Ok(delta)
    }
}

#[derive(Default)]
struct FlatBucket {
    path_count: u64,
    events: Vec<serde_json::Value>,
    states: Vec<serde_json::Value>,
}

impl FlatBucket {
    fn add_path(&mut self, _path: &SupervisionPath) {
        self.path_count += 1;
    }
}

/// Extract a `(workflow, run)` tuple from a path. Conventionally:
///
/// `/workflow/{wf}/run/{r}/...` → `Some(("{wf}", "{r}"))`.
///
/// Other layouts collapse into a single bucket per workflow.
fn workflow_run_key(path: &SupervisionPath) -> Option<(String, String)> {
    let mut wf: Option<&str> = None;
    let mut run: Option<&str> = None;
    let mut segs = path.segments.iter();
    while let Some(seg) = segs.next() {
        match seg.as_str() {
            "workflow" => {
                wf = segs.next().map(String::as_str);
            }
            "run" => {
                run = segs.next().map(String::as_str);
            }
            _ => {}
        }
    }
    match (wf, run) {
        (Some(w), Some(r)) => Some((w.into(), r.into())),
        (Some(w), None) => Some((w.into(), "<no-run>".into())),
        _ => None,
    }
}
