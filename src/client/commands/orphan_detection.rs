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
    }

    /// Total number of jobs that were failed
    pub fn total_jobs_failed(&self) -> usize {
        self.slurm_jobs_failed + self.running_jobs_failed
    }
}

/// Detect and clean up orphaned jobs from terminated Slurm allocations.
///
/// This function performs three types of cleanup:
/// 1. Fails jobs from active scheduled compute nodes whose Slurm jobs are no longer running
/// 2. Cleans up pending scheduled compute nodes whose Slurm jobs were cancelled
/// 3. Fails running jobs that have no active compute nodes (fallback for non-Slurm)
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
    // Get workflow status to retrieve run_id
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .map_err(|e| format!("Failed to get workflow status: {}", e))?;
    let run_id = workflow_status.run_id;

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

            if !dry_run {
                // Mark this compute node as inactive since its Slurm job is gone
                let mut updated_node = compute_node.clone();
                updated_node.is_active = Some(false);
                match apis::compute_nodes_api::update_compute_node(
                    config,
                    compute_node_id,
                    updated_node,
                ) {
                    Ok(_) => {
                        debug!(
                            "Marked compute node {} as inactive (Slurm job {} no longer running)",
                            compute_node_id, slurm_job_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to mark compute node {} as inactive: {}",
                            compute_node_id, e
                        );
                    }
                }
            }
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
    // Get workflow status to retrieve run_id
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .map_err(|e| format!("Failed to get workflow status: {}", e))?;
    let run_id = workflow_status.run_id;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orphan_cleanup_result_any_cleaned() {
        let empty = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 0,
            failed_job_details: Vec::new(),
        };
        assert!(!empty.any_cleaned());

        let with_slurm = OrphanCleanupResult {
            slurm_jobs_failed: 1,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 0,
            failed_job_details: Vec::new(),
        };
        assert!(with_slurm.any_cleaned());

        let with_pending = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 1,
            running_jobs_failed: 0,
            failed_job_details: Vec::new(),
        };
        assert!(with_pending.any_cleaned());

        let with_running = OrphanCleanupResult {
            slurm_jobs_failed: 0,
            pending_allocations_cleaned: 0,
            running_jobs_failed: 1,
            failed_job_details: Vec::new(),
        };
        assert!(with_running.any_cleaned());
    }

    #[test]
    fn test_orphan_cleanup_result_total_jobs_failed() {
        let result = OrphanCleanupResult {
            slurm_jobs_failed: 3,
            pending_allocations_cleaned: 2,
            running_jobs_failed: 1,
            failed_job_details: Vec::new(),
        };
        assert_eq!(result.total_jobs_failed(), 4);
    }
}
