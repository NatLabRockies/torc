use crate::client::apis;
use crate::client::apis::configuration::{BasicAuth, Configuration, TlsConfig};
use crate::client::config::TorcConfig;
use crate::client::workflow_spec::WorkflowSpec;
use crate::models::{
    ComputeNodeModel, FileModel, IsCompleteResponse, JobDependencyModel, JobModel, JobStatus,
    ResultModel, RunningJobModel, ScheduledComputeNodesModel, SlurmStatsModel, UserDataModel,
    WorkflowActionModel, WorkflowModel,
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

    /// List one page of workflows with optional server-side filters:
    /// `description` is a substring match; `name`, `user`, and `access_group`
    /// are exact. Returns the page plus a has-more flag. The TUI Workflows pane
    /// uses `access_group` server-side (it spans all owners) and narrows the
    /// other columns client-side on the loaded page.
    #[allow(clippy::too_many_arguments)]
    pub fn list_workflows_filtered(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        name: Option<&str>,
        user: Option<&str>,
        description: Option<&str>,
        access_group: Option<&str>,
    ) -> Result<(Vec<WorkflowModel>, bool)> {
        let response = apis::workflows_api::list_workflows(
            &self.config,
            offset,       // offset
            limit,        // limit
            None,         // sort_by
            None,         // reverse_sort
            name,         // name
            user,         // user
            description,  // description
            None,         // is_archived
            access_group, // access_group
        )
        .context("Failed to list workflows")?;

        Ok((response.items, response.has_more))
    }

    pub fn get_workflow(&self, workflow_id: i64) -> Result<WorkflowModel> {
        apis::workflows_api::get_workflow(&self.config, workflow_id)
            .context("Failed to get workflow")
    }

    pub fn is_workflow_complete(&self, workflow_id: i64) -> Result<IsCompleteResponse> {
        apis::workflows_api::is_workflow_complete(&self.config, workflow_id)
            .context("Failed to check workflow completion")
    }

    pub fn list_jobs(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<JobModel>> {
        // Callers of this unfiltered variant load the full list (no paging), so
        // the server's has_more is not needed here.
        Ok(self
            .list_jobs_filtered(workflow_id, offset, limit, None, None, None)?
            .0)
    }

    /// List jobs with server-side filters applied. `status` is an exact match;
    /// `name` and `command` are substring (`LIKE %value%`) matches. Used by the
    /// TUI Jobs pane so filtering spans the whole workflow, not just the loaded
    /// page. Returns the page items plus the server's `has_more` flag.
    pub fn list_jobs_filtered(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        status: Option<JobStatus>,
        name: Option<&str>,
        command: Option<&str>,
    ) -> Result<(Vec<JobModel>, bool)> {
        let response = apis::jobs_api::list_jobs(
            &self.config,
            workflow_id,
            status,  // status
            None,    // needs_file_id
            None,    // upstream_job_id
            offset,  // offset
            limit,   // limit
            None,    // sort_by
            None,    // reverse_sort
            None,    // include_relationships
            None,    // active_compute_node_id
            None,    // origin_is_set
            name,    // name
            command, // command
        )
        .context("Failed to list jobs")?;

        Ok((response.items, response.has_more))
    }

    /// List files with optional server-side filters: `name` is an exact match,
    /// `path` is a substring match.
    pub fn list_files(
        &self,
        workflow_id: i64,
        name: Option<&str>,
        path: Option<&str>,
    ) -> Result<Vec<FileModel>> {
        let response = apis::files_api::list_files(
            &self.config,
            workflow_id,
            None, // produced_by_job_id
            None, // offset
            None, // limit
            None, // sort_by
            None, // reverse_sort
            name, // name
            path, // path
            None, // is_output
        )
        .context("Failed to list files")?;

        Ok(response.items)
    }

    /// Returns the page items plus the server's `has_more` flag. Callers that
    /// load the full list (offset/limit `None`) can ignore the flag.
    pub fn list_results(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        return_code: Option<i64>,
        status: Option<JobStatus>,
    ) -> Result<(Vec<ResultModel>, bool)> {
        let response = apis::results_api::list_results(
            &self.config,
            workflow_id,
            None,        // job_id
            None,        // run_id
            return_code, // return_code
            status,      // status
            None,        // compute_node_id
            offset,      // offset
            limit,       // limit
            None,        // sort_by
            None,        // reverse_sort
            None,        // all_runs
        )
        .context("Failed to list results")?;

        Ok((response.items, response.has_more))
    }

    /// Returns the current page of running jobs (joined server-side to their
    /// compute node and scheduler job id) plus the `has_more` flag.
    pub fn list_running_jobs(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<(Vec<RunningJobModel>, bool)> {
        let response =
            apis::workflows_api::get_running_jobs(&self.config, workflow_id, offset, limit)
                .context("Failed to list running jobs")?;

        Ok((response.items, response.has_more))
    }

    /// Returns the page items plus the server's `has_more` flag for the
    /// workflow's user_data entries.
    pub fn list_user_data(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        name: Option<&str>,
    ) -> Result<(Vec<UserDataModel>, bool)> {
        let response = apis::user_data_api::list_user_data(
            &self.config,
            workflow_id,
            None,   // consumer_job_id
            None,   // producer_job_id
            offset, // offset
            limit,  // limit
            None,   // sort_by
            None,   // reverse_sort
            name,   // name
            None,   // is_ephemeral
        )
        .context("Failed to list user data")?;

        Ok((response.items, response.has_more))
    }

    pub fn list_job_dependencies(&self, workflow_id: i64) -> Result<Vec<JobDependencyModel>> {
        let response = apis::workflows_api::list_job_dependencies(
            &self.config,
            workflow_id,
            None, // offset
            None, // limit
            None, // sort_by
            None, // reverse_sort
        )
        .context("Failed to list job dependencies")?;

        Ok(response.items)
    }

    /// List Slurm stats with an optional server-side Job ID filter.
    pub fn list_slurm_stats(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
    ) -> Result<Vec<SlurmStatsModel>> {
        let response = apis::slurm_stats_api::list_slurm_stats(
            &self.config,
            workflow_id,
            job_id, // job_id
            None,   // run_id
            None,   // attempt_id
            None,   // offset
            None,   // limit
        )
        .context("Failed to list Slurm stats")?;

        Ok(response.items)
    }

    /// List scheduled compute nodes with an optional server-side Status filter.
    pub fn list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        status: Option<&str>,
    ) -> Result<Vec<ScheduledComputeNodesModel>> {
        let response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
            &self.config,
            workflow_id,
            None,   // offset
            None,   // limit
            None,   // sort_by
            None,   // reverse_sort
            None,   // scheduler_id
            None,   // scheduler_config_id
            status, // status
        )
        .context("Failed to list scheduled compute nodes")?;

        Ok(response.items)
    }

    pub fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        hostname: Option<&str>,
        is_active: Option<bool>,
    ) -> Result<(Vec<ComputeNodeModel>, bool)> {
        let response = apis::compute_nodes_api::list_compute_nodes(
            &self.config,
            workflow_id,
            offset,    // offset
            limit,     // limit
            None,      // sort_by
            None,      // reverse_sort
            hostname,  // hostname
            is_active, // is_active
            None,      // scheduled_compute_node_id
        )
        .context("Failed to list compute nodes")?;

        Ok((response.items, response.has_more))
    }

    // === Workflow Actions ===

    pub fn submit_workflow(&self, workflow_id: i64) -> Result<()> {
        // Create a workflow action to submit to scheduler
        let action = WorkflowActionModel {
            id: None,
            workflow_id,
            trigger_type: "on_workflow_start".to_string(),
            action_type: "schedule_nodes".to_string(),
            action_config: serde_json::json!({}),
            job_ids: None,
            trigger_count: 0,
            required_triggers: 1,
            executed: false,
            executed_at: None,
            executed_by: None,
            persistent: false,
            is_recovery: false,
        };

        apis::workflow_actions_api::create_workflow_action(&self.config, workflow_id, action)
            .context("Failed to create submit action")?;

        Ok(())
    }

    pub fn delete_workflow(&self, workflow_id: i64) -> Result<()> {
        apis::workflows_api::delete_workflow(&self.config, workflow_id)
            .context("Failed to delete workflow")?;

        Ok(())
    }

    pub fn cancel_workflow(&self, workflow_id: i64) -> Result<()> {
        apis::workflows_api::cancel_workflow(&self.config, workflow_id)
            .context("Failed to cancel workflow")?;

        Ok(())
    }

    // === Job Actions ===

    /// Get a job by ID to update it
    fn get_job(&self, job_id: i64) -> Result<crate::models::JobModel> {
        apis::jobs_api::get_job(&self.config, job_id).context("Failed to get job")
    }

    pub fn cancel_job(&self, job_id: i64) -> Result<()> {
        // Get the existing job, update status, and PUT back
        let mut job = self.get_job(job_id)?;
        job.status = Some(JobStatus::Canceled);

        apis::jobs_api::update_job(&self.config, job_id, job).context("Failed to cancel job")?;

        Ok(())
    }

    pub fn terminate_job(&self, job_id: i64) -> Result<()> {
        let mut job = self.get_job(job_id)?;
        job.status = Some(JobStatus::Terminated);

        apis::jobs_api::update_job(&self.config, job_id, job).context("Failed to terminate job")?;

        Ok(())
    }

    pub fn retry_job(&self, job_id: i64) -> Result<()> {
        let mut job = self.get_job(job_id)?;
        job.status = Some(JobStatus::Ready);

        apis::jobs_api::update_job(&self.config, job_id, job).context("Failed to retry job")?;

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
        // Validate and parse once, then reuse for creation
        let spec = WorkflowSpec::validate_for_creation(path)
            .map_err(|e| anyhow::anyhow!("Validation failed: {}", e))?;

        let user = crate::get_username();

        let workflow_id = WorkflowSpec::create_from_validated_spec(
            &self.config,
            spec,
            &user,
            false, // enable_resource_monitoring
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(workflow_id)
    }
}
