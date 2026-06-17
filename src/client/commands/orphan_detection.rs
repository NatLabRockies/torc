//! Orphan detection and cleanup for Slurm workflows.
//!
//! This module provides shared logic for detecting and failing orphaned jobs
//! that are stuck in "running" status after their Slurm allocation terminated.
//!
//! Used by:
//! - `torc watch` - continuous monitoring with automatic orphan detection
//! - `torc recover` - pre-recovery cleanup before retrying failed jobs
//! - `torc workflows sync-status` - standalone cleanup command

use chrono::Utc;
use log::{debug, info, warn};
use serde::Serialize;

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::pagination::{
    ComputeNodeListParams, JobListParams, ScheduledComputeNodeListParams, paginate_compute_nodes,
    paginate_jobs, paginate_scheduled_compute_nodes,
};
use crate::client::hpc::common::HpcJobStatus;
use crate::client::hpc::hpc_interface::HpcInterface;
use crate::client::hpc::slurm_interface::SlurmInterface;
use crate::models;

/// Return code used when failing jobs orphaned by an ungraceful job runner termination.
/// This value (-128) is chosen to be:
/// - Negative, clearly distinguishing it from normal exit codes
/// - Related to signal convention (128 is the base for signal exits)
/// - Easy to identify in logs and results
pub const ORPHANED_JOB_RETURN_CODE: i64 = -128;

/// Result of orphan cleanup operation
#[derive(Debug, Clone, Serialize)]
pub struct OrphanCleanupResult {
    /// Number of jobs failed due to terminated Slurm allocations
    pub slurm_jobs_failed: usize,
    /// Number of pending Slurm allocations that were cleaned up
    pub pending_allocations_cleaned: usize,
    /// Number of running jobs failed due to no active compute nodes
    pub running_jobs_failed: usize,
    /// Number of compute nodes deactivated because their Slurm allocation is gone
    pub compute_nodes_deactivated: usize,
    /// Details of each orphaned job that was failed
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failed_job_details: Vec<OrphanedJobDetail>,
}

/// Details about an orphaned job that was failed
#[derive(Debug, Clone, Serialize)]
pub struct OrphanedJobDetail {
    pub job_id: i64,
    pub job_name: String,
    pub reason: String,
    pub slurm_job_id: Option<String>,
}

impl OrphanCleanupResult {
    /// Returns true if any cleanup was performed
    pub fn any_cleaned(&self) -> bool {
        self.slurm_jobs_failed > 0
            || self.pending_allocations_cleaned > 0
            || self.running_jobs_failed > 0
            || self.compute_nodes_deactivated > 0
    }

    /// Total number of jobs that were failed
    pub fn total_jobs_failed(&self) -> usize {
        self.slurm_jobs_failed + self.running_jobs_failed
    }
}

/// Detect and clean up orphaned jobs from terminated Slurm allocations.
///
/// This function performs four types of cleanup:
/// 1. Fails jobs from active scheduled compute nodes whose Slurm jobs are no longer running
/// 2. Cleans up pending scheduled compute nodes whose Slurm jobs were cancelled
/// 3. Fails running jobs that have no active compute nodes (fallback for non-Slurm)
/// 4. Deactivates compute nodes still marked active whose Slurm allocation is gone
///
/// If `dry_run` is true, reports what would be done without making changes.
pub fn cleanup_orphaned_jobs(
    config: &Configuration,
    workflow_id: i64,
    dry_run: bool,
) -> Result<OrphanCleanupResult, String> {
    let mut result = OrphanCleanupResult {
        slurm_jobs_failed: 0,
        pending_allocations_cleaned: 0,
        running_jobs_failed: 0,
        compute_nodes_deactivated: 0,
        failed_job_details: Vec::new(),
    };

    // Step 1: Check for orphaned Slurm jobs (active allocations that are no longer running)
    let (slurm_failed, slurm_details) = fail_orphaned_slurm_jobs(config, workflow_id, dry_run)?;
    result.slurm_jobs_failed = slurm_failed;
    result.failed_job_details.extend(slurm_details);

    // Step 2: Clean up dead pending Slurm jobs
    result.pending_allocations_cleaned =
        cleanup_dead_pending_slurm_jobs(config, workflow_id, dry_run)?;

    // Step 3: Fail orphaned running jobs (jobs stuck in running with no active compute nodes)
    // This is a fallback for non-Slurm schedulers or edge cases
    let (running_failed, running_details) =
        fail_orphaned_running_jobs(config, workflow_id, dry_run)?;
    result.running_jobs_failed = running_failed;
    result.failed_job_details.extend(running_details);

    // Step 4: Deactivate compute nodes still marked active whose Slurm allocation
    // is gone. This catches nodes stranded by an ungraceful job runner exit (e.g.
    // after `torc cancel` issues `scancel`), which Step 1 misses because it only
    // looks at scheduled compute nodes still in "active" status.
    result.compute_nodes_deactivated =
        deactivate_orphaned_compute_nodes(config, workflow_id, dry_run)?;

    Ok(result)
}

/// Detect and fail orphaned Slurm jobs by checking Slurm as the source of truth.
///
/// This function:
/// 1. Gets scheduled compute nodes with status="active" and scheduler_type="slurm"
/// 2. For each, uses SlurmInterface to check if the Slurm job is still running
/// 3. If not running, finds all compute nodes associated with that scheduled compute node
/// 4. Finds all jobs with active_compute_node_id matching those compute nodes
/// 5. Fails those jobs with the orphaned return code
///
/// Returns the number of jobs that were failed and details about each.
fn fail_orphaned_slurm_jobs(
    config: &Configuration,
    workflow_id: i64,
    dry_run: bool,
) -> Result<(usize, Vec<OrphanedJobDetail>), String> {
    // Get workflow to retrieve run_id
    let workflow = apis::workflows_api::get_workflow(config, workflow_id)
        .map_err(|e| format!("Failed to get workflow: {}", e))?;
    let run_id = workflow.run_id.unwrap_or(0);

    // Get all scheduled compute nodes with status="active" and scheduler_type="slurm"
    let scheduled_nodes = paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new().with_status("active".to_string()),
    )
    .map_err(|e| format!("Failed to list scheduled compute nodes: {}", e))?;

    // Filter for Slurm scheduler type
    let slurm_nodes: Vec<_> = scheduled_nodes
        .iter()
        .filter(|node| node.scheduler_type.to_lowercase() == "slurm")
        .collect();

    if slurm_nodes.is_empty() {
        return Ok((0, Vec::new()));
    }

    // Create SlurmInterface to check job status
    let slurm = match SlurmInterface::new() {
        Ok(s) => s,
        Err(e) => {
            warn!("Could not create SlurmInterface: {}", e);
            return Ok((0, Vec::new()));
        }
    };

    let mut total_failed = 0;
    let mut details = Vec::new();

    for scheduled_node in slurm_nodes {
        let slurm_job_id = scheduled_node.scheduler_id.to_string();
        let scheduled_compute_node_id = match scheduled_node.id {
            Some(id) => id,
            None => continue,
        };

        // Check Slurm status
        let slurm_status = match slurm.get_status(&slurm_job_id) {
            Ok(info) => info.status,
            Err(e) => {
                warn!(
                    "Error checking Slurm status for job {}: {}",
                    slurm_job_id, e
                );
                continue;
            }
        };

        // If Slurm job is still running or queued, skip it
        if slurm_status == HpcJobStatus::Running || slurm_status == HpcJobStatus::Queued {
            continue;
        }

        // Slurm job is not running (Complete, Unknown, or None means it's gone)
        info!(
            "Slurm job {} is no longer running (status: {:?}), checking for orphaned jobs",
            slurm_job_id, slurm_status
        );

        // Find all compute nodes associated with this scheduled compute node
        let compute_nodes = paginate_compute_nodes(
            config,
            workflow_id,
            ComputeNodeListParams::new().with_scheduled_compute_node_id(scheduled_compute_node_id),
        )
        .map_err(|e| format!("Failed to list compute nodes: {}", e))?;

        for compute_node in &compute_nodes {
            let compute_node_id = match compute_node.id {
                Some(id) => id,
                None => continue,
            };

            // Find all jobs with this active_compute_node_id
            let orphaned_jobs = paginate_jobs(
                config,
                workflow_id,
                JobListParams::new().with_active_compute_node_id(compute_node_id),
            )
            .map_err(|e| format!("Failed to list jobs for compute node: {}", e))?;

            if orphaned_jobs.is_empty() {
                continue;
            }

            let action = if dry_run { "Would fail" } else { "Found" };
            info!(
                "{} {} orphaned job(s) from Slurm job {} (compute node {})",
                action,
                orphaned_jobs.len(),
                slurm_job_id,
                compute_node_id
            );

            // Fail each orphaned job
            for job in &orphaned_jobs {
                let job_id = match job.id {
                    Some(id) => id,
                    None => continue,
                };

                let reason = format!("Slurm job {} no longer running", slurm_job_id);
                details.push(OrphanedJobDetail {
                    job_id,
                    job_name: job.name.clone(),
                    reason: reason.clone(),
                    slurm_job_id: Some(slurm_job_id.clone()),
                });

                if dry_run {
                    info!(
                        "  [DRY RUN] Would mark orphaned job {} ({}) as failed",
                        job_id, job.name
                    );
                    total_failed += 1;
                    continue;
                }

                // Create a result for the orphaned job
                let attempt_id = job.attempt_id.unwrap_or(1);
                let result = models::ResultModel::new(
                    job_id,
                    workflow_id,
                    run_id,
                    attempt_id,
                    compute_node_id,
                    ORPHANED_JOB_RETURN_CODE,
                    0.0,
                    Utc::now().to_rfc3339(),
                    models::JobStatus::Failed,
                );

                // Mark the job as failed
                match apis::jobs_api::complete_job(
                    config,
                    job_id,
                    models::JobStatus::Failed,
                    run_id,
                    result,
                ) {
                    Ok(_) => {
                        info!(
                            "  Marked orphaned job {} ({}) as failed (Slurm job {} no longer running)",
                            job_id, job.name, slurm_job_id
                        );
                        total_failed += 1;
                    }
                    Err(e) => {
                        warn!("  Failed to mark job {} as failed: {}", job_id, e);
                    }
                }
            }

            // Compute-node deactivation is handled centrally by Step 4
            // (`deactivate_orphaned_compute_nodes`) so that every deactivation is
            // counted in `OrphanCleanupResult::compute_nodes_deactivated`. Step 4
            // runs after this step and sweeps the still-active node here.
        }

        if !dry_run {
            // Update the scheduled compute node status to "complete" since the Slurm job is done
            match apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
                config,
                scheduled_compute_node_id,
                models::ScheduledComputeNodesModel::new(
                    workflow_id,
                    scheduled_node.scheduler_id,
                    scheduled_node.scheduler_config_id,
                    scheduled_node.scheduler_type.clone(),
                    "complete".to_string(),
                ),
            ) {
                Ok(_) => {
                    info!(
                        "Updated scheduled compute node {} status to 'complete'",
                        scheduled_compute_node_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to update scheduled compute node {} status: {}",
                        scheduled_compute_node_id, e
                    );
                }
            }
        }
    }

    if total_failed > 0 {
        let action = if dry_run { "Would mark" } else { "Marked" };
        info!(
            "{} {} orphaned Slurm job(s) as failed (return code {})",
            action, total_failed, ORPHANED_JOB_RETURN_CODE
        );
    }

    Ok((total_failed, details))
}

/// Check for pending Slurm jobs that no longer exist and mark them as complete.
///
/// This handles the case where a Slurm job was submitted but cancelled or failed
/// before it ever started running. In this scenario:
/// - The ScheduledComputeNode remains in "pending" status
/// - The Slurm job no longer exists in the queue
///
/// Returns the number of pending nodes that were cleaned up.
fn cleanup_dead_pending_slurm_jobs(
    config: &Configuration,
    workflow_id: i64,
    dry_run: bool,
) -> Result<usize, String> {
    // Get all scheduled compute nodes with status="pending"
    let scheduled_nodes = paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new().with_status("pending".to_string()),
    )
    .map_err(|e| format!("Failed to list pending scheduled compute nodes: {}", e))?;

    // Filter for Slurm scheduler type
    let slurm_nodes: Vec<_> = scheduled_nodes
        .iter()
        .filter(|node| node.scheduler_type.to_lowercase() == "slurm")
        .collect();

    if slurm_nodes.is_empty() {
        return Ok(0);
    }

    // Create SlurmInterface to check job status
    let slurm = match SlurmInterface::new() {
        Ok(s) => s,
        Err(e) => {
            debug!(
                "Could not create SlurmInterface for pending job check: {}",
                e
            );
            return Ok(0);
        }
    };

    let mut total_cleaned = 0;

    for scheduled_node in slurm_nodes {
        let slurm_job_id = scheduled_node.scheduler_id.to_string();
        let scheduled_compute_node_id = match scheduled_node.id {
            Some(id) => id,
            None => continue,
        };

        // Check Slurm status
        let slurm_status = match slurm.get_status(&slurm_job_id) {
            Ok(info) => info.status,
            Err(e) => {
                debug!(
                    "Error checking Slurm status for pending job {}: {}",
                    slurm_job_id, e
                );
                continue;
            }
        };

        // If Slurm job is still queued or running, skip it (it's still valid)
        if slurm_status == HpcJobStatus::Queued || slurm_status == HpcJobStatus::Running {
            continue;
        }

        // If the job completed normally, it will transition through the normal path
        // We only care about jobs that no longer exist (None/Unknown)
        if slurm_status == HpcJobStatus::Complete {
            // Job completed but never started running in our system - this is unusual
            // but we should mark it as complete so it doesn't block
            info!(
                "Slurm job {} completed but was still pending in our system, marking as complete",
                slurm_job_id
            );
        } else {
            // Job no longer exists (None/Unknown) - was cancelled or failed before starting
            info!(
                "Pending Slurm job {} no longer exists (status: {:?}), marking as complete",
                slurm_job_id, slurm_status
            );
        }

        if dry_run {
            info!(
                "[DRY RUN] Would mark pending scheduled compute node {} (Slurm job {}) as complete",
                scheduled_compute_node_id, slurm_job_id
            );
            total_cleaned += 1;
            continue;
        }

        // Update the scheduled compute node status to "complete"
        match apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
            config,
            scheduled_compute_node_id,
            models::ScheduledComputeNodesModel::new(
                workflow_id,
                scheduled_node.scheduler_id,
                scheduled_node.scheduler_config_id,
                scheduled_node.scheduler_type.clone(),
                "complete".to_string(),
            ),
        ) {
            Ok(_) => {
                info!(
                    "Updated pending scheduled compute node {} (Slurm job {}) status to 'complete'",
                    scheduled_compute_node_id, slurm_job_id
                );
                total_cleaned += 1;
            }
            Err(e) => {
                warn!(
                    "Failed to update scheduled compute node {} status: {}",
                    scheduled_compute_node_id, e
                );
            }
        }
    }

    if total_cleaned > 0 {
        let action = if dry_run {
            "Would clean up"
        } else {
            "Cleaned up"
        };
        info!("{} {} dead pending Slurm job(s)", action, total_cleaned);
    }

    Ok(total_cleaned)
}

/// Detect and fail orphaned running jobs.
///
/// This handles the case where a job runner (e.g., torc-slurm-job-runner) was killed
/// ungracefully by the scheduler (e.g., Slurm). In this scenario:
/// - Jobs claimed by the runner remain in "running" status
/// - The ScheduledComputeNode remains in "active" status
/// - No active compute nodes exist to process the jobs
///
/// This is a fallback for non-Slurm schedulers or edge cases where the Slurm-specific
/// detection didn't catch the orphaned jobs.
///
/// Returns the number of jobs that were failed and details about each.
fn fail_orphaned_running_jobs(
    config: &Configuration,
    workflow_id: i64,
    dry_run: bool,
) -> Result<(usize, Vec<OrphanedJobDetail>), String> {
    // Get workflow to retrieve run_id
    let workflow = apis::workflows_api::get_workflow(config, workflow_id)
        .map_err(|e| format!("Failed to get workflow: {}", e))?;
    let run_id = workflow.run_id.unwrap_or(0);

    // Check for active compute nodes
    let active_nodes_response = apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,       // offset
        Some(1),    // limit - we only need to know if any exist
        None,       // sort_by
        None,       // reverse_sort
        None,       // hostname
        Some(true), // is_active = true
        None,       // scheduled_compute_node_id
    )
    .map_err(|e| format!("Failed to list active compute nodes: {}", e))?;

    let active_node_count = active_nodes_response.total_count;

    // If there are active compute nodes, jobs are being processed normally
    if active_node_count > 0 {
        return Ok((0, Vec::new()));
    }

    // Get all jobs with status=Running
    let running_jobs = paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_status(models::JobStatus::Running),
    )
    .map_err(|e| format!("Failed to list running jobs: {}", e))?;

    if running_jobs.is_empty() {
        return Ok((0, Vec::new()));
    }

    let action = if dry_run { "Would fail" } else { "Detected" };
    info!(
        "{} {} orphaned running job(s) with no active compute nodes",
        action,
        running_jobs.len()
    );

    if dry_run {
        let details: Vec<OrphanedJobDetail> = running_jobs
            .iter()
            .filter_map(|job| {
                let job_id = job.id?;
                info!(
                    "  [DRY RUN] Would mark orphaned job {} ({}) as failed",
                    job_id, job.name
                );
                Some(OrphanedJobDetail {
                    job_id,
                    job_name: job.name.clone(),
                    reason: "No active compute nodes".to_string(),
                    slurm_job_id: None,
                })
            })
            .collect();
        return Ok((details.len(), details));
    }

    // Get or create a compute node for recording the failure
    // First, try to find any existing compute node for this workflow
    let compute_node_id = match apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,    // offset
        Some(1), // limit
        None,    // sort_by
        None,    // reverse_sort
        None,    // hostname
        None,    // is_active - any status
        None,    // scheduled_compute_node_id
    ) {
        Ok(response) => response.items.first().and_then(|node| node.id).unwrap_or(0),
        Err(_) => 0,
    };

    // If no compute node exists, create a recovery node
    let compute_node_id = if compute_node_id == 0 {
        match apis::compute_nodes_api::create_compute_node(
            config,
            models::ComputeNodeModel::new(
                workflow_id,
                "orphan-recovery".to_string(),
                0, // pid
                Utc::now().to_rfc3339(),
                1,   // num_cpus
                1.0, // memory_gb
                0,   // num_gpus
                1,   // num_nodes
                "local".to_string(),
                None, // scheduler
            ),
        ) {
            Ok(node) => node.id.unwrap_or(0),
            Err(e) => {
                warn!("Could not create recovery compute node: {}", e);
                0
            }
        }
    } else {
        compute_node_id
    };

    let mut failed_count = 0;
    let mut details = Vec::new();

    for job in &running_jobs {
        let job_id = match job.id {
            Some(id) => id,
            None => continue,
        };

        details.push(OrphanedJobDetail {
            job_id,
            job_name: job.name.clone(),
            reason: "No active compute nodes".to_string(),
            slurm_job_id: None,
        });

        // Create a result for the orphaned job
        let attempt_id = job.attempt_id.unwrap_or(1);
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            run_id,
            attempt_id,
            compute_node_id,
            ORPHANED_JOB_RETURN_CODE, // Unique return code for orphaned jobs
            0.0,                      // exec_time_minutes - unknown
            Utc::now().to_rfc3339(),  // completion_time
            models::JobStatus::Failed, // status
        );

        // Mark the job as failed
        match apis::jobs_api::complete_job(
            config,
            job_id,
            models::JobStatus::Failed,
            run_id,
            result,
        ) {
            Ok(_) => {
                info!(
                    "  Marked orphaned job {} ({}) as failed with return code {}",
                    job_id, job.name, ORPHANED_JOB_RETURN_CODE
                );
                failed_count += 1;
            }
            Err(e) => {
                warn!("  Failed to mark job {} as failed: {}", job_id, e);
            }
        }
    }

    if failed_count > 0 {
        info!(
            "Marked {} orphaned job(s) as failed (return code {})",
            failed_count, ORPHANED_JOB_RETURN_CODE
        );
    }

    Ok((failed_count, details))
}

/// Deactivate compute nodes that are still marked `is_active = true` but whose
/// Slurm allocation is no longer running.
///
/// This handles compute nodes stranded by an ungraceful job runner exit: when a
/// runner is killed by `scancel` (via `torc cancel`) or a Slurm timeout, it never
/// reaches its own deactivation path, so the `ComputeNode` row stays active
/// forever and blocks `torc recover`.
///
/// Unlike [`fail_orphaned_slurm_jobs`], this does not require the scheduled
/// compute node to still be in "active" status (a cancel moves it to "canceled")
/// and does not require any jobs to still be in "running" status. It walks every
/// Slurm-type scheduled compute node regardless of status, and for those whose
/// Slurm job is gone, deactivates the associated active compute nodes.
///
/// Returns the number of compute nodes that were deactivated.
fn deactivate_orphaned_compute_nodes(
    config: &Configuration,
    workflow_id: i64,
    dry_run: bool,
) -> Result<usize, String> {
    // Cheap early-out: nothing to do if no compute nodes are active.
    let active_nodes = paginate_compute_nodes(
        config,
        workflow_id,
        ComputeNodeListParams::new()
            .with_is_active(true)
            .with_limit(1),
    )
    .map_err(|e| format!("Failed to list active compute nodes: {}", e))?;
    if active_nodes.is_empty() {
        return Ok(0);
    }

    // Slurm is the source of truth. If we can't talk to it, do nothing.
    let slurm = match SlurmInterface::new() {
        Ok(s) => s,
        Err(e) => {
            debug!(
                "Could not create SlurmInterface for compute node sweep: {}",
                e
            );
            return Ok(0);
        }
    };

    // Walk every Slurm-type scheduled compute node, regardless of status.
    let scheduled_nodes = paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new(),
    )
    .map_err(|e| format!("Failed to list scheduled compute nodes: {}", e))?;

    let mut total_deactivated = 0;

    for scheduled_node in scheduled_nodes
        .iter()
        .filter(|node| node.scheduler_type.to_lowercase() == "slurm")
    {
        let scheduled_compute_node_id = match scheduled_node.id {
            Some(id) => id,
            None => continue,
        };
        let slurm_job_id = scheduled_node.scheduler_id.to_string();

        // Skip allocations that are still alive in Slurm.
        match slurm.get_status(&slurm_job_id) {
            Ok(info)
                if info.status == HpcJobStatus::Running || info.status == HpcJobStatus::Queued =>
            {
                continue;
            }
            Ok(_) => {}
            Err(e) => {
                warn!(
                    "Error checking Slurm status for job {}: {}; leaving compute nodes untouched",
                    slurm_job_id, e
                );
                continue;
            }
        }

        match deactivate_compute_nodes_for_scheduled_node(
            config,
            workflow_id,
            scheduled_compute_node_id,
            &slurm_job_id,
            dry_run,
        ) {
            Ok(count) => total_deactivated += count,
            Err(e) => warn!(
                "Failed to deactivate compute nodes for scheduled compute node {}: {}",
                scheduled_compute_node_id, e
            ),
        }
    }

    if total_deactivated > 0 {
        let action = if dry_run {
            "Would deactivate"
        } else {
            "Deactivated"
        };
        info!(
            "{} {} orphaned compute node(s) whose Slurm allocation is gone",
            action, total_deactivated
        );
    }

    Ok(total_deactivated)
}

/// Mark all still-active compute nodes associated with a scheduled compute node
/// as inactive.
///
/// Used by [`deactivate_orphaned_compute_nodes`] during cleanup, and by
/// `torc cancel` after `scancel` (the job runner is killed and never deactivates
/// itself). `slurm_job_id` is used only for logging context.
///
/// Returns the number of compute nodes that were deactivated.
pub fn deactivate_compute_nodes_for_scheduled_node(
    config: &Configuration,
    workflow_id: i64,
    scheduled_compute_node_id: i64,
    slurm_job_id: &str,
    dry_run: bool,
) -> Result<usize, String> {
    let compute_nodes = paginate_compute_nodes(
        config,
        workflow_id,
        ComputeNodeListParams::new().with_scheduled_compute_node_id(scheduled_compute_node_id),
    )
    .map_err(|e| format!("Failed to list compute nodes: {}", e))?;

    let mut count = 0;

    for compute_node in &compute_nodes {
        // Only touch nodes that are still marked active.
        if compute_node.is_active != Some(true) {
            continue;
        }
        let compute_node_id = match compute_node.id {
            Some(id) => id,
            None => continue,
        };

        if dry_run {
            info!(
                "[DRY RUN] Would deactivate compute node {} (Slurm job {} no longer running)",
                compute_node_id, slurm_job_id
            );
            count += 1;
            continue;
        }

        let mut updated_node = compute_node.clone();
        updated_node.is_active = Some(false);
        apis::compute_nodes_api::update_compute_node(config, compute_node_id, updated_node)
            .map_err(|e| {
                format!(
                    "Failed to deactivate compute node {}: {}",
                    compute_node_id, e
                )
            })?;

        info!(
            "Deactivated compute node {} (Slurm job {} no longer running)",
            compute_node_id, slurm_job_id
        );
        count += 1;
    }

    Ok(count)
}

/// Returns true if the workflow still has work that an allocation could run.
///
/// "Runnable" means jobs in Ready, Pending, or Running status. Blocked jobs are
/// not runnable on their own: with no Ready/Pending/Running job left to complete
/// and unblock them, a queued allocation would have nothing to do.
fn has_runnable_jobs(counts: &models::JobStatusCounts) -> bool {
    counts.ready > 0 || counts.pending > 0 || counts.running > 0
}

/// Cancel queued Slurm allocations that the workflow no longer needs.
///
/// Addresses the "many small allocations" case: a workflow opens
/// several Slurm allocations but finishes all of its work inside the first few,
/// leaving the rest sitting in the Slurm queue with nothing left to run. The
/// standard orphan cleanup never touches them because Slurm still reports them
/// as `Queued` (i.e. valid), so they wait until they start or are canceled by
/// hand.
///
/// This cancels pending allocations only when the workflow has no runnable jobs
/// left (no Ready/Pending/Running jobs). For each pending Slurm scheduled
/// compute node still `Queued` in Slurm it issues `scancel`, marks the scheduled
/// compute node `canceled`, and deactivates any associated compute nodes (the
/// same bookkeeping `torc cancel` performs). Allocations that have already
/// started (`Running`) are left alone -- their job runner detects there is no
/// work and exits gracefully on its own; allocations already gone from Slurm are
/// handled by [`cleanup_dead_pending_slurm_jobs`].
///
/// If `dry_run` is true, reports what would be done without making changes.
/// Returns the number of allocations canceled.
pub fn cancel_unneeded_pending_allocations(
    config: &Configuration,
    workflow_id: i64,
    dry_run: bool,
) -> Result<usize, String> {
    // Cheap guard: if any runnable jobs remain, queued allocations may still be
    // needed, so bail before doing any per-allocation squeue work.
    let status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .map_err(|e| format!("Failed to get workflow status: {}", e))?;
    if has_runnable_jobs(&status.jobs_by_status) {
        return Ok(0);
    }

    // Enumerate pending Slurm allocations.
    let scheduled_nodes = paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new().with_status("pending".to_string()),
    )
    .map_err(|e| format!("Failed to list pending scheduled compute nodes: {}", e))?;

    let slurm_nodes: Vec<_> = scheduled_nodes
        .iter()
        .filter(|node| node.scheduler_type.to_lowercase() == "slurm")
        .collect();

    if slurm_nodes.is_empty() {
        return Ok(0);
    }

    // Slurm is the source of truth. If we can't talk to it, do nothing.
    let slurm = match SlurmInterface::new() {
        Ok(s) => s,
        Err(e) => {
            debug!(
                "Could not create SlurmInterface to cancel unneeded allocations: {}",
                e
            );
            return Ok(0);
        }
    };

    let mut total_canceled = 0;
    // Slurm job IDs actually canceled, used to record a single aggregate event below.
    let mut canceled_slurm_job_ids: Vec<String> = Vec::new();

    for scheduled_node in slurm_nodes {
        let slurm_job_id = scheduled_node.scheduler_id.to_string();
        let scheduled_compute_node_id = match scheduled_node.id {
            Some(id) => id,
            None => continue,
        };

        // Only cancel allocations still waiting in the queue. A Running
        // allocation already has a job runner attached that exits cleanly once
        // it sees there is no work; a job gone from Slurm is handled elsewhere.
        match slurm.get_status(&slurm_job_id) {
            Ok(info) if info.status == HpcJobStatus::Queued => {}
            Ok(_) => continue,
            Err(e) => {
                debug!(
                    "Error checking Slurm status for pending job {}: {}; leaving it untouched",
                    slurm_job_id, e
                );
                continue;
            }
        }

        if dry_run {
            info!(
                "[DRY RUN] Would cancel unneeded queued Slurm allocation {} (no runnable jobs remain) workflow_id={}",
                slurm_job_id, workflow_id
            );
            total_canceled += 1;
            continue;
        }

        // Cancel the queued allocation in Slurm.
        match slurm.cancel_job(&slurm_job_id) {
            Ok(0) => {
                info!(
                    "Canceled unneeded queued Slurm allocation {} (no runnable jobs remain) workflow_id={}",
                    slurm_job_id, workflow_id
                );
                canceled_slurm_job_ids.push(slurm_job_id.clone());
            }
            Ok(code) => {
                warn!(
                    "scancel for Slurm job {} returned non-zero code {}; skipping status update",
                    slurm_job_id, code
                );
                continue;
            }
            Err(e) => {
                warn!("Failed to cancel Slurm job {}: {}", slurm_job_id, e);
                continue;
            }
        }

        // Mark the scheduled compute node canceled so the watch loop stops
        // treating it as a live allocation.
        if let Err(e) = apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
            config,
            scheduled_compute_node_id,
            models::ScheduledComputeNodesModel::new(
                workflow_id,
                scheduled_node.scheduler_id,
                scheduled_node.scheduler_config_id,
                scheduled_node.scheduler_type.clone(),
                "canceled".to_string(),
            ),
        ) {
            warn!(
                "Failed to update scheduled compute node {} status to canceled: {}",
                scheduled_compute_node_id, e
            );
        }

        // `scancel` kills any job runner ungracefully, so deactivate associated
        // compute nodes to avoid stranding is_active=true rows (mirrors `torc
        // cancel`). A still-queued allocation usually has none, but this keeps
        // the bookkeeping consistent.
        if let Err(e) = deactivate_compute_nodes_for_scheduled_node(
            config,
            workflow_id,
            scheduled_compute_node_id,
            &slurm_job_id,
            dry_run,
        ) {
            warn!(
                "Failed to deactivate compute nodes for scheduled compute node {}: {}",
                scheduled_compute_node_id, e
            );
        }

        total_canceled += 1;
    }

    if total_canceled > 0 {
        let action = if dry_run { "Would cancel" } else { "Canceled" };
        info!(
            "{} {} unneeded queued Slurm allocation(s) workflow_id={}",
            action, total_canceled, workflow_id
        );
    }

    // Record a single aggregate event for the whole batch of cancellations (not one
    // per allocation). This surfaces in `torc events` so users can notice they are
    // scheduling more allocations than the workflow needs. Dry runs make no changes,
    // so they record nothing.
    if !canceled_slurm_job_ids.is_empty() {
        let data = serde_json::json!({
            "category": "scheduler",
            "action": "cancel_unneeded_pending_allocations",
            "message": format!(
                "Canceled {} unneeded queued Slurm allocation(s) because no runnable jobs remained",
                canceled_slurm_job_ids.len()
            ),
            "num_canceled": canceled_slurm_job_ids.len(),
            "slurm_job_ids": canceled_slurm_job_ids,
        });
        let event = models::EventModel::new(workflow_id, data);
        if let Err(e) = apis::events_api::create_event(config, event) {
            warn!(
                "Failed to record canceled-allocations event workflow_id={}: {}",
                workflow_id, e
            );
        }
    }

    Ok(total_canceled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_counts() -> models::JobStatusCounts {
        models::JobStatusCounts {
            uninitialized: 0,
            blocked: 0,
            ready: 0,
            pending: 0,
            running: 0,
            completed: 0,
            failed: 0,
            canceled: 0,
            terminated: 0,
            disabled: 0,
            pending_failed: 0,
        }
    }

    #[test]
    fn test_has_runnable_jobs() {
        // No jobs of any status -> nothing runnable.
        assert!(!has_runnable_jobs(&empty_counts()));

        // Blocked/completed/failed jobs are not runnable on their own.
        let mut counts = empty_counts();
        counts.blocked = 5;
        counts.completed = 10;
        counts.failed = 2;
        counts.canceled = 1;
        assert!(!has_runnable_jobs(&counts));

        // Any of Ready / Pending / Running counts as runnable.
        let mut counts = empty_counts();
        counts.ready = 1;
        assert!(has_runnable_jobs(&counts));

        let mut counts = empty_counts();
        counts.pending = 1;
        assert!(has_runnable_jobs(&counts));

        let mut counts = empty_counts();
        counts.running = 1;
        assert!(has_runnable_jobs(&counts));
    }

    #[test]
    fn test_orphan_cleanup_result_any_cleaned() {
        let empty = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 0,
            compute_nodes_deactivated: 0,
            failed_job_details: Vec::new(),
        };
        assert!(!empty.any_cleaned());

        let with_slurm = OrphanCleanupResult {
            slurm_jobs_failed: 1,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 0,
            compute_nodes_deactivated: 0,
            failed_job_details: Vec::new(),
        };
        assert!(with_slurm.any_cleaned());

        let with_pending = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 1,
            running_jobs_failed: 0,
            compute_nodes_deactivated: 0,
            failed_job_details: Vec::new(),
        };
        assert!(with_pending.any_cleaned());

        let with_running = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 1,
            compute_nodes_deactivated: 0,
            failed_job_details: Vec::new(),
        };
        assert!(with_running.any_cleaned());

        let with_deactivated = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 0,
            compute_nodes_deactivated: 1,
            failed_job_details: Vec::new(),
        };
        assert!(with_deactivated.any_cleaned());
    }

    #[test]
    fn test_orphan_cleanup_result_total_jobs_failed() {
        let result = OrphanCleanupResult {
            slurm_jobs_failed: 3,
            pending_allocations_cleaned: 2,
            running_jobs_failed: 1,
            compute_nodes_deactivated: 5,
            failed_job_details: Vec::new(),
        };
        assert_eq!(result.total_jobs_failed(), 4);
    }
}
