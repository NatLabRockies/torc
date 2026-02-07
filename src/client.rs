//! Client implementation for the Torc workflow orchestration system
//!
//! This module contains all client-side functionality including API wrappers,
//! CLI command handlers, workflow management, and job execution.

// apis module is generated from OpenAPI spec - suppress clippy warnings for generated code patterns
#[allow(
    clippy::too_many_arguments,
    clippy::result_large_err,
    clippy::needless_return
)]
pub mod apis;
pub mod async_cli_command;
pub mod commands;
pub mod errors;
pub mod resource_correction;

// Re-export config from the top-level module for backwards compatibility
#[cfg(feature = "config")]
pub use crate::config;
pub mod execution_plan;
pub mod hpc;
pub mod job_runner;
pub mod log_paths;
pub mod parameter_expansion;
pub mod remote;
pub mod report_models;
pub mod resource_monitor;
pub mod scheduler_plan;
pub mod sse_client;
pub mod utils;
pub mod version_check;
pub mod workflow_graph;
pub mod workflow_manager;
pub mod workflow_spec;

// Re-exports for convenience
pub use apis::configuration::Configuration;
pub use apis::default_api;
pub use hpc::{
    HpcDetection, HpcInterface, HpcJobInfo, HpcJobStats, HpcJobStatus, HpcManager, HpcPartition,
    HpcProfile, HpcProfileRegistry, HpcType, SlurmInterface, create_hpc_interface,
};
pub use job_runner::JobRunner;
// JobModel is re-exported from models (which re-exports from crate::models)
pub use utils::send_with_retries;
pub use workflow_manager::WorkflowManager;
pub use workflow_spec::{
    FileSpec, JobSpec, ResourceRequirementsSpec, SlurmSchedulerSpec, UserDataSpec, WorkflowSpec,
};

// Report model types for inter-command data sharing
pub use report_models::{
    FailedJobInfo, JobResultRecord, ResourceUtilizationReport, ResourceViolation, ResultsReport,
};

// Version checking utilities
pub use version_check::{
    VersionCheckResult, VersionMismatchSeverity, check_and_warn, check_version,
};
