//! Asynchronous CLI command execution for workflow jobs.
//!
//! This module provides [`AsyncCliCommand`], which wraps a subprocess for executing
//! workflow jobs. It supports:
//!
//! - Non-blocking process execution with status polling
//! - Graceful termination via SIGTERM (Unix) or immediate kill (Windows)
//! - Resource monitoring integration
//! - Exit code capture including signal-based terminations
//!
//! # Termination Signals
//!
//! On Unix systems, the module supports two termination methods:
//!
//! - **`terminate()`** / **`send_sigterm()`**: Sends SIGTERM to the process, allowing it
//!   to perform cleanup before exiting. The process should handle SIGTERM and exit
//!   gracefully within a reasonable time.
//!
//! - **`cancel()`**: Sends SIGKILL to immediately terminate the process. No cleanup
//!   is performed.
//!
//! On non-Unix systems, both methods result in immediate process termination.
//!
//! After calling `terminate()` or `cancel()`, call `wait_for_completion()` to wait
//! for the process to exit and capture its exit code.

use crate::client::log_paths::{get_job_stderr_path, get_job_stdout_path};
use crate::client::resource_monitor::ResourceMonitor;
use crate::memory_utils::memory_string_to_mb;
use crate::models::{JobModel, JobStatus, ResourceRequirementsModel, ResultModel, SlurmStatsModel};
use chrono::{DateTime, Utc};
use log::{self, debug, error, info, warn};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

const JOB_STDIO_DIR: &str = "job_stdio";

#[allow(dead_code)]
pub struct AsyncCliCommand {
    pub job: JobModel,
    pub job_id: i64,
    workflow_id: Option<i64>,
    run_id: Option<i64>,
    attempt_id: Option<i64>,
    /// Slurm step name set when running inside an allocation (for sacct lookup).
    step_name: Option<String>,
    /// Slurm accounting stats collected via sacct after step completion.
    slurm_stats: Option<SlurmStatsModel>,
    handle: Option<Child>,
    pid: Option<u32>,
    pub is_running: bool,
    start_time: DateTime<Utc>,
    completion_time: Option<DateTime<Utc>>,
    exec_time_s: f64,
    return_code: Option<i64>,
    pub is_complete: bool,
    status: JobStatus,
    stdout_fp: Option<BufWriter<File>>,
    stderr_fp: Option<BufWriter<File>>,
}

impl AsyncCliCommand {
    pub fn new(job: JobModel) -> Self {
        let job_id = job.id.expect("Job must have an ID");
        let status = job.status.expect("Job status must be set");
        AsyncCliCommand {
            job,
            job_id,
            workflow_id: None,
            run_id: None,
            attempt_id: None,
            step_name: None,
            slurm_stats: None,
            handle: None,
            pid: None,
            is_running: false,
            start_time: Utc::now(),
            completion_time: None,
            exec_time_s: 0.0,
            return_code: None,
            is_complete: false,
            status,
            stdout_fp: None,
            stderr_fp: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        output_dir: &Path,
        workflow_id: i64,
        run_id: i64,
        attempt_id: i64,
        resource_monitor: Option<&ResourceMonitor>,
        api_url: &str,
        resource_requirements: Option<&ResourceRequirementsModel>,
        limit_resources: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_running {
            return Err("Job is already running".into());
        }

        let job_id_str = self.job_id.to_string();
        let workflow_id_str = workflow_id.to_string();
        let attempt_id_str = attempt_id.to_string();

        // Create output file paths using consistent naming from log_paths
        let stdio_dir = output_dir.join(JOB_STDIO_DIR);
        std::fs::create_dir_all(&stdio_dir)?;

        let stdout_path =
            get_job_stdout_path(output_dir, workflow_id, self.job_id, run_id, attempt_id);
        let stderr_path =
            get_job_stderr_path(output_dir, workflow_id, self.job_id, run_id, attempt_id);

        let stdout_file = File::create(&stdout_path)?;
        let stderr_file = File::create(&stderr_path)?;
        self.stdout_fp = Some(BufWriter::new(stdout_file));
        self.stderr_fp = Some(BufWriter::new(stderr_file));

        let command_str = if let Some(ref invocation_script) = self.job.invocation_script {
            format!("{} {}", invocation_script, self.job.command)
        } else {
            self.job.command.clone()
        };

        let mut cmd = if let Ok(slurm_job_id) = std::env::var("SLURM_JOB_ID") {
            // Running inside a Slurm allocation — wrap with srun so Slurm creates a
            // per-job cgroup step, enables sacct accounting, and gives HPC admins visibility.
            let step_name = format!(
                "wf{}_j{}_r{}_a{}",
                workflow_id, self.job_id, run_id, attempt_id
            );
            debug!(
                "Wrapping job with srun: slurm_job_id={} step={}",
                slurm_job_id, step_name
            );
            // Allow tests to substitute a fake srun binary via TORC_FAKE_SRUN.
            let srun_binary =
                std::env::var("TORC_FAKE_SRUN").unwrap_or_else(|_| "srun".to_string());
            let mut srun = Command::new(&srun_binary);
            srun.arg("--ntasks=1");
            srun.arg(format!("--job-name={}", step_name));
            if let Some(rr) = resource_requirements {
                let num_nodes = rr.num_nodes.max(1);
                srun.arg(format!("--nodes={}", num_nodes));
                if limit_resources {
                    srun.arg(format!("--cpus-per-task={}", rr.num_cpus));
                    match memory_string_to_mb(&rr.memory) {
                        Some(mem_mb) if mem_mb > 0 => {
                            srun.arg(format!("--mem={}M", mem_mb));
                        }
                        Some(_) => {
                            // Sub-MB value rounded to 0; omit --mem to avoid --mem=0 which in
                            // Slurm means "request all available memory on the node".
                            warn!(
                                "Memory string {:?} for job {} rounds to 0 MB; omitting --mem from srun",
                                rr.memory, self.job_id
                            );
                        }
                        None => {
                            warn!(
                                "Could not parse memory string {:?} for job {}; omitting --mem from srun",
                                rr.memory, self.job_id
                            );
                        }
                    }
                }
            } else {
                srun.arg("--nodes=1");
            }
            // Run via bash so job.command can use shell features
            srun.args(["bash", "-c", &command_str]);
            self.step_name = Some(step_name);
            srun
        } else {
            // Local execution — use the standard shell wrapper
            let mut shell = crate::client::utils::shell_command();
            shell.arg(&command_str);
            shell
        };

        let child = cmd
            .env("TORC_WORKFLOW_ID", workflow_id_str)
            .env("TORC_JOB_ID", job_id_str)
            .env("TORC_JOB_NAME", &self.job.name)
            .env("TORC_OUTPUT_DIR", output_dir.to_string_lossy().to_string())
            .env("TORC_ATTEMPT_ID", attempt_id_str)
            .env("TORC_API_URL", api_url)
            .stdout(Stdio::from(File::create(&stdout_path)?))
            .stderr(Stdio::from(File::create(&stderr_path)?))
            .spawn()?;

        let pid = child.id();
        self.pid = Some(pid);
        self.handle = Some(child);
        self.workflow_id = Some(workflow_id);
        self.run_id = Some(run_id);
        self.attempt_id = Some(attempt_id);
        self.is_running = true;
        self.start_time = Utc::now();
        self.status = JobStatus::Running;
        debug!(
            "Job process started workflow_id={} job_id={} pid={}",
            workflow_id, self.job_id, pid
        );

        // Start resource monitoring if enabled.
        // When running inside a Slurm allocation with srun, the job executes inside
        // slurmstepd (not as a child of the srun process), so sysinfo process-tree
        // monitoring captures only the negligible srun overhead.  Instead:
        //   - TimeSeries mode: use sstat polling via start_monitoring_slurm().
        //   - Summary mode: skip the monitor; sacct backfill in job_runner provides final stats.
        if let Some(monitor) = resource_monitor {
            if let Some(ref step) = self.step_name {
                if let Ok(slurm_job_id) = std::env::var("SLURM_JOB_ID") {
                    monitor.start_monitoring_slurm(
                        pid,
                        slurm_job_id,
                        step.clone(),
                        self.job_id,
                        self.job.name.clone(),
                    )?;
                }
            } else {
                monitor.start_monitoring(pid, self.job_id, self.job.name.clone())?;
            }
        }

        // TODO: CPU Affinity
        Ok(())
    }

    pub fn check_status(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_running || self.handle.is_none() {
            return Ok(());
        }

        if let Some(ref mut child) = self.handle {
            match child.try_wait()? {
                None => {
                    // Process is still running
                }
                Some(exit_status) => {
                    let return_code = exit_status.code().unwrap_or(-1);
                    let status = if return_code == 0 {
                        JobStatus::Completed
                    } else {
                        JobStatus::Failed
                    };
                    return match self.handle_completion(return_code as i64, status) {
                        Ok(_) => Ok(()),
                        Err(e) => Err(e),
                    };
                }
            }
        }

        Ok(())
    }

    /// Get the result of the completed job as a ResultModel.
    pub fn get_result(
        &self,
        run_id: i64,
        attempt_id: i64,
        compute_node_id: i64,
        resource_monitor: Option<&ResourceMonitor>,
    ) -> ResultModel {
        assert!(self.is_complete, "Job is not yet complete");
        let timestamp = self
            .completion_time
            .expect("A completed job must have a completion_time");
        let timestamp_str = timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        // Get resource metrics if monitoring is enabled
        // NOTE: stop_monitoring() transfers metrics from the monitoring thread's local HashMap
        // to the shared HashMap and returns them. Using get_metrics() won't work because
        // metrics are only transferred when StopMonitoring command is processed.
        let (peak_mem, avg_mem, peak_cpu, avg_cpu) = if let Some(monitor) = resource_monitor {
            if let Some(pid) = self.pid {
                if let Some(metrics) = monitor.stop_monitoring(pid) {
                    (
                        Some(metrics.peak_memory_bytes as i64),
                        Some(metrics.avg_memory_bytes as i64),
                        Some(metrics.peak_cpu_percent),
                        Some(metrics.avg_cpu_percent),
                    )
                } else {
                    (None, None, None, None)
                }
            } else {
                (None, None, None, None)
            }
        } else {
            (None, None, None, None)
        };

        let mut result = ResultModel::new(
            self.job_id,
            self.job.workflow_id,
            run_id,
            attempt_id,
            compute_node_id,
            self.return_code
                .expect("A completed job must have a return code"),
            self.exec_time_s / 60.0,
            timestamp_str,
            self.status,
        );

        // Set resource metrics
        result.peak_memory_bytes = peak_mem;
        result.avg_memory_bytes = avg_mem;
        result.peak_cpu_percent = peak_cpu;
        result.avg_cpu_percent = avg_cpu;

        result
    }

    /// Returns the Slurm accounting stats collected for this job step, if any.
    /// Only populated when the job ran inside a Slurm allocation and sacct succeeded.
    pub fn take_slurm_stats(&mut self) -> Option<SlurmStatsModel> {
        self.slurm_stats.take()
    }

    /// Immediately kills the job process using SIGKILL.
    ///
    /// This method sends SIGKILL to the process, which cannot be caught or ignored.
    /// The process will be terminated immediately without any cleanup. Use this for
    /// jobs that don't support graceful termination.
    ///
    /// **Note**: This method does not wait for the process to exit. Call
    /// [`wait_for_completion()`] afterwards to wait for the process and capture its exit code.
    ///
    /// # Example
    ///
    /// ```ignore
    /// async_cmd.cancel()?;
    /// let exit_code = async_cmd.wait_for_completion()?;
    /// ```
    pub fn cancel(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut child) = self.handle {
            child.kill()?;
        }
        Ok(())
    }

    /// Sends SIGTERM to the process for graceful termination (Unix only).
    ///
    /// SIGTERM is a signal that requests the process to terminate gracefully. Well-behaved
    /// processes should catch this signal and perform cleanup (save state, flush buffers,
    /// release resources) before exiting.
    ///
    /// **Note**: This method does not wait for the process to exit. Call
    /// [`wait_for_completion()`] afterwards to wait for the process and capture its exit code.
    ///
    /// # Platform Behavior
    ///
    /// - **Unix**: Sends SIGTERM via `libc::kill()`
    /// - **Windows/Other**: Falls back to `kill()` (SIGKILL equivalent)
    ///
    /// # Example
    ///
    /// ```ignore
    /// async_cmd.send_sigterm()?;
    /// let exit_code = async_cmd.wait_for_completion()?;
    /// // exit_code will be negative (-15) if killed by SIGTERM on Unix
    /// ```
    #[cfg(unix)]
    pub fn send_sigterm(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref child) = self.handle {
            let pid = child.id();
            debug!("Sending SIGTERM to job {} (PID {})", self.job_id, pid);
            let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
            if result != 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!(
                    "Failed to send SIGTERM to job {} (PID {}): {}",
                    self.job_id, pid, err
                )
                .into());
            }
        }
        Ok(())
    }

    /// Sends a termination signal to the process (non-Unix fallback).
    ///
    /// On non-Unix systems (Windows, etc.), SIGTERM is not available, so this method
    /// falls back to immediately killing the process. Jobs running on these platforms
    /// will not have an opportunity for graceful cleanup.
    ///
    /// **Note**: This method does not wait for the process to exit. Call
    /// [`wait_for_completion()`] afterwards to wait for the process and capture its exit code.
    #[cfg(not(unix))]
    pub fn send_sigterm(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut child) = self.handle {
            debug!(
                "Sending kill signal to job {} (SIGTERM not available on this platform)",
                self.job_id
            );
            child.kill()?;
        }
        Ok(())
    }

    /// Requests graceful termination of the job by sending SIGTERM.
    ///
    /// This is an alias for [`send_sigterm()`]. Use this method when you want to give
    /// the job process an opportunity to clean up before exiting.
    ///
    /// **Note**: This method does not wait for the process to exit. Call
    /// [`wait_for_completion()`] afterwards to wait for the process and capture its exit code.
    ///
    /// # Graceful Shutdown Flow
    ///
    /// 1. Call `terminate()` to send SIGTERM
    /// 2. The process catches SIGTERM and performs cleanup
    /// 3. Call `wait_for_completion()` to wait for exit and get the exit code
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Graceful termination
    /// async_cmd.terminate()?;
    /// let exit_code = async_cmd.wait_for_completion()?;
    /// assert!(async_cmd.is_complete);
    /// ```
    pub fn terminate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_sigterm()
    }

    // Force the job to completion with a return code and status. Does not send anything
    // to the process.
    // pub fn force_complete(mut self, return_code: i64, status: JobStatus) -> Result<(), Box<dyn std::error::Error>>  {
    //     match self.handle_completion(return_code, status) {
    //         Ok(_) => Ok(()),
    //         Err(e) => Err(e),
    //     }
    // }

    /// Perform cleanup operations after the command has completed.
    fn handle_completion(
        &mut self,
        return_code: i64,
        status: JobStatus,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut child) = self.handle {
            child.kill()?;
            child.wait()?;
        }
        self.is_running = false;
        self.is_complete = true;
        self.completion_time = Some(Utc::now());
        self.exec_time_s =
            (self.completion_time.unwrap() - self.start_time).num_milliseconds() as f64 / 1000.0;
        self.status = status;
        self.return_code = Some(return_code);
        self.stdout_fp = None;
        self.stderr_fp = None;
        self.handle = None;

        // Collect Slurm accounting stats via sacct when running inside an allocation.
        // Note: collect_sacct_stats is synchronous and may delay this polling cycle: it sleeps
        // 5 seconds between retry attempts (up to 3 retries, worst-case ~15 seconds) when the
        // Slurm accounting daemon hasn't written the step record yet.
        if let (Ok(slurm_job_id), Some(step_name)) =
            (std::env::var("SLURM_JOB_ID"), self.step_name.as_deref())
        {
            info!(
                "Collecting sacct stats for workflow_id={} job_id={} step={}",
                self.workflow_id.unwrap_or(0),
                self.job_id,
                step_name
            );
            if let Some(stats) = collect_sacct_stats(&slurm_job_id, step_name)
                && let (Some(workflow_id), Some(run_id), Some(attempt_id)) =
                    (self.workflow_id, self.run_id, self.attempt_id)
            {
                let mut slurm_stats =
                    SlurmStatsModel::new(workflow_id, self.job_id, run_id, attempt_id);
                slurm_stats.slurm_job_id = Some(slurm_job_id);
                slurm_stats.max_rss_bytes = stats.max_rss_bytes;
                slurm_stats.max_vm_size_bytes = stats.max_vm_size_bytes;
                slurm_stats.max_disk_read_bytes = stats.max_disk_read_bytes;
                slurm_stats.max_disk_write_bytes = stats.max_disk_write_bytes;
                slurm_stats.ave_cpu_seconds = stats.ave_cpu_seconds;
                slurm_stats.node_list = stats.node_list;
                info!(
                    "Sacct stats collected workflow_id={} job_id={} step={}",
                    workflow_id, self.job_id, step_name
                );
                self.slurm_stats = Some(slurm_stats);
            }
        }

        let status_str = format!("{:?}", status).to_lowercase();
        info!(
            "Job process completed workflow_id={} job_id={} run_id={} return_code={} status={} exec_time_s={:.3}",
            self.workflow_id.unwrap_or(0),
            self.job_id,
            self.run_id.unwrap_or(0),
            return_code,
            status_str,
            self.exec_time_s
        );
        Ok(())
    }

    /// Return the job ID.
    #[allow(dead_code)]
    pub fn get_job_id(&self) -> i64 {
        self.job.id.expect("Job ID must be set")
    }

    // Get the process ID of the running job. Can only be called if the job is running.
    // pub fn get_pid(&self) -> Result<u32, Box<dyn std::error::Error>> {
    //     if !self.is_running {
    //         return Err("Job is not running".into());
    //     }

    //     if let Some(ref child) = self.handle {
    //         Ok(child.id())
    //     } else {
    //         Err("No process handle available".into())
    //     }
    // }

    // pub fn get_exec_time_minutes(&self) -> f64 {
    //     self.exec_time_s / 60.0
    // }

    /// Waits for the process to exit and returns its exit code.
    ///
    /// This method blocks until the process exits. It should be called after
    /// [`terminate()`] or [`cancel()`] to wait for the process to finish and
    /// capture its exit code.
    ///
    /// After this method returns, the job is marked as complete with status
    /// `JobStatus::Terminated`.
    ///
    /// # Returns
    ///
    /// - **Positive value**: Normal exit code from the process
    /// - **Negative value** (Unix): Signal number that killed the process (e.g., -15 for SIGTERM, -9 for SIGKILL)
    /// - **-1**: Unknown exit status
    ///
    /// # Example
    ///
    /// ```ignore
    /// async_cmd.terminate()?;  // Send SIGTERM
    /// let exit_code = async_cmd.wait_for_completion()?;
    ///
    /// if exit_code == 0 {
    ///     println!("Job exited normally");
    /// } else if exit_code < 0 {
    ///     println!("Job killed by signal {}", -exit_code);
    /// } else {
    ///     println!("Job exited with error code {}", exit_code);
    /// }
    /// ```
    pub fn wait_for_completion(&mut self) -> Result<i32, Box<dyn std::error::Error>> {
        let exit_code = if let Some(ref mut child) = self.handle {
            // If we have issues with the process hanging, we could try_wait
            // with a timeout.
            let exit_status = child.wait()?;

            #[cfg(unix)]
            {
                // On Unix, check if the process was terminated by a signal
                if let Some(code) = exit_status.code() {
                    code
                } else if let Some(signal) = exit_status.signal() {
                    // Process was killed by a signal - return negative signal number
                    // This is a common Unix convention
                    debug!("Job {} was terminated by signal {}", self.job_id, signal);
                    -signal
                } else {
                    -1
                }
            }
            #[cfg(not(unix))]
            {
                exit_status.code().unwrap_or(-1)
            }
        } else {
            -1
        };

        // Mark as terminated with the actual exit code
        self.handle_completion(exit_code as i64, JobStatus::Terminated)?;
        Ok(exit_code)
    }
}

/// Slurm accounting stats collected from `sacct` after step completion.
struct SacctStats {
    max_rss_bytes: Option<i64>,
    max_vm_size_bytes: Option<i64>,
    max_disk_read_bytes: Option<i64>,
    max_disk_write_bytes: Option<i64>,
    ave_cpu_seconds: Option<f64>,
    node_list: Option<String>,
}

/// Call `sacct` after a job step exits to collect Slurm accounting data.
///
/// `slurmdbd` often does not commit the step record immediately after the step exits, so this
/// function retries up to `MAX_SACCT_ATTEMPTS` times with a short sleep between each attempt.
/// Returns `None` if sacct is unavailable, returns no data for the step after all retries, or
/// the output cannot be parsed. This is a best-effort call — failures are logged at debug level
/// and do not affect job result reporting.
fn collect_sacct_stats(slurm_job_id: &str, step_name: &str) -> Option<SacctStats> {
    const MAX_SACCT_ATTEMPTS: u32 = 4;
    const SACCT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

    // Allow tests to substitute a fake sacct binary via TORC_FAKE_SACCT.
    let sacct_binary = std::env::var("TORC_FAKE_SACCT").unwrap_or_else(|_| "sacct".to_string());

    for attempt in 1..=MAX_SACCT_ATTEMPTS {
        // slurmdbd may not have written the step record yet; wait before retries.
        if attempt > 1 {
            std::thread::sleep(SACCT_RETRY_DELAY);
        }

        let output = std::process::Command::new(&sacct_binary)
            .args([
                "-j",
                slurm_job_id,
                "--allsteps", // include job step records, not just the allocation-level entry
                "--format",
                // JobName is first so we can filter by step name in code — more reliable than
                // sacct's --name flag, which on some Slurm versions matches the allocation name
                // rather than the step name.
                "JobName,MaxRSS,MaxVMSize,MaxDiskRead,MaxDiskWrite,AveCPU,NodeList",
                "-P", // pipe-separated output
                "-n", // no header
            ])
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => {
                debug!(
                    "sacct not available or failed for step {}: {}",
                    step_name, e
                );
                return None;
            }
        };

        if !output.status.success() {
            warn!(
                "sacct returned non-zero exit code for step {}: {}",
                step_name,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // sacct returns one row per step (and one for the allocation itself).
        // Find the row whose JobName matches our step name AND has at least one non-empty memory
        // field. Filtering by JobName in code is more portable than using sacct's --name flag.
        let line = stdout.lines().find(|l| {
            let fields: Vec<&str> = l.split('|').collect();
            fields.len() >= 4
                && fields[0].trim() == step_name
                && (!fields[1].trim().is_empty()
                    || !fields[2].trim().is_empty()
                    || !fields[3].trim().is_empty())
        });

        match line {
            Some(line) => {
                return parse_sacct_line(line, step_name);
            }
            None => {
                if attempt < MAX_SACCT_ATTEMPTS {
                    debug!(
                        "sacct returned no step data for step {} (attempt {}/{}), retrying",
                        step_name, attempt, MAX_SACCT_ATTEMPTS
                    );
                } else {
                    warn!(
                        "sacct returned no step data for step {} after {} attempts; Slurm stats will not be recorded",
                        step_name, MAX_SACCT_ATTEMPTS
                    );
                }
            }
        }
    }
    None
}

/// Parse a single pipe-separated `sacct` output line into a [`SacctStats`].
///
/// Expected format (7 fields): `JobName|MaxRSS|MaxVMSize|MaxDiskRead|MaxDiskWrite|AveCPU|NodeList`
fn parse_sacct_line(line: &str, step_name: &str) -> Option<SacctStats> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 7 {
        debug!(
            "sacct output for step {} has fewer than 7 fields: {:?}",
            step_name, fields
        );
        return None;
    }

    debug!(
        "sacct stats for step {}: MaxRSS={} MaxVMSize={} MaxDiskRead={} MaxDiskWrite={} AveCPU={} NodeList={}",
        step_name, fields[1], fields[2], fields[3], fields[4], fields[5], fields[6]
    );

    let node_list = {
        let v = fields[6].trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    };

    Some(SacctStats {
        max_rss_bytes: parse_slurm_memory(fields[1]),
        max_vm_size_bytes: parse_slurm_memory(fields[2]),
        max_disk_read_bytes: parse_slurm_memory(fields[3]),
        max_disk_write_bytes: parse_slurm_memory(fields[4]),
        ave_cpu_seconds: parse_slurm_cpu_time(fields[5]),
        node_list,
    })
}

/// Parse a Slurm memory string (e.g. "512K", "1.50M", "2G") into bytes.
/// Returns `None` for empty or unparseable values; `Some(0)` for "0".
pub(crate) fn parse_slurm_memory(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s == "0" {
        return Some(0);
    }
    let (num_str, multiplier) = if let Some(rest) = s.strip_suffix('K') {
        (rest, 1_024i64)
    } else if let Some(rest) = s.strip_suffix('M') {
        (rest, 1_024 * 1_024)
    } else if let Some(rest) = s.strip_suffix('G') {
        (rest, 1_024 * 1_024 * 1_024)
    } else if let Some(rest) = s.strip_suffix('T') {
        (rest, 1_024 * 1_024 * 1_024 * 1_024)
    } else {
        (s, 1)
    };
    let n: f64 = num_str.parse().ok()?;
    Some((n * multiplier as f64) as i64)
}

/// Parse a Slurm CPU time string (`[D-]HH:MM:SS`) into seconds.
/// Returns `None` for empty or unparseable values.
pub(crate) fn parse_slurm_cpu_time(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (days, rest) = if let Some(dash) = s.find('-') {
        let d: u64 = s[..dash].parse().ok()?;
        (d, &s[dash + 1..])
    } else {
        (0, s)
    };
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec: f64 = parts[2].parse().ok()?;
    Some((days * 86_400 + h * 3_600 + m * 60) as f64 + sec)
}

impl Drop for AsyncCliCommand {
    fn drop(&mut self) {
        if self.is_running {
            error!(
                "Job is being dropped while running. Terminating job {}",
                self.get_job_id()
            );
            let _ = self.terminate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_memory_units() {
        assert_eq!(parse_slurm_memory("0"), Some(0));
        assert_eq!(parse_slurm_memory("512K"), Some(512 * 1_024));
        assert_eq!(parse_slurm_memory("2M"), Some(2 * 1_024 * 1_024));
        assert_eq!(parse_slurm_memory("1G"), Some(1_024 * 1_024 * 1_024));
        assert_eq!(
            parse_slurm_memory("1T"),
            Some(1_024 * 1_024 * 1_024 * 1_024)
        );
    }

    #[test]
    fn test_parse_slurm_memory_decimal() {
        // sacct can emit fractional values like "1.50M"
        let result = parse_slurm_memory("1.50M").unwrap();
        assert!((result as f64 - 1.5 * 1_024.0 * 1_024.0).abs() < 1.0);
    }

    #[test]
    fn test_parse_slurm_memory_no_suffix() {
        // Raw bytes
        assert_eq!(parse_slurm_memory("1024"), Some(1024));
    }

    #[test]
    fn test_parse_slurm_memory_empty() {
        assert_eq!(parse_slurm_memory(""), None);
        assert_eq!(parse_slurm_memory("  "), None);
    }

    #[test]
    fn test_parse_slurm_cpu_time_hhmmss() {
        assert_eq!(parse_slurm_cpu_time("00:01:30"), Some(90.0));
        assert_eq!(parse_slurm_cpu_time("01:00:00"), Some(3_600.0));
        assert_eq!(parse_slurm_cpu_time("00:00:00"), Some(0.0));
    }

    #[test]
    fn test_parse_slurm_cpu_time_with_days() {
        // Format: D-HH:MM:SS
        assert_eq!(parse_slurm_cpu_time("1-02:30:00"), Some(95_400.0));
        assert_eq!(parse_slurm_cpu_time("0-00:00:01"), Some(1.0));
    }

    #[test]
    fn test_parse_slurm_cpu_time_empty() {
        assert_eq!(parse_slurm_cpu_time(""), None);
        assert_eq!(parse_slurm_cpu_time("  "), None);
    }

    #[test]
    fn test_parse_slurm_cpu_time_fractional_seconds() {
        // Some sacct versions emit sub-second values
        let result = parse_slurm_cpu_time("00:00:01.5").unwrap();
        assert!((result - 1.5).abs() < 0.001);
    }
}
