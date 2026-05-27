//! Minimal HTTP/1.1 server that dispatches RPCs to a local
//! [`OntologyStore`].
//!
//! The implementation is intentionally hand-rolled on top of
//! `tokio::net::TcpListener` so the dependency surface stays small.
//! Only the surface required by [`crate::client::RemoteClient`] is
//! handled: `POST /rpc/<method>` with a JSON body sized by an explicit
//! `Content-Length` header. Anything else returns `404` or `400`.

use std::net::SocketAddr;
use std::sync::Arc;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use atomr_ontology_store::OntologyStore;

use crate::protocol::{
    method, CommitRequest, CommitResponse, DiffRequest, DiffResponse, GetEdgeRequest,
    GetEdgeResponse, GetNodeRequest, GetNodeResponse, MatchPatternRequest, MatchPatternResponse,
    ProvenanceRequest, ProvenanceResponse, RemoteError, RpcEnvelope, RpcResponse, SnapshotRequest,
    SnapshotResponse, TraverseRequest, TraverseResponse, UpsertAxiomRequest, UpsertAxiomResponse,
    UpsertEdgeRequest, UpsertEdgeResponse, UpsertNodeRequest, UpsertNodeResponse,
};

/// Handle to a running RPC server. Drop or call [`ServerHandle::shutdown`]
/// to stop the listener.
#[derive(Debug)]
pub struct ServerHandle {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// Bound socket address (useful when the caller supplied port `0`).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Signal the server to stop accepting new connections and await its
    /// task. Returns once the accept loop has exited; in-flight requests
    /// may still complete after the listener stops accepting.
    pub async fn shutdown(mut self) -> Result<(), RemoteError> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(|e| RemoteError::Transport(format!("join: {e}")))?;
        }
        Ok(())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Bind a TCP listener on `addr` and dispatch HTTP requests to `store`.
///
/// Returns a [`ServerHandle`] carrying the bound address (useful when
/// `addr.port() == 0`) and a shutdown channel.
pub async fn serve(
    addr: SocketAddr,
    store: Arc<dyn OntologyStore>,
) -> Result<ServerHandle, RemoteError> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| RemoteError::Transport(format!("bind {addr}: {e}")))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| RemoteError::Transport(format!("local_addr: {e}")))?;
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_rx => break,
                accept = listener.accept() => {
                    match accept {
                        Ok((socket, _peer)) => {
                            let store = Arc::clone(&store);
                            tokio::spawn(async move {
                                if let Err(err) = handle_connection(socket, store).await {
                                    tracing::debug!(?err, "rpc connection error");
                                }
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "accept failed");
                            // Avoid busy-loop on persistent accept errors.
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }
    });

    Ok(ServerHandle { addr: local_addr, shutdown: Some(shutdown_tx), task: Some(task) })
}

// --- connection handling --------------------------------------------------

async fn handle_connection(
    mut socket: TcpStream,
    store: Arc<dyn OntologyStore>,
) -> Result<(), RemoteError> {
    let request = match read_request(&mut socket).await {
        Ok(req) => req,
        Err(err) => {
            write_error_response(&mut socket, 400, &err.to_string()).await?;
            return Ok(());
        }
    };

    if request.method != "POST" {
        write_error_response(&mut socket, 405, "method not allowed").await?;
        return Ok(());
    }

    let method_name = match request.path.strip_prefix("/rpc/") {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => {
            write_error_response(&mut socket, 404, "unknown route").await?;
            return Ok(());
        }
    };

    let response_bytes = dispatch(&store, &method_name, &request.body).await;
    write_ok_response(&mut socket, &response_bytes).await?;
    Ok(())
}

async fn dispatch(
    store: &Arc<dyn OntologyStore>,
    method_name: &str,
    body: &[u8],
) -> Vec<u8> {
    match method_name {
        method::UPSERT_NODE => run::<UpsertNodeRequest, UpsertNodeResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let id = store.upsert_node(params.node).await.map_err(|e| e.to_string())?;
                Ok(UpsertNodeResponse { id })
            }
        })
        .await,
        method::UPSERT_EDGE => run::<UpsertEdgeRequest, UpsertEdgeResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let id = store.upsert_edge(params.edge).await.map_err(|e| e.to_string())?;
                Ok(UpsertEdgeResponse { id })
            }
        })
        .await,
        method::UPSERT_AXIOM => {
            run::<UpsertAxiomRequest, UpsertAxiomResponse, _, _>(body, |params| {
                let store = Arc::clone(store);
                async move {
                    store.upsert_axiom(params.axiom).await.map_err(|e| e.to_string())?;
                    Ok(UpsertAxiomResponse::default())
                }
            })
            .await
        }
        method::GET_NODE => run::<GetNodeRequest, GetNodeResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let node = store.node(&params.id).await.map_err(|e| e.to_string())?;
                Ok(GetNodeResponse { node })
            }
        })
        .await,
        method::GET_EDGE => run::<GetEdgeRequest, GetEdgeResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let edge = store.edge(&params.id).await.map_err(|e| e.to_string())?;
                Ok(GetEdgeResponse { edge })
            }
        })
        .await,
        method::MATCH_PATTERN => {
            run::<MatchPatternRequest, MatchPatternResponse, _, _>(body, |params| {
                let store = Arc::clone(store);
                async move {
                    let pattern = atomr_ontology_store::NodePattern::from(params.pattern);
                    let rows = store.match_pattern(&pattern).await.map_err(|e| e.to_string())?;
                    Ok(MatchPatternResponse {
                        rows: rows.into_iter().map(Into::into).collect(),
                    })
                }
            })
            .await
        }
        method::TRAVERSE => run::<TraverseRequest, TraverseResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let plan: atomr_ontology_store::TraversalPlan = params.plan.into();
                let rows = store.traverse(&plan).await.map_err(|e| e.to_string())?;
                Ok(TraverseResponse { rows: rows.into_iter().map(Into::into).collect() })
            }
        })
        .await,
        method::SNAPSHOT => run::<SnapshotRequest, SnapshotResponse, _, _>(body, |_params| {
            let store = Arc::clone(store);
            async move {
                let ontology = store.snapshot().await.map_err(|e| e.to_string())?;
                Ok(SnapshotResponse { ontology: ontology.into() })
            }
        })
        .await,
        method::DIFF => run::<DiffRequest, DiffResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let other: atomr_ontology_core::Ontology = params.other.into();
                let diff = store.diff(&other).await.map_err(|e| e.to_string())?;
                Ok(DiffResponse { diff: diff.into() })
            }
        })
        .await,
        method::COMMIT => run::<CommitRequest, CommitResponse, _, _>(body, |params| {
            let store = Arc::clone(store);
            async move {
                let delta = atomr_ontology_store::OntologyDelta::from(params.delta);
                let activity = atomr_ontology_provenance::Activity::from(params.activity);
                let provenance_id = store
                    .commit_with_provenance(delta, activity)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(CommitResponse { provenance_id })
            }
        })
        .await,
        method::PROVENANCE => run::<ProvenanceRequest, ProvenanceResponse, _, _>(body, |_params| {
            let store = Arc::clone(store);
            async move {
                let log = store.provenance().await.map_err(|e| e.to_string())?;
                Ok(ProvenanceResponse { log: log.into() })
            }
        })
        .await,
        other => encode_response(RpcResponse::<()>::err(format!("unknown method: {other}"))),
    }
}

async fn run<Req, Res, F, Fut>(body: &[u8], f: F) -> Vec<u8>
where
    Req: serde::de::DeserializeOwned,
    Res: Serialize,
    F: FnOnce(Req) -> Fut,
    Fut: std::future::Future<Output = Result<Res, String>>,
{
    let envelope: RpcEnvelope<Req> = match serde_json::from_slice(body) {
        Ok(e) => e,
        Err(e) => {
            return encode_response(RpcResponse::<()>::err(format!("decode: {e}")));
        }
    };
    match f(envelope.params).await {
        Ok(value) => encode_response(RpcResponse::ok(value)),
        Err(message) => encode_response(RpcResponse::<()>::err(message)),
    }
}

fn encode_response<T: Serialize>(resp: RpcResponse<T>) -> Vec<u8> {
    match serde_json::to_vec(&resp) {
        Ok(bytes) => bytes,
        Err(e) => {
            // Last-resort fallback: a manually-encoded error response.
            let fallback = format!(r#"{{"error":"encode response: {e}"}}"#);
            fallback.into_bytes()
        }
    }
}

// --- HTTP/1.1 framing -----------------------------------------------------

#[derive(Debug)]
struct ParsedRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

async fn read_request(socket: &mut TcpStream) -> Result<ParsedRequest, RemoteError> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = socket
            .read(&mut chunk)
            .await
            .map_err(|e| RemoteError::Transport(format!("read: {e}")))?;
        if n == 0 {
            return Err(RemoteError::Transport("connection closed before headers".into()));
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(idx) = find_header_terminator(&buf) {
            break idx;
        }
        if buf.len() > 64 * 1024 {
            return Err(RemoteError::Transport("headers exceeded 64 KiB".into()));
        }
    };

    let header_bytes = &buf[..header_end];
    let header_str = std::str::from_utf8(header_bytes)
        .map_err(|e| RemoteError::Transport(format!("non-utf8 headers: {e}")))?;
    let mut lines = header_str.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| RemoteError::Transport("missing request line".into()))?;
    let mut parts = request_line.split_ascii_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| RemoteError::Transport("missing method".into()))?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| RemoteError::Transport("missing path".into()))?
        .to_string();
    // The HTTP-version token is ignored — we always respond with HTTP/1.1.

    let mut content_length: usize = 0;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value
                    .trim()
                    .parse()
                    .map_err(|e| RemoteError::Transport(format!("bad content-length: {e}")))?;
            }
        }
    }

    let body_start = header_end + 4; // skip "\r\n\r\n"
    let mut body = if buf.len() > body_start {
        buf[body_start..].to_vec()
    } else {
        Vec::new()
    };
    while body.len() < content_length {
        let n = socket
            .read(&mut chunk)
            .await
            .map_err(|e| RemoteError::Transport(format!("read body: {e}")))?;
        if n == 0 {
            return Err(RemoteError::Transport("body truncated".into()));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Ok(ParsedRequest { method, path, body })
}

fn find_header_terminator(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn write_ok_response(socket: &mut TcpStream, body: &[u8]) -> Result<(), RemoteError> {
    write_response(socket, 200, "OK", body).await
}

async fn write_error_response(
    socket: &mut TcpStream,
    status: u16,
    message: &str,
) -> Result<(), RemoteError> {
    let resp = RpcResponse::<()>::err(message);
    let bytes = encode_response(resp);
    let reason = match status {
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Error",
    };
    write_response(socket, status, reason, &bytes).await
}

async fn write_response(
    socket: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &[u8],
) -> Result<(), RemoteError> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket
        .write_all(header.as_bytes())
        .await
        .map_err(|e| RemoteError::Transport(format!("write header: {e}")))?;
    socket
        .write_all(body)
        .await
        .map_err(|e| RemoteError::Transport(format!("write body: {e}")))?;
    let _ = socket.shutdown().await;
    Ok(())
}
