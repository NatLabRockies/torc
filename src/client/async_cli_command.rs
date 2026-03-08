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
use crate::client::slurm_utils::{parse_slurm_cpu_time, parse_slurm_memory};
use crate::memory_utils::memory_string_to_mb;
use crate::models::{JobModel, JobStatus, ResourceRequirementsModel, ResultModel, SlurmStatsModel};
use chrono::{DateTime, Utc};
use log::{self, debug, error, info, warn};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::{Child, Command, Stdio};

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

    /// Returns the Slurm step name, if running inside an allocation.
    /// Set after `start()` is called.
    pub fn step_name(&self) -> Option<&str> {
        self.step_name.as_deref()
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
        use_srun: bool,
        enable_cpu_bind: bool,
        end_time: Option<DateTime<Utc>>,
        srun_termination_signal: Option<&str>,
        target_node: Option<&str>,
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

        let slurm_job_id = if use_srun {
            std::env::var("SLURM_JOB_ID").ok()
        } else {
            None
        };
        let mut cmd = if let Some(slurm_job_id) = slurm_job_id {
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
            srun.arg(format!("--jobid={}", slurm_job_id));
            srun.arg("--ntasks=1");
            if !enable_cpu_bind {
                srun.arg("--cpu-bind=none");
            }
            // --exact tells srun to use exactly the requested CPUs/memory without
            // claiming the entire node exclusively. This allows concurrent steps
            // to share nodes in multi-node allocations.
            srun.arg("--exact");
            srun.arg(format!("--job-name={}", step_name));
            // Pin the step to a specific node when the job runner has claimed
            // resources on that node. This enables accurate per-node resource
            // tracking in multi-node allocations.
            if let Some(node) = target_node {
                srun.arg(format!("--nodelist={}", node));
            }
            if let Some(rr) = resource_requirements {
                let step_nodes = rr.step_nodes.unwrap_or(1).max(1);
                srun.arg(format!("--nodes={}", step_nodes));
                if limit_resources && rr.name != "default" {
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
            }
            // Set per-step walltime from the remaining allocation time so Slurm
            // kills the step with State=TIMEOUT (and return code 152) instead of
            // letting it run until the allocation walltime expires (which produces
            // State=CANCELLED). Integer division rounds down so the step timeout
            // fires before the allocation expires. Floor of 1 minute because
            // --time=0 means unlimited in Slurm. In practice, the job runner's
            // compute_node_min_time_for_new_jobs_seconds (default 300s) prevents
            // starting jobs with little time remaining.
            if let Some(end) = end_time {
                let remaining_secs = (end - Utc::now()).num_seconds();
                let remaining_minutes = (remaining_secs / 60).max(1);
                srun.arg(format!("--time={}", remaining_minutes));
            }
            // Pass --signal to give jobs advance warning before timeout.
            // Format: "<signal>@<seconds>" e.g. "TERM@120"
            if let Some(signal_spec) = srun_termination_signal {
                srun.arg(format!("--signal={}", signal_spec));
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
                    // Discover the numeric step ID that Slurm assigned. sstat requires
                    // numeric IDs (e.g., "1") — name-based lookup doesn't work on all
                    // Slurm installations (notably HPE Cray EX).
                    let numeric_step_id =
                        crate::client::resource_monitor::discover_step_id_with_retries(
                            &slurm_job_id,
                            step,
                        );
                    monitor.start_monitoring_slurm(
                        pid,
                        slurm_job_id,
                        step.clone(),
                        numeric_step_id,
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
                    let return_code = exit_status_to_return_code(&exit_status);
                    let status = if return_code == 0 {
                        JobStatus::Completed
                    } else {
                        JobStatus::Failed
                    };
                    return match self.handle_completion(return_code, status) {
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

        // Get resource metrics if monitoring is enabled.
        // stop_monitoring() sends a command to the monitoring thread and waits for it to
        // return the collected metrics via a response channel.
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
    /// // exit_code will be 143 (128 + 15) if killed by SIGTERM on Unix
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
        // 5 seconds between retry attempts (up to 6 attempts, worst-case ~25 seconds) when the
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
                // Override the return code based on sacct State.
                // When Slurm's cgroup OOM-kills a step, srun exits with code 1
                // and sacct ExitCode is 0:125 — neither produces the conventional
                // 137 (128+SIGKILL) that recovery heuristics check. The sacct State
                // field reliably reports OUT_OF_MEMORY / TIMEOUT.
                //
                // TIMEOUT is only overridden when the process did not exit cleanly
                // (return_code != 0). When the process handled SIGTERM (from
                // --signal) and exited 0, we keep the successful result even though
                // sacct may report State=TIMEOUT for the step.
                //
                // TIMEOUT maps to Terminated (system-initiated kill due to walltime)
                // rather than Failed (job error), matching the old behaviour where
                // the runner would send SIGTERM before the allocation expired.
                if let Some(ref state) = stats.state {
                    let override_rc = match state.as_str() {
                        "OUT_OF_MEMORY" => Some((137i64, JobStatus::Failed)),
                        "TIMEOUT" if return_code != 0 => Some((152i64, JobStatus::Terminated)),
                        _ => None,
                    };
                    if let Some((sacct_rc, sacct_status)) = override_rc {
                        info!(
                            "Overriding srun return_code {} with {} (sacct State={}) for \
                             workflow_id={} job_id={} step={}",
                            return_code, sacct_rc, state, workflow_id, self.job_id, step_name
                        );
                        self.return_code = Some(sacct_rc);
                        self.status = sacct_status;
                    }
                }

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

        let final_rc = self.return_code.unwrap_or(return_code);
        let final_status = format!("{:?}", self.status).to_lowercase();
        info!(
            "Job process completed workflow_id={} job_id={} run_id={} return_code={} status={} exec_time_s={:.3}",
            self.workflow_id.unwrap_or(0),
            self.job_id,
            self.run_id.unwrap_or(0),
            final_rc,
            final_status,
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
    /// - **128 + signal** (POSIX convention): Signal number that killed the process (e.g., 137 for SIGKILL, 143 for SIGTERM)
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
    /// } else if exit_code > 128 {
    ///     println!("Job killed by signal {}", exit_code - 128);
    /// } else {
    ///     println!("Job exited with error code {}", exit_code);
    /// }
    /// ```
    pub fn wait_for_completion(&mut self) -> Result<i64, Box<dyn std::error::Error>> {
        let exit_code = if let Some(ref mut child) = self.handle {
            // If we have issues with the process hanging, we could try_wait
            // with a timeout.
            let exit_status = child.wait()?;
            exit_status_to_return_code(&exit_status)
        } else {
            -1
        };

        // Mark as terminated with the actual exit code
        self.handle_completion(exit_code, JobStatus::Terminated)?;
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
    /// Slurm step state (e.g. "COMPLETED", "OUT_OF_MEMORY", "TIMEOUT", "FAILED").
    /// When Slurm's cgroup OOM-kills a step, the ExitCode is often `0:0` and `srun`
    /// exits with code 1, losing the OOM signal. The State field is the reliable way
    /// to detect OOM kills and timeouts.
    state: Option<String>,
}

/// Convert a `std::process::ExitStatus` to a return code.
///
/// On Unix, `ExitStatus::code()` returns `None` when the process was killed by a signal
/// (e.g. OOM kill sends SIGKILL = 9, Slurm time-limit sends SIGTERM = 15). The standard
/// shell convention encodes signal deaths as `128 + signal`, so SIGKILL → 137, which is
/// what the recovery heuristics check for OOM detection.  Falling back to `-1` would lose
/// this information and prevent correct OOM/timeout classification.
fn exit_status_to_return_code(status: &std::process::ExitStatus) -> i64 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(code) = status.code() {
            return code as i64;
        }
        // Killed by signal — encode as 128 + signal (POSIX shell convention)
        if let Some(signal) = status.signal() {
            return 128 + signal as i64;
        }
        -1
    }
    #[cfg(not(unix))]
    {
        status.code().unwrap_or(-1) as i64
    }
}

/// Call `sacct` after a job step exits to collect Slurm accounting data.
///
/// `slurmdbd` often does not commit the step record immediately after the step exits, so this
/// function retries up to `MAX_SACCT_ATTEMPTS` times with a short sleep between each attempt.
/// Returns `None` if sacct is unavailable, returns no data for the step after all retries, or
/// the output cannot be parsed. This is a best-effort call — failures are logged at debug level
/// and do not affect job result reporting.
fn collect_sacct_stats(slurm_job_id: &str, step_name: &str) -> Option<SacctStats> {
    const MAX_SACCT_ATTEMPTS: u32 = 6;
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
                // sacct -j <jobid> already returns all step records (allocation, batch, srun
                // steps) for the specified job without any extra flag.
                "--format",
                // JobName is first so we can filter by step name in code — more reliable than
                // sacct's --name flag, which on some Slurm versions matches the allocation name
                // rather than the step name.
                "JobName,MaxRSS,MaxVMSize,MaxDiskRead,MaxDiskWrite,AveCPU,NodeList,State",
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            if attempt < MAX_SACCT_ATTEMPTS {
                debug!(
                    "sacct returned non-zero exit code for step {} (attempt {}/{}): {}",
                    step_name,
                    attempt,
                    MAX_SACCT_ATTEMPTS,
                    stderr.trim()
                );
                continue;
            } else {
                warn!(
                    "sacct returned non-zero exit code for step {} after {} attempts: {}",
                    step_name,
                    MAX_SACCT_ATTEMPTS,
                    stderr.trim()
                );
                return None;
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!(
            "sacct output for step {} (attempt {}/{}): {:?}",
            step_name,
            attempt,
            MAX_SACCT_ATTEMPTS,
            stdout.as_ref()
        );
        // sacct returns one row per step (and one for the allocation itself).
        // Match by JobName only — do NOT also require non-empty memory fields: clusters
        // without cgroup memory accounting return an otherwise valid step row with empty
        // memory columns, and filtering those out causes all retries to fail silently.
        let line = stdout.lines().find(|l| {
            let fields: Vec<&str> = l.split('|').collect();
            fields.len() >= 2 && fields[0].trim() == step_name
        });

        match line {
            Some(line) => {
                let stats = parse_sacct_line(line, step_name);
                // The step row can appear with node_list populated but MaxRSS/AveCPU still
                // empty while slurmdbd is committing the accounting data asynchronously.
                // Retry if we have no useful data yet, rather than returning empty stats.
                let has_data = stats
                    .as_ref()
                    .is_some_and(|s| s.max_rss_bytes.is_some() || s.ave_cpu_seconds.is_some());
                if has_data || attempt == MAX_SACCT_ATTEMPTS {
                    return stats;
                }
                debug!(
                    "sacct row for step {} found but data fields are empty (attempt {}/{}), retrying",
                    step_name, attempt, MAX_SACCT_ATTEMPTS
                );
            }
            None => {
                if attempt < MAX_SACCT_ATTEMPTS {
                    debug!(
                        "sacct has no record for step {} yet (attempt {}/{}), retrying",
                        step_name, attempt, MAX_SACCT_ATTEMPTS
                    );
                } else {
                    warn!(
                        "sacct has no record for step {} after {} attempts; \
                         raw sacct output: {:?}",
                        step_name,
                        MAX_SACCT_ATTEMPTS,
                        stdout.as_ref()
                    );
                }
            }
        }
    }
    None
}

/// Parse a single pipe-separated `sacct` output line into a [`SacctStats`].
///
/// Expected format (8 fields): `JobName|MaxRSS|MaxVMSize|MaxDiskRead|MaxDiskWrite|AveCPU|NodeList|State`
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
        "sacct stats for step {}: MaxRSS={} MaxVMSize={} MaxDiskRead={} MaxDiskWrite={} AveCPU={} NodeList={} State={}",
        step_name,
        fields[1],
        fields[2],
        fields[3],
        fields[4],
        fields[5],
        fields[6],
        fields.get(7).unwrap_or(&"")
    );

    let node_list = {
        let v = fields[6].trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    };

    let state = fields.get(7).and_then(|s| {
        let s = s.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    Some(SacctStats {
        max_rss_bytes: parse_slurm_memory(fields[1]),
        max_vm_size_bytes: parse_slurm_memory(fields[2]),
        max_disk_read_bytes: parse_slurm_memory(fields[3]),
        max_disk_write_bytes: parse_slurm_memory(fields[4]),
        ave_cpu_seconds: parse_slurm_cpu_time(fields[5]),
        node_list,
        state,
    })
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
    fn test_parse_sacct_line_with_state() {
        let line = "step1|1024K|2048K|512K|256K|00:01:30|node01|COMPLETED";
        let stats = parse_sacct_line(line, "step1").unwrap();
        assert_eq!(stats.state, Some("COMPLETED".to_string()));
        assert_eq!(stats.max_rss_bytes, Some(1024 * 1024));
        assert_eq!(stats.node_list, Some("node01".to_string()));
    }

    #[test]
    fn test_parse_sacct_line_out_of_memory_state() {
        let line = "step1|0|0|0|0|00:00:00|node01|OUT_OF_MEMORY";
        let stats = parse_sacct_line(line, "step1").unwrap();
        assert_eq!(stats.state, Some("OUT_OF_MEMORY".to_string()));
    }

    #[test]
    fn test_parse_sacct_line_timeout_state() {
        let line = "step1|512K|1024K|0|0|00:05:00|node01|TIMEOUT";
        let stats = parse_sacct_line(line, "step1").unwrap();
        assert_eq!(stats.state, Some("TIMEOUT".to_string()));
    }

    #[test]
    fn test_parse_sacct_line_failed_state() {
        let line = "step1|512K|1024K|0|0|00:05:00|node01|FAILED";
        let stats = parse_sacct_line(line, "step1").unwrap();
        assert_eq!(stats.state, Some("FAILED".to_string()));
    }

    #[test]
    fn test_parse_sacct_line_missing_state_field() {
        // Only 7 fields (no State column) — should still parse successfully
        let line = "step1|1024K|2048K|512K|256K|00:01:30|node01";
        let stats = parse_sacct_line(line, "step1").unwrap();
        assert_eq!(stats.state, None);
        assert_eq!(stats.max_rss_bytes, Some(1024 * 1024));
    }

    #[test]
    fn test_parse_sacct_line_empty_state() {
        let line = "step1|1024K|2048K|512K|256K|00:01:30|node01|";
        let stats = parse_sacct_line(line, "step1").unwrap();
        assert_eq!(stats.state, None);
    }

    #[test]
    fn test_parse_sacct_line_step_name_is_for_logging_only() {
        // parse_sacct_line doesn't filter by step name — the caller (collect_sacct_stats) does.
        // The step_name parameter is only used for debug log messages.
        let line = "other_step|1024K|2048K|512K|256K|00:01:30|node01|COMPLETED";
        let stats = parse_sacct_line(line, "step1");
        assert!(stats.is_some());
    }

    #[test]
    fn test_parse_sacct_line_too_few_fields() {
        let line = "step1|1024K|2048K";
        let stats = parse_sacct_line(line, "step1");
        assert!(stats.is_none());
    }
}
