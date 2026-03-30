//! Shared recovery functionality for Slurm workflows.
//!
//! This module provides the core recovery logic used by both:
//! - `torc recover` standalone command
//! - `torc watch --recover` automatic recovery

use log::{debug, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::reports::{build_resource_utilization_report, build_results_report};
use crate::client::commands::slurm::RegenerateDryRunResult;
use crate::client::report_models::{ResourceUtilizationReport, ResultsReport};
use crate::client::resource_correction::{
    ResourceAdjustmentReport, ResourceCorrectionContext, ResourceCorrectionOptions,
    apply_resource_corrections,
};
use crate::client::workflow_manager::WorkflowManager;
use crate::config::TorcConfig;
use crate::models::JobStatus;

fn torc_command() -> Result<Command, String> {
    if let Ok(path) = std::env::var("TORC_BIN")
        && !path.trim().is_empty()
    {
        return Ok(Command::new(path));
    }

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to determine current torc executable: {}", e))?;
    Ok(Command::new(current_exe))
}

/// Arguments for workflow recovery
pub struct RecoverArgs {
    pub workflow_id: i64,
    pub output_dir: PathBuf,
    pub memory_multiplier: f64,
    pub runtime_multiplier: f64,
    pub retry_unknown: bool,
    pub recovery_hook: Option<String>,
    pub dry_run: bool,
    /// Run the interactive recovery wizard (default when stdin is a TTY)
    pub interactive: bool,
    /// [EXPERIMENTAL] Enable AI-assisted recovery for pending_failed jobs
    pub ai_recovery: bool,
    /// AI agent CLI to use for --ai-recovery (e.g., "claude")
    pub ai_agent: String,
}

/// Result of applying recovery heuristics
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryResult {
    pub oom_fixed: usize,
    pub timeout_fixed: usize,
    pub unknown_retried: usize,
    pub other_failures: usize,
    pub jobs_to_retry: Vec<i64>,
    /// Detailed resource adjustments (for JSON output)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<ResourceAdjustmentReport>,
    /// Slurm scheduler dry-run result (only in dry-run mode)
    /// Memory values are updated to reflect the adjusted values from recovery heuristics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_dry_run: Option<RegenerateDryRunResult>,
}

/// Full recovery report for JSON output
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryReport {
    pub workflow_id: i64,
    pub dry_run: bool,
    pub memory_multiplier: f64,
    pub runtime_multiplier: f64,
    pub result: RecoveryResult,
    /// The diagnosis data (failed jobs info)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnosis: Option<ResourceUtilizationReport>,
}

/// Information about Slurm logs for a job
#[derive(Debug)]
pub struct SlurmLogInfo {
    pub slurm_job_id: Option<String>,
    pub slurm_stdout: Option<String>,
    pub slurm_stderr: Option<String>,
}

/// Recover a Slurm workflow by:
/// 1. Cleaning up orphaned jobs (from terminated Slurm allocations)
/// 2. Checking preconditions (workflow complete, no active workers)
/// 3. Diagnosing failures (OOM, timeout, etc.)
/// 4. Applying recovery heuristics (adjusting resources)
/// 5. Running recovery hook (if provided)
/// 6. Resetting failed jobs
/// 7. Reinitializing workflow
/// 8. Regenerating and submitting Slurm schedulers
pub fn recover_workflow(
    config: &Configuration,
    args: &RecoverArgs,
) -> Result<RecoveryResult, String> {
    if args.dry_run {
        info!("Recovery dry_run workflow_id={}", args.workflow_id);
    }

    // Step 0: Clean up orphaned jobs from terminated Slurm allocations
    // This must happen before checking preconditions because orphaned jobs/allocations
    // would otherwise block recovery (preconditions check for no active workers)
    info!("Orphan check workflow_id={}", args.workflow_id);
    match super::orphan_detection::cleanup_orphaned_jobs(config, args.workflow_id, args.dry_run) {
        Ok(result) => {
            if result.any_cleaned() {
                if args.dry_run {
                    info!(
                        "Orphan cleanup dry_run workflow_id={} slurm_jobs={} pending_allocations={} running_jobs={}",
                        args.workflow_id,
                        result.slurm_jobs_failed,
                        result.pending_allocations_cleaned,
                        result.running_jobs_failed
                    );
                } else {
                    info!(
                        "Orphans cleaned workflow_id={} slurm_jobs_failed={} pending_allocations_cleaned={} running_jobs_failed={}",
                        args.workflow_id,
                        result.slurm_jobs_failed,
                        result.pending_allocations_cleaned,
                        result.running_jobs_failed
                    );
                }
            } else {
                info!("No orphans found workflow_id={}", args.workflow_id);
            }
        }
        Err(e) => {
            warn!(
                "Orphan cleanup error workflow_id={} error={}",
                args.workflow_id, e
            );
            // Continue with recovery - orphan cleanup is best-effort
        }
    }

    // Check for pending_failed jobs (requires AI classification)
    let pending_failed_count = count_pending_failed_jobs(config, args.workflow_id).unwrap_or(0);
    if pending_failed_count > 0 {
        if args.ai_recovery {
            info!(
                "[EXPERIMENTAL] AI recovery: {} job(s) in pending_failed status",
                pending_failed_count
            );
            info!("These jobs failed without a matching failure handler rule.");

            if args.dry_run {
                info!(
                    "[DRY RUN] Would invoke AI agent '{}' for classification",
                    args.ai_agent
                );
            } else {
                // Invoke the AI agent to classify pending_failed jobs
                match invoke_ai_agent(args.workflow_id, &args.ai_agent, &args.output_dir) {
                    Ok(()) => {
                        // Re-check pending_failed count after AI classification
                        let remaining =
                            count_pending_failed_jobs(config, args.workflow_id).unwrap_or(0);
                        if remaining > 0 {
                            warn!(
                                "{} job(s) still in pending_failed status after AI classification",
                                remaining
                            );
                        } else {
                            info!("All pending_failed jobs have been classified");
                        }
                    }
                    Err(e) => {
                        warn!("AI agent invocation failed: {}", e);
                        warn!("You can manually classify jobs using the torc MCP server:");
                        warn!("  1. list_pending_failed_jobs - View jobs with their stderr");
                        warn!("  2. classify_and_resolve_failures - Apply retry/fail decisions");
                        warn!(
                            "Or reset them manually: torc workflows reset-status {} --failed-only",
                            args.workflow_id
                        );
                    }
                }
            }
        } else {
            warn!(
                "{} job(s) in pending_failed status (awaiting classification)",
                pending_failed_count
            );
            warn!("Use --ai-recovery to enable AI-assisted classification via MCP tools.");
            warn!(
                "Or reset them manually: torc workflows reset-status {} --failed-only",
                args.workflow_id
            );
        }
    }

    // Step 1: Check preconditions
    check_recovery_preconditions(config, args.workflow_id)?;

    // Interactive mode: hand off to the interactive wizard
    if args.interactive {
        return recover_workflow_interactive(config, args);
    }

    // Step 2: Diagnose failures
    info!("Diagnosing failures...");
    let diagnosis = diagnose_failures(config, args.workflow_id)?;

    // Step 3: Apply recovery heuristics (in dry_run mode, this shows changes without applying them)
    if args.dry_run {
        info!("[DRY RUN] Proposed resource adjustments:");
    } else {
        info!("Applying recovery heuristics...");
    }
    let mut result = apply_recovery_heuristics(
        config,
        args.workflow_id,
        &diagnosis,
        args.memory_multiplier,
        args.runtime_multiplier,
        args.retry_unknown,
        &args.output_dir,
        args.dry_run,
    )?;

    if result.oom_fixed > 0 || result.timeout_fixed > 0 {
        if args.dry_run {
            info!(
                "  Would apply fixes: {} OOM, {} timeout",
                result.oom_fixed, result.timeout_fixed
            );
        } else {
            info!(
                "  Applied fixes: {} OOM, {} timeout",
                result.oom_fixed, result.timeout_fixed
            );
        }
    }

    if result.other_failures > 0 {
        if args.retry_unknown {
            if args.recovery_hook.is_some() {
                info!(
                    "  {} job(s) with unknown failure cause (would run recovery hook)",
                    result.other_failures
                );
            } else {
                info!(
                    "  {} job(s) with unknown failure cause (would retry)",
                    result.other_failures
                );
            }
            // Track unknown retried count
            result.unknown_retried = result.other_failures;
        } else {
            info!(
                "  {} job(s) with unknown failure cause (skipped, use --retry-unknown to include)",
                result.other_failures
            );
        }
    }

    // In dry_run mode, stop here
    if args.dry_run {
        if result.jobs_to_retry.is_empty() {
            info!("[DRY RUN] No recoverable jobs found.");
        } else {
            info!(
                "[DRY RUN] Would reset {} job(s) for retry",
                result.jobs_to_retry.len()
            );
            info!("[DRY RUN] Would reinitialize workflow");

            // Get the real scheduler plan using slurm regenerate --dry-run --include-job-ids
            info!("[DRY RUN] Slurm schedulers that would be created:");
            match get_scheduler_dry_run(args.workflow_id, &args.output_dir, &result.jobs_to_retry) {
                Ok(mut dry_run_result) => {
                    // Apply the adjusted memory/runtime values to the scheduler info.
                    // slurm regenerate reads from the database, but in dry-run mode
                    // the adjustments haven't been applied yet. We need to update
                    // the scheduler memory/runtime to reflect what would be used.
                    for sched in &mut dry_run_result.planned_schedulers {
                        // Find if any of the jobs in this scheduler have adjustments
                        for adj in &result.adjustments {
                            // Check if any job in this scheduler matches the adjustment
                            let has_matching_job = sched
                                .job_names
                                .iter()
                                .any(|name| adj.job_names.contains(name));

                            if has_matching_job {
                                // Apply memory adjustment
                                if adj.memory_adjusted
                                    && let Some(ref new_mem) = adj.new_memory
                                {
                                    sched.mem = Some(new_mem.clone());
                                }
                                // Note: walltime is determined by partition max, not by
                                // resource requirements runtime, so we don't update it here
                                break;
                            }
                        }
                    }

                    for sched in &dry_run_result.planned_schedulers {
                        let deps = if sched.has_dependencies {
                            " (deferred)"
                        } else {
                            ""
                        };
                        info!(
                            "  {} - {} job(s), {} allocation(s){}",
                            sched.name, sched.job_count, sched.num_allocations, deps
                        );
                        info!(
                            "    Account: {}, Partition: {}, Walltime: {}, Nodes: {}, Mem: {}",
                            sched.account,
                            sched.partition.as_deref().unwrap_or("default"),
                            sched.walltime,
                            sched.nodes,
                            sched.mem.as_deref().unwrap_or("default")
                        );
                    }
                    info!(
                        "[DRY RUN] Total: {} allocation(s) would be submitted",
                        dry_run_result.total_allocations
                    );

                    // Fix would_submit: slurm regenerate --dry-run doesn't pass --submit,
                    // but actual recovery does call `slurm regenerate --submit`
                    dry_run_result.would_submit = true;

                    // Include the full dry-run result for JSON output
                    result.slurm_dry_run = Some(dry_run_result);
                }
                Err(e) => {
                    warn!("  Could not get scheduler preview: {}", e);
                    info!(
                        "[DRY RUN] Would submit Slurm allocations for {} job(s)",
                        result.jobs_to_retry.len()
                    );
                }
            }
        }
        return Ok(result);
    }

    // Step 4: Run recovery hook if provided and there are unknown failures
    if result.other_failures > 0
        && let Some(ref hook_cmd) = args.recovery_hook
    {
        info!(
            "{} job(s) with unknown failure cause - running recovery hook...",
            result.other_failures
        );
        run_recovery_hook(args.workflow_id, hook_cmd)?;
    }

    // Check if there are any jobs to retry
    if result.jobs_to_retry.is_empty() {
        return Err(format!(
            "No recoverable jobs found. {} job(s) failed with unknown causes. \
             Use --retry-unknown to retry jobs with unknown failure causes.",
            result.other_failures
        ));
    }

    // Step 5: Reset failed jobs
    info!(
        "Jobs resetting workflow_id={} count={}",
        args.workflow_id,
        result.jobs_to_retry.len()
    );
    let reset_count = reset_failed_jobs(config, args.workflow_id, &result.jobs_to_retry)?;
    info!(
        "Jobs reset workflow_id={} count={}",
        args.workflow_id, reset_count
    );

    // Step 6: Reinitialize workflow (must happen BEFORE regenerate)
    // reset_workflow_status rejects requests when there are pending scheduled compute nodes,
    // so we must reinitialize before creating new allocations.
    info!("Workflow reinitializing workflow_id={}", args.workflow_id);
    reinitialize_workflow(config, args.workflow_id)?;

    // Step 7: Regenerate Slurm schedulers and submit
    info!("Schedulers regenerating workflow_id={}", args.workflow_id);
    regenerate_and_submit(args.workflow_id, &args.output_dir, None, None)?;

    Ok(result)
}

/// Check that the workflow is in a valid state for recovery:
/// - Workflow must be complete (all jobs in terminal state)
/// - No active workers (compute nodes or scheduled compute nodes)
fn check_recovery_preconditions(config: &Configuration, workflow_id: i64) -> Result<(), String> {
    // Check if workflow is complete
    let is_complete = apis::workflows_api::is_workflow_complete(config, workflow_id)
        .map_err(|e| format!("Failed to check workflow completion status: {}", e))?;

    if !is_complete.is_complete && !is_complete.is_canceled {
        return Err("Cannot recover: workflow is not complete. \
             Wait for all jobs to finish or use 'torc workflows cancel' first."
            .to_string());
    }

    // Check for active compute nodes
    let active_nodes = apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,       // offset
        Some(1),    // limit - just need to know if any exist
        None,       // sort_by
        None,       // reverse_sort
        None,       // hostname
        Some(true), // is_active = true
        None,       // scheduled_compute_node_id
    )
    .map_err(|e| format!("Failed to check for active compute nodes: {}", e))?;

    if !active_nodes.items.is_empty() {
        return Err("Cannot recover: there are still active compute nodes. \
             Wait for all workers to exit."
            .to_string());
    }

    // Check for pending/active scheduled compute nodes
    let pending_scn = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None,            // offset
        Some(1),         // limit
        None,            // sort_by
        None,            // reverse_sort
        None,            // scheduler_id
        None,            // scheduler_config_id
        Some("pending"), // status
    )
    .map_err(|e| format!("Failed to check for pending scheduled compute nodes: {}", e))?;

    if pending_scn.total_count > 0 {
        return Err("Cannot recover: there are pending Slurm allocations. \
             Wait for them to start or cancel them with 'torc slurm cancel'."
            .to_string());
    }

    let active_scn = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None,           // offset
        Some(1),        // limit
        None,           // sort_by
        None,           // reverse_sort
        None,           // scheduler_id
        None,           // scheduler_config_id
        Some("active"), // status
    )
    .map_err(|e| format!("Failed to check for active scheduled compute nodes: {}", e))?;

    if active_scn.total_count > 0 {
        return Err(
            "Cannot recover: there are active Slurm allocations still running. \
             Wait for all workers to exit."
                .to_string(),
        );
    }

    // Check that there are actually failed/terminated/canceled jobs to recover
    let failed_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(crate::models::JobStatus::Failed), // status
        None,                                   // needs_file_id
        None,                                   // upstream_job_id
        None,                                   // offset
        Some(1),                                // limit
        None,                                   // sort_by
        None,                                   // reverse_sort
        None,                                   // include_relationships
        None,                                   // active_compute_node_id
    )
    .map_err(|e| format!("Failed to list failed jobs: {}", e))?;

    let terminated_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(crate::models::JobStatus::Terminated), // status
        None,                                       // needs_file_id
        None,                                       // upstream_job_id
        None,                                       // offset
        Some(1),                                    // limit
        None,                                       // sort_by
        None,                                       // reverse_sort
        None,                                       // include_relationships
        None,                                       // active_compute_node_id
    )
    .map_err(|e| format!("Failed to list terminated jobs: {}", e))?;

    if failed_jobs.total_count == 0 && terminated_jobs.total_count == 0 {
        return Err("No failed or terminated jobs to recover. \
             Workflow may have completed successfully."
            .to_string());
    }

    Ok(())
}

/// Invoke an AI agent CLI to classify pending_failed jobs
///
/// Spawns the specified AI agent (e.g., "claude") with a prompt to use
/// the torc MCP tools for classifying pending_failed jobs.
pub fn invoke_ai_agent(workflow_id: i64, agent: &str, output_dir: &Path) -> Result<(), String> {
    let prompt = format!(
        "You are helping recover a Torc workflow. Workflow {} has jobs in 'pending_failed' status \
         that need classification. \n\n\
         Please use the torc MCP tools to:\n\
         1. Call list_pending_failed_jobs with workflow_id={} to see the jobs and their stderr\n\
         2. Analyze each job's stderr to determine if the error is transient (retry) or permanent (fail)\n\
         3. Call classify_and_resolve_failures with your classifications\n\n\
         The output directory is: {}\n\n\
         After classification, the workflow can continue with recovery.",
        workflow_id,
        workflow_id,
        output_dir.display()
    );

    info!(
        "[EXPERIMENTAL] Invoking AI agent '{}' for pending_failed classification...",
        agent
    );

    match agent {
        "claude" => {
            // Check if claude CLI is available by attempting to run it
            let check = Command::new("claude").arg("--version").output();

            match check {
                Ok(output) if output.status.success() => {
                    // Claude CLI is available
                }
                Ok(_) | Err(_) => {
                    return Err(
                        "Claude CLI not found. Install it from https://claude.ai/code \
                         or use --ai-agent to specify a different agent."
                            .to_string(),
                    );
                }
            }

            // Invoke claude with the prompt using --print for non-interactive mode
            info!("Running: claude --print \"<prompt>\"");
            let output = Command::new("claude")
                .arg("--print")
                .arg(&prompt)
                .output()
                .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

            // Print stdout
            if !output.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    info!("[claude] {}", line);
                }
            }

            // Print stderr
            if !output.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                for line in stderr.lines() {
                    warn!("[claude] {}", line);
                }
            }

            if !output.status.success() {
                let exit_code = output.status.code().unwrap_or(-1);
                return Err(format!("Claude CLI exited with code {}", exit_code));
            }

            info!("AI agent completed classification");
            Ok(())
        }
        "copilot" | "github-copilot" => {
            // Check if gh CLI is available
            let check = Command::new("gh").arg("--version").output();

            match check {
                Ok(output) if output.status.success() => {
                    // gh CLI is available
                }
                Ok(_) | Err(_) => {
                    return Err(
                        "GitHub CLI (gh) not found. Install it from https://cli.github.com/ \
                         or use --ai-agent to specify a different agent."
                            .to_string(),
                    );
                }
            }

            // Invoke GitHub Copilot via gh CLI
            info!("Running: gh copilot suggest \"<prompt>\"");
            let output = Command::new("gh")
                .args(["copilot", "suggest", &prompt])
                .output()
                .map_err(|e| format!("Failed to run gh copilot: {}", e))?;

            // Print stdout
            if !output.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    info!("[copilot] {}", line);
                }
            }

            // Print stderr
            if !output.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                for line in stderr.lines() {
                    warn!("[copilot] {}", line);
                }
            }

            if !output.status.success() {
                let exit_code = output.status.code().unwrap_or(-1);
                return Err(format!("GitHub Copilot CLI exited with code {}", exit_code));
            }

            info!("AI agent completed classification");
            Ok(())
        }
        other => Err(format!(
            "Unsupported AI agent '{}'. Supported agents: claude, copilot",
            other
        )),
    }
}

/// Count jobs in pending_failed status that need AI classification
fn count_pending_failed_jobs(config: &Configuration, workflow_id: i64) -> Result<i64, String> {
    let pending_failed_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(JobStatus::PendingFailed),
        None,    // needs_file_id
        None,    // upstream_job_id
        None,    // offset
        Some(1), // limit - just need count
        None,    // sort_by
        None,    // reverse_sort
        None,    // include_relationships
        None,    // active_compute_node_id
    )
    .map_err(|e| format!("Failed to list pending_failed jobs: {}", e))?;

    Ok(pending_failed_jobs.total_count)
}

/// Diagnose failures and return resource utilization report
pub fn diagnose_failures(
    config: &Configuration,
    workflow_id: i64,
) -> Result<ResourceUtilizationReport, String> {
    build_resource_utilization_report(config, Some(workflow_id), None, true, 1.0)
}

/// Get Slurm log information for failed jobs
fn get_slurm_log_info(
    config: &Configuration,
    workflow_id: i64,
    output_dir: &Path,
) -> Result<ResultsReport, String> {
    build_results_report(config, Some(workflow_id), output_dir, false, &[])
}

/// Correlate failed jobs with their Slurm allocation logs
fn correlate_slurm_logs(
    diagnosis: &ResourceUtilizationReport,
    slurm_info: &ResultsReport,
) -> HashMap<i64, SlurmLogInfo> {
    let mut log_map = HashMap::new();

    // Build map from job_id to slurm log paths (using `results` field, not `jobs`)
    for result in &slurm_info.results {
        if result.slurm_stdout.is_some() || result.slurm_stderr.is_some() {
            log_map.insert(
                result.job_id,
                SlurmLogInfo {
                    slurm_job_id: result.slurm_job_id.clone(),
                    slurm_stdout: result.slurm_stdout.clone(),
                    slurm_stderr: result.slurm_stderr.clone(),
                },
            );
        }
    }

    // Filter to only resource violations
    let mut failed_log_map = HashMap::new();
    for violation in &diagnosis.resource_violations {
        if let Some(log_info) = log_map.remove(&violation.job_id) {
            failed_log_map.insert(violation.job_id, log_info);
        }
    }

    failed_log_map
}

/// Apply recovery heuristics and update job resources
///
/// If `dry_run` is true, shows what would be done without making changes.
///
/// This function combines recovery-specific logic (Slurm logs, retry_unknown handling)
/// with the shared resource correction algorithm.
#[allow(clippy::too_many_arguments)]
pub fn apply_recovery_heuristics(
    config: &Configuration,
    workflow_id: i64,
    diagnosis: &ResourceUtilizationReport,
    memory_multiplier: f64,
    runtime_multiplier: f64,
    retry_unknown: bool,
    output_dir: &Path,
    dry_run: bool,
) -> Result<RecoveryResult, String> {
    // Try to get Slurm log info for correlation and logging
    let slurm_log_map = match get_slurm_log_info(config, workflow_id, output_dir) {
        Ok(slurm_info) => {
            let log_map = correlate_slurm_logs(diagnosis, &slurm_info);
            if !log_map.is_empty() {
                info!("  Found Slurm logs for {} failed job(s)", log_map.len());
            }
            log_map
        }
        Err(e) => {
            debug!("Could not get Slurm log info: {}", e);
            HashMap::new()
        }
    };

    // Log Slurm info for each resource violation if available
    for violation in &diagnosis.resource_violations {
        if let Some(slurm_info) = slurm_log_map.get(&violation.job_id)
            && let Some(slurm_job_id) = &slurm_info.slurm_job_id
        {
            debug!(
                "  Job {} ran in Slurm allocation {}",
                violation.job_id, slurm_job_id
            );
        }
    }

    // Count other failures for recovery report
    let mut other_failures = 0;
    let mut unknown_job_ids = Vec::new();

    for violation in &diagnosis.resource_violations {
        if !violation.memory_violation && !violation.likely_timeout {
            other_failures += 1;
            if retry_unknown {
                unknown_job_ids.push(violation.job_id);
            }
        }
    }

    // Call shared resource correction algorithm (recovery never downsizes)
    let correction_ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis,
        all_results: &[],
        all_jobs: &[],
        all_resource_requirements: &[],
    };
    let correction_opts = ResourceCorrectionOptions {
        memory_multiplier,
        cpu_multiplier: memory_multiplier, // recovery uses memory_multiplier for CPU
        runtime_multiplier,
        include_jobs: vec![],
        dry_run,
        no_downsize: true,
    };
    let correction_result = apply_resource_corrections(&correction_ctx, &correction_opts)?;

    // Extract counts from shared result
    let oom_fixed = correction_result.memory_corrections;
    let timeout_fixed = correction_result.runtime_corrections;

    // Combine jobs that need retry: those with corrected resources + unknown failures
    let mut jobs_to_retry = Vec::new();
    for adj in &correction_result.adjustments {
        jobs_to_retry.extend(&adj.job_ids);
    }
    jobs_to_retry.extend(&unknown_job_ids);

    Ok(RecoveryResult {
        oom_fixed,
        timeout_fixed,
        unknown_retried: unknown_job_ids.len(),
        other_failures,
        jobs_to_retry,
        adjustments: correction_result.adjustments,
        slurm_dry_run: None, // Set in recover_workflow dry_run block
    })
}

/// Reset specific failed jobs for retry (without reinitializing)
pub fn reset_failed_jobs(
    config: &Configuration,
    workflow_id: i64,
    job_ids: &[i64],
) -> Result<usize, String> {
    if job_ids.is_empty() {
        return Ok(0);
    }

    let job_count = job_ids.len();

    apis::workflows_api::reset_workflow_status(config, workflow_id, None)
        .map_err(|e| format!("Failed to reset workflow status: {}", e))?;
    info!("  Reset workflow status for workflow {}", workflow_id);

    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .map_err(|e| format!("Failed to reset failed job status: {}", e))?;
    info!("  Reset failed job status for workflow {}", workflow_id);

    Ok(job_count)
}

/// Reinitialize the workflow (set up dependencies and fire on_workflow_start actions)
pub fn reinitialize_workflow(config: &Configuration, workflow_id: i64) -> Result<(), String> {
    let workflow = apis::workflows_api::get_workflow(config, workflow_id)
        .map_err(|e| format!("Failed to fetch workflow for reinitialize: {}", e))?;
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    workflow_manager
        .reinitialize(false, false)
        .map_err(|e| format!("workflow reinitialize failed: {}", e))
}

/// Run the user's custom recovery hook command
pub fn run_recovery_hook(workflow_id: i64, hook_command: &str) -> Result<(), String> {
    info!("Running recovery hook: {}", hook_command);

    // Parse the command using shell-like quoting rules
    let parts = shlex::split(hook_command)
        .ok_or_else(|| format!("Invalid quoting in recovery hook command: {}", hook_command))?;
    if parts.is_empty() {
        return Err("Recovery hook command is empty".to_string());
    }

    // If the program doesn't contain a path separator and exists in the current directory,
    // prepend "./" so it's found (Command::new searches PATH, not CWD)
    let program = &parts[0];
    let program_path = if !program.contains('/') && std::path::Path::new(program).exists() {
        format!("./{}", program)
    } else {
        program.to_string()
    };
    let mut cmd = Command::new(&program_path);

    // Add any arguments from the hook command
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    // Add workflow ID as final argument
    cmd.arg(workflow_id.to_string());

    // Also set as environment variable for convenience
    cmd.env("TORC_WORKFLOW_ID", workflow_id.to_string());

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute recovery hook '{}': {}", hook_command, e))?;

    // Log stdout if present
    if !output.stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            info!("  [hook] {}", line);
        }
    }

    // Log stderr if present
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            warn!("  [hook] {}", line);
        }
    }

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        return Err(format!(
            "Recovery hook '{}' failed with exit code {}",
            hook_command, exit_code
        ));
    }

    info!("Recovery hook completed successfully");
    Ok(())
}

/// Regenerate Slurm schedulers and submit allocations
pub fn regenerate_and_submit(
    workflow_id: i64,
    output_dir: &Path,
    partition: Option<&str>,
    walltime: Option<&str>,
) -> Result<(), String> {
    let mut args = vec![
        "slurm".to_string(),
        "regenerate".to_string(),
        workflow_id.to_string(),
        "--submit".to_string(),
        "-o".to_string(),
        output_dir.to_str().unwrap_or("torc_output").to_string(),
    ];
    if let Some(p) = partition {
        args.push("--partition".to_string());
        args.push(p.to_string());
    }
    if let Some(w) = walltime {
        args.push("--walltime".to_string());
        args.push(w.to_string());
    }
    let mut cmd = torc_command()?;
    let output = cmd
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run slurm regenerate: {}", e))?;

    // Print stdout so user sees what schedulers were created and submitted
    if !output.stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            info!("  {}", line);
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("slurm regenerate failed: {}", stderr));
    }

    Ok(())
}

/// Get a dry-run preview of what schedulers would be created, including specific job IDs
fn get_scheduler_dry_run(
    workflow_id: i64,
    output_dir: &Path,
    job_ids: &[i64],
) -> Result<RegenerateDryRunResult, String> {
    // Build the --include-job-ids argument
    let job_ids_str = job_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut cmd = torc_command()?;
    let output = cmd
        .args([
            "-f",
            "json",
            "slurm",
            "regenerate",
            &workflow_id.to_string(),
            "--dry-run",
            "--include-job-ids",
            &job_ids_str,
            "-o",
            output_dir.to_str().unwrap_or("torc_output"),
        ])
        .output()
        .map_err(|e| format!("Failed to run slurm regenerate --dry-run: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("slurm regenerate --dry-run failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse slurm regenerate dry-run output: {}", e))
}

// ---------------------------------------------------------------------------
// Interactive recovery wizard
// ---------------------------------------------------------------------------

/// Read a line from stdin, trimmed. Returns the default if the user presses Enter.
fn prompt_line(prompt: &str) -> Result<String, String> {
    eprint!("{}", prompt);
    io::stderr().flush().ok();
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .map_err(|e| format!("Failed to read input: {}", e))?;
    Ok(buf.trim().to_string())
}

/// Prompt the user for a choice. `valid` lists accepted single-char answers (lowercase).
/// Returns the default if the user presses Enter.
fn prompt_choice(prompt: &str, valid: &[&str], default: &str) -> Result<String, String> {
    loop {
        let input = prompt_line(prompt)?;
        let answer = if input.is_empty() {
            default.to_string()
        } else {
            input.to_lowercase()
        };
        if valid.contains(&answer.as_str()) {
            return Ok(answer);
        }
        eprintln!(
            "  Invalid choice '{}'. Valid options: {}",
            answer,
            valid.join(", ")
        );
    }
}

/// Prompt for a floating-point multiplier with a default value.
fn prompt_multiplier(label: &str, default: f64) -> Result<f64, String> {
    loop {
        let input = prompt_line(&format!(
            "  Enter {} multiplier [default: {}]: ",
            label, default
        ))?;
        if input.is_empty() {
            return Ok(default);
        }
        match input.parse::<f64>() {
            Ok(v) if v > 0.0 => return Ok(v),
            _ => eprintln!("  Please enter a positive number."),
        }
    }
}

/// Interactive recovery wizard (default when stdin is a TTY). Guides the user
/// through failure diagnosis, resource adjustment, and scheduler selection.
fn recover_workflow_interactive(
    config: &Configuration,
    args: &RecoverArgs,
) -> Result<RecoveryResult, String> {
    // --- Diagnose failures ---------------------------------------------------
    eprintln!("\n=== Recovery Wizard ===\n");
    eprintln!("Diagnosing failures for workflow {}...\n", args.workflow_id);

    let diagnosis = diagnose_failures(config, args.workflow_id)?;

    // Categorize violations
    let mut oom_jobs: Vec<&crate::client::report_models::ResourceViolationInfo> = Vec::new();
    let mut timeout_jobs: Vec<&crate::client::report_models::ResourceViolationInfo> = Vec::new();
    let mut unknown_jobs: Vec<&crate::client::report_models::ResourceViolationInfo> = Vec::new();

    for v in &diagnosis.resource_violations {
        if v.memory_violation {
            oom_jobs.push(v);
        } else if v.likely_timeout {
            timeout_jobs.push(v);
        } else {
            unknown_jobs.push(v);
        }
    }

    if oom_jobs.is_empty() && timeout_jobs.is_empty() && unknown_jobs.is_empty() {
        eprintln!("No failed jobs with resource violations found.");
        return Ok(RecoveryResult {
            oom_fixed: 0,
            timeout_fixed: 0,
            unknown_retried: 0,
            other_failures: 0,
            jobs_to_retry: vec![],
            adjustments: vec![],
            slurm_dry_run: None,
        });
    }

    // --- Display summary table -----------------------------------------------
    if !oom_jobs.is_empty() {
        eprintln!(
            "OOM Failures ({} job{}):",
            oom_jobs.len(),
            plural(oom_jobs.len())
        );
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<10} {:<14} Reason",
            "ID", "Name", "RC", "Memory", "Peak Memory"
        );
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<10} {:<14} ------",
            "---", "----", "---", "------", "-----------"
        );
        for v in &oom_jobs {
            eprintln!(
                "  {:<8} {:<30} {:<6} {:<10} {:<14} {}",
                v.job_id,
                truncate(&v.job_name, 30),
                v.return_code,
                &v.configured_memory,
                v.peak_memory_formatted.as_deref().unwrap_or("-"),
                v.oom_reason.as_deref().unwrap_or("-"),
            );
        }
        eprintln!();
    }

    if !timeout_jobs.is_empty() {
        eprintln!(
            "Timeout Failures ({} job{}):",
            timeout_jobs.len(),
            plural(timeout_jobs.len())
        );
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<12} {:<12} Reason",
            "ID", "Name", "RC", "Runtime", "Exec (min)"
        );
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<12} {:<12} ------",
            "---", "----", "---", "-------", "----------"
        );
        for v in &timeout_jobs {
            eprintln!(
                "  {:<8} {:<30} {:<6} {:<12} {:<12.1} {}",
                v.job_id,
                truncate(&v.job_name, 30),
                v.return_code,
                &v.configured_runtime,
                v.exec_time_minutes,
                v.timeout_reason.as_deref().unwrap_or("-"),
            );
        }
        eprintln!();
    }

    if !unknown_jobs.is_empty() {
        eprintln!(
            "Unknown Failures ({} job{}):",
            unknown_jobs.len(),
            plural(unknown_jobs.len())
        );
        eprintln!("  {:<8} {:<30} {:<6} {:<10}", "ID", "Name", "RC", "Memory");
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<10}",
            "---", "----", "---", "------"
        );
        for v in &unknown_jobs {
            eprintln!(
                "  {:<8} {:<30} {:<6} {:<10}",
                v.job_id,
                truncate(&v.job_name, 30),
                v.return_code,
                &v.configured_memory,
            );
        }
        eprintln!();
    }

    // --- Per-category decisions -----------------------------------------------
    let mut memory_multiplier = args.memory_multiplier;
    let mut runtime_multiplier = args.runtime_multiplier;
    let mut include_oom = false;
    let mut include_timeout = false;
    let mut include_unknown = false;

    if !oom_jobs.is_empty() {
        let choice = prompt_choice(
            &format!(
                "OOM failures ({} job{}): [R]etry with {}x memory / [A]djust multiplier / [S]kip (default: R): ",
                oom_jobs.len(),
                plural(oom_jobs.len()),
                args.memory_multiplier,
            ),
            &["r", "a", "s"],
            "r",
        )?;
        match choice.as_str() {
            "r" => include_oom = true,
            "a" => {
                memory_multiplier = prompt_multiplier("memory", args.memory_multiplier)?;
                include_oom = true;
            }
            _ => eprintln!("  Skipping OOM jobs."),
        }
    }

    if !timeout_jobs.is_empty() {
        let choice = prompt_choice(
            &format!(
                "Timeout failures ({} job{}): [R]etry with {}x runtime / [A]djust multiplier / [S]kip (default: R): ",
                timeout_jobs.len(),
                plural(timeout_jobs.len()),
                args.runtime_multiplier,
            ),
            &["r", "a", "s"],
            "r",
        )?;
        match choice.as_str() {
            "r" => include_timeout = true,
            "a" => {
                runtime_multiplier = prompt_multiplier("runtime", args.runtime_multiplier)?;
                include_timeout = true;
            }
            _ => eprintln!("  Skipping timeout jobs."),
        }
    }

    if !unknown_jobs.is_empty() {
        let choice = prompt_choice(
            &format!(
                "Unknown failures ({} job{}): [R]etry as-is / [S]kip (default: S): ",
                unknown_jobs.len(),
                plural(unknown_jobs.len()),
            ),
            &["r", "s"],
            "s",
        )?;
        if choice == "r" {
            include_unknown = true;
        } else {
            eprintln!("  Skipping unknown failures.");
        }
    }

    // Build the list of job IDs to include in resource corrections
    let mut correction_job_ids: Vec<i64> = Vec::new();
    if include_oom {
        correction_job_ids.extend(oom_jobs.iter().map(|v| v.job_id));
    }
    if include_timeout {
        correction_job_ids.extend(timeout_jobs.iter().map(|v| v.job_id));
    }
    // Unknown jobs get retried without resource adjustment
    let unknown_job_ids: Vec<i64> = if include_unknown {
        unknown_jobs.iter().map(|v| v.job_id).collect()
    } else {
        vec![]
    };

    if correction_job_ids.is_empty() && unknown_job_ids.is_empty() {
        eprintln!("\nNo jobs selected for recovery.");
        return Ok(RecoveryResult {
            oom_fixed: 0,
            timeout_fixed: 0,
            unknown_retried: 0,
            other_failures: unknown_jobs.len(),
            jobs_to_retry: vec![],
            adjustments: vec![],
            slurm_dry_run: None,
        });
    }

    // --- Apply resource corrections ------------------------------------------
    let correction_ctx = ResourceCorrectionContext {
        config,
        workflow_id: args.workflow_id,
        diagnosis: &diagnosis,
        all_results: &[],
        all_jobs: &[],
        all_resource_requirements: &[],
    };
    let correction_opts = ResourceCorrectionOptions {
        memory_multiplier,
        cpu_multiplier: memory_multiplier,
        runtime_multiplier,
        include_jobs: correction_job_ids,
        dry_run: true, // always preview first in interactive mode
        no_downsize: true,
    };
    let correction_result = apply_resource_corrections(&correction_ctx, &correction_opts)?;

    // --- Show proposed changes and confirm ------------------------------------
    eprintln!("\n--- Recovery Plan ---\n");

    if !correction_result.adjustments.is_empty() {
        for adj in &correction_result.adjustments {
            if adj.memory_adjusted {
                eprintln!(
                    "  Memory: {} -> {} ({}x) for {} job{}: {}",
                    adj.original_memory.as_deref().unwrap_or("?"),
                    adj.new_memory.as_deref().unwrap_or("?"),
                    memory_multiplier,
                    adj.job_names.len(),
                    plural(adj.job_names.len()),
                    adj.job_names.join(", "),
                );
            }
            if adj.runtime_adjusted {
                eprintln!(
                    "  Runtime: {} -> {} ({}x) for {} job{}: {}",
                    adj.original_runtime.as_deref().unwrap_or("?"),
                    adj.new_runtime.as_deref().unwrap_or("?"),
                    runtime_multiplier,
                    adj.job_names.len(),
                    plural(adj.job_names.len()),
                    adj.job_names.join(", "),
                );
            }
        }
    }

    if !unknown_job_ids.is_empty() {
        let unknown_names: Vec<&str> = unknown_jobs
            .iter()
            .filter(|v| unknown_job_ids.contains(&v.job_id))
            .map(|v| v.job_name.as_str())
            .collect();
        eprintln!(
            "  Retry as-is: {} job{}: {}",
            unknown_job_ids.len(),
            plural(unknown_job_ids.len()),
            unknown_names.join(", "),
        );
    }

    let mut all_jobs_to_retry: Vec<i64> = Vec::new();
    for adj in &correction_result.adjustments {
        all_jobs_to_retry.extend(&adj.job_ids);
    }
    all_jobs_to_retry.extend(&unknown_job_ids);
    // Deduplicate
    all_jobs_to_retry.sort_unstable();
    all_jobs_to_retry.dedup();

    eprintln!(
        "\n  Total: {} job{} to retry",
        all_jobs_to_retry.len(),
        plural(all_jobs_to_retry.len()),
    );

    if args.dry_run {
        eprintln!("\n[DRY RUN] No changes applied.");
        let slurm_dry_run =
            match get_scheduler_dry_run(args.workflow_id, &args.output_dir, &all_jobs_to_retry) {
                Ok(mut dr) => {
                    dr.would_submit = true;
                    for sched in &dr.planned_schedulers {
                        let deps = if sched.has_dependencies {
                            " (deferred)"
                        } else {
                            ""
                        };
                        eprintln!(
                            "  {} - {} job(s), {} allocation(s){}",
                            sched.name, sched.job_count, sched.num_allocations, deps
                        );
                    }
                    Some(dr)
                }
                Err(e) => {
                    warn!("Could not get scheduler preview: {}", e);
                    None
                }
            };

        return Ok(RecoveryResult {
            oom_fixed: correction_result.memory_corrections,
            timeout_fixed: correction_result.runtime_corrections,
            unknown_retried: unknown_job_ids.len(),
            other_failures: unknown_jobs.len(),
            jobs_to_retry: all_jobs_to_retry,
            adjustments: correction_result.adjustments,
            slurm_dry_run,
        });
    }

    // --- Scheduler selection ----------------------------------------------------
    eprintln!("\n--- Slurm Scheduler ---\n");

    let scheduler_choice = prompt_scheduler_choice(config, args)?;

    // Confirm before executing
    match &scheduler_choice {
        SchedulerChoice::Regenerate {
            partition,
            walltime,
        } => {
            eprintln!("\n  Scheduler: auto-generate new schedulers");
            if let Some(p) = partition {
                eprintln!("  Partition: {}", p);
            }
            if let Some(w) = walltime {
                eprintln!("  Walltime: {}", w);
            }
        }
        SchedulerChoice::Existing {
            scheduler_id,
            scheduler_name,
            num_allocations,
        } => {
            eprintln!(
                "\n  Scheduler: {} (ID {}), {} allocation(s)",
                scheduler_name, scheduler_id, num_allocations
            );
        }
    }

    let confirm = prompt_choice("\nProceed with recovery? (y/N): ", &["y", "n"], "n")?;
    if confirm != "y" {
        return Err("Recovery cancelled.".to_string());
    }

    // --- Execute recovery (apply for real) ------------------------------------
    eprintln!();

    // Re-apply corrections with dry_run=false
    let real_opts = ResourceCorrectionOptions {
        memory_multiplier,
        cpu_multiplier: memory_multiplier,
        runtime_multiplier,
        include_jobs: correction_opts.include_jobs.clone(),
        dry_run: false,
        no_downsize: true,
    };
    let real_result = apply_resource_corrections(&correction_ctx, &real_opts)?;

    // Run recovery hook if applicable
    if !unknown_job_ids.is_empty()
        && let Some(ref hook_cmd) = args.recovery_hook
    {
        info!("Running recovery hook...");
        run_recovery_hook(args.workflow_id, hook_cmd)?;
    }

    // Reset failed jobs
    info!("Resetting {} job(s) for retry...", all_jobs_to_retry.len());
    reset_failed_jobs(config, args.workflow_id, &all_jobs_to_retry)?;

    // Reinitialize workflow
    info!("Reinitializing workflow...");
    reinitialize_workflow(config, args.workflow_id)?;

    // Submit Slurm schedulers
    match &scheduler_choice {
        SchedulerChoice::Regenerate {
            partition,
            walltime,
        } => {
            info!("Regenerating and submitting Slurm schedulers...");
            regenerate_and_submit(
                args.workflow_id,
                &args.output_dir,
                partition.as_deref(),
                walltime.as_deref(),
            )?;
        }
        SchedulerChoice::Existing {
            scheduler_id,
            num_allocations,
            ..
        } => {
            info!(
                "Submitting {} allocation(s) with scheduler ID {}...",
                num_allocations, scheduler_id
            );
            submit_existing_scheduler(
                args.workflow_id,
                *scheduler_id,
                *num_allocations,
                &args.output_dir,
            )?;
        }
    }

    eprintln!(
        "\nRecovery complete. {} job(s) reset for retry.",
        all_jobs_to_retry.len()
    );

    Ok(RecoveryResult {
        oom_fixed: real_result.memory_corrections,
        timeout_fixed: real_result.runtime_corrections,
        unknown_retried: unknown_job_ids.len(),
        other_failures: unknown_jobs.len(),
        jobs_to_retry: all_jobs_to_retry,
        adjustments: real_result.adjustments,
        slurm_dry_run: None,
    })
}

/// User's choice for how to handle Slurm scheduler submission.
enum SchedulerChoice {
    /// Auto-generate new schedulers via `torc slurm regenerate --submit`
    Regenerate {
        partition: Option<String>,
        walltime: Option<String>,
    },
    /// Reuse an existing scheduler config
    Existing {
        scheduler_id: i64,
        scheduler_name: String,
        num_allocations: i32,
    },
}

/// Prompt the user to choose between auto-generating schedulers or reusing an existing one.
fn prompt_scheduler_choice(
    config: &Configuration,
    args: &RecoverArgs,
) -> Result<SchedulerChoice, String> {
    // List existing schedulers for the workflow
    let schedulers = apis::slurm_schedulers_api::list_slurm_schedulers(
        config,
        args.workflow_id,
        None,
        None,
        None,
        None,
    )
    .map_err(|e| format!("Failed to list schedulers: {}", e))?;

    if schedulers.items.is_empty() {
        eprintln!("No existing schedulers found. Will auto-generate new ones.");
        return Ok(SchedulerChoice::Regenerate {
            partition: None,
            walltime: None,
        });
    }

    // Display existing schedulers
    eprintln!("Existing schedulers for this workflow:\n");
    eprintln!(
        "  {:<6} {:<25} {:<14} {:<14} {:<12} {:<6}",
        "ID", "Name", "Account", "Partition", "Walltime", "Nodes"
    );
    eprintln!(
        "  {:<6} {:<25} {:<14} {:<14} {:<12} {:<6}",
        "---", "----", "-------", "---------", "--------", "-----"
    );
    for s in &schedulers.items {
        eprintln!(
            "  {:<6} {:<25} {:<14} {:<14} {:<12} {:<6}",
            s.id.unwrap_or(0),
            truncate(s.name.as_deref().unwrap_or("-"), 25),
            truncate(&s.account, 14),
            s.partition.as_deref().unwrap_or("-"),
            &s.walltime,
            s.nodes,
        );
    }

    eprintln!();
    let choice = prompt_choice(
        "Scheduler: [A]uto-generate new / [E]xisting (enter ID) (default: A): ",
        &["a", "e"],
        "a",
    )?;

    if choice == "a" {
        // Optionally let user specify partition/walltime overrides
        let partition = {
            let input = prompt_line("  Partition override (press Enter to auto-detect): ")?;
            if input.is_empty() { None } else { Some(input) }
        };
        let walltime = {
            let input = prompt_line(
                "  Walltime override (e.g., 04:00:00, press Enter to auto-calculate): ",
            )?;
            if input.is_empty() { None } else { Some(input) }
        };
        return Ok(SchedulerChoice::Regenerate {
            partition,
            walltime,
        });
    }

    // User chose existing scheduler — prompt for ID
    loop {
        let id_input = prompt_line("  Enter scheduler ID: ")?;
        let id = match id_input.parse::<i64>() {
            Ok(id) => id,
            Err(_) => {
                eprintln!("  Invalid ID. Please enter a number.");
                continue;
            }
        };

        let scheduler = match schedulers.items.iter().find(|s| s.id == Some(id)) {
            Some(s) => s,
            None => {
                eprintln!(
                    "  Scheduler ID {} not found. Choose from the list above.",
                    id
                );
                continue;
            }
        };

        // Prompt for walltime override
        let walltime_input = prompt_line(&format!(
            "  Walltime [default: {}] (press Enter to keep): ",
            &scheduler.walltime
        ))?;

        let (final_id, final_name) = if walltime_input.is_empty() {
            (id, scheduler.name.clone().unwrap_or_default())
        } else {
            // Create a new scheduler cloned from the selected one with the new walltime
            eprintln!(
                "  Creating new scheduler with walltime {}...",
                &walltime_input
            );
            let mut new_sched = scheduler.clone();
            new_sched.id = None;
            new_sched.walltime = walltime_input;
            let base_name = scheduler.name.as_deref().unwrap_or("scheduler");
            new_sched.name = Some(format!("{}_recovery", base_name));
            let created = apis::slurm_schedulers_api::create_slurm_scheduler(config, new_sched)
                .map_err(|e| format!("Failed to create scheduler: {}", e))?;
            let new_id = created.id.ok_or("Created scheduler missing ID")?;
            let new_name = created.name.unwrap_or_default();
            eprintln!(
                "  Created scheduler '{}' (ID {}) with walltime {}",
                &new_name, new_id, &created.walltime
            );
            (new_id, new_name)
        };

        // Prompt for number of allocations
        let default_allocs = 1;
        let num_allocations = loop {
            let input = prompt_line(&format!(
                "  Number of allocations [default: {}]: ",
                default_allocs
            ))?;
            if input.is_empty() {
                break default_allocs;
            }
            match input.parse::<i32>() {
                Ok(n) if n > 0 => break n,
                _ => eprintln!("  Please enter a positive integer."),
            }
        };

        return Ok(SchedulerChoice::Existing {
            scheduler_id: final_id,
            scheduler_name: final_name,
            num_allocations,
        });
    }
}

/// Submit allocations using an existing scheduler config via `torc slurm schedule-nodes`.
fn submit_existing_scheduler(
    workflow_id: i64,
    scheduler_id: i64,
    num_allocations: i32,
    output_dir: &Path,
) -> Result<(), String> {
    let mut cmd = torc_command()?;
    let output = cmd
        .args([
            "slurm",
            "schedule-nodes",
            &workflow_id.to_string(),
            "--scheduler-config-id",
            &scheduler_id.to_string(),
            "--num-hpc-jobs",
            &num_allocations.to_string(),
            "-o",
            output_dir.to_str().unwrap_or("torc_output"),
        ])
        .output()
        .map_err(|e| format!("Failed to run slurm schedule-nodes: {}", e))?;

    if !output.stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            info!("  {}", line);
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("slurm schedule-nodes failed: {}", stderr));
    }

    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let prefix: String = s.chars().take(max - 3).collect();
        format!("{}...", prefix)
    }
}
