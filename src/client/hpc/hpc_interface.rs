//! HPC interface trait definition
//!
//! This module defines the `HpcInterface` trait which serves as the abstraction
//! for different HPC scheduler implementations (Slurm, PBS, etc.).

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;

use super::common::{HpcJobInfo, HpcJobStats, HpcJobStatus};

/// Defines the interface for managing an HPC scheduler
///
/// This trait is the Rust equivalent of Python's abc.ABC base class.
/// All HPC scheduler implementations must implement this trait.
pub trait HpcInterface: Send + Sync {
    /// Cancel a job by ID
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID to cancel
    ///
    /// # Returns
    /// The return code from the cancellation command (0 = success)
    fn cancel_job(&self, job_id: &str) -> Result<i32>;

    /// Get the status of a specific job
    ///
    /// Handles transient errors with retries.
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID to check
    ///
    /// # Returns
    /// HpcJobInfo containing job ID, name, and status
    fn get_status(&self, job_id: &str) -> Result<HpcJobInfo>;

    /// Get the statuses of all user jobs
    ///
    /// Handles transient errors with retries.
    ///
    /// # Returns
    /// HashMap mapping job_id to HpcJobStatus
    fn get_statuses(&self) -> Result<HashMap<String, HpcJobStatus>>;

    /// Create a submission script for the HPC scheduler
    ///
    /// # Arguments
    /// * `name` - Job name
    /// * `server_url` - URL of the torc server
    /// * `workflow_id` - Workflow ID for the job runner
    /// * `output_path` - Path for stdout and stderr files
    /// * `poll_interval` - Poll interval in seconds for the job runner
    /// * `max_parallel_jobs` - Optional maximum number of parallel jobs
    /// * `filename` - Path where the submission script should be written
    /// * `config` - Configuration parameters for the HPC scheduler
    /// * `start_one_worker_per_node` - Whether to launch one worker per node via srun
    /// * `tls_ca_cert` - Optional path to a PEM-encoded CA certificate
    /// * `tls_insecure` - Whether to skip certificate verification
    /// * `startup_delay_seconds` - Maximum startup jitter in seconds (0 to disable)
    #[allow(clippy::too_many_arguments)]
    fn create_submission_script(
        &self,
        name: &str,
        server_url: &str,
        workflow_id: i64,
        output_path: &str,
        poll_interval: i32,
        max_parallel_jobs: Option<i32>,
        filename: &Path,
        config: &HashMap<String, String>,
        start_one_worker_per_node: bool,
        tls_ca_cert: Option<&str>,
        tls_insecure: bool,
        startup_delay_seconds: u64,
    ) -> Result<()>;

    /// Get the current HPC job ID from environment variables
    ///
    /// # Returns
    /// The job ID of the current job
    fn get_current_job_id(&self) -> String;

    /// Get all relevant HPC environment variables
    ///
    /// # Returns
    /// HashMap of environment variable names to values
    fn get_environment_variables(&self) -> HashMap<String, String>;

    /// Get the end time for the current job
    ///
    /// # Returns
    /// DateTime when the job will end
    fn get_job_end_time(&self) -> Result<DateTime<Utc>>;

    /// Get statistics for a completed job
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID
    ///
    /// # Returns
    /// HpcJobStats with detailed job information
    fn get_job_stats(&self, job_id: &str) -> Result<HpcJobStats>;

    /// Get path to local scratch/temporary storage
    ///
    /// # Returns
    /// Path to local storage space
    fn get_local_scratch(&self) -> Result<String>;

    /// Get the memory available to the current job in GiB
    ///
    /// # Returns
    /// Memory in gigabytes
    fn get_memory_gb(&self) -> f64;

    /// Get the node ID of the current system
    ///
    /// # Returns
    /// Node ID string
    fn get_node_id(&self) -> String;

    /// Get the number of CPUs available on the current node
    ///
    /// # Returns
    /// Number of CPUs
    fn get_num_cpus(&self) -> usize;

    /// Get the number of CPUs per task
    ///
    /// # Returns
    /// Number of CPUs per task
    fn get_num_cpus_per_task(&self) -> usize;

    /// Get the number of GPUs available on the current node
    ///
    /// # Returns
    /// Number of GPUs
    fn get_num_gpus(&self) -> usize;

    /// Get the number of compute nodes in the current job
    ///
    /// # Returns
    /// Number of nodes
    fn get_num_nodes(&self) -> usize;

    /// Get the task process ID
    ///
    /// # Returns
    /// Task process ID
    fn get_task_pid(&self) -> usize;

    /// Returns true if the current node is the head node.
    fn is_head_node(&self) -> bool;

    /// List the active node hostnames for a job
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID
    ///
    /// # Returns
    /// Vector of node hostnames in deterministic order
    fn list_active_nodes(&self, job_id: &str) -> Result<Vec<String>>;

    /// Submit a job to the HPC queue
    ///
    /// Handles transient errors with retries.
    ///
    /// # Arguments
    /// * `filename` - Path to the submission script
    ///
    /// # Returns
    /// Tuple of (return_code, job_id, stderr)
    fn submit(&self, filename: &Path) -> Result<(i32, String, String)>;

    /// Get the username for HPC operations
    ///
    /// # Returns
    /// The current scheduler account username.
    ///
    /// This intentionally does not consult `TORC_USERNAME`, which is a Torc API
    /// identity override and may differ from the Unix account running Slurm jobs.
    fn get_user(&self) -> Result<String> {
        Ok(std::env::var("USER").or_else(|_| std::env::var("USERNAME"))?)
    }
}
