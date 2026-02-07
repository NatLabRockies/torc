//! Shared data models for report commands.
//!
//! These structs define the JSON output format for `torc reports` commands
//! and are used by both the producers (in `commands/reports.rs`) and consumers
//! (in `commands/recover.rs` and elsewhere).

use serde::{Deserialize, Serialize};

/// Output of `torc reports check-resource-utilization`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilizationReport {
    pub workflow_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<i64>,
    pub total_results: usize,
    pub over_utilization_count: usize,
    pub violations: Vec<ResourceViolation>,
    /// Number of failed jobs (only present when `--include-failed` is used)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub failed_jobs_count: usize,
    /// Failed job information (only present when `--include-failed` is used)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_jobs: Vec<FailedJobInfo>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// A resource utilization violation (job exceeded its specified resources)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceViolation {
    pub job_id: i64,
    pub job_name: String,
    pub resource_type: String,
    pub specified: String,
    pub peak_used: String,
    pub over_utilization: String,
}

/// Information about a failed job for recovery diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedJobInfo {
    pub job_id: i64,
    pub job_name: String,
    pub return_code: i64,
    pub exec_time_minutes: f64,
    pub configured_memory: String,
    pub configured_runtime: String,
    pub configured_cpus: i64,

    /// Peak memory usage in bytes (if available from resource monitoring)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_memory_bytes: Option<i64>,

    /// Human-readable peak memory (e.g., "1.5 GB")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_memory_formatted: Option<String>,

    /// Whether this job likely failed due to OOM
    #[serde(default, skip_serializing_if = "is_false")]
    pub likely_oom: bool,

    /// Reason for OOM detection (e.g., "memory_exceeded", "sigkill_137")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oom_reason: Option<String>,

    /// How much memory was over-utilized (e.g., "+25.3%")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_over_utilization: Option<String>,

    /// Whether this job likely failed due to timeout
    #[serde(default, skip_serializing_if = "is_false")]
    pub likely_timeout: bool,

    /// Reason for timeout detection (e.g., "sigxcpu_152")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_reason: Option<String>,

    /// Runtime utilization percentage (e.g., "95.2%")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_utilization: Option<String>,

    /// Whether this job exceeded its CPU allocation
    #[serde(default, skip_serializing_if = "is_false")]
    pub likely_cpu_violation: bool,

    /// Peak CPU percentage used (e.g., 501.4%)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_cpu_percent: Option<f64>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Output of `torc reports results`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsReport {
    pub workflow_id: i64,
    pub workflow_name: String,
    pub workflow_user: String,
    pub all_runs: bool,
    pub total_results: usize,
    /// Job result records
    ///
    /// Note: This field is named `results` in JSON output (not `jobs`).
    pub results: Vec<JobResultRecord>,
}

/// A single job result record with log file paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResultRecord {
    pub job_id: i64,
    pub job_name: String,
    pub status: String,
    pub run_id: i64,
    pub return_code: i64,
    pub completion_time: String,
    pub exec_time_minutes: f64,
    pub compute_node_id: i64,

    /// Path to job stdout log
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_stdout: Option<String>,

    /// Path to job stderr log
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_stderr: Option<String>,

    /// Type of compute node ("local" or "slurm")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_node_type: Option<String>,

    /// Path to job runner log file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_runner_log: Option<String>,

    /// Slurm job ID (only for slurm jobs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slurm_job_id: Option<String>,

    /// Path to Slurm stdout log (only for slurm jobs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slurm_stdout: Option<String>,

    /// Path to Slurm stderr log (only for slurm jobs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slurm_stderr: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_utilization_report_serialization() {
        let report = ResourceUtilizationReport {
            workflow_id: 1,
            run_id: Some(2),
            total_results: 10,
            over_utilization_count: 2,
            violations: vec![ResourceViolation {
                job_id: 1,
                job_name: "test_job".to_string(),
                resource_type: "Memory".to_string(),
                specified: "1.0 GB".to_string(),
                peak_used: "1.5 GB".to_string(),
                over_utilization: "+50.0%".to_string(),
            }],
            failed_jobs_count: 1,
            failed_jobs: vec![FailedJobInfo {
                job_id: 2,
                job_name: "failed_job".to_string(),
                return_code: 137i64,
                exec_time_minutes: 5.5,
                configured_memory: "2g".to_string(),
                configured_runtime: "PT1H".to_string(),
                configured_cpus: 4i64,
                peak_memory_bytes: Some(3_000_000_000),
                peak_memory_formatted: Some("2.8 GB".to_string()),
                likely_oom: true,
                oom_reason: Some("sigkill_137".to_string()),
                memory_over_utilization: Some("+40.0%".to_string()),
                likely_timeout: false,
                timeout_reason: None,
                runtime_utilization: Some("9.2%".to_string()),
                likely_cpu_violation: false,
                peak_cpu_percent: None,
            }],
        };

        let json = serde_json::to_string(&report).unwrap();
        let parsed: ResourceUtilizationReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.workflow_id, 1);
        assert_eq!(parsed.failed_jobs.len(), 1);
        assert!(parsed.failed_jobs[0].likely_oom);
    }

    #[test]
    fn test_results_report_serialization() {
        let report = ResultsReport {
            workflow_id: 1,
            workflow_name: "test_workflow".to_string(),
            workflow_user: "testuser".to_string(),
            all_runs: false,
            total_results: 1,
            results: vec![JobResultRecord {
                job_id: 1,
                job_name: "job1".to_string(),
                status: "Completed".to_string(),
                run_id: 1,
                return_code: 0i64,
                completion_time: "2024-01-01T00:00:00Z".to_string(),
                exec_time_minutes: 10.5,
                compute_node_id: 1,
                job_stdout: Some("/path/to/stdout".to_string()),
                job_stderr: Some("/path/to/stderr".to_string()),
                compute_node_type: Some("slurm".to_string()),
                job_runner_log: Some("/path/to/runner.log".to_string()),
                slurm_job_id: Some("12345".to_string()),
                slurm_stdout: Some("/path/to/slurm.out".to_string()),
                slurm_stderr: Some("/path/to/slurm.err".to_string()),
            }],
        };

        let json = serde_json::to_string(&report).unwrap();
        let parsed: ResultsReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.workflow_id, 1);
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].slurm_job_id, Some("12345".to_string()));
    }

    #[test]
    fn test_failed_job_info_optional_fields() {
        // Test that optional fields can be omitted
        // Note: return_code and configured_cpus are i64 in the struct
        let json = r#"{
            "job_id": 1,
            "job_name": "test",
            "return_code": 1,
            "exec_time_minutes": 5.0,
            "configured_memory": "1g",
            "configured_runtime": "PT1H",
            "configured_cpus": 2
        }"#;

        let parsed: FailedJobInfo = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.job_id, 1);
        assert!(!parsed.likely_oom);
        assert!(!parsed.likely_timeout);
        assert!(parsed.peak_memory_bytes.is_none());
    }
}
