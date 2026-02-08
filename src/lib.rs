//! Torc - Workflow Orchestration System
//!
//! This library provides shared functionality for the Torc workflow orchestration system.
//! It includes data models, server implementation, and client utilities.

// Shared modules (always available)
// models.rs is generated from OpenAPI spec - suppress clippy warnings for generated code patterns
pub mod memory_utils;
#[allow(clippy::to_string_trait_impl, clippy::too_many_arguments)]
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

#[cfg(feature = "tui")]
pub mod tui_runner;

#[cfg(feature = "plot_resources")]
pub mod plot_resources_cmd;

// MCP server modules (behind feature flag)
#[cfg(feature = "mcp-server")]
pub mod mcp_server;

// CLI types module - requires all features for the unified CLI
#[cfg(all(feature = "client", feature = "tui", feature = "plot_resources"))]
pub mod cli;

// Re-export model types explicitly
pub use models::{
    ClaimJobsBasedOnResources, ClaimJobsSortMethod, ClaimNextJobsResponse, ComputeNodeModel,
    ComputeNodeSchedule, ComputeNodesResources, CreateJobsResponse, ErrorResponse, EventModel,
    FileModel, GetDotGraphResponse, GetReadyJobRequirementsResponse, IsCompleteResponse,
    JobDependencyModel, JobFileRelationshipModel, JobModel, JobStatus, JobStatusMap,
    JobUserDataRelationshipModel, JobsModel, ListComputeNodesResponse, ListEventsResponse,
    ListFilesResponse, ListJobDependenciesResponse, ListJobFileRelationshipsResponse,
    ListJobUserDataRelationshipsResponse, ListJobsResponse, ListLocalSchedulersResponse,
    ListMissingUserDataResponse, ListRequiredExistingFilesResponse,
    ListResourceRequirementsResponse, ListResultsResponse, ListScheduledComputeNodesResponse,
    ListSlurmSchedulersResponse, ListUserDataResponse, ListWorkflowsResponse, LocalSchedulerModel,
    ProcessChangedJobInputsResponse, ResourceRequirementsModel, ResultModel,
    ScheduledComputeNodesModel, SlurmSchedulerModel, UserDataModel, WorkflowActionModel,
    WorkflowModel, WorkflowStatusModel,
};

// Re-export client types when client feature is enabled
#[cfg(feature = "client")]
pub use client::apis::configuration::Configuration;
