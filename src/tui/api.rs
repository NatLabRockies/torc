use crate::client::apis::configuration::{BasicAuth, Configuration, TlsConfig};
use crate::client::apis::default_api;
use crate::client::config::TorcConfig;
use crate::client::workflow_spec::WorkflowSpec;
use crate::models::{
    FileModel, JobDependencyModel, JobModel, JobStatus, ResultModel, ScheduledComputeNodesModel,
    SlurmStatsModel, WorkflowModel,
};
use anyhow::{Context, Result};

pub struct TorcClient {
    config: Configuration,
}

impl TorcClient {
    #[allow(dead_code)]
    pub fn new() -> Result<Self> {
        Self::new_with_tls(TlsConfig::default(), None)
    }

    pub fn new_with_tls(tls: TlsConfig, basic_auth: Option<BasicAuth>) -> Result<Self> {
        // Load configuration from files (system, user, local) and environment variables
        // Priority: TORC_API_URL env var > config system > default
        //
        // Check TORC_API_URL directly for CLI compatibility. The config system uses
        // TORC_CLIENT__API_URL (double underscore), but the CLI uses TORC_API_URL,
        // so we check both to maintain consistency across all torc commands.
        let base_url = std::env::var("TORC_API_URL").unwrap_or_else(|_| {
            let file_config = TorcConfig::load().unwrap_or_default();
            file_config.client.api_url.clone()
        });

        let mut config = Configuration::with_tls(tls);
        config.base_path = base_url;
        config.basic_auth = basic_auth;

        config
            .apply_cookie_header_from_env()
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(Self { config })
    }

    #[allow(dead_code)]
    pub fn from_url(base_url: String) -> Result<Self> {
        Self::from_url_with_tls(base_url, TlsConfig::default(), None)
    }

    pub fn from_url_with_tls(
        base_url: String,
        tls: TlsConfig,
        basic_auth: Option<BasicAuth>,
    ) -> Result<Self> {
        let mut config = Configuration::with_tls(tls);
        config.base_path = base_url;
        config.basic_auth = basic_auth;

        config
            .apply_cookie_header_from_env()
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(Self { config })
    }

    pub fn get_base_url(&self) -> &str {
        &self.config.base_path
    }

    pub fn set_base_url(&mut self, base_url: &str) {
        self.config.base_path = base_url.to_string();
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowModel>> {
        let response = default_api::list_workflows(
            &self.config,
            None, // offset
            None, // sort_by
            None, // reverse_sort
            None, // limit
            None, // name
            None, // user
            None, // description
            None, // is_archived
        )
        .context("Failed to list workflows")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_workflows_for_user(&self, user: &str) -> Result<Vec<WorkflowModel>> {
        let response = default_api::list_workflows(
            &self.config,
            None,       // offset
            None,       // sort_by
            None,       // reverse_sort
            None,       // limit
            None,       // name
            Some(user), // user filter
            None,       // description
            None,       // is_archived
        )
        .context("Failed to list workflows")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_jobs(&self, workflow_id: i64) -> Result<Vec<JobModel>> {
        let response = default_api::list_jobs(
            &self.config,
            workflow_id,
            None, // status
            None, // needs_file_id
            None, // upstream_job_id
            None, // offset
            None, // limit
            None, // sort_by
            None, // reverse_sort
            None, // include_relationships
            None, // active_compute_node_id
        )
        .context("Failed to list jobs")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_files(&self, workflow_id: i64) -> Result<Vec<FileModel>> {
        let response = default_api::list_files(
            &self.config,
            workflow_id,
            None, // produced_by_job_id
            None, // offset
            None, // limit
            None, // sort_by
            None, // reverse_sort
            None, // name
            None, // path
            None, // is_output
        )
        .context("Failed to list files")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_results(&self, workflow_id: i64) -> Result<Vec<ResultModel>> {
        let response = default_api::list_results(
            &self.config,
            workflow_id,
            None, // job_id
            None, // run_id
            None, // offset
            None, // limit
            None, // sort_by
            None, // reverse_sort
            None, // return_code
            None, // status
            None, // all_runs
            None, // compute_node_id
        )
        .context("Failed to list results")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_job_dependencies(&self, workflow_id: i64) -> Result<Vec<JobDependencyModel>> {
        let response = default_api::list_job_dependencies(
            &self.config,
            workflow_id,
            None, // offset
            None, // limit
        )
        .context("Failed to list job dependencies")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_slurm_stats(&self, workflow_id: i64) -> Result<Vec<SlurmStatsModel>> {
        let response = default_api::list_slurm_stats(
            &self.config,
            workflow_id,
            None, // job_id
            None, // run_id
            None, // attempt_id
            None, // offset
            None, // limit
        )
        .context("Failed to list Slurm stats")?;

        Ok(response.items.unwrap_or_default())
    }

    pub fn list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
    ) -> Result<Vec<ScheduledComputeNodesModel>> {
        let response = default_api::list_scheduled_compute_nodes(
            &self.config,
            workflow_id,
            None, // offset
            None, // limit
            None, // sort_by
            None, // reverse_sort
            None, // scheduler_id
            None, // scheduler_config_id
            None, // status
        )
        .context("Failed to list scheduled compute nodes")?;

        Ok(response.items.unwrap_or_default())
    }

    // === Workflow Actions ===

    pub fn submit_workflow(&self, workflow_id: i64) -> Result<()> {
        // Create a workflow action to submit to scheduler
        let action = serde_json::json!({
            "workflow_id": workflow_id,
            "trigger_type": "on_workflow_start",
            "action_type": "schedule_nodes",
            "action_config": {}
        });

        default_api::create_workflow_action(&self.config, workflow_id, action)
            .context("Failed to create submit action")?;

        Ok(())
    }

    pub fn delete_workflow(&self, workflow_id: i64) -> Result<()> {
        default_api::delete_workflow(&self.config, workflow_id, None)
            .context("Failed to delete workflow")?;

        Ok(())
    }

    pub fn cancel_workflow(&self, workflow_id: i64) -> Result<()> {
        default_api::cancel_workflow(&self.config, workflow_id, None)
            .context("Failed to cancel workflow")?;

        Ok(())
    }

    // === Job Actions ===

    /// Get a job by ID to update it
    fn get_job(&self, job_id: i64) -> Result<crate::models::JobModel> {
        default_api::get_job(&self.config, job_id).context("Failed to get job")
    }

    pub fn cancel_job(&self, job_id: i64) -> Result<()> {
        // Get the existing job, update status, and PUT back
        let mut job = self.get_job(job_id)?;
        job.status = Some(JobStatus::Canceled);

        default_api::update_job(&self.config, job_id, job).context("Failed to cancel job")?;

        Ok(())
    }

    pub fn terminate_job(&self, job_id: i64) -> Result<()> {
        let mut job = self.get_job(job_id)?;
        job.status = Some(JobStatus::Terminated);

        default_api::update_job(&self.config, job_id, job).context("Failed to terminate job")?;

        Ok(())
    }

    pub fn retry_job(&self, job_id: i64) -> Result<()> {
        let mut job = self.get_job(job_id)?;
        job.status = Some(JobStatus::Ready);

        default_api::update_job(&self.config, job_id, job).context("Failed to retry job")?;

        Ok(())
    }

    // === Workflow Creation ===

    /// Validate a workflow specification without creating it
    /// Available for future use by the TUI to show validation info before creation
    #[allow(dead_code)]
    pub fn validate_workflow_spec(
        &self,
        path: &str,
    ) -> crate::client::workflow_spec::ValidationResult {
        crate::client::workflow_spec::WorkflowSpec::validate_spec(path)
    }

    pub fn create_workflow_from_file(&self, path: &str) -> Result<i64> {
        // Get current user
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        // Create the workflow using the spec
        let workflow_id = WorkflowSpec::create_workflow_from_spec(
            &self.config,
            path,
            &user,
            false, // enable_resource_monitoring
            false, // skip_checks
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(workflow_id)
    }
}
