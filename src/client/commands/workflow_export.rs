//! Workflow export/import functionality
//!
//! This module provides the ability to export workflows to a portable JSON format
//! and import them into the same or different torc-server instances.
//!
//! ## Export Format
//!
//! The export format is a self-contained JSON document that includes:
//! - Workflow metadata
//! - All jobs with their relationships (input/output files, user_data, dependencies)
//! - Files and user_data
//! - Resource requirements
//! - Schedulers (Slurm and local)
//! - Workflow actions
//! - Optionally: results and events
//!
//! ## ID Remapping
//!
//! During import, all entity IDs are remapped to new IDs assigned by the target server.
//! Cross-references (e.g., job's input_file_ids) are updated to use the new IDs.

use serde::{Deserialize, Serialize};

use crate::models::{
    ComputeNodeModel, EventModel, FailureHandlerModel, FileModel, JobModel, LocalSchedulerModel,
    ResourceRequirementsModel, ResultModel, SlurmSchedulerModel, UserDataModel,
    WorkflowActionModel, WorkflowModel,
};

/// Current version of the export format
pub const EXPORT_VERSION: &str = "1.0";

/// Complete workflow export document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExport {
    /// Version of the export format
    pub export_version: String,

    /// Timestamp when the export was created (ISO 8601)
    pub exported_at: String,

    /// The workflow metadata
    pub workflow: WorkflowModel,

    /// All files in the workflow
    pub files: Vec<FileModel>,

    /// All user data in the workflow
    pub user_data: Vec<UserDataModel>,

    /// All resource requirements in the workflow
    pub resource_requirements: Vec<ResourceRequirementsModel>,

    /// Slurm schedulers in the workflow
    pub slurm_schedulers: Vec<SlurmSchedulerModel>,

    /// Local schedulers in the workflow
    pub local_schedulers: Vec<LocalSchedulerModel>,

    /// Failure handlers in the workflow
    #[serde(default)]
    pub failure_handlers: Vec<FailureHandlerModel>,

    /// All jobs in the workflow (includes relationship IDs)
    pub jobs: Vec<JobModel>,

    /// Workflow actions (triggers like on_workflow_start)
    pub workflow_actions: Vec<WorkflowActionModel>,

    /// Compute nodes (included when results are included, since results reference them)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_nodes: Option<Vec<ComputeNodeModel>>,

    /// Job results (optional, only included with --include-results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<ResultModel>>,

    /// Workflow events (optional, only included with --include-events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<EventModel>>,
}

impl WorkflowExport {
    /// Create a new empty export with the current version
    pub fn new(workflow: WorkflowModel) -> Self {
        Self {
            export_version: EXPORT_VERSION.to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            workflow,
            files: Vec::new(),
            user_data: Vec::new(),
            resource_requirements: Vec::new(),
            slurm_schedulers: Vec::new(),
            local_schedulers: Vec::new(),
            failure_handlers: Vec::new(),
            jobs: Vec::new(),
            workflow_actions: Vec::new(),
            compute_nodes: None,
            results: None,
            events: None,
        }
    }
}

/// Statistics about an export or import operation
#[derive(Debug, Clone, Default)]
pub struct ExportImportStats {
    pub jobs: usize,
    pub files: usize,
    pub user_data: usize,
    pub resource_requirements: usize,
    pub slurm_schedulers: usize,
    pub local_schedulers: usize,
    pub failure_handlers: usize,
    pub workflow_actions: usize,
    pub compute_nodes: usize,
    pub results: usize,
    pub events: usize,
}

impl ExportImportStats {
    pub fn from_export(export: &WorkflowExport) -> Self {
        Self {
            jobs: export.jobs.len(),
            files: export.files.len(),
            user_data: export.user_data.len(),
            resource_requirements: export.resource_requirements.len(),
            slurm_schedulers: export.slurm_schedulers.len(),
            local_schedulers: export.local_schedulers.len(),
            failure_handlers: export.failure_handlers.len(),
            workflow_actions: export.workflow_actions.len(),
            compute_nodes: export.compute_nodes.as_ref().map(|c| c.len()).unwrap_or(0),
            results: export.results.as_ref().map(|r| r.len()).unwrap_or(0),
            events: export.events.as_ref().map(|e| e.len()).unwrap_or(0),
        }
    }
}

use std::collections::HashMap;

/// ID mapping tables used during import
#[derive(Debug, Default)]
pub struct IdMappings {
    pub files: HashMap<i64, i64>,
    pub user_data: HashMap<i64, i64>,
    pub resource_requirements: HashMap<i64, i64>,
    pub slurm_schedulers: HashMap<i64, i64>,
    pub local_schedulers: HashMap<i64, i64>,
    pub failure_handlers: HashMap<i64, i64>,
    pub jobs: HashMap<i64, i64>,
    pub compute_nodes: HashMap<i64, i64>,
}

impl IdMappings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Remap a file ID using the mapping table
    pub fn remap_file_id(&self, old_id: i64) -> Option<i64> {
        self.files.get(&old_id).copied()
    }

    /// Remap a user_data ID using the mapping table
    pub fn remap_user_data_id(&self, old_id: i64) -> Option<i64> {
        self.user_data.get(&old_id).copied()
    }

    /// Remap a resource_requirements ID using the mapping table
    pub fn remap_resource_requirements_id(&self, old_id: i64) -> Option<i64> {
        self.resource_requirements.get(&old_id).copied()
    }

    /// Remap a scheduler ID (tries both slurm and local)
    pub fn remap_scheduler_id(&self, old_id: i64) -> Option<i64> {
        self.slurm_schedulers
            .get(&old_id)
            .or_else(|| self.local_schedulers.get(&old_id))
            .copied()
    }

    /// Remap a failure_handler ID using the mapping table
    pub fn remap_failure_handler_id(&self, old_id: i64) -> Option<i64> {
        self.failure_handlers.get(&old_id).copied()
    }

    /// Remap a job ID using the mapping table
    pub fn remap_job_id(&self, old_id: i64) -> Option<i64> {
        self.jobs.get(&old_id).copied()
    }

    /// Remap a compute_node ID using the mapping table
    pub fn remap_compute_node_id(&self, old_id: i64) -> Option<i64> {
        self.compute_nodes.get(&old_id).copied()
    }

    /// Remap a vector of file IDs
    pub fn remap_file_ids(&self, old_ids: &[i64]) -> Vec<i64> {
        old_ids
            .iter()
            .filter_map(|id| self.remap_file_id(*id))
            .collect()
    }

    /// Remap a vector of user_data IDs
    pub fn remap_user_data_ids(&self, old_ids: &[i64]) -> Vec<i64> {
        old_ids
            .iter()
            .filter_map(|id| self.remap_user_data_id(*id))
            .collect()
    }

    /// Remap a vector of job IDs
    pub fn remap_job_ids(&self, old_ids: &[i64]) -> Vec<i64> {
        old_ids
            .iter()
            .filter_map(|id| self.remap_job_id(*id))
            .collect()
    }
}
