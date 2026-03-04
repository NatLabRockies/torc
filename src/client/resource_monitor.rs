use crate::client::slurm_utils::{parse_slurm_cpu_time, parse_slurm_memory};
use log::{debug, error, info, warn};
use rusqlite::{Connection, Result as SqliteResult};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use sysinfo::{
    CpuRefreshKind, Pid, ProcessExt, ProcessRefreshKind, RefreshKind, System, SystemExt,
};

const DB_FILENAME_PREFIX: &str = "resource_metrics";

/// Configuration for resource monitoring
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ResourceMonitorConfig {
    pub enabled: bool,
    pub granularity: MonitorGranularity,
    pub sample_interval_seconds: i32,
    pub generate_plots: bool,
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        ResourceMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
            sample_interval_seconds: 5,
            generate_plots: false,
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
        // Sanitize: reject garbage CPU values (NaN, infinity, or unreasonably high).
        let cpu_percent =
            if cpu_percent.is_finite() && cpu_percent <= Self::MAX_PLAUSIBLE_CPU_PERCENT {
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
    Shutdown,
}

/// Active job being monitored
struct MonitoredJob {
    job_id: i64,
    /// PID used as map key.  For Slurm jobs this is the srun PID.
    #[allow(dead_code)]
    pid: u32,
    source: MonitorJobSource,
    metrics: JobMetrics,
}

/// Resource monitor manages a single background thread that monitors all running jobs
pub struct ResourceMonitor {
    tx: Sender<MonitorCommand>,
    handle: Option<JoinHandle<()>>,
    config: ResourceMonitorConfig,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(
        config: ResourceMonitorConfig,
        output_dir: PathBuf,
        unique_label: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = channel();
        let config_clone = config.clone();

        let handle = thread::spawn(move || {
            if let Err(e) = run_monitoring_loop(config_clone, output_dir, unique_label, rx) {
                error!("Resource monitoring thread failed: {}", e);
            }
        });

        Ok(ResourceMonitor {
            tx,
            handle: Some(handle),
            config,
        })
    }

    /// Returns `true` when the monitor is configured for `TimeSeries` granularity.
    pub fn is_timeseries(&self) -> bool {
        matches!(self.config.granularity, MonitorGranularity::TimeSeries)
    }

    /// Start monitoring a local process (sysinfo process-tree walk).
    pub fn start_monitoring(
        &self,
        pid: u32,
        job_id: i64,
        job_name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.tx.send(MonitorCommand::StartMonitoring {
            pid,
            job_id,
            job_name,
        })?;
        debug!("Started monitoring job {} with PID {}", job_id, pid);
        Ok(())
    }

    /// Register a Slurm step for sstat-based monitoring.
    ///
    /// Registers a Slurm step for sstat-based monitoring in both `TimeSeries` and `Summary` modes.
    ///
    /// In `TimeSeries` mode the per-sample data is written to the time-series database.
    /// In `Summary` mode only the peak metrics are kept in memory — this provides a fallback
    /// when sacct has no useful data for short or failed steps (sacct may report MaxRSS=0 /
    /// AveCPU=00:00:00 for steps that finished before the accounting daemon could flush).
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

    /// Shutdown the monitoring thread
    pub fn shutdown(self) {
        if let Err(e) = self.tx.send(MonitorCommand::Shutdown) {
            error!("Failed to send shutdown command: {}", e);
            return;
        }

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
    }
}

/// Main monitoring loop that runs in a background thread
fn run_monitoring_loop(
    config: ResourceMonitorConfig,
    output_dir: PathBuf,
    unique_label: String,
    rx: Receiver<MonitorCommand>,
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

    // Allow tests to substitute a fake sstat binary via TORC_FAKE_SSTAT.
    let sstat_binary = std::env::var("TORC_FAKE_SSTAT").unwrap_or_else(|_| "sstat".to_string());

    // Initialize database if using TimeSeries
    let mut db_conn = match config.granularity {
        MonitorGranularity::TimeSeries => Some(init_timeseries_db(&output_dir, &unique_label)?),
        MonitorGranularity::Summary => None,
    };

    info!(
        "Resource monitoring started: granularity={:?}, sample_interval={}s",
        config.granularity, config.sample_interval_seconds
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
                        },
                    );
                    debug!("Now monitoring {} jobs", monitored_jobs.len());
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
                MonitorCommand::Shutdown => {
                    info!("Resource monitor received shutdown command");
                    return Ok(());
                }
            }
        }

        // Sample all monitored jobs if interval has elapsed
        if last_sample_time.elapsed() >= sample_interval && !monitored_jobs.is_empty() {
            // Refresh sysinfo once for all local jobs.
            let has_local_jobs = monitored_jobs
                .values()
                .any(|j| matches!(j.source, MonitorJobSource::Local { .. }));
            if has_local_jobs {
                sys.refresh_processes();
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

            let timestamp = chrono::Utc::now().timestamp();

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
                        let now = Instant::now();
                        let elapsed_s = now.duration_since(*prev_sample_at).as_secs_f64();
                        match collect_sstat_sample(
                            slurm_job_id,
                            step_id,
                            &sstat_binary,
                            prev_ave_cpu_s.unwrap_or(0.0),
                            elapsed_s,
                        ) {
                            Some((cpu, mem, new_ave_cpu_s)) => {
                                *prev_sample_at = now;
                                // On the first successful sample, AveCPU is cumulative
                                // since step start and we have no valid baseline for
                                // delta computation. Record the baseline and skip.
                                if prev_ave_cpu_s.is_none() {
                                    *prev_ave_cpu_s = Some(new_ave_cpu_s);
                                    continue;
                                }
                                *prev_ave_cpu_s = Some(new_ave_cpu_s);
                                (cpu, mem, 1)
                            }
                            None => {
                                // sstat returned no data; skip this sample.
                                continue;
                            }
                        }
                    }
                };

                job.metrics.add_sample(cpu_percent, memory_bytes);

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

/// Poll `sstat` for the named Slurm step and return `(cpu_percent, max_rss_bytes, new_ave_cpu_s)`.
///
/// `sstat` reports `AveCPU` as cumulative CPU time since step start.  To convert to an
/// instantaneous utilisation rate we compute:
///
/// ```text
/// cpu_percent = (new_ave_cpu_s - prev_ave_cpu_s) / elapsed_s * 100.0
/// ```
///
/// Returns `None` when the step cannot be found (not yet started, already exited, or sstat
/// unavailable).
fn collect_sstat_sample(
    slurm_job_id: &str,
    step_id: &str,
    sstat_binary: &str,
    prev_ave_cpu_s: f64,
    elapsed_s: f64,
) -> Option<(f64, u64, f64)> {
    // Query the specific step via its numeric ID (e.g., "12893794.1").
    // Name-based lookup ("jobid.stepname") is not supported on all Slurm installations
    // (notably HPE Cray EX clusters).
    let job_step = format!("{}.{}", slurm_job_id, step_id);
    let output = std::process::Command::new(sstat_binary)
        .args([
            "-j",
            &job_step,
            "--format",
            "AveCPU,MaxRSS",
            "-P", // pipe-separated
            "-n", // no header
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        debug!(
            "sstat returned non-zero exit code for step {}: {}",
            job_step,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!("sstat output for step {}: {:?}", job_step, stdout.trim());

    // Take the first non-empty line — we queried a single step so there should be at most one.
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() < 2 {
            continue;
        }

        let new_ave_cpu_s = parse_slurm_cpu_time(fields[0]).unwrap_or(0.0);
        let max_rss = parse_slurm_memory(fields[1]).unwrap_or(0).max(0) as u64;

        let cpu_percent = if elapsed_s > 0.0 {
            ((new_ave_cpu_s - prev_ave_cpu_s) / elapsed_s * 100.0).max(0.0)
        } else {
            0.0
        };

        debug!(
            "sstat sample for step {}: AveCPU={:.3}s (delta={:.3}s over {:.3}s) \
             => cpu_pct={:.1}%, MaxRSS={}B",
            job_step,
            new_ave_cpu_s,
            new_ave_cpu_s - prev_ave_cpu_s,
            elapsed_s,
            cpu_percent,
            max_rss
        );

        return Some((cpu_percent, max_rss, new_ave_cpu_s));
    }

    // sstat ran successfully but returned no output for this step.
    // Normal during the brief window before the step appears in slurmstepd.
    debug!(
        "sstat returned no data for step {:?} (step may not be visible yet)",
        job_step,
    );

    None
}

/// Initialize the TimeSeries database
fn init_timeseries_db(output_dir: &Path, unique_label: &str) -> SqliteResult<Connection> {
    // Create resource_utilization subdirectory
    let resource_util_dir = output_dir.join("resource_utilization");
    if let Err(e) = std::fs::create_dir_all(&resource_util_dir) {
        error!("Failed to create resource_utilization directory: {}", e);
        return Err(rusqlite::Error::InvalidPath(resource_util_dir.clone()));
    }

    let db_path = resource_util_dir.join(format!("{}_{}.db", DB_FILENAME_PREFIX, unique_label));
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
