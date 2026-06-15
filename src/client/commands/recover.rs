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
    ResourceCorrectionResult, apply_resource_corrections,
};
use crate::client::workflow_manager::WorkflowManager;
use crate::config::TorcConfig;
use crate::models::JobStatus;

/// Maximum time to wait for the server's database to become healthy when claiming actions.
const WAIT_FOR_HEALTHY_DATABASE_MINUTES: u64 = 20;

fn torc_command(config: &Configuration) -> Result<Command, String> {
    let mut cmd = if let Ok(path) = std::env::var("TORC_BIN")
        && !path.trim().is_empty()
    {
        // Use the trimmed value: a TORC_BIN with surrounding whitespace passes the emptiness
        // check above but would fail to spawn if passed verbatim.
        Command::new(path.trim())
    } else {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to determine current torc executable: {}", e))?;
        Command::new(current_exe)
    };

    // Forward the resolved server connection settings so the spawned `torc` talks to the same
    // server with the same TLS/auth. The child reads these from env-mapped CLI args, and it
    // inherits our environment — but only ambient env vars, not values the parent received via
    // CLI flags (`--url`, `--tls-*`, `--password`) or `--standalone` (whose ephemeral URL is
    // deliberately kept out of the environment). Setting them explicitly covers both cases.
    cmd.env("TORC_API_URL", &config.base_path);
    if let Some(ref ca_cert) = config.tls.ca_cert_path {
        cmd.env("TORC_TLS_CA_CERT", ca_cert);
    }
    if config.tls.insecure {
        cmd.env("TORC_TLS_INSECURE", "true");
    }
    if let Some(ref cookie_header) = config.cookie_header {
        cmd.env("TORC_COOKIE_HEADER", cookie_header);
    }
    if let Some((_, Some(ref password))) = config.basic_auth {
        cmd.env("TORC_PASSWORD", password);
    }

    Ok(cmd)
}

/// Render an output directory as a `-o` argument, erroring on non-UTF-8 paths instead of silently
/// substituting a different directory.
fn output_dir_arg(output_dir: &Path) -> Result<String, String> {
    output_dir
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Output directory path is not valid UTF-8: {:?}", output_dir))
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
    /// Fixed Slurm partition for regenerated schedulers (bypasses auto-selection).
    /// Only used by the non-interactive path; the wizard prompts for it instead.
    pub partition: Option<String>,
    /// Fixed Slurm walltime for regenerated schedulers (bypasses auto-calculation).
    /// Only used by the non-interactive path; the wizard prompts for it instead.
    pub walltime: Option<String>,
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

/// Whether unknown-cause failures should be retried.
///
/// `--retry-unknown` retries them directly. A `--recovery-hook` also implies retry-unknown:
/// the hook exists to fix unknown failures, so the user clearly intends those jobs to be
/// retried after it runs. Without this implication the hook would run (with real side
/// effects) and recovery would then abort with "no auto-recoverable jobs" because the
/// unknown jobs were never added to the retry set. Both `torc recover` and
/// `torc watch --recover` flow through here, so the rule is applied identically.
pub(crate) fn effective_retry_unknown(retry_unknown: bool, recovery_hook: Option<&str>) -> bool {
    retry_unknown || recovery_hook.is_some()
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

    // A recovery hook implies the user wants unknown failures retried (see
    // effective_retry_unknown); otherwise the hook would run and recovery would abort.
    let retry_unknown = effective_retry_unknown(args.retry_unknown, args.recovery_hook.as_deref());

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
        retry_unknown,
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
        if retry_unknown {
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
            info!("[DRY RUN] No auto-recoverable jobs found.");
        } else {
            info!(
                "[DRY RUN] Would reset {} job(s) for retry",
                result.jobs_to_retry.len()
            );
            info!("[DRY RUN] Would reinitialize workflow");

            // Get the real scheduler plan using slurm regenerate --dry-run --include-job-ids
            info!("[DRY RUN] Slurm schedulers that would be created:");
            match get_scheduler_dry_run(
                config,
                args.workflow_id,
                &args.output_dir,
                &result.jobs_to_retry,
            ) {
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
        run_recovery_hook(config, args.workflow_id, hook_cmd)?;
    }

    // Check if there are any jobs to retry
    if result.jobs_to_retry.is_empty() {
        let mut msg = format!(
            "No auto-recoverable jobs found. {} job(s) failed with unknown causes. \
             Use --retry-unknown to retry jobs with unknown failure causes.",
            result.other_failures
        );
        if let Some(hint) = canceled_jobs_hint(config, args.workflow_id) {
            msg.push(' ');
            msg.push_str(&hint);
        }
        return Err(msg);
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
    regenerate_and_submit(
        config,
        args.workflow_id,
        &args.output_dir,
        args.partition.as_deref(),
        args.walltime.as_deref(),
    )?;

    Ok(result)
}

/// Check that the workflow is quiesced (safe to modify job statuses):
/// - Workflow must be complete or canceled (all jobs in terminal state)
/// - No active compute node workers
/// - No pending or active scheduled compute nodes (Slurm allocations)
///
/// Returns `Err(message)` with a neutral description of what is wrong. Callers can
/// prepend their own action-specific prefix when surfacing the error to the user.
pub(crate) fn check_workflow_quiesced(
    config: &Configuration,
    workflow_id: i64,
) -> Result<(), String> {
    // Check if workflow is complete
    let is_complete = apis::workflows_api::is_workflow_complete(config, workflow_id)
        .map_err(|e| format!("Failed to check workflow completion status: {}", e))?;

    if !is_complete.is_complete && !is_complete.is_canceled {
        return Err(
            "workflow is not complete; wait for all jobs to finish or use \
             'torc workflows cancel' first"
                .to_string(),
        );
    }

    check_no_active_workers(config, workflow_id)
}

/// Check that no workers are active on the workflow:
/// - No active compute node workers
/// - No pending or active scheduled compute nodes (Slurm allocations)
///
/// Unlike [`check_workflow_quiesced`], this does NOT require the workflow to be
/// complete — non-terminal job statuses (uninitialized, blocked, ready) are fine
/// as long as nothing is executing or about to execute.
pub(crate) fn check_no_active_workers(
    config: &Configuration,
    workflow_id: i64,
) -> Result<(), String> {
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
        return Err(
            "there are still active compute nodes; wait for all workers to exit".to_string(),
        );
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
        return Err(
            "there are pending Slurm allocations; wait for them to start or cancel them with \
             'torc slurm cancel'"
                .to_string(),
        );
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
            "there are active Slurm allocations still running; wait for all workers to exit"
                .to_string(),
        );
    }

    Ok(())
}

/// Check that the workflow is in a valid state for recovery:
/// - Workflow must be complete (all jobs in terminal state)
/// - No active workers (compute nodes or scheduled compute nodes)
/// - There must be at least one recoverable job
fn check_recovery_preconditions(config: &Configuration, workflow_id: i64) -> Result<(), String> {
    // Delegate quiescence checks (1–4) to the shared helper
    check_workflow_quiesced(config, workflow_id)
        .map_err(|msg| format!("Cannot recover: {}", msg))?;

    // Check that there are actually recoverable jobs. The reset path
    // (`reset_failed_jobs_only`) resets failed, terminated, canceled, and pending_failed jobs, so
    // accept any of those here — otherwise a workflow whose jobs are only canceled or pending_failed
    // (which passes the completeness/cancellation gate above) would be rejected even though recovery
    // can act on it.
    let recoverable_statuses = [
        crate::models::JobStatus::Failed,
        crate::models::JobStatus::Terminated,
        crate::models::JobStatus::Canceled,
        crate::models::JobStatus::PendingFailed,
    ];
    let mut has_recoverable_jobs = false;
    for status in recoverable_statuses {
        let jobs = apis::jobs_api::list_jobs(
            config,
            workflow_id,
            Some(status), // status
            None,         // needs_file_id
            None,         // upstream_job_id
            None,         // offset
            Some(1),      // limit - just need to know if any exist
            None,         // sort_by
            None,         // reverse_sort
            None,         // include_relationships
            None,         // active_compute_node_id
            None,         // origin_is_set
            None,         // name
            None,         // command
        )
        .map_err(|e| format!("Failed to list {:?} jobs: {}", status, e))?;
        if jobs.total_count > 0 {
            has_recoverable_jobs = true;
            break;
        }
    }

    if !has_recoverable_jobs {
        return Err(
            "No failed, terminated, canceled, or pending_failed jobs to recover. \
             Workflow may have completed successfully."
                .to_string(),
        );
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

/// Count jobs in the given status for a workflow.
fn count_jobs_with_status(
    config: &Configuration,
    workflow_id: i64,
    status: JobStatus,
) -> Result<i64, String> {
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(status),
        None,    // needs_file_id
        None,    // upstream_job_id
        None,    // offset
        Some(1), // limit - just need count
        None,    // sort_by
        None,    // reverse_sort
        None,    // include_relationships
        None,    // active_compute_node_id
        None,    // origin_is_set
        None,    // name
        None,    // command
    )
    .map_err(|e| format!("Failed to list {:?} jobs: {}", status, e))?;

    Ok(jobs.total_count)
}

/// Count jobs in pending_failed status that need AI classification
fn count_pending_failed_jobs(config: &Configuration, workflow_id: i64) -> Result<i64, String> {
    count_jobs_with_status(config, workflow_id, JobStatus::PendingFailed)
}

/// Append a hint about canceled jobs to recovery's "nothing to retry" messaging.
/// Canceled jobs pass `check_recovery_preconditions` (they are recoverable by
/// `reset_failed_jobs`) but never enter `jobs_to_retry`: failure diagnosis is
/// built from job results, and canceled jobs usually have none. Without this
/// hint the user is told nothing can be done when a manual reset would work.
fn canceled_jobs_hint(config: &Configuration, workflow_id: i64) -> Option<String> {
    match count_jobs_with_status(config, workflow_id, JobStatus::Canceled) {
        Ok(n) if n > 0 => Some(format!(
            "{} canceled job(s) are not auto-recovered; rerun them with \
             'torc jobs reset-status <job_ids> --reinit'.",
            n
        )),
        _ => None,
    }
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
    build_results_report(config, Some(workflow_id), output_dir, false, false, &[])
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

    // Count other failures for recovery report. "Unknown" means no correctable
    // violation at all: CPU/runtime violators are corrected (and retried) by
    // apply_resource_corrections just like OOM/timeout, so classifying them as
    // unknown here would add them to jobs_to_retry a second time via
    // unknown_job_ids and misreport them as unknown-cause failures.
    let mut other_failures = 0;
    let mut unknown_job_ids = Vec::new();

    for violation in &diagnosis.resource_violations {
        if !violation.memory_violation
            && !violation.likely_timeout
            && !violation.likely_cpu_violation
            && !violation.likely_runtime_violation
        {
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
    // A job must be reset only once even if it shows up in more than one source
    // list (a duplicate would be skipped by reset_failed_jobs with a spurious
    // "not recoverable" warning after the first reset succeeds).
    jobs_to_retry.sort_unstable();
    jobs_to_retry.dedup();

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

/// Build a capped, human-readable summary of jobs that could not be reset. The
/// full per-job list is logged separately; this keeps the returned error from
/// ballooning when many job IDs are passed or API errors carry verbose payloads.
fn summarize_not_reset(total: usize, reset_count: usize, not_reset: &[String]) -> String {
    const MAX_DETAIL: usize = 5;
    let shown = not_reset.len().min(MAX_DETAIL);
    let mut msg = format!(
        "Reset {} of {} job(s); {} skipped or failed: {}",
        reset_count,
        total,
        not_reset.len(),
        not_reset[..shown].join("; ")
    );
    if not_reset.len() > shown {
        msg.push_str(&format!(
            " ({} more not shown; see logs)",
            not_reset.len() - shown
        ));
    }
    msg
}

/// Reset specific failed jobs for retry (without reinitializing).
///
/// Best-effort: every job ID is attempted, accumulating a per-job reason for any
/// that is skipped (wrong workflow / non-recoverable status) or fails (fetch or
/// reset error). Returns `Ok(reset_count)` as long as at least one job was reset
/// -- partial success is not an error -- and `Err` only when nothing could be
/// reset. Skips and failures are always logged.
pub fn reset_failed_jobs(
    config: &Configuration,
    workflow_id: i64,
    job_ids: &[i64],
) -> Result<usize, String> {
    if job_ids.is_empty() {
        return Ok(0);
    }

    // Reset the selected jobs.
    // NOTE: do not reset workflow status here. `reset_workflow_status` bumps
    // run_id, and every caller of this function follows it with
    // `reinitialize_workflow`, which already resets workflow status (and bumps
    // run_id) exactly once. Resetting here too bumps run_id twice per recovery,
    // leaving a gap (e.g. a recovered job jumping from run 1 to run 3). We pass
    // the current run_id (no bump) so the single bump stays with reinitialize.
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .map_err(|e| format!("Failed to fetch workflow for reset: {}", e))?
        .run_id
        .unwrap_or(1);

    // Statuses that recovery is allowed to reset. Mirrors
    // `check_recovery_preconditions`; resetting a job in any other state (e.g. a
    // still-running or already-completed job) would be unexpected, so we skip it
    // and report rather than silently clobber it.
    let recoverable_statuses = [
        JobStatus::Failed,
        JobStatus::Terminated,
        JobStatus::Canceled,
        JobStatus::PendingFailed,
    ];

    // Deduplicate defensively: a repeated ID would reset once and then burn an
    // extra get_job/manage_status_change round-trip to report a misleading
    // "status Uninitialized is not recoverable" skip for its duplicate.
    let mut job_ids = job_ids.to_vec();
    job_ids.sort_unstable();
    job_ids.dedup();

    // Attempt every reset, accumulating the reason any job was not reset instead
    // of bailing on the first problem. There is no server-side bulk/atomic reset
    // endpoint, so a mid-loop early return would leave the workflow partially
    // reset with no report of what succeeded.
    // PERF: make a new API endpoint to do this in one command.
    let mut reset_count = 0;
    let mut not_reset: Vec<String> = Vec::new();
    for &job_id in &job_ids {
        // Validate the job before touching it. `manage_status_change` is not
        // workflow-scoped, so without this check a stray job_id could reset a
        // job in a different workflow.
        let job = match apis::jobs_api::get_job(config, job_id) {
            Ok(job) => job,
            Err(e) => {
                not_reset.push(format!("job {}: failed to fetch ({})", job_id, e));
                continue;
            }
        };
        if job.workflow_id != workflow_id {
            not_reset.push(format!(
                "job {}: belongs to workflow {}, not {}; refusing to reset",
                job_id, job.workflow_id, workflow_id
            ));
            continue;
        }
        match job.status {
            Some(status) if recoverable_statuses.contains(&status) => {}
            other => {
                not_reset.push(format!(
                    "job {}: status {:?} is not recoverable; skipped",
                    job_id, other
                ));
                continue;
            }
        }

        match apis::jobs_api::manage_status_change(config, job_id, JobStatus::Uninitialized, run_id)
        {
            Ok(_) => reset_count += 1,
            Err(e) => not_reset.push(format!("job {}: reset failed ({})", job_id, e)),
        }
    }
    // Not "failed job(s)": the recoverable allow-list above also admits
    // terminated, canceled, and pending_failed jobs.
    info!(
        "  Reset {} job(s) for workflow {}",
        reset_count, workflow_id
    );

    if !not_reset.is_empty() {
        // Log the full list for diagnosis regardless of outcome; the returned
        // message is capped so a large `job_ids` (or verbose API errors) can't
        // produce an oversized CLI error.
        for reason in &not_reset {
            warn!("Job not reset (workflow {}): {}", workflow_id, reason);
        }

        // Only a hard error when *nothing* was reset -- a total failure the
        // caller should abort on. Partial success returns Ok(reset_count): the
        // callers propagate with `?` and go on to reinitialize, so turning a
        // partial reset into an Err would abort recovery and strand the workflow
        // in a partially-reset state. The skips/failures are surfaced via the
        // warnings above.
        if reset_count == 0 {
            return Err(summarize_not_reset(job_ids.len(), reset_count, &not_reset));
        }
    }

    Ok(reset_count)
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
pub fn run_recovery_hook(
    config: &Configuration,
    workflow_id: i64,
    hook_command: &str,
) -> Result<(), String> {
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

    // Mirror the env surface JobRunner exposes to per-job recovery scripts so
    // workflow-level hooks can also tag artifacts by run and call the API.
    // Best-effort fetch of run_id: if the lookup fails we still run the hook
    // with TORC_WORKFLOW_ID + TORC_API_URL set; we just skip TORC_RUN_ID.
    cmd.env("TORC_WORKFLOW_ID", workflow_id.to_string());
    cmd.env("TORC_API_URL", &config.base_path);
    match apis::workflows_api::get_workflow(config, workflow_id) {
        Ok(workflow) => {
            cmd.env("TORC_RUN_ID", workflow.run_id.unwrap_or(0).to_string());
        }
        Err(e) => {
            warn!(
                "Failed to fetch workflow_id={} for recovery hook env: {} - TORC_RUN_ID will not be set",
                workflow_id, e
            );
        }
    }

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
    config: &Configuration,
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
        output_dir_arg(output_dir)?,
    ];
    if let Some(p) = partition {
        args.push("--partition".to_string());
        args.push(p.to_string());
    }
    if let Some(w) = walltime {
        args.push("--walltime".to_string());
        args.push(w.to_string());
    }
    let mut cmd = torc_command(config)?;
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
    config: &Configuration,
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

    let output_dir_str = output_dir_arg(output_dir)?;
    let mut cmd = torc_command(config)?;
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
            &output_dir_str,
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

const MAX_DISPLAY_ROWS: usize = 500;

/// Print a list of names, one per line, truncated to `max` entries.
fn print_truncated_names<S: AsRef<str>>(names: &[S], max: usize) {
    for name in names.iter().take(max) {
        eprintln!("    {}", name.as_ref());
    }
    if names.len() > max {
        eprintln!("    ... and {} more not shown", names.len() - max);
    }
}

/// Read a line from `reader`, trimmed, writing `prompt` to `writer` first. Returns the
/// default (empty string) if the user presses Enter. Errors on EOF (e.g. Ctrl-D): no more
/// input will ever arrive, so aborting beats prompt loops re-prompting forever on empty reads.
///
/// The reader/writer are injected so the wizard's input parsing can be unit-tested; the
/// public [`prompt_line`] wrapper binds them to stdin/stderr.
fn prompt_line_from<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
) -> Result<String, String> {
    write!(writer, "{}", prompt).ok();
    writer.flush().ok();
    let mut buf = String::new();
    let bytes_read = reader
        .read_line(&mut buf)
        .map_err(|e| format!("Failed to read input: {}", e))?;
    if bytes_read == 0 {
        return Err("Input stream closed (EOF); recovery cancelled".to_string());
    }
    Ok(buf.trim().to_string())
}

/// Prompt the user for a choice over `reader`/`writer`. `valid` lists accepted single-char
/// answers (lowercase). Returns the default if the user presses Enter.
fn prompt_choice_from<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    valid: &[&str],
    default: &str,
) -> Result<String, String> {
    loop {
        let input = prompt_line_from(reader, writer, prompt)?;
        let answer = if input.is_empty() {
            default.to_string()
        } else {
            input.to_lowercase()
        };
        if valid.contains(&answer.as_str()) {
            return Ok(answer);
        }
        writeln!(
            writer,
            "  Invalid choice '{}'. Valid options: {}",
            answer,
            valid.join(", ")
        )
        .ok();
    }
}

/// Prompt for a floating-point multiplier over `reader`/`writer` with a default value.
/// Only accepts strictly positive numbers.
fn prompt_multiplier_from<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: f64,
) -> Result<f64, String> {
    loop {
        let input = prompt_line_from(
            reader,
            writer,
            &format!("  Enter {} multiplier [default: {}]: ", label, default),
        )?;
        if input.is_empty() {
            return Ok(default);
        }
        match input.parse::<f64>() {
            Ok(v) if v > 0.0 => return Ok(v),
            _ => writeln!(writer, "  Please enter a positive number.").ok(),
        };
    }
}

/// Read a line from stdin, trimmed, prompting on stderr. See [`prompt_line_from`].
fn prompt_line(prompt: &str) -> Result<String, String> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut writer = io::stderr();
    prompt_line_from(&mut reader, &mut writer, prompt)
}

/// Prompt the user for a choice on stdin/stderr. See [`prompt_choice_from`].
fn prompt_choice(prompt: &str, valid: &[&str], default: &str) -> Result<String, String> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut writer = io::stderr();
    prompt_choice_from(&mut reader, &mut writer, prompt, valid, default)
}

/// Prompt for a floating-point multiplier on stdin/stderr. See [`prompt_multiplier_from`].
fn prompt_multiplier(label: &str, default: f64) -> Result<f64, String> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut writer = io::stderr();
    prompt_multiplier_from(&mut reader, &mut writer, label, default)
}

/// Failed-job resource violations grouped by failure cause for the interactive wizard.
struct CategorizedViolations<'a> {
    oom: Vec<&'a crate::client::report_models::ResourceViolationInfo>,
    timeout: Vec<&'a crate::client::report_models::ResourceViolationInfo>,
    cpu: Vec<&'a crate::client::report_models::ResourceViolationInfo>,
    unknown: Vec<&'a crate::client::report_models::ResourceViolationInfo>,
}

/// Categorize resource violations by failure cause. The precedence (memory, then
/// timeout/runtime, then cpu, then unknown) and the "unknown = no flags set" definition
/// mirror the non-interactive path's classification in [`apply_recovery_heuristics`], so
/// both surfaces agree on which jobs are correctable vs unknown-cause. Pure (no I/O) so it
/// can be unit-tested.
fn categorize_violations(
    violations: &[crate::client::report_models::ResourceViolationInfo],
) -> CategorizedViolations<'_> {
    let mut categorized = CategorizedViolations {
        oom: Vec::new(),
        timeout: Vec::new(),
        cpu: Vec::new(),
        unknown: Vec::new(),
    };
    for v in violations {
        if v.memory_violation {
            categorized.oom.push(v);
        } else if v.likely_timeout || v.likely_runtime_violation {
            // likely_runtime_violation (exec > 100% of runtime) implies likely_timeout
            // (exec > 90%) today, but check it explicitly so the wizard's classification
            // matches the non-interactive path even if the diagnosis thresholds drift apart.
            categorized.timeout.push(v);
        } else if v.likely_cpu_violation {
            categorized.cpu.push(v);
        } else {
            categorized.unknown.push(v);
        }
    }
    categorized
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

    // Categorize violations. CPU-only violators get their own category so they
    // are offered the same correct-and-retry treatment as OOM/timeout (matching
    // the non-interactive path) instead of being lumped in with unknown-cause
    // failures and retried without a CPU correction.
    let CategorizedViolations {
        oom: oom_jobs,
        timeout: timeout_jobs,
        cpu: cpu_jobs,
        unknown: unknown_jobs,
    } = categorize_violations(&diagnosis.resource_violations);

    if oom_jobs.is_empty()
        && timeout_jobs.is_empty()
        && cpu_jobs.is_empty()
        && unknown_jobs.is_empty()
    {
        eprintln!("No failed jobs with resource violations found.");
        if let Some(hint) = canceled_jobs_hint(config, args.workflow_id) {
            eprintln!("{}", hint);
        }
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
        for v in oom_jobs.iter().take(MAX_DISPLAY_ROWS) {
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
        if oom_jobs.len() > MAX_DISPLAY_ROWS {
            eprintln!(
                "  ... and {} more OOM failures not shown",
                oom_jobs.len() - MAX_DISPLAY_ROWS
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
        for v in timeout_jobs.iter().take(MAX_DISPLAY_ROWS) {
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
        if timeout_jobs.len() > MAX_DISPLAY_ROWS {
            eprintln!(
                "  ... and {} more timeout failures not shown",
                timeout_jobs.len() - MAX_DISPLAY_ROWS
            );
        }
        eprintln!();
    }

    if !cpu_jobs.is_empty() {
        eprintln!(
            "CPU Over-utilization Failures ({} job{}):",
            cpu_jobs.len(),
            plural(cpu_jobs.len())
        );
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<6} Peak CPU",
            "ID", "Name", "RC", "CPUs"
        );
        eprintln!(
            "  {:<8} {:<30} {:<6} {:<6} --------",
            "---", "----", "---", "----"
        );
        for v in cpu_jobs.iter().take(MAX_DISPLAY_ROWS) {
            eprintln!(
                "  {:<8} {:<30} {:<6} {:<6} {}",
                v.job_id,
                truncate(&v.job_name, 30),
                v.return_code,
                v.configured_cpus,
                v.peak_cpu_percent
                    .map(|p| format!("{:.1}%", p))
                    .as_deref()
                    .unwrap_or("-"),
            );
        }
        if cpu_jobs.len() > MAX_DISPLAY_ROWS {
            eprintln!(
                "  ... and {} more CPU over-utilization failures not shown",
                cpu_jobs.len() - MAX_DISPLAY_ROWS
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
        for v in unknown_jobs.iter().take(MAX_DISPLAY_ROWS) {
            eprintln!(
                "  {:<8} {:<30} {:<6} {:<10}",
                v.job_id,
                truncate(&v.job_name, 30),
                v.return_code,
                &v.configured_memory,
            );
        }
        if unknown_jobs.len() > MAX_DISPLAY_ROWS {
            eprintln!(
                "  ... and {} more unknown failures not shown",
                unknown_jobs.len() - MAX_DISPLAY_ROWS
            );
        }
        eprintln!();
    }

    // --- Per-category decisions -----------------------------------------------
    let mut memory_multiplier = args.memory_multiplier;
    let mut runtime_multiplier = args.runtime_multiplier;
    let mut include_oom = false;
    let mut include_timeout = false;
    let mut include_cpu = false;
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

    if !cpu_jobs.is_empty() {
        let choice = prompt_choice(
            &format!(
                "CPU over-utilization failures ({} job{}): [R]etry with corrected CPUs / [S]kip (default: R): ",
                cpu_jobs.len(),
                plural(cpu_jobs.len()),
            ),
            &["r", "s"],
            "r",
        )?;
        if choice == "r" {
            include_cpu = true;
        } else {
            eprintln!("  Skipping CPU over-utilization jobs.");
        }
    }

    if !unknown_jobs.is_empty() {
        // A recovery hook is meant to fix unknown failures, so default to retrying them
        // when one is configured — consistent with effective_retry_unknown on the
        // non-interactive path.
        let unknown_default = if args.recovery_hook.is_some() {
            "r"
        } else {
            "s"
        };
        let choice = prompt_choice(
            &format!(
                "Unknown failures ({} job{}): [R]etry as-is / [S]kip (default: {}): ",
                unknown_jobs.len(),
                plural(unknown_jobs.len()),
                unknown_default.to_uppercase(),
            ),
            &["r", "s"],
            unknown_default,
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
    if include_cpu {
        correction_job_ids.extend(cpu_jobs.iter().map(|v| v.job_id));
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
    let correction_result = if !correction_opts.include_jobs.is_empty() {
        apply_resource_corrections(&correction_ctx, &correction_opts)?
    } else {
        ResourceCorrectionResult::default()
    };

    // --- Show proposed changes and confirm ------------------------------------
    eprintln!("\n--- Recovery Plan ---\n");

    if !correction_result.adjustments.is_empty() {
        for adj in &correction_result.adjustments {
            if adj.memory_adjusted {
                eprintln!(
                    "  Memory: {} -> {} ({}x) for {} job{}",
                    adj.original_memory.as_deref().unwrap_or("?"),
                    adj.new_memory.as_deref().unwrap_or("?"),
                    memory_multiplier,
                    adj.job_names.len(),
                    plural(adj.job_names.len()),
                );
                print_truncated_names(&adj.job_names, MAX_DISPLAY_ROWS);
            }
            if adj.runtime_adjusted {
                eprintln!(
                    "  Runtime: {} -> {} ({}x) for {} job{}",
                    adj.original_runtime.as_deref().unwrap_or("?"),
                    adj.new_runtime.as_deref().unwrap_or("?"),
                    runtime_multiplier,
                    adj.job_names.len(),
                    plural(adj.job_names.len()),
                );
                print_truncated_names(&adj.job_names, MAX_DISPLAY_ROWS);
            }
            if adj.cpu_adjusted {
                eprintln!(
                    "  CPUs: {} -> {} for {} job{}",
                    adj.original_cpus
                        .map(|c| c.to_string())
                        .as_deref()
                        .unwrap_or("?"),
                    adj.new_cpus
                        .map(|c| c.to_string())
                        .as_deref()
                        .unwrap_or("?"),
                    adj.job_names.len(),
                    plural(adj.job_names.len()),
                );
                print_truncated_names(&adj.job_names, MAX_DISPLAY_ROWS);
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
            "  Retry as-is: {} job{}",
            unknown_job_ids.len(),
            plural(unknown_job_ids.len()),
        );
        print_truncated_names(&unknown_names, MAX_DISPLAY_ROWS);
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
        let slurm_dry_run = match get_scheduler_dry_run(
            config,
            args.workflow_id,
            &args.output_dir,
            &all_jobs_to_retry,
        ) {
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
            source,
            num_allocations,
            start_one_worker_per_node,
        } => {
            eprintln!(
                "\n  Scheduler: {}, {} allocation(s)",
                source.display_label(),
                num_allocations
            );
            if *start_one_worker_per_node {
                eprintln!("  Start one worker per node: yes");
            }
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
    let real_result = if !real_opts.include_jobs.is_empty() {
        apply_resource_corrections(&correction_ctx, &real_opts)?
    } else {
        ResourceCorrectionResult::default()
    };

    // Run recovery hook if applicable
    if !unknown_job_ids.is_empty()
        && let Some(ref hook_cmd) = args.recovery_hook
    {
        info!("Running recovery hook...");
        run_recovery_hook(config, args.workflow_id, hook_cmd)?;
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
                config,
                args.workflow_id,
                &args.output_dir,
                partition.as_deref(),
                walltime.as_deref(),
            )?;
        }
        SchedulerChoice::Existing {
            source,
            num_allocations,
            start_one_worker_per_node,
        } => {
            // Now that the user has confirmed, materialize the scheduler (creating the
            // walltime-override clone if one was deferred — see ExistingSchedulerSource).
            let (scheduler_id, _scheduler_name) =
                resolve_existing_scheduler_source(config, source)?;

            // Reinitialization above re-armed the workflow's on_workflow_start schedule_nodes
            // action. Mark it executed before submitting so it doesn't re-fire on the first
            // compute node and submit the action's original allocation count instead of the
            // user's choice. (The Regenerate path does the equivalent inside handle_regenerate.)
            mark_on_workflow_start_schedule_actions_executed(config, args.workflow_id)?;
            info!(
                "Submitting {} allocation(s) with scheduler ID {}...",
                num_allocations, scheduler_id
            );
            submit_existing_scheduler(
                config,
                args.workflow_id,
                scheduler_id,
                *num_allocations,
                *start_one_worker_per_node,
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
    /// Reuse an existing scheduler config (possibly a deferred clone)
    Existing {
        source: ExistingSchedulerSource,
        num_allocations: i32,
        start_one_worker_per_node: bool,
    },
}

/// Where an `Existing` scheduler submission gets its scheduler config.
///
/// A walltime override clones the selected scheduler, but that database write is DEFERRED
/// (see [`resolve_existing_scheduler_source`]) until after the user confirms recovery, so
/// declining at the confirmation prompt leaves no orphaned `*_recovery` scheduler behind.
enum ExistingSchedulerSource {
    /// Reuse an existing scheduler config by ID.
    Existing { id: i64, name: String },
    /// Create a new scheduler cloned from `base` with `walltime`, after confirmation.
    CloneWithWalltime {
        base: Box<crate::models::SlurmSchedulerModel>,
        walltime: String,
    },
}

impl ExistingSchedulerSource {
    /// Human-readable label for the confirmation prompt (before any clone is created).
    fn display_label(&self) -> String {
        match self {
            ExistingSchedulerSource::Existing { id, name } => format!("{} (ID {})", name, id),
            ExistingSchedulerSource::CloneWithWalltime { base, walltime } => format!(
                "{}_recovery (new scheduler, walltime {})",
                base.name.as_deref().unwrap_or("scheduler"),
                walltime
            ),
        }
    }
}

/// Resolve an [`ExistingSchedulerSource`] to a concrete `(scheduler_id, name)`, creating the
/// cloned scheduler now if one was deferred. Called only after the user confirms recovery.
fn resolve_existing_scheduler_source(
    config: &Configuration,
    source: &ExistingSchedulerSource,
) -> Result<(i64, String), String> {
    match source {
        ExistingSchedulerSource::Existing { id, name } => Ok((*id, name.clone())),
        ExistingSchedulerSource::CloneWithWalltime { base, walltime } => {
            eprintln!("  Creating new scheduler with walltime {}...", walltime);
            let mut new_sched = (**base).clone();
            new_sched.id = None;
            new_sched.walltime = walltime.clone();
            let base_name = base.name.as_deref().unwrap_or("scheduler");
            new_sched.name = Some(format!("{}_recovery", base_name));
            let created = apis::slurm_schedulers_api::create_slurm_scheduler(config, new_sched)
                .map_err(|e| format!("Failed to create scheduler: {}", e))?;
            let new_id = created.id.ok_or("Created scheduler missing ID")?;
            let new_name = created.name.unwrap_or_default();
            eprintln!(
                "  Created scheduler '{}' (ID {}) with walltime {}",
                &new_name, new_id, &created.walltime
            );
            Ok((new_id, new_name))
        }
    }
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

        // Prompt for walltime override. A clone is NOT created here: we capture the intent
        // and defer the database write to resolve_existing_scheduler_source, which runs only
        // after the user confirms recovery (so declining leaves no orphaned scheduler).
        let walltime_input = prompt_line(&format!(
            "  Walltime [default: {}] (press Enter to keep): ",
            &scheduler.walltime
        ))?;

        let source = if walltime_input.is_empty() {
            ExistingSchedulerSource::Existing {
                id,
                name: scheduler.name.clone().unwrap_or_default(),
            }
        } else {
            ExistingSchedulerSource::CloneWithWalltime {
                base: Box::new(scheduler.clone()),
                walltime: walltime_input,
            }
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

        // Prompt for start_one_worker_per_node if multi-node scheduler and direct mode.
        // start_one_worker_per_node is only valid when execution_config.mode is "direct".
        let start_one_worker_per_node = if scheduler.nodes > 1 {
            let workflow = apis::workflows_api::get_workflow(config, args.workflow_id)
                .map_err(|e| format!("Failed to fetch workflow to check execution mode: {}", e))?;
            let exec_config =
                crate::client::workflow_spec::ExecutionConfig::from_workflow_model(&workflow);
            if exec_config.mode == crate::client::workflow_spec::ExecutionMode::Direct {
                let choice =
                    prompt_choice("  Start one worker per node? (y/N): ", &["y", "n"], "n")?;
                choice == "y"
            } else {
                false
            }
        } else {
            false
        };

        return Ok(SchedulerChoice::Existing {
            source,
            num_allocations,
            start_one_worker_per_node,
        });
    }
}

/// Mark the workflow's `on_workflow_start` `schedule_nodes` actions as executed.
///
/// `reinitialize_workflow` re-arms these actions (the server clears their `executed` flag and
/// re-triggers them). When recovery reuses an existing scheduler and submits a user-chosen number
/// of allocations directly via `torc slurm schedule-nodes`, a re-armed action would otherwise fire
/// again on the first compute node that starts and submit the action's original `num_allocations`,
/// ignoring the user's entry. Claiming the actions here marks them executed and prevents that
/// duplicate, action-defined submission. The `regenerate` path performs the equivalent step inside
/// `handle_regenerate`. Recovery actions (`is_recovery`) are left untouched.
pub fn mark_on_workflow_start_schedule_actions_executed(
    config: &Configuration,
    workflow_id: i64,
) -> Result<(), String> {
    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .map_err(|e| format!("Failed to list workflow actions: {}", e))?;

    for action in actions {
        if action.trigger_type == "on_workflow_start"
            && action.action_type == "schedule_nodes"
            && !action.is_recovery
            && !action.executed
            && let Some(action_id) = action.id
        {
            // Delegate to the shared helper, which treats a 409 Conflict (already claimed by
            // another process) as `Ok(false)` and surfaces any other failure as an error. We
            // propagate that error rather than logging and continuing: if the claim genuinely
            // fails, the action stays armed and would re-fire on a compute node, reintroducing
            // the duplicate-allocation bug — better to stop and report it.
            match crate::client::utils::claim_action(
                config,
                workflow_id,
                action_id,
                None, // login-node submission, no compute node
                WAIT_FOR_HEALTHY_DATABASE_MINUTES,
            ) {
                Ok(true) => {
                    info!(
                        "Marked on_workflow_start schedule_nodes action {} as executed to avoid duplicate allocations",
                        action_id
                    );
                }
                Ok(false) => {
                    debug!("Action {} already claimed", action_id);
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to mark on_workflow_start schedule_nodes action {} as executed: {}",
                        action_id, e
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Submit allocations using an existing scheduler config via `torc slurm schedule-nodes`.
fn submit_existing_scheduler(
    config: &Configuration,
    workflow_id: i64,
    scheduler_id: i64,
    num_allocations: i32,
    start_one_worker_per_node: bool,
    output_dir: &Path,
) -> Result<(), String> {
    let mut cmd = torc_command(config)?;
    let mut args = vec![
        "slurm".to_string(),
        "schedule-nodes".to_string(),
        workflow_id.to_string(),
        "--scheduler-config-id".to_string(),
        scheduler_id.to_string(),
        "--num-hpc-jobs".to_string(),
        num_allocations.to_string(),
        "-o".to_string(),
        output_dir_arg(output_dir)?,
    ];
    if start_one_worker_per_node {
        args.push("--start-one-worker-per-node".to_string());
    }
    let output = cmd
        .args(&args)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::apis::configuration::TlsConfig;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Collect the env overrides explicitly set on a `Command` (not inherited parent env).
    fn env_overrides(cmd: &Command) -> HashMap<String, String> {
        cmd.get_envs()
            .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
            .collect()
    }

    #[test]
    fn torc_command_forwards_connection_settings() {
        let mut config = Configuration::new();
        config.base_path = "https://example.test:9000/torc-service/v1".to_string();
        config.tls = TlsConfig {
            ca_cert_path: Some(PathBuf::from("/tmp/ca.pem")),
            insecure: false,
        };
        config.cookie_header = Some("session=abc".to_string());
        config.basic_auth = Some(("alice".to_string(), Some("s3cret".to_string())));

        let cmd = torc_command(&config).expect("torc_command");
        let envs = env_overrides(&cmd);

        // URL, TLS CA, cookie, and password are forwarded so the child resolves the same server
        // even when the parent received these via CLI flags rather than the environment.
        assert_eq!(
            envs.get("TORC_API_URL").map(String::as_str),
            Some("https://example.test:9000/torc-service/v1")
        );
        assert_eq!(
            envs.get("TORC_TLS_CA_CERT").map(String::as_str),
            Some("/tmp/ca.pem")
        );
        assert_eq!(
            envs.get("TORC_COOKIE_HEADER").map(String::as_str),
            Some("session=abc")
        );
        assert_eq!(
            envs.get("TORC_PASSWORD").map(String::as_str),
            Some("s3cret")
        );
        // insecure=false must not set the flag (clap would parse it as true).
        assert!(!envs.contains_key("TORC_TLS_INSECURE"));
    }

    #[test]
    fn torc_command_sets_tls_insecure_only_when_enabled() {
        let mut config = Configuration::new();
        config.tls = TlsConfig {
            ca_cert_path: None,
            insecure: true,
        };

        let cmd = torc_command(&config).expect("torc_command");
        let envs = env_overrides(&cmd);

        assert_eq!(
            envs.get("TORC_TLS_INSECURE").map(String::as_str),
            Some("true")
        );
        // Nothing else should be forwarded when not configured.
        assert!(!envs.contains_key("TORC_TLS_CA_CERT"));
        assert!(!envs.contains_key("TORC_PASSWORD"));
        assert!(!envs.contains_key("TORC_COOKIE_HEADER"));
    }

    #[test]
    fn output_dir_arg_accepts_utf8_path() {
        assert_eq!(
            output_dir_arg(Path::new("torc_output")).unwrap(),
            "torc_output"
        );
    }

    #[test]
    #[cfg(unix)]
    fn output_dir_arg_rejects_non_utf8_path() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let bad = OsStr::from_bytes(b"out\xffput");
        assert!(output_dir_arg(Path::new(bad)).is_err());
    }

    // ---- effective_retry_unknown -----------------------------------------------------

    #[test]
    fn retry_unknown_flag_alone_enables_retry() {
        assert!(effective_retry_unknown(true, None));
    }

    #[test]
    fn recovery_hook_implies_retry_unknown() {
        // The core of Bug #2: a hook must imply retry-unknown so it doesn't run then abort.
        assert!(effective_retry_unknown(false, Some("bash fix.sh")));
        assert!(effective_retry_unknown(true, Some("bash fix.sh")));
    }

    #[test]
    fn no_flag_and_no_hook_means_no_retry() {
        assert!(!effective_retry_unknown(false, None));
    }

    // ---- prompt_line_from ------------------------------------------------------------

    /// Drive a prompt helper with scripted stdin, capturing what was written.
    fn run_line(input: &str) -> (Result<String, String>, String) {
        let mut reader = input.as_bytes();
        let mut writer: Vec<u8> = Vec::new();
        let result = prompt_line_from(&mut reader, &mut writer, "prompt> ");
        (result, String::from_utf8(writer).unwrap())
    }

    #[test]
    fn prompt_line_trims_and_returns_value() {
        let (result, written) = run_line("  hello \n");
        assert_eq!(result.unwrap(), "hello");
        // The prompt itself is written to the writer (stderr in production).
        assert!(written.contains("prompt> "));
    }

    #[test]
    fn prompt_line_empty_input_returns_empty_string() {
        let (result, _) = run_line("\n");
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn prompt_line_eof_is_error() {
        // No newline and no data: read_line returns 0 bytes -> EOF error, not an empty string.
        let (result, _) = run_line("");
        assert!(result.unwrap_err().contains("EOF"));
    }

    // ---- prompt_choice_from ----------------------------------------------------------

    fn run_choice(input: &str, valid: &[&str], default: &str) -> Result<String, String> {
        let mut reader = input.as_bytes();
        let mut writer: Vec<u8> = Vec::new();
        prompt_choice_from(&mut reader, &mut writer, "choose: ", valid, default)
    }

    #[test]
    fn prompt_choice_uses_default_on_empty() {
        assert_eq!(run_choice("\n", &["y", "n"], "n").unwrap(), "n");
    }

    #[test]
    fn prompt_choice_is_case_insensitive() {
        assert_eq!(run_choice("Y\n", &["y", "n"], "n").unwrap(), "y");
    }

    #[test]
    fn prompt_choice_reprompts_until_valid() {
        // "maybe" is rejected, then "y" is accepted.
        let mut reader = "maybe\ny\n".as_bytes();
        let mut writer: Vec<u8> = Vec::new();
        let answer =
            prompt_choice_from(&mut reader, &mut writer, "choose: ", &["y", "n"], "n").unwrap();
        assert_eq!(answer, "y");
        let written = String::from_utf8(writer).unwrap();
        assert!(written.contains("Invalid choice 'maybe'"));
    }

    #[test]
    fn prompt_choice_eof_mid_reprompt_is_error() {
        // Invalid answer then EOF (stream closes): must error rather than loop forever.
        assert!(run_choice("nope", &["y", "n"], "n").is_err());
    }

    // ---- prompt_multiplier_from ------------------------------------------------------

    fn run_multiplier(input: &str, default: f64) -> Result<f64, String> {
        let mut reader = input.as_bytes();
        let mut writer: Vec<u8> = Vec::new();
        prompt_multiplier_from(&mut reader, &mut writer, "memory", default)
    }

    #[test]
    fn prompt_multiplier_uses_default_on_empty() {
        assert_eq!(run_multiplier("\n", 1.5).unwrap(), 1.5);
    }

    #[test]
    fn prompt_multiplier_accepts_positive() {
        assert_eq!(run_multiplier("2.0\n", 1.5).unwrap(), 2.0);
    }

    #[test]
    fn prompt_multiplier_rejects_nonpositive_then_accepts() {
        // Zero, negative, and non-numeric are all rejected; the loop continues to "3".
        let answer = run_multiplier("0\n-1\nabc\n3\n", 1.5).unwrap();
        assert_eq!(answer, 3.0);
    }

    // ---- categorize_violations -------------------------------------------------------

    /// Build a violation with all flags off; callers flip the ones they need.
    fn violation(job_id: i64) -> crate::client::report_models::ResourceViolationInfo {
        crate::client::report_models::ResourceViolationInfo {
            job_id,
            job_name: format!("job_{}", job_id),
            return_code: 1,
            exec_time_minutes: 0.0,
            configured_memory: "1g".to_string(),
            configured_runtime: "PT1H".to_string(),
            configured_cpus: 1,
            peak_memory_bytes: None,
            peak_memory_formatted: None,
            memory_violation: false,
            oom_reason: None,
            memory_over_utilization: None,
            likely_timeout: false,
            timeout_reason: None,
            runtime_utilization: None,
            likely_cpu_violation: false,
            peak_cpu_percent: None,
            likely_runtime_violation: false,
        }
    }

    #[test]
    fn categorize_sorts_each_failure_type() {
        let mut mem = violation(1);
        mem.memory_violation = true;
        let mut timeout = violation(2);
        timeout.likely_timeout = true;
        let mut runtime = violation(3);
        runtime.likely_runtime_violation = true;
        let mut cpu = violation(4);
        cpu.likely_cpu_violation = true;
        let unknown = violation(5);

        let all = [mem, timeout, runtime, cpu, unknown];
        let c = categorize_violations(&all);
        assert_eq!(c.oom.iter().map(|v| v.job_id).collect::<Vec<_>>(), [1]);
        // runtime violations join the timeout bucket.
        assert_eq!(
            c.timeout.iter().map(|v| v.job_id).collect::<Vec<_>>(),
            [2, 3]
        );
        assert_eq!(c.cpu.iter().map(|v| v.job_id).collect::<Vec<_>>(), [4]);
        assert_eq!(c.unknown.iter().map(|v| v.job_id).collect::<Vec<_>>(), [5]);
    }

    #[test]
    fn categorize_precedence_memory_wins_over_other_flags() {
        // A job flagged for memory AND cpu AND timeout is classified as OOM (memory first),
        // so it is corrected once, not double-counted across buckets.
        let mut v = violation(1);
        v.memory_violation = true;
        v.likely_timeout = true;
        v.likely_cpu_violation = true;
        let c = categorize_violations(std::slice::from_ref(&v));
        assert_eq!(c.oom.len(), 1);
        assert!(c.timeout.is_empty());
        assert!(c.cpu.is_empty());
        assert!(c.unknown.is_empty());
    }

    #[test]
    fn categorize_no_flags_is_unknown() {
        let all = [violation(9)];
        let c = categorize_violations(&all);
        assert_eq!(c.unknown.iter().map(|v| v.job_id).collect::<Vec<_>>(), [9]);
        assert!(c.oom.is_empty() && c.timeout.is_empty() && c.cpu.is_empty());
    }

    #[test]
    fn categorize_empty_input_is_all_empty() {
        let c = categorize_violations(&[]);
        assert!(
            c.oom.is_empty() && c.timeout.is_empty() && c.cpu.is_empty() && c.unknown.is_empty()
        );
    }

    // ---- summarize_not_reset ---------------------------------------------------------

    #[test]
    fn summarize_not_reset_lists_all_when_under_cap() {
        let not_reset = vec!["job 1: nope".to_string(), "job 2: nope".to_string()];
        let msg = summarize_not_reset(5, 3, &not_reset);
        assert!(msg.contains("Reset 3 of 5"));
        assert!(msg.contains("job 1: nope"));
        assert!(msg.contains("job 2: nope"));
        assert!(!msg.contains("more not shown"));
    }

    #[test]
    fn summarize_not_reset_caps_detail_and_counts_remainder() {
        let not_reset: Vec<String> = (0..8).map(|i| format!("job {}: nope", i)).collect();
        let msg = summarize_not_reset(10, 2, &not_reset);
        // Only the first 5 are shown; the remaining 3 are summarized.
        assert!(msg.contains("job 4: nope"));
        assert!(!msg.contains("job 5: nope"));
        assert!(msg.contains("3 more not shown"));
    }
}
