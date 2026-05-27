//! HTTP/JSON remote `OntologyStore` server and client.
//!
//! The `client` feature exposes [`RemoteClient`], which implements
//! `OntologyStore` by issuing HTTP requests to a compatible server.
//! The `server` feature exposes [`serve`] which runs an Axum-shaped
//! HTTP service backed by any local `OntologyStore`.

#![forbid(unsafe_code)]

pub mod protocol;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

pub use protocol::{RemoteError, RpcEnvelope};

#[cfg(feature = "client")]
pub use client::RemoteClient;

#[cfg(feature = "server")]
pub use server::{serve, ServerHandle};
