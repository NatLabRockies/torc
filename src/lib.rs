//! Torc - Workflow Orchestration System
//!
//! This library provides shared functionality for the Torc workflow orchestration system.
//! It includes data models, server implementation, and client utilities.

/// Maximum number of records that can be transferred in a single API request or response.
/// Used for both batch creation limits and pagination limits.
pub const MAX_RECORD_TRANSFER_COUNT: i64 = 100_000;

/// Get the current username from environment variables.
///
/// Checks in order: `TORC_USERNAME` (explicit override), `USER` (Unix),
/// `USERNAME` (Windows). Returns `"unknown"` if none are set.
pub fn get_username() -> String {
    std::env::var("TORC_USERNAME")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

// Shared modules (always available)
pub mod api_version;
pub mod memory_utils;
pub mod models;
pub mod network_utils;
pub mod time_utils;

// Configuration module (requires config feature, enabled by client)
#[cfg(feature = "config")]
pub mod config;

// Server modules (behind feature flag)
#[cfg(feature = "server")]
pub mod server;

// Client modules (behind feature flag)
#[cfg(feature = "client")]
pub mod client;

// TUI module (behind feature flag)
#[cfg(feature = "tui")]
pub mod tui;

// Binary command modules (behind feature flags) - re-exported for standalone binaries
#[cfg(feature = "client")]
pub mod run_jobs_cmd;

#[cfg(feature = "client")]
pub mod exec_cmd;

#[cfg(feature = "tui")]
pub mod tui_runner;

#[cfg(feature = "plot_resources")]
pub mod plot_resources_cmd;

// MCP server modules (behind feature flag)
#[cfg(feature = "mcp-server")]
pub mod mcp_server;

// Rust-owned OpenAPI emission
#[cfg(feature = "openapi-codegen")]
pub mod openapi_spec;

// CLI types module - requires all features for the unified CLI
#[cfg(all(feature = "client", feature = "tui", feature = "plot_resources"))]
pub mod cli;

// Re-export model types explicitly
pub use models::{
    ClaimJobsBasedOnResources, ClaimNextJobsResponse, ComputeNodeModel, ComputeNodeSchedule,
    ComputeNodesResources, CreateJobsResponse, ErrorResponse, EventModel, FileModel,
    GetReadyJobRequirementsResponse, IsCompleteResponse, JobDependencyModel,
    JobFileRelationshipModel, JobModel, JobStatus, JobUserDataRelationshipModel, JobsModel,
    ListComputeNodesResponse, ListEventsResponse, ListFilesResponse, ListJobDependenciesResponse,
    ListJobFileRelationshipsResponse, ListJobUserDataRelationshipsResponse, ListJobsResponse,
    ListLocalSchedulersResponse, ListMissingUserDataResponse, ListRequiredExistingFilesResponse,
    ListResourceRequirementsResponse, ListResultsResponse, ListScheduledComputeNodesResponse,
    ListSlurmSchedulersResponse, ListUserDataResponse, ListWorkflowsResponse, LocalSchedulerModel,
    ProcessChangedJobInputsResponse, ResourceRequirementsModel, ResultModel,
    ScheduledComputeNodesModel, SlurmSchedulerModel, UserDataModel, WorkflowActionModel,
    WorkflowModel, WorkflowStatusModel,
};

// Re-export client types when client feature is enabled
#[cfg(feature = "client")]
pub use client::apis::configuration::Configuration;
