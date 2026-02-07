//! Shared resource correction logic for both recovery and proactive optimization.
//!
//! This module provides the core algorithms for analyzing resource utilization
//! and automatically adjusting resource requirements based on actual job usage.
//! It's used by both `torc recover` (reactive) and `torc workflows correct-resources` (proactive).

use std::collections::HashMap;

use log::{debug, info, warn};
use serde::Serialize;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::report_models::ResourceUtilizationReport;
use crate::time_utils::duration_string_to_seconds;

/// Result of applying resource corrections
#[derive(Debug, Clone, Serialize)]
pub struct ResourceCorrectionResult {
    pub resource_requirements_updated: usize,
    pub jobs_analyzed: usize,
    pub memory_corrections: usize,
    pub runtime_corrections: usize,
    /// Detailed adjustment reports for JSON output
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<ResourceAdjustmentReport>,
}

/// Detailed report of a resource adjustment for JSON output
#[derive(Debug, Clone, Serialize)]
pub struct ResourceAdjustmentReport {
    /// The resource_requirements_id being adjusted
    pub resource_requirements_id: i64,
    /// Job IDs that share this resource requirement
    pub job_ids: Vec<i64>,
    /// Job names for reference
    pub job_names: Vec<String>,
    /// Whether memory was adjusted
    pub memory_adjusted: bool,
    /// Original memory setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_memory: Option<String>,
    /// New memory setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_memory: Option<String>,
    /// Maximum peak memory observed (bytes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_peak_memory_bytes: Option<u64>,
    /// Whether runtime was adjusted
    pub runtime_adjusted: bool,
    /// Original runtime setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_runtime: Option<String>,
    /// New runtime setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_runtime: Option<String>,
}

/// Aggregated resource adjustment data for a single resource_requirements_id.
/// When multiple jobs share the same resource requirements, we take the maximum
/// peak memory and runtime to ensure all jobs can succeed on retry.
#[derive(Debug)]
struct ResourceAdjustment {
    /// The resource_requirements_id
    rr_id: i64,
    /// Job IDs that share this resource requirement
    job_ids: Vec<i64>,
    /// Job names for logging
    job_names: Vec<String>,
    /// Maximum peak memory observed across all OOM jobs (in bytes)
    max_peak_memory_bytes: Option<u64>,
    /// Whether any job had OOM without peak data (fall back to multiplier)
    has_oom_without_peak: bool,
    /// Whether any job had a timeout
    has_timeout: bool,
    /// Current memory setting (for fallback calculation)
    current_memory: String,
    /// Current runtime setting
    current_runtime: String,
    /// Maximum peak CPU percentage observed across all CPU violation jobs
    max_peak_cpu_percent: Option<f64>,
    /// Whether any job had a CPU violation
    has_cpu_violation: bool,
    /// Current CPU count (for CPU violation calculation)
    current_cpus: i64,
}

/// Parse memory string (e.g., "8g", "512m", "1024k") to bytes
pub fn parse_memory_bytes(mem: &str) -> Option<u64> {
    let mem = mem.trim().to_lowercase();
    let (num_str, multiplier) = if mem.ends_with("gb") {
        (mem.trim_end_matches("gb"), 1024u64 * 1024 * 1024)
    } else if mem.ends_with("g") {
        (mem.trim_end_matches("g"), 1024u64 * 1024 * 1024)
    } else if mem.ends_with("mb") {
        (mem.trim_end_matches("mb"), 1024u64 * 1024)
    } else if mem.ends_with("m") {
        (mem.trim_end_matches("m"), 1024u64 * 1024)
    } else if mem.ends_with("kb") {
        (mem.trim_end_matches("kb"), 1024u64)
    } else if mem.ends_with("k") {
        (mem.trim_end_matches("k"), 1024u64)
    } else {
        (mem.as_str(), 1u64)
    };
    num_str
        .parse::<f64>()
        .ok()
        .map(|n| (n * multiplier as f64) as u64)
}

/// Format bytes to memory string (e.g., "12g", "512m")
pub fn format_memory_bytes_short(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{}g", bytes / (1024 * 1024 * 1024))
    } else if bytes >= 1024 * 1024 {
        format!("{}m", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}k", bytes / 1024)
    } else {
        format!("{}b", bytes)
    }
}

/// Format seconds to ISO8601 duration (e.g., "PT2H30M")
pub fn format_duration_iso8601(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    if hours > 0 && mins > 0 {
        format!("PT{}H{}M", hours, mins)
    } else if hours > 0 {
        format!("PT{}H", hours)
    } else {
        format!("PT{}M", mins.max(1))
    }
}

/// Apply resource corrections based on utilization analysis
///
/// This function analyzes job resource utilization data and adjusts resource
/// requirements (memory and runtime) to better match actual job needs.
///
/// When multiple jobs share the same `resource_requirements_id`, this function
/// finds the maximum peak memory across all jobs in that group and applies
/// that (with multiplier) to the shared resource requirement. This ensures all
/// jobs in the group can succeed.
///
/// # Arguments
///
/// * `config` - API configuration
/// * `workflow_id` - The workflow to correct resources for
/// * `diagnosis` - Resource utilization analysis (from `check-resource-utilization`)
/// * `memory_multiplier` - Factor to multiply peak memory by (e.g., 1.2 for 20% buffer)
/// * `runtime_multiplier` - Factor to multiply runtime by (e.g., 1.2 for 20% buffer)
/// * `include_jobs` - If non-empty, only correct these specific job IDs
/// * `dry_run` - If true, show what would be done without applying changes
///
/// # Returns
///
/// Result containing the correction result with counts and detailed adjustments
pub fn apply_resource_corrections(
    config: &Configuration,
    _workflow_id: i64,
    diagnosis: &ResourceUtilizationReport,
    memory_multiplier: f64,
    runtime_multiplier: f64,
    include_jobs: &[i64],
    dry_run: bool,
) -> Result<ResourceCorrectionResult, String> {
    let mut memory_corrections = 0;
    let mut runtime_corrections = 0;

    // Phase 1: Collect and aggregate data by resource_requirements_id
    // This ensures that when multiple jobs share the same RR, we use the
    // maximum peak memory across all of them.
    let mut rr_adjustments: HashMap<i64, ResourceAdjustment> = HashMap::new();

    let jobs_to_analyze = if include_jobs.is_empty() {
        diagnosis.resource_violations.clone()
    } else {
        diagnosis
            .resource_violations
            .iter()
            .filter(|j| include_jobs.contains(&j.job_id))
            .cloned()
            .collect()
    };

    let jobs_analyzed = jobs_to_analyze.len();

    for job_info in &jobs_to_analyze {
        let job_id = job_info.job_id;
        let likely_oom = job_info.likely_oom;
        let likely_timeout = job_info.likely_timeout;
        let likely_cpu_violation = job_info.likely_cpu_violation;

        // Skip if no violations detected
        if !likely_oom && !likely_timeout && !likely_cpu_violation {
            continue;
        }

        // Get current job to find resource requirements
        let job = match default_api::get_job(config, job_id) {
            Ok(j) => j,
            Err(e) => {
                warn!("Warning: couldn't get job {}: {}", job_id, e);
                continue;
            }
        };

        let rr_id = match job.resource_requirements_id {
            Some(id) => id,
            None => {
                warn!("Warning: job {} has no resource requirements", job_id);
                continue;
            }
        };

        // Get or create the adjustment entry for this resource_requirements_id
        let adjustment = rr_adjustments.entry(rr_id).or_insert_with(|| {
            // Fetch current resource requirements (only once per rr_id)
            let (current_memory, current_runtime, current_cpus) =
                match default_api::get_resource_requirements(config, rr_id) {
                    Ok(rr) => (rr.memory, rr.runtime, rr.num_cpus),
                    Err(e) => {
                        warn!(
                            "Warning: couldn't get resource requirements {}: {}",
                            rr_id, e
                        );
                        (String::new(), String::new(), 0)
                    }
                };
            ResourceAdjustment {
                rr_id,
                job_ids: Vec::new(),
                job_names: Vec::new(),
                max_peak_memory_bytes: None,
                has_oom_without_peak: false,
                has_timeout: false,
                current_memory,
                current_runtime,
                max_peak_cpu_percent: None,
                has_cpu_violation: false,
                current_cpus,
            }
        });

        // Skip if we couldn't fetch the resource requirements
        if adjustment.current_memory.is_empty() {
            continue;
        }

        adjustment.job_ids.push(job_id);
        adjustment.job_names.push(job.name.clone());

        // Track OOM data
        if likely_oom {
            let peak_bytes = job_info
                .peak_memory_bytes
                .filter(|&v| v > 0)
                .map(|v| v as u64);

            if let Some(peak) = peak_bytes {
                // Update max if this job used more memory
                adjustment.max_peak_memory_bytes = Some(
                    adjustment
                        .max_peak_memory_bytes
                        .map_or(peak, |current_max| current_max.max(peak)),
                );
            } else {
                adjustment.has_oom_without_peak = true;
            }
        }

        // Track timeout
        if likely_timeout {
            adjustment.has_timeout = true;
        }

        // Track CPU violation
        if likely_cpu_violation && let Some(peak_cpu) = job_info.peak_cpu_percent {
            adjustment.has_cpu_violation = true;
            adjustment.max_peak_cpu_percent = Some(
                adjustment
                    .max_peak_cpu_percent
                    .map_or(peak_cpu, |current_max| current_max.max(peak_cpu)),
            );
        }
    }

    // Phase 2: Apply adjustments once per resource_requirements_id
    let mut adjustment_reports = Vec::new();

    for adjustment in rr_adjustments.values() {
        let rr_id = adjustment.rr_id;
        let mut updated = false;
        let mut memory_adjusted = false;
        let mut runtime_adjusted = false;
        let mut original_memory = None;
        let mut new_memory_str = None;
        let mut original_runtime = None;
        let mut new_runtime_str = None;

        // Fetch current resource requirements for update
        let rr = match default_api::get_resource_requirements(config, rr_id) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Warning: couldn't get resource requirements {}: {}",
                    rr_id, e
                );
                continue;
            }
        };
        let mut new_rr = rr.clone();

        // Apply OOM fix using maximum peak memory across all jobs sharing this RR
        if adjustment.max_peak_memory_bytes.is_some() || adjustment.has_oom_without_peak {
            let new_bytes = if let Some(max_peak) = adjustment.max_peak_memory_bytes {
                // Use the maximum observed peak memory * multiplier
                (max_peak as f64 * memory_multiplier) as u64
            } else if let Some(current_bytes) = parse_memory_bytes(&adjustment.current_memory) {
                // Fall back to current specified * multiplier
                (current_bytes as f64 * memory_multiplier) as u64
            } else {
                warn!(
                    "RR {}: OOM detected but couldn't determine new memory",
                    rr_id
                );
                continue;
            };

            let new_memory = format_memory_bytes_short(new_bytes);
            let job_count = adjustment.job_ids.len();

            if let Some(max_peak) = adjustment.max_peak_memory_bytes {
                if job_count > 1 {
                    info!(
                        "{} job(s) with RR {}: OOM detected, max peak usage {} -> allocating {} ({}x)",
                        job_count,
                        rr_id,
                        format_memory_bytes_short(max_peak),
                        new_memory,
                        memory_multiplier
                    );
                    debug!("  Jobs: {:?}", adjustment.job_names);
                } else if let (Some(job_id), Some(job_name)) =
                    (adjustment.job_ids.first(), adjustment.job_names.first())
                {
                    info!(
                        "Job {} ({}): OOM detected, peak usage {} -> allocating {} ({}x)",
                        job_id,
                        job_name,
                        format_memory_bytes_short(max_peak),
                        new_memory,
                        memory_multiplier
                    );
                }
            } else {
                info!(
                    "{} job(s) with RR {}: OOM detected, increasing memory {} -> {} ({}x, no peak data)",
                    job_count, rr_id, adjustment.current_memory, new_memory, memory_multiplier
                );
            }

            // Track for JSON report
            original_memory = Some(adjustment.current_memory.clone());
            new_memory_str = Some(new_memory.clone());
            memory_adjusted = true;

            new_rr.memory = new_memory;
            updated = true;
            memory_corrections += adjustment.job_ids.len();
        }

        // Apply timeout fix
        if adjustment.has_timeout
            && let Ok(current_secs) = duration_string_to_seconds(&adjustment.current_runtime)
        {
            let new_secs = (current_secs as f64 * runtime_multiplier) as u64;
            let new_runtime = format_duration_iso8601(new_secs);
            let job_count = adjustment.job_ids.len();

            if job_count > 1 {
                info!(
                    "{} job(s) with RR {}: Timeout detected, increasing runtime {} -> {}",
                    job_count, rr_id, adjustment.current_runtime, new_runtime
                );
            } else if let (Some(job_id), Some(job_name)) =
                (adjustment.job_ids.first(), adjustment.job_names.first())
            {
                info!(
                    "Job {} ({}): Timeout detected, increasing runtime {} -> {}",
                    job_id, job_name, adjustment.current_runtime, new_runtime
                );
            }

            // Track for JSON report
            original_runtime = Some(adjustment.current_runtime.clone());
            new_runtime_str = Some(new_runtime.clone());
            runtime_adjusted = true;

            new_rr.runtime = new_runtime;
            updated = true;
            runtime_corrections += adjustment.job_ids.len();
        }

        // Apply CPU violation fix
        if adjustment.has_cpu_violation
            && let Some(max_peak_cpu) = adjustment.max_peak_cpu_percent
        {
            // peak_cpu_percent is the total percentage for all CPUs
            // e.g., 501.4% with 3 CPUs allocated (300%)
            // We need: new_cpus = ceil(max_peak_cpu / 100.0) with multiplier
            let required_cpus = (max_peak_cpu / 100.0 * memory_multiplier).ceil() as i64;
            let new_cpus = std::cmp::max(required_cpus, 1); // At least 1 CPU

            if new_cpus > adjustment.current_cpus {
                let job_count = adjustment.job_ids.len();
                if job_count > 1 {
                    info!(
                        "{} job(s) with RR {}: CPU over-utilization detected, peak {}% -> allocating {} CPUs ({}x)",
                        job_count, rr_id, max_peak_cpu, new_cpus, memory_multiplier
                    );
                } else if let (Some(job_id), Some(job_name)) =
                    (adjustment.job_ids.first(), adjustment.job_names.first())
                {
                    info!(
                        "Job {} ({}): CPU over-utilization detected, peak {}% -> allocating {} CPUs ({}x)",
                        job_id, job_name, max_peak_cpu, new_cpus, memory_multiplier
                    );
                }

                new_rr.num_cpus = new_cpus;
                updated = true;
            }
        }

        // Update resource requirements if changed (only once per rr_id)
        #[allow(clippy::collapsible_if)]
        if updated {
            if !dry_run {
                if let Err(e) = default_api::update_resource_requirements(config, rr_id, new_rr) {
                    warn!(
                        "Warning: failed to update resource requirements {}: {}",
                        rr_id, e
                    );
                }
            }

            // Create adjustment report for JSON output
            adjustment_reports.push(ResourceAdjustmentReport {
                resource_requirements_id: rr_id,
                job_ids: adjustment.job_ids.clone(),
                job_names: adjustment.job_names.clone(),
                memory_adjusted,
                original_memory,
                new_memory: new_memory_str,
                max_peak_memory_bytes: adjustment.max_peak_memory_bytes,
                runtime_adjusted,
                original_runtime,
                new_runtime: new_runtime_str,
            });
        }
    }

    Ok(ResourceCorrectionResult {
        resource_requirements_updated: adjustment_reports.len(),
        jobs_analyzed,
        memory_corrections,
        runtime_corrections,
        adjustments: adjustment_reports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_bytes_gigabytes() {
        assert_eq!(parse_memory_bytes("8g"), Some(8 * 1024 * 1024 * 1024));
        assert_eq!(parse_memory_bytes("8gb"), Some(8 * 1024 * 1024 * 1024));
        assert_eq!(parse_memory_bytes("8G"), Some(8 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_memory_bytes_megabytes() {
        assert_eq!(parse_memory_bytes("512m"), Some(512 * 1024 * 1024));
        assert_eq!(parse_memory_bytes("512mb"), Some(512 * 1024 * 1024));
    }

    #[test]
    fn test_parse_memory_bytes_kilobytes() {
        assert_eq!(parse_memory_bytes("1024k"), Some(1024 * 1024));
        assert_eq!(parse_memory_bytes("1024kb"), Some(1024 * 1024));
    }

    #[test]
    fn test_parse_memory_bytes_bytes() {
        assert_eq!(parse_memory_bytes("1000"), Some(1000));
    }

    #[test]
    fn test_parse_memory_bytes_with_whitespace() {
        assert_eq!(parse_memory_bytes("  8g  "), Some(8 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_memory_bytes_invalid() {
        assert_eq!(parse_memory_bytes("invalid"), None);
        assert_eq!(parse_memory_bytes("8x"), None);
    }

    #[test]
    fn test_format_memory_bytes_short_gigabytes() {
        assert_eq!(
            format_memory_bytes_short(8 * 1024 * 1024 * 1024),
            "8g".to_string()
        );
    }

    #[test]
    fn test_format_memory_bytes_short_megabytes() {
        assert_eq!(
            format_memory_bytes_short(512 * 1024 * 1024),
            "512m".to_string()
        );
    }

    #[test]
    fn test_format_memory_bytes_short_kilobytes() {
        assert_eq!(format_memory_bytes_short(1024 * 1024), "1m".to_string());
        assert_eq!(format_memory_bytes_short(512 * 1024), "512k".to_string());
    }

    #[test]
    fn test_format_memory_bytes_short_bytes() {
        assert_eq!(format_memory_bytes_short(512), "512b".to_string());
    }

    #[test]
    fn test_format_duration_iso8601_hours_and_minutes() {
        assert_eq!(format_duration_iso8601(7200 + 1800), "PT2H30M".to_string());
    }

    #[test]
    fn test_format_duration_iso8601_only_hours() {
        assert_eq!(format_duration_iso8601(7200), "PT2H".to_string());
    }

    #[test]
    fn test_format_duration_iso8601_only_minutes() {
        assert_eq!(format_duration_iso8601(900), "PT15M".to_string());
    }

    #[test]
    fn test_format_duration_iso8601_less_than_minute() {
        assert_eq!(format_duration_iso8601(30), "PT1M".to_string());
    }

    #[test]
    fn test_parse_format_memory_roundtrip() {
        let original = "12g";
        let bytes = parse_memory_bytes(original).unwrap();
        let formatted = format_memory_bytes_short(bytes);
        assert_eq!(formatted, original);
    }
}
