use crate::client::slurm_utils::{parse_slurm_cpu_time, parse_slurm_memory};
use log::{debug, error, info, warn};
use rusqlite::{Connection, Result as SqliteResult};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use sysinfo::{
    CpuExt, CpuRefreshKind, Pid, ProcessExt, ProcessRefreshKind, RefreshKind, System, SystemExt,
};

const DB_FILENAME_PREFIX: &str = "resource_metrics";

/// Notification sent when a job exceeds its memory limit.
///
/// The job runner should kill the job and mark it as OOM-killed.
#[derive(Debug, Clone)]
pub struct OomViolation {
    /// PID of the job process (used to identify the job in running_jobs map).
    pub pid: u32,
    /// Torc job ID.
    pub job_id: i64,
    /// Current memory usage in bytes.
    pub memory_bytes: u64,
    /// Configured memory limit in bytes.
    pub limit_bytes: u64,
}

/// Configuration for resource monitoring
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ResourceMonitorConfig {
    /// Deprecated compatibility field. Use `jobs.enabled` for new workflow specs.
    pub enabled: bool,
    /// Deprecated compatibility field. Use `jobs.granularity` for new workflow specs.
    pub granularity: MonitorGranularity,
    pub sample_interval_seconds: i32,
    pub generate_plots: bool,
    pub jobs: Option<JobMonitorConfig>,
    pub compute_node: Option<ComputeNodeMonitorConfig>,
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        ResourceMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
            sample_interval_seconds: 10,
            generate_plots: false,
            jobs: None,
            compute_node: None,
        }
    }
}

impl ResourceMonitorConfig {
    pub fn jobs_config(&self) -> JobMonitorConfig {
        self.jobs.clone().unwrap_or(JobMonitorConfig {
            enabled: self.enabled,
            granularity: self.granularity.clone(),
        })
    }

    pub fn compute_node_config(&self) -> Option<ComputeNodeMonitorConfig> {
        self.compute_node.clone().filter(|config| config.enabled)
    }

    pub fn is_enabled(&self) -> bool {
        self.jobs_config().enabled || self.compute_node_config().is_some()
    }

    /// Returns true if any enabled scope uses time-series granularity, which is when the
    /// time-series SQLite database is created and populated.
    pub fn has_timeseries_db(&self) -> bool {
        let jobs_ts = {
            let jobs = self.jobs_config();
            jobs.enabled && matches!(jobs.granularity, MonitorGranularity::TimeSeries)
        };
        let node_ts = self
            .compute_node_config()
            .is_some_and(|c| matches!(c.granularity, MonitorGranularity::TimeSeries));
        jobs_ts || node_ts
    }
}

/// Returns the path of the time-series metrics database that would be produced for the
/// given `output_dir` / `unique_label`. This mirrors the layout created by
/// `init_timeseries_db` so callers (e.g. post-run plot generation) can locate the file.
pub fn timeseries_db_path(output_dir: &Path, unique_label: &str) -> PathBuf {
    output_dir
        .join("resource_utilization")
        .join(format!("{}_{}.db", DB_FILENAME_PREFIX, unique_label))
}

/// Configuration for per-job resource monitoring.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct JobMonitorConfig {
    pub enabled: bool,
    pub granularity: MonitorGranularity,
}

impl Default for JobMonitorConfig {
    fn default() -> Self {
        JobMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
        }
    }
}

/// Configuration for compute-node resource monitoring.
///
/// Compute-node monitoring is intentionally configured separately from per-job monitoring so
/// future node-level GPU sampling can be added without changing job metrics semantics.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ComputeNodeMonitorConfig {
    pub enabled: bool,
    pub granularity: MonitorGranularity,
    pub cpu: bool,
    pub memory: bool,
}

impl Default for ComputeNodeMonitorConfig {
    fn default() -> Self {
        ComputeNodeMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
            cpu: true,
            memory: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorGranularity {
    Summary,
    TimeSeries,
}

/// Metrics collected for a single job
#[derive(Debug, Clone)]
pub struct JobMetrics {
    pub peak_memory_bytes: u64,
    pub avg_memory_bytes: u64,
    pub peak_cpu_percent: f64,
    pub avg_cpu_percent: f64,
    sample_count: usize,
    total_memory_bytes: u64,
    total_cpu_percent: f64,
}

impl JobMetrics {
    fn new() -> Self {
        JobMetrics {
            peak_memory_bytes: 0,
            avg_memory_bytes: 0,
            peak_cpu_percent: 0.0,
            avg_cpu_percent: 0.0,
            sample_count: 0,
            total_memory_bytes: 0,
            total_cpu_percent: 0.0,
        }
    }

    /// Upper bound for a plausible CPU percentage.  Even on a 1024-core node at
    /// 100 % per core the value would be 102400 %.  Anything above this threshold
    /// is treated as a garbage sample (e.g. sstat returning stale data for an
    /// OOM-killed step, or sysinfo reading /proc for a dying process).
    const MAX_PLAUSIBLE_CPU_PERCENT: f64 = 100_000.0;

    fn add_sample(&mut self, cpu_percent: f64, memory_bytes: u64) {
        // Sanitize: reject garbage CPU values (NaN, infinity, negative, or unreasonably high).
        let cpu_percent = if cpu_percent.is_finite()
            && (0.0..=Self::MAX_PLAUSIBLE_CPU_PERCENT).contains(&cpu_percent)
        {
            cpu_percent
        } else {
            0.0
        };

        self.sample_count += 1;
        self.total_cpu_percent += cpu_percent;
        self.total_memory_bytes += memory_bytes;

        if cpu_percent > self.peak_cpu_percent {
            self.peak_cpu_percent = cpu_percent;
        }
        if memory_bytes > self.peak_memory_bytes {
            self.peak_memory_bytes = memory_bytes;
        }

        self.avg_cpu_percent = self.total_cpu_percent / self.sample_count as f64;
        self.avg_memory_bytes = self.total_memory_bytes / self.sample_count as u64;
    }
}

#[derive(Debug, Clone)]
pub struct SystemMetricsSummary {
    pub sample_count: i64,
    pub peak_cpu_percent: f64,
    pub avg_cpu_percent: f64,
    pub peak_memory_bytes: u64,
    pub avg_memory_bytes: u64,
}

/// Metrics collected for the whole system while this runner is active.
#[derive(Debug, Clone)]
struct SystemMetrics {
    peak_cpu_percent: f64,
    avg_cpu_percent: f64,
    peak_memory_bytes: u64,
    avg_memory_bytes: u64,
    sample_count: usize,
    total_cpu_percent: f64,
    total_memory_bytes: u64,
}

impl SystemMetrics {
    fn new() -> Self {
        SystemMetrics {
            peak_cpu_percent: 0.0,
            avg_cpu_percent: 0.0,
            peak_memory_bytes: 0,
            avg_memory_bytes: 0,
            sample_count: 0,
            total_cpu_percent: 0.0,
            total_memory_bytes: 0,
        }
    }

    fn add_sample(&mut self, cpu_percent: f64, memory_bytes: u64) {
        let cpu_percent = if cpu_percent.is_finite()
            && (0.0..=JobMetrics::MAX_PLAUSIBLE_CPU_PERCENT).contains(&cpu_percent)
        {
            cpu_percent
        } else {
            0.0
        };

        self.sample_count += 1;
        self.total_cpu_percent += cpu_percent;
        self.total_memory_bytes += memory_bytes;

        if cpu_percent > self.peak_cpu_percent {
            self.peak_cpu_percent = cpu_percent;
        }
        if memory_bytes > self.peak_memory_bytes {
            self.peak_memory_bytes = memory_bytes;
        }

        self.avg_cpu_percent = self.total_cpu_percent / self.sample_count as f64;
        self.avg_memory_bytes = self.total_memory_bytes / self.sample_count as u64;
    }

    fn summary(&self) -> Option<SystemMetricsSummary> {
        if self.sample_count == 0 {
            return None;
        }

        Some(SystemMetricsSummary {
            sample_count: self.sample_count as i64,
            peak_cpu_percent: self.peak_cpu_percent,
            avg_cpu_percent: self.avg_cpu_percent,
            peak_memory_bytes: self.peak_memory_bytes,
            avg_memory_bytes: self.avg_memory_bytes,
        })
    }
}

/// Source of resource samples for a monitored job.
enum MonitorJobSource {
    /// Local execution: walk the process tree via sysinfo.
    Local { pid: u32 },
    /// Slurm step: poll `sstat` for live accounting data (TimeSeries mode only).
    ///
    /// `prev_ave_cpu_s` and `prev_sample_at` are used to derive an instantaneous
    /// CPU-utilisation rate from the monotonically-increasing `AveCPU` counter that
    /// sstat returns.
    Slurm {
        slurm_job_id: String,
        step_name: String,
        /// Numeric step ID (e.g., "1") discovered via `squeue --steps`.
        /// `None` until the step is registered in Slurm's accounting.
        numeric_step_id: Option<String>,
        /// AveCPU value (in seconds) from the previous sstat poll.
        /// `None` until the first successful sstat sample (used to skip the first
        /// sample whose cumulative AveCPU has no valid baseline for delta computation).
        prev_ave_cpu_s: Option<f64>,
        /// Wall-clock time of the previous sstat poll.
        prev_sample_at: Instant,
    },
}

/// Commands sent to the monitoring thread
enum MonitorCommand {
    StartMonitoring {
        pid: u32,
        job_id: i64,
        job_name: String,
        /// Memory limit in bytes. If set and exceeded, an OOM violation is sent.
        memory_limit_bytes: Option<u64>,
    },
    /// Register a Slurm step for sstat-based monitoring (TimeSeries mode).
    /// `pid` is the srun PID, used as the map key so that `stop_monitoring(pid)` works
    /// without API changes.
    StartMonitoringSlurm {
        pid: u32,
        slurm_job_id: String,
        step_name: String,
        /// Numeric step ID (e.g., "1") discovered at launch time. `None` if discovery
        /// failed, in which case the monitor will attempt batch discovery via squeue --steps.
        numeric_step_id: Option<String>,
        job_id: i64,
        job_name: String,
    },
    StopMonitoring {
        pid: u32,
        /// Channel to send back the collected metrics for this PID.
        response_tx: Sender<Option<JobMetrics>>,
    },
    Shutdown {
        response_tx: Sender<Option<SystemMetricsSummary>>,
    },
}

/// Active job being monitored
struct MonitoredJob {
    job_id: i64,
    /// PID used as map key.  For Slurm jobs this is the srun PID.
    #[allow(dead_code)]
    pid: u32,
    source: MonitorJobSource,
    metrics: JobMetrics,
    /// Memory limit in bytes. If set, the job will be flagged for OOM kill when exceeded.
    memory_limit_bytes: Option<u64>,
    /// Whether an OOM violation has already been sent for this job (to avoid duplicates).
    oom_violation_sent: bool,
}

/// Resource monitor manages a single background thread that monitors all running jobs
pub struct ResourceMonitor {
    tx: Sender<MonitorCommand>,
    handle: Option<JoinHandle<()>>,
    config: ResourceMonitorConfig,
    /// Path of the time-series SQLite DB, set only when a time-series scope is active.
    db_path: Option<PathBuf>,
    /// Receiver for OOM violation notifications from the monitoring thread.
    oom_rx: Receiver<OomViolation>,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(
        config: ResourceMonitorConfig,
        output_dir: PathBuf,
        unique_label: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = channel();
        let (oom_tx, oom_rx) = channel();
        let config_clone = config.clone();
        let db_path = config
            .has_timeseries_db()
            .then(|| timeseries_db_path(&output_dir, &unique_label));

        let handle = thread::spawn(move || {
            if let Err(e) = run_monitoring_loop(config_clone, output_dir, unique_label, rx, oom_tx)
            {
                error!("Resource monitoring thread failed: {}", e);
            }
        });

        Ok(ResourceMonitor {
            tx,
            handle: Some(handle),
            config,
            db_path,
            oom_rx,
        })
    }

    /// Path to the time-series metrics DB, or `None` if no time-series scope is enabled.
    pub fn timeseries_db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    /// Whether the workflow requested post-run plot generation.
    pub fn generate_plots(&self) -> bool {
        self.config.generate_plots
    }

    /// Returns `true` when the monitor is configured for `TimeSeries` granularity.
    pub fn is_time_series(&self) -> bool {
        matches!(
            self.config.jobs_config().granularity,
            MonitorGranularity::TimeSeries
        )
    }

    /// Returns `true` when per-job monitoring is enabled.
    pub fn jobs_enabled(&self) -> bool {
        self.config.jobs_config().enabled
    }

    /// Start monitoring a local process (sysinfo process-tree walk).
    ///
    /// If `memory_limit_bytes` is set and the job exceeds this limit, an OOM violation
    /// will be sent via [`recv_oom_violations()`].
    pub fn start_monitoring(
        &self,
        pid: u32,
        job_id: i64,
        job_name: String,
        memory_limit_bytes: Option<u64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.tx.send(MonitorCommand::StartMonitoring {
            pid,
            job_id,
            job_name,
            memory_limit_bytes,
        })?;
        debug!(
            "Started monitoring job {} with PID {} (memory_limit={:?})",
            job_id, pid, memory_limit_bytes
        );
        Ok(())
    }

    /// Receive all pending OOM violations (non-blocking).
    ///
    /// Returns a vector of jobs that have exceeded their memory limits.
    /// The job runner should kill these jobs and mark them as OOM-killed.
    pub fn recv_oom_violations(&self) -> Vec<OomViolation> {
        let mut violations = Vec::new();
        loop {
            match self.oom_rx.try_recv() {
                Ok(v) => violations.push(v),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("OOM violation channel disconnected");
                    break;
                }
            }
        }
        violations
    }

    /// Register a Slurm step for sstat-based monitoring (`TimeSeries` mode only).
    ///
    /// In `TimeSeries` mode the per-sample data is written to the time-series database,
    /// enabling detailed resource utilization plots over time.
    ///
    /// In `Summary` mode this method should **not** be called — sacct backfill after job
    /// completion provides authoritative peak memory (MaxRSS) and average CPU data without
    /// the overhead of periodic sstat/squeue polling.
    ///
    /// `pid` must be the srun process PID so that the existing `stop_monitoring(pid)` API
    /// continues to work without changes.
    pub fn start_monitoring_slurm(
        &self,
        pid: u32,
        slurm_job_id: String,
        step_name: String,
        numeric_step_id: Option<String>,
        job_id: i64,
        job_name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.tx.send(MonitorCommand::StartMonitoringSlurm {
            pid,
            slurm_job_id,
            step_name,
            numeric_step_id,
            job_id,
            job_name,
        })?;
        debug!(
            "Started sstat monitoring for job {} (srun PID {})",
            job_id, pid
        );
        Ok(())
    }

    /// Stop monitoring a process and return its metrics.
    ///
    /// Sends a stop command to the monitoring thread and waits for it to return
    /// the collected metrics via a response channel, with a 5-second timeout.
    pub fn stop_monitoring(&self, pid: u32) -> Option<JobMetrics> {
        let (response_tx, response_rx) = channel();
        if let Err(e) = self
            .tx
            .send(MonitorCommand::StopMonitoring { pid, response_tx })
        {
            error!("Failed to send stop monitoring command: {}", e);
            return None;
        }

        match response_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(metrics) => metrics,
            Err(e) => {
                warn!(
                    "Timed out or error waiting for metrics from monitoring thread for PID {}: {}",
                    pid, e
                );
                None
            }
        }
    }

    /// Shutdown the monitoring thread and return compute-node summary metrics, if collected.
    pub fn shutdown(self) -> Option<SystemMetricsSummary> {
        let (response_tx, response_rx) = channel();
        if let Err(e) = self.tx.send(MonitorCommand::Shutdown { response_tx }) {
            error!("Failed to send shutdown command: {}", e);
            return None;
        }

        let system_summary = match response_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(summary) => summary,
            Err(e) => {
                warn!(
                    "Timed out or error waiting for system metrics from monitoring thread: {}",
                    e
                );
                None
            }
        };

        if let Some(handle) = self.handle {
            // Wait up to 10 seconds for shutdown
            let start = Instant::now();
            while !handle.is_finished() && start.elapsed() < Duration::from_secs(10) {
                thread::sleep(Duration::from_millis(100));
            }

            if !handle.is_finished() {
                warn!("Resource monitor thread did not shutdown within 10 seconds");
            } else {
                let _ = handle.join();
                info!("Resource monitor thread shutdown successfully");
            }
        }

        system_summary
    }
}

/// Main monitoring loop that runs in a background thread
fn run_monitoring_loop(
    config: ResourceMonitorConfig,
    output_dir: PathBuf,
    unique_label: String,
    rx: Receiver<MonitorCommand>,
    oom_tx: Sender<OomViolation>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use new_with_specifics to only refresh processes, CPU, and memory, avoiding user enumeration
    // which can crash on HPC systems with large LDAP user databases
    let refresh_kind = RefreshKind::new()
        .with_processes(ProcessRefreshKind::everything())
        .with_cpu(CpuRefreshKind::everything())
        .with_memory();
    let mut sys = System::new_with_specifics(refresh_kind);
    let mut monitored_jobs: HashMap<u32, MonitoredJob> = HashMap::new();
    let sample_interval = Duration::from_secs(config.sample_interval_seconds as u64);
    let jobs_config = config.jobs_config();
    let compute_node_config = config.compute_node_config();
    let mut system_metrics = compute_node_config.as_ref().map(|_| SystemMetrics::new());

    // Allow tests to substitute a fake sstat binary via TORC_FAKE_SSTAT.
    let sstat_binary = std::env::var("TORC_FAKE_SSTAT").unwrap_or_else(|_| "sstat".to_string());

    // Initialize database if job time series or compute-node monitoring needs durable storage.
    let compute_node_time_series = compute_node_config
        .as_ref()
        .is_some_and(|c| matches!(c.granularity, MonitorGranularity::TimeSeries));
    let mut db_conn = if matches!(jobs_config.granularity, MonitorGranularity::TimeSeries)
        || compute_node_time_series
    {
        Some(init_timeseries_db(&output_dir, &unique_label)?)
    } else {
        None
    };

    info!(
        "Resource monitoring started: jobs_enabled={}, jobs_granularity={:?}, \
         compute_node_enabled={}, sample_interval={}s",
        jobs_config.enabled,
        jobs_config.granularity,
        compute_node_config.is_some(),
        config.sample_interval_seconds
    );

    let mut last_sample_time = Instant::now();

    loop {
        // Process all pending commands (non-blocking)
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                MonitorCommand::StartMonitoring {
                    pid,
                    job_id,
                    job_name,
                    memory_limit_bytes,
                } => {
                    // Store job metadata in database
                    if let Some(ref mut conn) = db_conn
                        && let Err(e) = store_job_metadata(conn, job_id, &job_name)
                    {
                        error!("Failed to store job metadata for job {}: {}", job_id, e);
                    }

                    monitored_jobs.insert(
                        pid,
                        MonitoredJob {
                            job_id,
                            pid,
                            source: MonitorJobSource::Local { pid },
                            metrics: JobMetrics::new(),
                            memory_limit_bytes,
                            oom_violation_sent: false,
                        },
                    );
                    debug!(
                        "Now monitoring {} jobs (memory_limit={:?})",
                        monitored_jobs.len(),
                        memory_limit_bytes
                    );
                }
                MonitorCommand::StartMonitoringSlurm {
                    pid,
                    slurm_job_id,
                    step_name,
                    numeric_step_id,
                    job_id,
                    job_name,
                } => {
                    if let Some(ref mut conn) = db_conn
                        && let Err(e) = store_job_metadata(conn, job_id, &job_name)
                    {
                        error!("Failed to store job metadata for job {}: {}", job_id, e);
                    }

                    // Slurm mode: OOM is handled by Slurm's cgroups, not by us.
                    monitored_jobs.insert(
                        pid,
                        MonitoredJob {
                            job_id,
                            pid,
                            source: MonitorJobSource::Slurm {
                                slurm_job_id,
                                step_name,
                                numeric_step_id,
                                prev_ave_cpu_s: None,
                                prev_sample_at: Instant::now(),
                            },
                            metrics: JobMetrics::new(),
                            memory_limit_bytes: None, // Slurm handles OOM
                            oom_violation_sent: false,
                        },
                    );
                    debug!(
                        "Now monitoring {} jobs (Slurm sstat mode)",
                        monitored_jobs.len()
                    );
                }
                MonitorCommand::StopMonitoring { pid, response_tx } => {
                    let metrics = monitored_jobs.remove(&pid).map(|job| job.metrics);
                    debug!(
                        "Stopped monitoring PID {}, {} jobs remaining",
                        pid,
                        monitored_jobs.len()
                    );
                    // Send metrics back; ignore error if receiver was dropped.
                    let _ = response_tx.send(metrics);
                }
                MonitorCommand::Shutdown { response_tx } => {
                    if let Some(compute_node_config) = &compute_node_config {
                        sys.refresh_cpu();
                        sys.refresh_memory();
                        let timestamp = chrono::Utc::now().timestamp();
                        let cpu_percent = if compute_node_config.cpu {
                            sys.global_cpu_info().cpu_usage() as f64
                        } else {
                            0.0
                        };
                        let memory_bytes = if compute_node_config.memory {
                            sys.used_memory()
                        } else {
                            0
                        };
                        let total_memory_bytes = if compute_node_config.memory {
                            sys.total_memory()
                        } else {
                            0
                        };

                        if let Some(metrics) = &mut system_metrics {
                            metrics.add_sample(cpu_percent, memory_bytes);
                        }

                        if matches!(
                            compute_node_config.granularity,
                            MonitorGranularity::TimeSeries
                        ) && let Some(ref mut conn) = db_conn
                            && let Err(e) = store_system_sample(
                                conn,
                                timestamp,
                                cpu_percent,
                                memory_bytes,
                                total_memory_bytes,
                            )
                        {
                            error!("Failed to store final system resource sample: {}", e);
                        }
                    }

                    let summary = system_metrics.as_ref().and_then(SystemMetrics::summary);
                    if let Some(ref mut conn) = db_conn
                        && let Some(ref summary) = summary
                        && let Err(e) = store_system_summary(conn, summary)
                    {
                        error!("Failed to store system resource summary: {}", e);
                    }
                    let _ = response_tx.send(summary);
                    info!("Resource monitor received shutdown command");
                    return Ok(());
                }
            }
        }

        // Sample all monitored jobs if interval has elapsed
        if last_sample_time.elapsed() >= sample_interval
            && (!monitored_jobs.is_empty() || compute_node_config.is_some())
        {
            // Refresh sysinfo once for all local jobs.
            let has_local_jobs = monitored_jobs
                .values()
                .any(|j| matches!(j.source, MonitorJobSource::Local { .. }));
            if has_local_jobs || compute_node_config.is_some() {
                sys.refresh_processes();
            }
            if compute_node_config.is_some() {
                sys.refresh_cpu();
                sys.refresh_memory();
            }

            // Batch-discover numeric step IDs for any Slurm steps that need them.
            // Run `squeue --steps` once per unique slurm_job_id (typically just one)
            // instead of once per step.
            let needs_discovery: Vec<String> = monitored_jobs
                .values()
                .filter_map(|j| match &j.source {
                    MonitorJobSource::Slurm {
                        slurm_job_id,
                        numeric_step_id: None,
                        ..
                    } => Some(slurm_job_id.clone()),
                    _ => None,
                })
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            for job_id in &needs_discovery {
                let step_map = discover_step_ids(job_id);
                for job in monitored_jobs.values_mut() {
                    if let MonitorJobSource::Slurm {
                        slurm_job_id,
                        step_name,
                        numeric_step_id,
                        ..
                    } = &mut job.source
                        && numeric_step_id.is_none()
                        && slurm_job_id == job_id
                        && let Some(id) = step_map.get(step_name.as_str())
                    {
                        debug!(
                            "Discovered numeric step ID for {}: {}.{}",
                            step_name, slurm_job_id, id
                        );
                        *numeric_step_id = Some(id.clone());
                    }
                }
            }

            // Batch-fetch sstat data for all Slurm steps in a single subprocess call.
            let slurm_steps: Vec<String> = monitored_jobs
                .values()
                .filter_map(|j| match &j.source {
                    MonitorJobSource::Slurm {
                        slurm_job_id,
                        numeric_step_id: Some(step_id),
                        ..
                    } => Some(format!("{}.{}", slurm_job_id, step_id)),
                    _ => None,
                })
                .collect();
            let sstat_data = collect_sstat_samples_batch(&slurm_steps, &sstat_binary);
            // Capture a single timestamp right after the batch call so that all Slurm
            // steps use the same reference point for elapsed-time / CPU% calculations.
            // Using per-job Instant::now() would skew deltas as the loop iterates.
            let sstat_sample_at = Instant::now();
            if !slurm_steps.is_empty() {
                debug!(
                    "Batched sstat query for {} steps, got {} results",
                    slurm_steps.len(),
                    sstat_data.len()
                );
            }

            let timestamp = chrono::Utc::now().timestamp();

            if let Some(compute_node_config) = &compute_node_config {
                let cpu_percent = if compute_node_config.cpu {
                    sys.global_cpu_info().cpu_usage() as f64
                } else {
                    0.0
                };
                let memory_bytes = if compute_node_config.memory {
                    sys.used_memory()
                } else {
                    0
                };
                let total_memory_bytes = if compute_node_config.memory {
                    sys.total_memory()
                } else {
                    0
                };

                if let Some(metrics) = &mut system_metrics {
                    metrics.add_sample(cpu_percent, memory_bytes);
                }

                if matches!(
                    compute_node_config.granularity,
                    MonitorGranularity::TimeSeries
                ) && let Some(ref mut conn) = db_conn
                    && let Err(e) = store_system_sample(
                        conn,
                        timestamp,
                        cpu_percent,
                        memory_bytes,
                        total_memory_bytes,
                    )
                {
                    error!("Failed to store system resource sample: {}", e);
                }

                debug!(
                    "System resources: CPU={:.1}%, Mem={:.1}/{:.1}MB",
                    cpu_percent,
                    memory_bytes as f64 / (1024.0 * 1024.0),
                    total_memory_bytes as f64 / (1024.0 * 1024.0)
                );
            }

            for (pid, job) in monitored_jobs.iter_mut() {
                let (cpu_percent, memory_bytes, num_processes) = match &mut job.source {
                    MonitorJobSource::Local { pid: local_pid } => {
                        collect_process_tree_stats(*local_pid, &sys)
                    }
                    MonitorJobSource::Slurm {
                        slurm_job_id,
                        numeric_step_id,
                        prev_ave_cpu_s,
                        prev_sample_at,
                        ..
                    } => {
                        let step_id = match numeric_step_id {
                            Some(id) => id.as_str(),
                            None => {
                                // Step not registered yet; skip this sample.
                                continue;
                            }
                        };
                        let job_step = format!("{}.{}", slurm_job_id, step_id);
                        let (ave_cpu_s, max_rss) = match sstat_data.get(&job_step) {
                            Some(sample) => *sample,
                            None => {
                                // sstat returned no data for this step; skip.
                                continue;
                            }
                        };

                        let elapsed_s = sstat_sample_at
                            .duration_since(*prev_sample_at)
                            .as_secs_f64();
                        *prev_sample_at = sstat_sample_at;

                        // On the first successful sample, AveCPU is cumulative since step
                        // start and we have no valid baseline for delta computation.
                        // Record the baseline and skip.
                        if prev_ave_cpu_s.is_none() {
                            *prev_ave_cpu_s = Some(ave_cpu_s);
                            continue;
                        }

                        let cpu_percent = if elapsed_s > 0.0 {
                            ((ave_cpu_s - prev_ave_cpu_s.unwrap_or(0.0)) / elapsed_s * 100.0)
                                .max(0.0)
                        } else {
                            0.0
                        };
                        *prev_ave_cpu_s = Some(ave_cpu_s);

                        debug!(
                            "sstat sample for step {}: AveCPU={:.3}s => cpu_pct={:.1}%, \
                             MaxRSS={}B",
                            job_step, ave_cpu_s, cpu_percent, max_rss
                        );

                        (cpu_percent, max_rss, 1)
                    }
                };

                job.metrics.add_sample(cpu_percent, memory_bytes);

                // Check for OOM violation (only for local jobs with memory limits)
                if let Some(limit) = job.memory_limit_bytes
                    && !job.oom_violation_sent
                    && memory_bytes > limit
                {
                    warn!(
                        "Job {} (PID {}) exceeded memory limit: {}MB > {}MB",
                        job.job_id,
                        pid,
                        memory_bytes / (1024 * 1024),
                        limit / (1024 * 1024)
                    );
                    job.oom_violation_sent = true;
                    if let Err(e) = oom_tx.send(OomViolation {
                        pid: *pid,
                        job_id: job.job_id,
                        memory_bytes,
                        limit_bytes: limit,
                    }) {
                        error!("Failed to send OOM violation for job {}: {}", job.job_id, e);
                    }
                }

                // Store in database if using TimeSeries
                if let Some(ref mut conn) = db_conn
                    && let Err(e) = store_sample(
                        conn,
                        job.job_id,
                        timestamp,
                        cpu_percent,
                        memory_bytes,
                        num_processes,
                    )
                {
                    error!("Failed to store sample for job {}: {}", job.job_id, e);
                }

                debug!(
                    "Job {} (PID {}): CPU={:.1}%, Mem={:.1}MB, Procs={}",
                    job.job_id,
                    pid,
                    cpu_percent,
                    memory_bytes as f64 / (1024.0 * 1024.0),
                    num_processes
                );
            }

            last_sample_time = Instant::now();
        }

        // Sleep briefly to avoid busy-waiting
        thread::sleep(Duration::from_millis(100));
    }
}

/// Collect CPU and memory stats for a process and all its children
fn collect_process_tree_stats(root_pid: u32, sys: &System) -> (f64, u64, usize) {
    let mut pids_to_check = vec![Pid::from(root_pid as usize)];
    let mut visited = HashSet::new();
    let mut total_cpu = 0.0;
    let mut total_memory = 0;

    while let Some(pid) = pids_to_check.pop() {
        if visited.contains(&pid) {
            continue;
        }
        visited.insert(pid);

        if let Some(process) = sys.process(pid) {
            total_cpu += process.cpu_usage() as f64;
            total_memory += process.memory(); // sysinfo already gives bytes

            // Find all children of this process
            for (child_pid, child_proc) in sys.processes() {
                if child_proc.parent() == Some(pid) && !visited.contains(child_pid) {
                    pids_to_check.push(*child_pid);
                }
            }
        }
    }

    (total_cpu, total_memory, visited.len())
}

/// Discover the numeric step ID for a named step, retrying a few times for Slurm registration.
///
/// Called at srun launch time. Returns `None` if the step doesn't appear within ~1 second.
pub fn discover_step_id_with_retries(slurm_job_id: &str, step_name: &str) -> Option<String> {
    for attempt in 0..5 {
        let map = discover_step_ids(slurm_job_id);
        if let Some(id) = map.get(step_name) {
            return Some(id.clone());
        }
        if attempt < 4 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    debug!(
        "Could not discover numeric step ID for {} in job {} after retries",
        step_name, slurm_job_id
    );
    None
}

/// Discover numeric step IDs for all steps in a Slurm job via `squeue --steps`.
///
/// Slurm assigns numeric IDs to steps (e.g., `.0`, `.1`, `.2`). Our srun commands set
/// `--job-name=<step_name>` which appears in squeue output. We parse that to build a
/// map from step name to numeric ID, since `sstat` requires numeric step IDs on HPE Cray
/// and other Slurm installations.
///
/// Returns an empty map if squeue fails or no steps are found.
fn discover_step_ids(slurm_job_id: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    // squeue --steps is much more compact than scontrol show step, producing one
    // line per step instead of a verbose multi-line block. Critical for allocations
    // with thousands of concurrent steps.
    let output = match std::process::Command::new("squeue")
        .args(["--steps", "-j", slurm_job_id, "-o", "%i|%j", "--noheader"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            debug!(
                "squeue --steps for job {} failed to execute: {}",
                slurm_job_id, e
            );
            return map;
        }
    };

    if !output.status.success() {
        debug!(
            "squeue --steps for job {} returned non-zero: {}",
            slurm_job_id,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return map;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Each line is "<jobid>.<stepid>|<stepname>", e.g., "12893801.2|my-sleep-job"
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((step_full_id, name)) = line.split_once('|') {
            // Extract the part after the dot: "12893801.2" -> "2"
            if let Some(numeric_id) = step_full_id.split('.').nth(1) {
                map.insert(name.to_string(), numeric_id.to_string());
            }
        }
    }

    map
}

/// Raw sstat data for a single step: `(ave_cpu_seconds, max_rss_bytes)`.
type SstatRawSample = (f64, u64);

/// Poll `sstat` for multiple Slurm steps in a single subprocess invocation and return
/// the raw accounting counters keyed by `"jobid.stepid"`.
///
/// This batches all step queries into one `sstat -j <comma-separated-steps>` call,
/// dramatically reducing subprocess overhead when monitoring many concurrent jobs.
///
/// Returns an empty map when sstat is unavailable or returns no data.
fn collect_sstat_samples_batch(
    job_steps: &[String],
    sstat_binary: &str,
) -> HashMap<String, SstatRawSample> {
    let mut results = HashMap::new();
    if job_steps.is_empty() {
        return results;
    }

    let steps_arg = job_steps.join(",");
    let output = match std::process::Command::new(sstat_binary)
        .args([
            "-j",
            &steps_arg,
            "--format",
            "JobID,AveCPU,MaxRSS",
            "-P", // pipe-separated
            "-n", // no header
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            debug!("sstat batch query failed to execute: {}", e);
            return results;
        }
    };

    if !output.status.success() {
        debug!(
            "sstat batch query returned non-zero for steps [{}]: {}",
            steps_arg,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return results;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!("sstat batch output: {:?}", stdout.trim());

    // Each line is "jobid.stepid|AveCPU|MaxRSS", e.g., "12893801.1|00:01:30|1024K"
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() < 3 {
            continue;
        }

        let job_step_id = fields[0].trim();
        let ave_cpu_s = parse_slurm_cpu_time(fields[1]).unwrap_or(0.0);
        let max_rss = parse_slurm_memory(fields[2]).unwrap_or(0).max(0) as u64;

        results.insert(job_step_id.to_string(), (ave_cpu_s, max_rss));
    }

    results
}

/// Initialize the TimeSeries database
fn init_timeseries_db(output_dir: &Path, unique_label: &str) -> SqliteResult<Connection> {
    // Create resource_utilization subdirectory
    let resource_util_dir = output_dir.join("resource_utilization");
    if let Err(e) = std::fs::create_dir_all(&resource_util_dir) {
        error!("Failed to create resource_utilization directory: {}", e);
        return Err(rusqlite::Error::InvalidPath(resource_util_dir.clone()));
    }

    let db_path = timeseries_db_path(output_dir, unique_label);
    info!(
        "Initializing resource metrics database at: {}",
        db_path.display()
    );

    let conn = Connection::open(&db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS job_resource_samples (
            job_id INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            cpu_percent REAL NOT NULL,
            memory_bytes INTEGER NOT NULL,
            num_processes INTEGER NOT NULL,
            PRIMARY KEY (job_id, timestamp)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_job_resource_samples_job_id
         ON job_resource_samples(job_id)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS job_metadata (
            job_id INTEGER PRIMARY KEY,
            job_name TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS system_resource_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            cpu_percent REAL NOT NULL,
            memory_bytes INTEGER NOT NULL,
            total_memory_bytes INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS system_resource_summary (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            sample_count INTEGER NOT NULL,
            peak_cpu_percent REAL NOT NULL,
            avg_cpu_percent REAL NOT NULL,
            peak_memory_bytes INTEGER NOT NULL,
            avg_memory_bytes INTEGER NOT NULL
        )",
        [],
    )?;

    Ok(conn)
}

/// Store job metadata in the TimeSeries database
fn store_job_metadata(conn: &mut Connection, job_id: i64, job_name: &str) -> SqliteResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO job_metadata (job_id, job_name)
         VALUES (?1, ?2)",
        rusqlite::params![job_id, job_name],
    )?;
    Ok(())
}

/// Store a sample in the TimeSeries database
fn store_sample(
    conn: &mut Connection,
    job_id: i64,
    timestamp: i64,
    cpu_percent: f64,
    memory_bytes: u64,
    num_processes: usize,
) -> SqliteResult<()> {
    conn.execute(
        "INSERT INTO job_resource_samples (job_id, timestamp, cpu_percent, memory_bytes, num_processes)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![job_id, timestamp, cpu_percent, memory_bytes as i64, num_processes as i64],
    )?;
    Ok(())
}

/// Store an overall compute-node resource sample in the TimeSeries database.
fn store_system_sample(
    conn: &mut Connection,
    timestamp: i64,
    cpu_percent: f64,
    memory_bytes: u64,
    total_memory_bytes: u64,
) -> SqliteResult<()> {
    conn.execute(
        "INSERT INTO system_resource_samples
            (timestamp, cpu_percent, memory_bytes, total_memory_bytes)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            timestamp,
            cpu_percent,
            memory_bytes as i64,
            total_memory_bytes as i64
        ],
    )?;
    Ok(())
}

/// Store summary statistics for overall compute-node resource usage.
fn store_system_summary(conn: &mut Connection, summary: &SystemMetricsSummary) -> SqliteResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO system_resource_summary
            (id, sample_count, peak_cpu_percent, avg_cpu_percent,
             peak_memory_bytes, avg_memory_bytes)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            summary.sample_count,
            summary.peak_cpu_percent,
            summary.avg_cpu_percent,
            summary.peak_memory_bytes as i64,
            summary.avg_memory_bytes as i64
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scoped_monitor_config() {
        let json = r#"{
            "sample_interval_seconds": 1,
            "generate_plots": false,
            "jobs": {
                "enabled": true,
                "granularity": "summary"
            },
            "compute_node": {
                "enabled": true,
                "granularity": "time_series",
                "cpu": true,
                "memory": true
            }
        }"#;

        let config: ResourceMonitorConfig = serde_json::from_str(json).unwrap();

        let jobs = config.jobs_config();
        assert!(jobs.enabled);
        assert_eq!(jobs.granularity, MonitorGranularity::Summary);
        let compute_node = config.compute_node_config().unwrap();
        assert!(compute_node.enabled);
        assert_eq!(compute_node.granularity, MonitorGranularity::TimeSeries);
        assert!(compute_node.cpu);
        assert!(compute_node.memory);
    }

    #[test]
    fn legacy_top_level_config_controls_jobs() {
        let json = r#"{
            "enabled": true,
            "granularity": "time_series",
            "sample_interval_seconds": 1,
            "generate_plots": false
        }"#;

        let config: ResourceMonitorConfig = serde_json::from_str(json).unwrap();

        let jobs = config.jobs_config();
        assert!(jobs.enabled);
        assert_eq!(jobs.granularity, MonitorGranularity::TimeSeries);
        assert!(config.compute_node_config().is_none());
        assert!(config.is_enabled());
    }

    #[test]
    fn stores_system_samples_and_summary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut conn = init_timeseries_db(temp_dir.path(), "test").unwrap();

        store_system_sample(&mut conn, 123, 42.0, 1024, 4096).unwrap();
        store_system_sample(&mut conn, 123, 43.0, 2048, 4096).unwrap();

        let mut metrics = SystemMetrics::new();
        metrics.add_sample(10.0, 100);
        metrics.add_sample(30.0, 300);
        store_system_summary(&mut conn, &metrics.summary().unwrap()).unwrap();

        let sample_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM system_resource_samples", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(sample_count, 2);

        let (summary_count, peak_cpu, avg_memory): (i64, f64, i64) = conn
            .query_row(
                "SELECT sample_count, peak_cpu_percent, avg_memory_bytes
                 FROM system_resource_summary WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(summary_count, 2);
        assert_eq!(peak_cpu, 30.0);
        assert_eq!(avg_memory, 200);
    }
}
