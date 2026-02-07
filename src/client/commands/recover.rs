//! Shared recovery functionality for Slurm workflows.
//!
//! This module provides the core recovery logic used by both:
//! - `torc recover` standalone command
//! - `torc watch --recover` automatic recovery

use log::{debug, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::slurm::RegenerateDryRunResult;
use crate::client::report_models::{ResourceUtilizationReport, ResultsReport};
use crate::client::resource_correction::{ResourceAdjustmentReport, apply_resource_corrections};
use crate::models::JobStatus;

/// Arguments for workflow recovery
pub struct RecoverArgs {
    pub workflow_id: i64,
    pub output_dir: PathBuf,
    pub memory_multiplier: f64,
    pub runtime_multiplier: f64,
    pub retry_unknown: bool,
    pub recovery_hook: Option<String>,
    pub dry_run: bool,
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

    // Step 2: Diagnose failures
    info!("Diagnosing failures...");
    let diagnosis = diagnose_failures(args.workflow_id, &args.output_dir)?;

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
    reinitialize_workflow(args.workflow_id)?;

    // Step 7: Regenerate Slurm schedulers and submit
    info!("Schedulers regenerating workflow_id={}", args.workflow_id);
    regenerate_and_submit(args.workflow_id, &args.output_dir)?;

    Ok(result)
}

/// Check that the workflow is in a valid state for recovery:
/// - Workflow must be complete (all jobs in terminal state)
/// - No active workers (compute nodes or scheduled compute nodes)
fn check_recovery_preconditions(config: &Configuration, workflow_id: i64) -> Result<(), String> {
    // Check if workflow is complete
    let is_complete = default_api::is_workflow_complete(config, workflow_id)
        .map_err(|e| format!("Failed to check workflow completion status: {}", e))?;

    if !is_complete.is_complete && !is_complete.is_canceled {
        return Err("Cannot recover: workflow is not complete. \
             Wait for all jobs to finish or use 'torc workflows cancel' first."
            .to_string());
    }

    // Check for active compute nodes
    let active_nodes = default_api::list_compute_nodes(
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

    if let Some(nodes) = active_nodes.items
        && !nodes.is_empty()
    {
        return Err("Cannot recover: there are still active compute nodes. \
             Wait for all workers to exit."
            .to_string());
    }

    // Check for pending/active scheduled compute nodes
    let pending_scn = default_api::list_scheduled_compute_nodes(
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

    let active_scn = default_api::list_scheduled_compute_nodes(
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
    let failed_jobs = default_api::list_jobs(
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

    let terminated_jobs = default_api::list_jobs(
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
    let pending_failed_jobs = default_api::list_jobs(
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
    workflow_id: i64,
    _output_dir: &Path,
) -> Result<ResourceUtilizationReport, String> {
    let output = Command::new("torc")
        .args([
            "-f",
            "json",
            "reports",
            "check-resource-utilization",
            &workflow_id.to_string(),
            "--include-failed",
        ])
        .output()
        .map_err(|e| format!("Failed to run check-resource-utilization: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("check-resource-utilization failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse resource utilization output: {}", e))
}

/// Get Slurm log information for failed jobs
fn get_slurm_log_info(workflow_id: i64, output_dir: &Path) -> Result<ResultsReport, String> {
    let output = Command::new("torc")
        .args([
            "-f",
            "json",
            "reports",
            "results",
            &workflow_id.to_string(),
            "-o",
            output_dir.to_str().unwrap_or("output"),
        ])
        .output()
        .map_err(|e| format!("Failed to run reports results: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("reports results failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse reports results output: {}", e))
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
    let slurm_log_map = match get_slurm_log_info(workflow_id, output_dir) {
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
        if !violation.likely_oom && !violation.likely_timeout {
            other_failures += 1;
            if retry_unknown {
                unknown_job_ids.push(violation.job_id);
            }
        }
    }

    // Call shared resource correction algorithm
    let correction_result = apply_resource_corrections(
        config,
        workflow_id,
        diagnosis,
        memory_multiplier,
        runtime_multiplier,
        &[], // include_jobs empty means all jobs
        dry_run,
    )?;

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
    _config: &Configuration,
    workflow_id: i64,
    job_ids: &[i64],
) -> Result<usize, String> {
    if job_ids.is_empty() {
        return Ok(0);
    }

    let job_count = job_ids.len();

    // Reset failed jobs WITHOUT --reinitialize (we'll reinitialize separately)
    let output = Command::new("torc")
        .args([
            "workflows",
            "reset-status",
            &workflow_id.to_string(),
            "--failed-only",
            "--no-prompts",
        ])
        .output()
        .map_err(|e| format!("Failed to run workflow reset-status: {}", e))?;

    // Print stdout so user sees what was reset
    if !output.stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            info!("  {}", line);
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("workflow reset-status failed: {}", stderr));
    }

    Ok(job_count)
}

/// Reinitialize the workflow (set up dependencies and fire on_workflow_start actions)
pub fn reinitialize_workflow(workflow_id: i64) -> Result<(), String> {
    let output = Command::new("torc")
        .args(["workflows", "reinitialize", &workflow_id.to_string()])
        .output()
        .map_err(|e| format!("Failed to run workflow reinitialize: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("workflow reinitialize failed: {}", stderr));
    }

    Ok(())
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
pub fn regenerate_and_submit(workflow_id: i64, output_dir: &Path) -> Result<(), String> {
    let output = Command::new("torc")
        .args([
            "slurm",
            "regenerate",
            &workflow_id.to_string(),
            "--submit",
            "-o",
            output_dir.to_str().unwrap_or("output"),
        ])
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

    let output = Command::new("torc")
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
            output_dir.to_str().unwrap_or("output"),
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
