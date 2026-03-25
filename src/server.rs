//! Server implementation for the Torc workflow orchestration system
//!
//! This module contains all server-side functionality including API implementations,
//! authentication, transport, and database operations.

pub mod api;
pub mod api_constants;
pub mod api_contract;
pub mod api_responses;
pub mod auth;
pub mod authorization;
pub mod context;
pub mod credential_cache;
pub mod dashboard;
pub mod event_broadcast;
pub mod header;
pub mod htpasswd;
// These modules are needed by the server binary and by the Rust-owned OpenAPI emitter.
#[cfg(any(feature = "server-bin", feature = "openapi-codegen"))]
pub mod http_server;
#[cfg(feature = "openapi-codegen")]
pub mod http_transport;
#[cfg(feature = "openapi-codegen")]
pub mod live_router;
#[cfg(any(feature = "server-bin", feature = "openapi-codegen"))]
pub mod live_state;
#[cfg(feature = "server-bin")]
pub mod logging;
pub mod response_types;
#[cfg(feature = "server-bin")]
pub mod service;
pub mod transport_types;
