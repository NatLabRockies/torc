//! Workflow scheduling diagnostics.
//!
//! Surfaces *why* ready jobs are not being packed onto running allocations even
//! when CPU/memory/GPU are free. The common, hard-to-diagnose case: a job's
//! required runtime exceeds an allocation's remaining walltime, so Torc refuses
//! to start it (it would be killed mid-run). As allocations age, fewer long jobs
//! fit and node-packing silently degrades.
//!
//! This runs entirely off persisted server state (active `compute_node` rows with
//! their `end_time`, ready jobs, and resource requirements), so it works from a
//! login node or laptop without querying Slurm or touching the live runner.

use crate::client::apis::configuration::Configuration;
use crate::client::commands::pagination::{
    ComputeNodeListParams, JobListParams, ResourceRequirementsListParams, paginate_compute_nodes,
    paginate_jobs, paginate_resource_requirements,
};
use crate::client::commands::{get_env_user_name, print_error, select_workflow_interactively};
use crate::models::JobStatus;
use crate::time_utils::duration_string_to_seconds;
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;

/// Grace period (seconds) the runner adds to remaining walltime when claiming
/// jobs (see `JobRunner::STARTUP_GRACE_PERIOD_SECONDS`). Mirrored here so the
/// diagnosis matches the server's actual `runtime_s <= time_limit` filter.
const STARTUP_GRACE_PERIOD_SECONDS: i64 = 120;

/// Summary statistics for a set of durations (seconds).
#[derive(Debug, Serialize)]
struct DurationStats {
    count: usize,
    min_seconds: i64,
    median_seconds: i64,
    max_seconds: i64,
}

impl DurationStats {
    /// Build stats from a slice of second values. Returns `None` when empty.
    fn from(values: &[i64]) -> Option<DurationStats> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        Some(DurationStats {
            count: sorted.len(),
            min_seconds: sorted[0],
            median_seconds: sorted[sorted.len() / 2],
            max_seconds: sorted[sorted.len() - 1],
        })
    }
}

/// Machine-readable diagnosis result (also drives the human-readable report).
#[derive(Debug, Serialize)]
struct PackingDiagnosis {
    workflow_id: i64,
    active_allocations: usize,
    /// Active allocations that report an `end_time` (Slurm allocations).
    allocations_with_walltime: usize,
    /// Active allocations with no walltime limit (local / unlimited).
    allocations_unlimited: usize,
    ready_jobs: usize,
    /// Ready jobs whose resource requirement has a parseable runtime.
    ready_jobs_with_runtime: usize,
    allocation_remaining: Option<DurationStats>,
    ready_job_runtime: Option<DurationStats>,
    /// Ready jobs whose runtime exceeds the remaining walltime of *every* active
    /// allocation — they cannot start until a fresh allocation appears.
    runtime_blocked_jobs: usize,
    /// Active allocations that cannot run the single longest ready job.
    allocations_too_short_for_longest: usize,
    /// True when runtime is the binding constraint on packing.
    runtime_blocked: bool,
}

/// Entry point for `torc workflows diagnose`.
pub fn diagnose_packing(config: &Configuration, workflow_id: Option<i64>, format: &str) {
    let user = get_env_user_name();
    let workflow_id = match workflow_id {
        Some(id) => id,
        None => match select_workflow_interactively(config, &user) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Error selecting workflow: {}", e);
                std::process::exit(1);
            }
        },
    };

    // Active compute nodes for the workflow.
    let nodes = match paginate_compute_nodes(
        config,
        workflow_id,
        ComputeNodeListParams::new().with_is_active(true),
    ) {
        Ok(nodes) => nodes,
        Err(e) => {
            print_error("listing compute nodes", &e);
            std::process::exit(1);
        }
    };

    // Ready jobs for the workflow.
    let ready_jobs = match paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_status(JobStatus::Ready),
    ) {
        Ok(jobs) => jobs,
        Err(e) => {
            print_error("listing ready jobs", &e);
            std::process::exit(1);
        }
    };

    // Resource requirements: map id -> runtime seconds.
    let requirements = match paginate_resource_requirements(
        config,
        workflow_id,
        ResourceRequirementsListParams::new(),
    ) {
        Ok(reqs) => reqs,
        Err(e) => {
            print_error("listing resource requirements", &e);
            std::process::exit(1);
        }
    };
    let runtime_by_rr: HashMap<i64, i64> = requirements
        .iter()
        .filter_map(|rr| {
            let id = rr.id?;
            let secs = duration_string_to_seconds(&rr.runtime).ok()?;
            Some((id, secs))
        })
        .collect();

    let now = Utc::now();

    // Remaining walltime (seconds) for each active allocation that reports one.
    let mut remaining: Vec<i64> = Vec::new();
    let mut unlimited = 0usize;
    for node in &nodes {
        match node.end_time.as_deref() {
            Some(end_str) => match chrono::DateTime::parse_from_rfc3339(end_str) {
                Ok(end) => {
                    let secs = (end.with_timezone(&Utc) - now).num_seconds().max(0);
                    remaining.push(secs);
                }
                // Unparseable end_time: treat as unknown rather than crash.
                Err(_) => unlimited += 1,
            },
            None => unlimited += 1,
        }
    }

    // Required runtime (seconds) for each ready job we can resolve.
    let ready_runtimes: Vec<i64> = ready_jobs
        .iter()
        .filter_map(|job| {
            let rr_id = job.resource_requirements_id?;
            runtime_by_rr.get(&rr_id).copied()
        })
        .collect();

    let max_remaining = remaining.iter().copied().max();
    let longest_ready = ready_runtimes.iter().copied().max();

    // A job fits an allocation when runtime <= remaining + grace (matching the
    // server's claim filter). Blocked = exceeds the most generous allocation.
    let runtime_blocked_jobs = match max_remaining {
        Some(max_rem) => ready_runtimes
            .iter()
            .filter(|&&r| r > max_rem + STARTUP_GRACE_PERIOD_SECONDS)
            .count(),
        None => 0,
    };

    let allocations_too_short_for_longest = match longest_ready {
        Some(longest) => remaining
            .iter()
            .filter(|&&rem| longest > rem + STARTUP_GRACE_PERIOD_SECONDS)
            .count(),
        None => 0,
    };

    let diagnosis = PackingDiagnosis {
        workflow_id,
        active_allocations: nodes.len(),
        allocations_with_walltime: remaining.len(),
        allocations_unlimited: unlimited,
        ready_jobs: ready_jobs.len(),
        ready_jobs_with_runtime: ready_runtimes.len(),
        allocation_remaining: DurationStats::from(&remaining),
        ready_job_runtime: DurationStats::from(&ready_runtimes),
        runtime_blocked_jobs,
        allocations_too_short_for_longest,
        runtime_blocked: runtime_blocked_jobs > 0,
    };

    if format == "json" {
        match serde_json::to_string_pretty(&diagnosis) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Error serializing diagnosis to JSON: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    print_human_report(&diagnosis);
}

/// Render the human-readable report.
fn print_human_report(d: &PackingDiagnosis) {
    println!("Packing diagnosis — workflow {}", d.workflow_id);
    println!();

    println!(
        "Active allocations: {} (with known walltime: {}, unlimited/local: {})",
        d.active_allocations, d.allocations_with_walltime, d.allocations_unlimited
    );
    if let Some(s) = &d.allocation_remaining {
        println!(
            "  remaining walltime   min {}   median {}   max {}",
            format_secs_human(s.min_seconds),
            format_secs_human(s.median_seconds),
            format_secs_human(s.max_seconds),
        );
    }
    println!(
        "Ready jobs: {} (with known runtime: {})",
        d.ready_jobs, d.ready_jobs_with_runtime
    );
    if let Some(s) = &d.ready_job_runtime {
        println!(
            "  required runtime     min {}   median {}   max {}",
            format_secs_human(s.min_seconds),
            format_secs_human(s.median_seconds),
            format_secs_human(s.max_seconds),
        );
    }
    println!();

    // Nothing to diagnose cases.
    if d.allocations_with_walltime == 0 {
        println!(
            "No active allocations report a walltime limit, so runtime-based packing \
             limits don't apply (local or unlimited runs)."
        );
        return;
    }
    if d.ready_jobs_with_runtime == 0 {
        println!("No ready jobs with a known runtime are waiting; nothing to diagnose.");
        return;
    }

    if !d.runtime_blocked {
        println!("\u{2713} No runtime-blocked packing detected.");
        println!(
            "  All {} ready jobs fit within at least one active allocation's remaining walltime.",
            d.ready_jobs_with_runtime
        );
        return;
    }

    let longest = d
        .ready_job_runtime
        .as_ref()
        .map(|s| s.max_seconds)
        .unwrap_or(0);
    let most_remaining = d
        .allocation_remaining
        .as_ref()
        .map(|s| s.max_seconds)
        .unwrap_or(0);

    println!("\u{26a0} Runtime-blocked packing detected");
    println!(
        "  {} of {} ready jobs need more runtime than ANY active allocation has left.",
        d.runtime_blocked_jobs, d.ready_jobs_with_runtime
    );
    println!(
        "      longest ready job: {}   most remaining on any allocation: {}",
        format_secs_human(longest),
        format_secs_human(most_remaining),
    );
    println!(
        "  {} of {} active allocations can't pick up the longest ready jobs;",
        d.allocations_too_short_for_longest, d.allocations_with_walltime
    );
    println!("      their freed cores will idle instead of packing new work.");
    println!();
    println!("  Why: Torc won't start a job whose required runtime exceeds an allocation's");
    println!("  remaining walltime — it would be killed mid-run. As allocations age, fewer");
    println!("  long jobs fit, so node-packing drops even while cores are free.");
    println!();
    println!("  Fixes:");
    println!(
        "    \u{2022} Reduce job runtime requirements if over-estimated \
         (torc workflows check-resources {})",
        d.workflow_id
    );
    println!("    \u{2022} Add checkpointing so jobs can resume in a later allocation");
    println!("    \u{2022} Submit fresh allocations sized for these runtimes");
}

/// Format a duration in seconds as a compact human string, e.g. "3d 1h", "45m".
fn format_secs_human(secs: i64) -> String {
    if secs <= 0 {
        return "0m".to_string();
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let mut parts: Vec<String> = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    // Show minutes when there are no larger units, or to refine sub-hour values.
    if minutes > 0 && days == 0 {
        parts.push(format!("{}m", minutes));
    }
    if parts.is_empty() {
        // Sub-minute, non-zero.
        return "<1m".to_string();
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_secs_human_formats_common_durations() {
        assert_eq!(format_secs_human(0), "0m");
        assert_eq!(format_secs_human(30), "<1m");
        assert_eq!(format_secs_human(45 * 60), "45m");
        assert_eq!(format_secs_human(3_600), "1h");
        assert_eq!(format_secs_human(90 * 60), "1h 30m");
        assert_eq!(format_secs_human(86_400), "1d");
        assert_eq!(format_secs_human(3 * 86_400 + 3_600), "3d 1h");
    }

    #[test]
    fn duration_stats_computes_min_median_max() {
        let stats = DurationStats::from(&[300, 100, 200]).expect("non-empty");
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_seconds, 100);
        assert_eq!(stats.median_seconds, 200);
        assert_eq!(stats.max_seconds, 300);
        assert!(DurationStats::from(&[]).is_none());
    }
}
