//! Server implementation for the Torc workflow orchestration system
//!
//! This module contains all server-side functionality including API implementations,
//! authentication, routing, and database operations.

pub mod api;
pub mod api_types;
pub mod auth;
pub mod authorization;
pub mod context;
pub mod credential_cache;
pub mod dashboard;
pub mod event_broadcast;
pub mod header;
pub mod htpasswd;
pub mod routing;

// These modules are only needed for the server binary, not the server library
#[cfg(feature = "server-bin")]
pub mod http_server;
#[cfg(feature = "server-bin")]
pub mod logging;
#[cfg(feature = "server-bin")]
pub mod service;

// Re-exports from api_types (OpenAPI-generated)
pub use api_types::*;

// Re-exports from event_broadcast
pub use event_broadcast::{BroadcastEvent, EventBroadcaster};

// Re-exports from swagger crate
pub use swagger::ContextWrapper;
