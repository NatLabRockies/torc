//! Shared workflow cancellation logic.
//!
//! Canceling a workflow has two parts: telling the server to cancel the workflow (which
//! cancels its jobs) and canceling any outstanding scheduler allocations. Skipping the
//! second part leaves Slurm allocations queued; when one starts it finds no runnable jobs
//! and exits immediately.
//!
//! Both `torc cancel` and the TUI call [`cancel_scheduler_allocations`] so they behave
//! identically.

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::orphan_detection::deactivate_compute_nodes_for_scheduled_node;
use crate::client::commands::pagination::{
    ScheduledComputeNodeListParams, paginate_scheduled_compute_nodes,
};
use crate::client::hpc::hpc_interface::HpcInterface;
use crate::client::hpc::slurm_interface::SlurmInterface;
use crate::models;

/// Error returned when the scheduled compute nodes for a workflow cannot be listed.
pub type ListScheduledNodesError =
    apis::Error<apis::scheduled_compute_nodes_api::ListScheduledComputeNodesError>;

/// A progress message emitted while canceling scheduler allocations.
///
/// Callers decide how to surface these; `torc cancel` prints them, the TUI ignores them
/// and reports the summary in [`CanceledAllocations`] instead.
pub enum CancelProgress<'a> {
    /// Something succeeded (an allocation was canceled, a status was updated).
    Info(&'a str),
    /// A non-fatal failure. The same text is also recorded in [`CanceledAllocations::errors`].
    Error(&'a str),
}

/// Summary of what happened while canceling a workflow's scheduler allocations.
#[derive(Debug, Default)]
pub struct CanceledAllocations {
    /// Scheduler job IDs that were canceled.
    pub canceled_slurm_jobs: Vec<i64>,
    /// Number of compute nodes deactivated after their allocation was canceled.
    pub deactivated_compute_nodes: usize,
    /// Non-fatal errors encountered along the way.
    pub errors: Vec<String>,
}

/// Cancel every Slurm allocation associated with `workflow_id`.
///
/// For each canceled allocation the scheduled compute node is marked `canceled` and its
/// compute nodes are deactivated -- `scancel` kills the job runner ungracefully, so it never
/// deactivates its own compute node, and the leftover `is_active=true` rows block
/// `torc recover`.
///
/// This does not cancel the workflow itself; call `apis::workflows_api::cancel_workflow`
/// first. Per-allocation failures are collected in the returned [`CanceledAllocations`]
/// rather than aborting the sweep; only a failure to list the workflow's scheduled compute
/// nodes is returned as an error.
#[allow(clippy::result_large_err)]
pub fn cancel_scheduler_allocations(
    config: &Configuration,
    workflow_id: i64,
    progress: &mut dyn FnMut(CancelProgress<'_>),
) -> Result<CanceledAllocations, ListScheduledNodesError> {
    let nodes = paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new(),
    )?;

    let mut outcome = CanceledAllocations::default();

    let slurm_nodes: Vec<models::ScheduledComputeNodesModel> = nodes
        .into_iter()
        .filter(|node| node.scheduler_type == "slurm")
        .collect();
    if slurm_nodes.is_empty() {
        return Ok(outcome);
    }

    let slurm_interface = match SlurmInterface::new() {
        Ok(interface) => interface,
        Err(e) => {
            record_error(
                &mut outcome,
                format!("Failed to create SlurmInterface: {}", e),
                progress,
            );
            return Ok(outcome);
        }
    };

    for node in slurm_nodes {
        if let Err(e) = slurm_interface.cancel_job(&node.scheduler_id.to_string()) {
            record_error(
                &mut outcome,
                format!("Failed to cancel Slurm job {}: {}", node.scheduler_id, e),
                progress,
            );
            continue;
        }

        outcome.canceled_slurm_jobs.push(node.scheduler_id);
        progress(CancelProgress::Info(&format!(
            "Canceled Slurm job: {}",
            node.scheduler_id
        )));

        let Some(node_id) = node.id else {
            continue;
        };

        let updated_node = models::ScheduledComputeNodesModel::new(
            node.workflow_id,
            node.scheduler_id,
            node.scheduler_config_id,
            node.scheduler_type.clone(),
            "canceled".to_string(),
        );
        match apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
            config,
            node_id,
            updated_node,
        ) {
            Ok(_) => progress(CancelProgress::Info(&format!(
                "Updated node {} status to canceled",
                node.scheduler_id
            ))),
            Err(e) => record_error(
                &mut outcome,
                format!("Failed to update node {} status: {}", node_id, e),
                progress,
            ),
        }

        match deactivate_compute_nodes_for_scheduled_node(
            config,
            node.workflow_id,
            node_id,
            &node.scheduler_id.to_string(),
            false,
        ) {
            Ok(count) => {
                outcome.deactivated_compute_nodes += count;
                if count > 0 {
                    progress(CancelProgress::Info(&format!(
                        "Deactivated {} compute node(s) for node {}",
                        count, node.scheduler_id
                    )));
                }
            }
            Err(e) => record_error(
                &mut outcome,
                format!(
                    "Failed to deactivate compute nodes for node {}: {}",
                    node.scheduler_id, e
                ),
                progress,
            ),
        }
    }

    Ok(outcome)
}

fn record_error(
    outcome: &mut CanceledAllocations,
    message: String,
    progress: &mut dyn FnMut(CancelProgress<'_>),
) {
    progress(CancelProgress::Error(&message));
    outcome.errors.push(message);
}
