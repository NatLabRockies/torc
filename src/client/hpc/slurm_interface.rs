//! Slurm scheduler interface implementation

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use log::{debug, error, info, trace, warn};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;
use sysinfo::{RefreshKind, System, SystemExt};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::common::{HpcJobInfo, HpcJobStats, HpcJobStatus};
use super::hpc_interface::HpcInterface;

/// Slurm scheduler implementation
pub struct SlurmInterface {
    user: String,
    sbatch_regex: Regex,
}

impl SlurmInterface {
    /// Create a new Slurm interface
    pub fn new() -> Result<Self> {
        let user = env::var("USER").or_else(|_| env::var("USERNAME"))?;
        let sbatch_regex = Regex::new(r"Submitted batch job (\d+)")?;

        Ok(Self { user, sbatch_regex })
    }

    /// Map Slurm status to HpcJobStatus
    fn map_status(slurm_status: &str) -> HpcJobStatus {
        match slurm_status {
            "PENDING" | "CONFIGURING" => HpcJobStatus::Queued,
            "RUNNING" => HpcJobStatus::Running,
            "COMPLETED" | "COMPLETING" => HpcJobStatus::Complete,
            _ => HpcJobStatus::Unknown,
        }
    }

    /// Get the squeue executable path (allows for testing with fake binary)
    fn get_squeue_exec() -> String {
        env::var("TORC_FAKE_SQUEUE").unwrap_or_else(|_| "squeue".to_string())
    }

    /// Get the sbatch executable path (allows for testing with fake binary)
    fn get_sbatch_exec() -> String {
        env::var("TORC_FAKE_SBATCH").unwrap_or_else(|_| "sbatch".to_string())
    }

    /// Run a command with retries for transient errors
    fn run_command_with_retries(
        &self,
        cmd: &str,
        args: &[&str],
        num_retries: usize,
        retry_delay_secs: u64,
        ignore_errors: &[&str],
    ) -> Result<(i32, String, String)> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            trace!("Running command: {} {:?} (attempt {})", cmd, args, attempts);

            let output = Command::new(cmd).args(args).output()?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let return_code = output.status.code().unwrap_or(-1);

            // Check if this is an ignorable error
            let should_ignore = ignore_errors
                .iter()
                .any(|err| stderr.contains(err) || stdout.contains(err));

            if return_code == 0 || should_ignore || attempts >= num_retries {
                return Ok((return_code, stdout, stderr));
            }

            warn!(
                "Command failed (attempt {}/{}): {} - {}",
                attempts, num_retries, return_code, stderr
            );

            if attempts < num_retries {
                thread::sleep(Duration::from_secs(retry_delay_secs));
            }
        }
    }
}

impl HpcInterface for SlurmInterface {
    fn cancel_job(&self, job_id: &str) -> Result<i32> {
        let output = Command::new("scancel").arg(job_id).output()?;

        let return_code = output.status.code().unwrap_or(-1);
        if return_code != 0 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to cancel Slurm job {}: {}", job_id, stderr);
        } else {
            info!("Canceled Slurm job {}", job_id);
        }

        Ok(return_code)
    }

    fn get_status(&self, job_id: &str) -> Result<HpcJobInfo> {
        let field_names = ["jobid", "name", "state"];
        let format = field_names.join(",");
        let squeue = Self::get_squeue_exec();

        let (return_code, stdout, stderr) = self.run_command_with_retries(
            &squeue,
            &["-u", &self.user, "--Format", &format, "-h", "-j", job_id],
            6,
            10,
            &["Invalid job id specified"],
        )?;

        if return_code != 0 {
            if stderr.contains("Invalid job id specified") {
                return Ok(HpcJobInfo::none());
            }

            return Err(anyhow::anyhow!(
                "squeue command failed: {} - {}",
                return_code,
                stderr
            ));
        }

        trace!("squeue output: [{}]", stdout);
        let fields: Vec<&str> = stdout.split_whitespace().collect();

        if fields.is_empty() {
            // No jobs are currently running
            return Ok(HpcJobInfo::none());
        }

        if fields.len() != field_names.len() {
            return Err(anyhow::anyhow!(
                "Unexpected squeue output format: got {} fields, expected {}",
                fields.len(),
                field_names.len()
            ));
        }

        Ok(HpcJobInfo::new(
            fields[0].to_string(),
            fields[1].to_string(),
            Self::map_status(fields[2]),
        ))
    }

    fn get_statuses(&self) -> Result<HashMap<String, HpcJobStatus>> {
        let field_names = ["jobid", "state"];
        let format = field_names.join(",");
        let squeue = Self::get_squeue_exec();

        let (return_code, stdout, stderr) = self.run_command_with_retries(
            &squeue,
            &["-u", &self.user, "--Format", &format, "-h"],
            6,
            10,
            &[],
        )?;

        if return_code != 0 {
            return Err(anyhow::anyhow!(
                "squeue command failed: {} - {}",
                return_code,
                stderr
            ));
        }

        trace!("squeue output: [{}]", stdout);

        let mut statuses = HashMap::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() != field_names.len() {
                warn!("Skipping malformed squeue line: {}", line);
                continue;
            }

            let job_id = fields[0].to_string();
            let status = Self::map_status(fields[1]);
            statuses.insert(job_id, status);
        }

        Ok(statuses)
    }

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
    ) -> Result<()> {
        let mut script = format!(
            "#!/bin/bash\n\
             #SBATCH --account={}\n\
             #SBATCH --job-name={}\n\
             #SBATCH --time={}\n\
             #SBATCH --output={}/slurm_output_wf{}_sl%j.o\n\
             #SBATCH --error={}/slurm_output_wf{}_sl%j.e\n",
            config
                .get("account")
                .context("Missing 'account' in config")?,
            name,
            config
                .get("walltime")
                .context("Missing 'walltime' in config")?,
            output_path,
            workflow_id,
            output_path,
            workflow_id
        );

        // Add other SBATCH parameters
        for (key, value) in config.iter() {
            if key == "account" || key == "walltime" || key == "extra" {
                continue;
            }

            let param_name = key.replace('_', "-");
            script.push_str(&format!("#SBATCH --{}={}\n", param_name, value));
        }

        // Add extra parameter if present
        if let Some(extra) = config.get("extra") {
            script.push_str(&format!("#SBATCH {}\n", extra));
        }

        script.push('\n');
        script.push_str(&format!("TORC_URL=\"{}\"\n\n", server_url));

        // Build the torc-slurm-job-runner command
        let mut command = format!(
            "torc-slurm-job-runner $TORC_URL {} {} --poll-interval {}",
            workflow_id, output_path, poll_interval
        );

        if let Some(max_jobs) = max_parallel_jobs {
            command.push_str(&format!(" --max-parallel-jobs {}", max_jobs));
        }

        // Add the command with optional srun prefix
        if start_one_worker_per_node {
            // Unset conflicting Slurm memory variables before srun.
            // These can be inherited from a parent allocation and conflict with --mem.
            // We only unset SLURM_MEM_PER_CPU and SLURM_MEM_PER_GPU since those conflict
            // with the --mem directive (which sets SLURM_MEM_PER_NODE).
            // SLURM_MEM_PER_NODE is needed by torc-slurm-job-runner to report resources.
            // TODO: this is still not ideal.
            // This will have to change if we ever rely on these environment variables.
            script.push_str("unset SLURM_MEM_PER_CPU SLURM_MEM_PER_GPU\n");
            script.push_str("srun ");
        }
        script.push_str(&command);
        script.push('\n');

        fs::write(filename, script)
            .with_context(|| format!("Failed to write submission script to {:?}", filename))?;

        // Make the script executable
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(filename)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(filename, perms)?;
        }

        debug!("Created submission script: {:?}", filename);
        Ok(())
    }

    fn get_current_job_id(&self) -> String {
        env::var("SLURM_JOB_ID").expect("SLURM_JOB_ID environment variable not set")
    }

    fn get_environment_variables(&self) -> HashMap<String, String> {
        env::vars().filter(|(k, _)| k.contains("SLURM")).collect()
    }

    fn get_job_end_time(&self) -> Result<DateTime<Utc>> {
        // Check for fake/test mode
        if env::var("TORC_FAKE_SBATCH").is_ok() {
            return Ok(Utc::now() + chrono::Duration::days(10));
        }

        let job_id = self.get_current_job_id();
        let squeue = Self::get_squeue_exec();

        let output = Command::new(&squeue)
            .args(["-j", &job_id, "--format='%20e'"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to get job end time"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let cleaned = stdout.trim().replace('\'', "");
        let lines: Vec<&str> = cleaned.split('\n').collect();

        if lines.len() < 2 {
            return Err(anyhow::anyhow!("Unexpected squeue output format"));
        }

        let timestamp = lines[1].trim();
        let naive_dt = NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S")
            .expect("Failed to parse timestamp");
        let local_dt = Local.from_local_datetime(&naive_dt).unwrap();
        let utc_dt = local_dt.with_timezone(&Utc);
        Ok(utc_dt)
    }

    fn get_job_stats(&self, job_id: &str) -> Result<HpcJobStats> {
        let output = Command::new("sacct")
            .args([
                "-j",
                job_id,
                "--format=JobID,JobName%20,state,start,end,Account,Partition%15,QOS",
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to run sacct command"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.trim().split('\n').collect();

        if lines.len() != 6 {
            return Err(anyhow::anyhow!(
                "Unknown sacct output format: expected 6 lines, got {}",
                lines.len()
            ));
        }

        // Parse the job data line (3rd line, index 2)
        let fields: Vec<&str> = lines[2].split_whitespace().collect();

        if fields[0] != job_id {
            return Err(anyhow::anyhow!(
                "sacct returned unexpected job_id: {}",
                fields[0]
            ));
        }

        let fmt = "%Y-%m-%dT%H:%M:%S";
        let start = DateTime::parse_from_str(fields[3], fmt)?.with_timezone(&Utc);

        let end = if fields[4] == "Unknown" {
            None
        } else {
            Some(DateTime::parse_from_str(fields[4], fmt)?.with_timezone(&Utc))
        };

        Ok(HpcJobStats {
            hpc_job_id: job_id.to_string(),
            name: fields[1].to_string(),
            start,
            end,
            state: fields[2].to_string(),
            account: fields[5].to_string(),
            partition: fields[6].to_string(),
            qos: fields[7].to_string(),
        })
    }

    fn get_local_scratch(&self) -> Result<String> {
        for key in &["TMPDIR"] {
            if let Ok(value) = env::var(key) {
                return Ok(value);
            }
        }

        Ok(env::temp_dir().to_string_lossy().to_string())
    }

    fn get_memory_gb(&self) -> f64 {
        // Prefer SLURM_MEM_PER_NODE if available, as it reflects the allocated memory.
        if let Some(mem_mb) = env::var("SLURM_MEM_PER_NODE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            return mem_mb / 1024.0;
        }

        // Fall back to system total memory if SLURM_MEM_PER_NODE is unavailable
        // Note: This may not be correct for shared nodes, as it returns the total
        // memory on the node rather than the allocation. However, this is the best
        // we can do when SLURM_MEM_PER_NODE is not set (the user did not set --mem).
        // Use new_with_specifics to only refresh memory, avoiding user enumeration
        // which can crash on HPC systems with large LDAP user databases
        let sys = System::new_with_specifics(RefreshKind::new().with_memory());
        sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    fn get_node_id(&self) -> String {
        env::var("SLURM_NODEID").expect("SLURM_NODEID not set")
    }

    fn get_num_cpus(&self) -> usize {
        let cpus = env::var("SLURM_CPUS_ON_NODE").expect("SLURM_CPUS_ON_NODE not set");
        cpus.parse().expect("Failed to parse SLURM_CPUS_ON_NODE")
    }

    fn get_num_cpus_per_task(&self) -> usize {
        let cpus_per_task = env::var("SLURM_CPUS_PER_TASK").expect("SLURM_CPUS_PER_TASK not set");
        cpus_per_task
            .parse()
            .expect("Failed to parse SLURM_CPUS_PER_TASK")
    }

    fn get_num_gpus(&self) -> usize {
        if let Ok(gpus) = env::var("SLURM_JOB_GPUS") {
            gpus.split(',').count()
        } else {
            0
        }
    }

    fn get_num_nodes(&self) -> usize {
        let nodes = env::var("SLURM_JOB_NUM_NODES").expect("SLURM_JOB_NUM_NODES not set");
        nodes.parse().expect("Failed to parse SLURM_JOB_NUM_NODES")
    }

    fn get_task_pid(&self) -> usize {
        let task_pid = env::var("SLURM_TASK_PID").expect("SLURM_TASK_PID not set");
        task_pid.parse().expect("Failed to parse SLURM_TASK_PID")
    }

    fn is_head_node(&self) -> bool {
        self.get_node_id() == "0"
    }

    fn list_active_nodes(&self, job_id: &str) -> Result<Vec<String>> {
        // Check for fake/test mode
        if env::var("TORC_FAKE_SBATCH").is_ok() {
            return Ok(vec![hostname::get()?.to_string_lossy().to_string()]);
        }

        let squeue = Self::get_squeue_exec();

        // Get compact node list
        let output = Command::new(&squeue)
            .args(["-j", job_id, "--format='%5D %500N'", "-h"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to get node list from squeue"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let cleaned = stdout.trim().replace('\'', "");
        let result: Vec<&str> = cleaned.split_whitespace().collect();

        if result.len() != 2 {
            return Err(anyhow::anyhow!(
                "Unexpected squeue output format: expected 2 fields, got {}",
                result.len()
            ));
        }

        let num_nodes: usize = result[0].parse()?;
        let nodes_compact = result[1];

        // Expand compact node notation
        let output = Command::new("scontrol")
            .args(["show", "hostnames", nodes_compact])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to expand node names"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let nodes: Vec<String> = stdout.trim().split('\n').map(|s| s.to_string()).collect();

        if nodes.len() != num_nodes {
            return Err(anyhow::anyhow!(
                "Node count mismatch: got {} nodes, expected {}",
                nodes.len(),
                num_nodes
            ));
        }

        Ok(nodes)
    }

    fn submit(&self, filename: &Path) -> Result<(i32, String, String)> {
        let sbatch = Self::get_sbatch_exec();
        let filename_str = filename.to_string_lossy();

        let (return_code, stdout, stderr) =
            self.run_command_with_retries(&sbatch, &[&filename_str], 6, 10, &[])?;

        if return_code != 0 {
            return Ok((return_code, String::new(), stderr));
        }

        // Extract job ID from output
        if let Some(captures) = self.sbatch_regex.captures(&stdout) {
            let job_id = captures.get(1).unwrap().as_str().to_string();
            Ok((0, job_id, stderr))
        } else {
            error!("Failed to parse sbatch output: {}", stdout);
            Ok((
                1,
                String::new(),
                "Failed to parse job ID from sbatch output".to_string(),
            ))
        }
    }
}
