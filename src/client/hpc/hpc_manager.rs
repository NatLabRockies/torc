//! HPC management functionality

use anyhow::{Context, Result};
use log::{error, info, trace};
use std::collections::HashMap;
use std::path::Path;

use super::common::{HpcJobStats, HpcJobStatus, HpcType};
use super::hpc_interface::HpcInterface;

/// Manages HPC job submission and monitoring
pub struct HpcManager {
    output: String,
    config: HashMap<String, String>,
    hpc_type: HpcType,
    interface: Box<dyn HpcInterface>,
}

impl HpcManager {
    /// Create a new HPC manager
    ///
    /// # Arguments
    /// * `config` - Configuration parameters for the HPC scheduler
    /// * `hpc_type` - Type of HPC scheduler (Slurm, PBS, etc.)
    /// * `output` - Directory path for job output files
    pub fn new(config: HashMap<String, String>, hpc_type: HpcType, output: String) -> Result<Self> {
        let interface = super::create_hpc_interface(hpc_type)?;

        trace!("Constructed HpcManager with output={}", output);

        Ok(Self {
            output,
            config,
            hpc_type,
            interface,
        })
    }

    /// Cancel a job by ID
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID to cancel
    ///
    /// # Returns
    /// The return code from the cancellation command (0 = success)
    pub fn cancel_job(&self, job_id: &str) -> Result<i32> {
        let ret = self.interface.cancel_job(job_id)?;

        if ret == 0 {
            info!("Successfully cancelled job ID {}", job_id);
        } else {
            info!("Failed to cancel job ID {}", job_id);
        }

        Ok(ret)
    }

    /// Get the status of a specific job
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID to check
    ///
    /// # Returns
    /// The current status of the job
    pub fn get_status(&self, job_id: &str) -> Result<HpcJobStatus> {
        let info = self.interface.get_status(job_id)?;
        trace!("Job {} status: {:?}", job_id, info.status);
        Ok(info.status)
    }

    /// Get the statuses of all user jobs
    ///
    /// # Returns
    /// HashMap mapping job_id to HpcJobStatus
    pub fn get_statuses(&self) -> Result<HashMap<String, HpcJobStatus>> {
        self.interface.get_statuses()
    }

    /// Get statistics for a completed job
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID
    ///
    /// # Returns
    /// HpcJobStats with detailed job information
    pub fn get_job_stats(&self, job_id: &str) -> Result<HpcJobStats> {
        self.interface.get_job_stats(job_id)
    }

    /// Get path to local scratch/temporary storage
    ///
    /// # Returns
    /// Path to local storage space
    pub fn get_local_scratch(&self) -> Result<String> {
        self.interface.get_local_scratch()
    }

    /// Return the type of HPC management system
    pub fn hpc_type(&self) -> HpcType {
        self.hpc_type
    }

    /// List the active node hostnames for a job
    ///
    /// # Arguments
    /// * `job_id` - The HPC job ID
    ///
    /// # Returns
    /// Vector of node hostnames in deterministic order
    pub fn list_active_nodes(&self, job_id: &str) -> Result<Vec<String>> {
        self.interface.list_active_nodes(job_id)
    }

    /// Submit a job to the HPC queue
    ///
    /// # Arguments
    /// * `directory` - Directory to contain the submission script
    /// * `name` - Job name
    /// * `server_url` - URL of the torc server
    /// * `workflow_id` - Workflow ID for the job runner
    /// * `poll_interval` - Poll interval in seconds for the job runner
    /// * `max_parallel_jobs` - Optional maximum number of parallel jobs
    /// * `keep_submission_script` - Whether to keep the submission script after submission
    ///
    /// # Returns
    /// The HPC job ID
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &self,
        directory: &Path,
        name: &str,
        server_url: &str,
        workflow_id: i64,
        poll_interval: i32,
        max_parallel_jobs: Option<i32>,
        start_one_worker_per_node: bool,
        keep_submission_script: bool,
        tls_ca_cert: Option<&str>,
        tls_insecure: bool,
        startup_delay_seconds: u64,
    ) -> Result<String> {
        let filename = directory.join(format!("{}.sh", name));

        self.interface.create_submission_script(
            name,
            server_url,
            workflow_id,
            &self.output,
            poll_interval,
            max_parallel_jobs,
            &filename,
            &self.config,
            start_one_worker_per_node,
            tls_ca_cert,
            tls_insecure,
            startup_delay_seconds,
        )?;

        trace!("Created submission script {:?}", filename);

        let (ret, job_id, err) = self.interface.submit(&filename)?;

        if ret == 0 {
            info!("Job '{}' with ID={} running successfully", name, job_id);
            if !keep_submission_script {
                std::fs::remove_file(&filename).with_context(|| {
                    format!("Failed to remove submission script {:?}", filename)
                })?;
            }
        } else {
            error!("Failed to submit job '{}': ret={}: {}", name, ret, err);
            return Err(anyhow::anyhow!(
                "Failed to submit HPC job {}: {}",
                name,
                ret
            ));
        }

        Ok(job_id)
    }
}
