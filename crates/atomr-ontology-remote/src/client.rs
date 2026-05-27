//! HTTP/JSON client implementing [`OntologyStore`].
//!
//! Each method serializes its parameters into a [`RpcEnvelope`],
//! POSTs `<base_url>/rpc/<method>` with `Content-Type: application/json`,
//! and deserializes the response body as the matching `*Response`
//! struct. Server-side failures are surfaced as [`StoreError::Io`].

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};

use atomr_ontology_core::{Axiom, Edge, EdgeId, Node, NodeId, Ontology};
use atomr_ontology_provenance::{Activity, ProvenanceId, ProvenanceLog};
use atomr_ontology_store::{
    MatchRow, NodePattern, OntologyDelta, OntologyStore, StoreDiff, StoreError, TraversalPlan,
};

use crate::protocol::{
    method, CommitRequest, CommitResponse, DiffRequest, DiffResponse, GetEdgeRequest,
    GetEdgeResponse, GetNodeRequest, GetNodeResponse, MatchPatternRequest, MatchPatternResponse,
    ProvenanceRequest, ProvenanceResponse, RemoteError, RpcEnvelope, RpcResponse, SnapshotRequest,
    SnapshotResponse, TraverseRequest, TraverseResponse, UpsertAxiomRequest, UpsertAxiomResponse,
    UpsertEdgeRequest, UpsertEdgeResponse, UpsertNodeRequest, UpsertNodeResponse,
};

/// HTTP client that drives a remote `OntologyStore` over JSON.
///
/// The client is `Clone`-cheap; the inner [`reqwest::Client`] keeps a
/// shared connection pool.
#[derive(Clone, Debug)]
pub struct RemoteClient {
    base_url: String,
    http: Client,
}

impl RemoteClient {
    /// Build a client targeting `base_url` (e.g. `http://127.0.0.1:8080`).
    ///
    /// The trailing `/` on `base_url` is normalized.
    pub fn new(base_url: impl Into<String>) -> Result<Self, RemoteError> {
        let http = Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            .build()
            .map_err(|e| RemoteError::Transport(format!("build http client: {e}")))?;
        Ok(Self { base_url: base_url.into().trim_end_matches('/').to_string(), http })
    }

    /// Build a client from an existing [`reqwest::Client`].
    pub fn with_http(base_url: impl Into<String>, http: Client) -> Self {
        Self { base_url: base_url.into().trim_end_matches('/').to_string(), http }
    }

    /// Borrow the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Issue a single RPC. Used by every trait method.
    async fn call<Req, Res>(&self, method_name: &str, params: Req) -> Result<Res, RemoteError>
    where
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let envelope = RpcEnvelope::new(method_name, params);
        let url = format!("{}/rpc/{}", self.base_url, method_name);
        let body = serde_json::to_vec(&envelope)
            .map_err(|e| RemoteError::Encoding(format!("serialize {method_name}: {e}")))?;
        let resp = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| RemoteError::Transport(format!("send {method_name}: {e}")))?;
        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| RemoteError::Transport(format!("read body {method_name}: {e}")))?;
        if !status.is_success() {
            // Try to surface a body-level error message, falling back to status text.
            let parsed: Result<RpcResponse<serde_json::Value>, _> = serde_json::from_slice(&bytes);
            let message = match parsed {
                Ok(r) => r.error.unwrap_or_else(|| format!("HTTP {status}")),
                Err(_) => format!("HTTP {status}: {}", String::from_utf8_lossy(&bytes)),
            };
            return Err(RemoteError::Server(message));
        }
        let parsed: RpcResponse<Res> = serde_json::from_slice(&bytes)
            .map_err(|e| RemoteError::Encoding(format!("deserialize {method_name}: {e}")))?;
        if let Some(err) = parsed.error {
            return Err(RemoteError::Server(err));
        }
        parsed.result.ok_or_else(|| {
            RemoteError::Encoding(format!("response for {method_name} missing both result and error"))
        })
    }
}

fn into_store_err(err: RemoteError) -> StoreError {
    StoreError::Io(err.to_string())
}

#[async_trait]
impl OntologyStore for RemoteClient {
    async fn upsert_node(&self, node: Node) -> Result<NodeId, StoreError> {
        let res: UpsertNodeResponse =
            self.call(method::UPSERT_NODE, UpsertNodeRequest { node }).await.map_err(into_store_err)?;
        Ok(res.id)
    }

    async fn upsert_edge(&self, edge: Edge) -> Result<EdgeId, StoreError> {
        let res: UpsertEdgeResponse =
            self.call(method::UPSERT_EDGE, UpsertEdgeRequest { edge }).await.map_err(into_store_err)?;
        Ok(res.id)
    }

    async fn upsert_axiom(&self, axiom: Axiom) -> Result<(), StoreError> {
        let _: UpsertAxiomResponse = self
            .call(method::UPSERT_AXIOM, UpsertAxiomRequest { axiom })
            .await
            .map_err(into_store_err)?;
        Ok(())
    }

    async fn node(&self, id: &NodeId) -> Result<Option<Node>, StoreError> {
        let res: GetNodeResponse =
            self.call(method::GET_NODE, GetNodeRequest { id: *id }).await.map_err(into_store_err)?;
        Ok(res.node)
    }

    async fn edge(&self, id: &EdgeId) -> Result<Option<Edge>, StoreError> {
        let res: GetEdgeResponse =
            self.call(method::GET_EDGE, GetEdgeRequest { id: *id }).await.map_err(into_store_err)?;
        Ok(res.edge)
    }

    async fn match_pattern(&self, pattern: &NodePattern) -> Result<Vec<MatchRow>, StoreError> {
        let res: MatchPatternResponse = self
            .call(method::MATCH_PATTERN, MatchPatternRequest { pattern: pattern.into() })
            .await
            .map_err(into_store_err)?;
        Ok(res.rows.into_iter().map(MatchRow::from).collect())
    }

    async fn traverse(&self, plan: &TraversalPlan) -> Result<Vec<MatchRow>, StoreError> {
        let res: TraverseResponse = self
            .call(method::TRAVERSE, TraverseRequest { plan: plan.into() })
            .await
            .map_err(into_store_err)?;
        Ok(res.rows.into_iter().map(MatchRow::from).collect())
    }

    async fn snapshot(&self) -> Result<Ontology, StoreError> {
        let res: SnapshotResponse =
            self.call(method::SNAPSHOT, SnapshotRequest::default()).await.map_err(into_store_err)?;
        Ok(res.ontology.into())
    }

    async fn diff(&self, other: &Ontology) -> Result<StoreDiff, StoreError> {
        let res: DiffResponse = self
            .call(method::DIFF, DiffRequest { other: other.into() })
            .await
            .map_err(into_store_err)?;
        Ok(res.diff.into())
    }

    async fn commit_with_provenance(
        &self,
        delta: OntologyDelta,
        activity: Activity,
    ) -> Result<ProvenanceId, StoreError> {
        let res: CommitResponse = self
            .call(method::COMMIT, CommitRequest { delta: delta.into(), activity: activity.into() })
            .await
            .map_err(into_store_err)?;
        Ok(res.provenance_id)
    }

    async fn provenance(&self) -> Result<ProvenanceLog, StoreError> {
        let res: ProvenanceResponse =
            self.call(method::PROVENANCE, ProvenanceRequest::default()).await.map_err(into_store_err)?;
        Ok(res.log.into())
    }
}
