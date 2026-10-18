//! `torc monitor-node`: run only the compute-node resource monitor on this host.
//!
//! Used on nodes of a multi-node Slurm allocation that have no job runner of their
//! own (`start_one_worker_per_node=false`), so their CPU and memory utilization is
//! still recorded. Writes the same SQLite layout as the job runner's monitor, under a
//! distinct label, and exits on SIGTERM/SIGINT after a final flush.

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::resource_monitor::{ResourceMonitor, write_system_summary};
use crate::models::JobMonitorConfig;
use clap::Parser;
use log::{error, info};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Monitor compute-node CPU/memory on this host (no job execution)")]
pub struct Args {
    /// Workflow ID
    pub workflow_id: i64,
    /// Output directory (same as the job runner's)
    #[arg(short, long, default_value = "torc_output")]
    pub output_dir: PathBuf,
}

/// Returns the process exit code.
pub fn run(config: &Configuration, args: &Args) -> i32 {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let workflow = match apis::workflows_api::get_workflow(config, args.workflow_id) {
        Ok(w) => w,
        Err(e) => {
            error!("Error getting workflow: {}", e);
            return 1;
        }
    };

    let mut monitor_config = match workflow.resource_monitor_config {
        Some(c) if c.compute_node_config().is_some() => c,
        _ => {
            error!(
                "workflow_id={} has no enabled resource_monitor.compute_node config; nothing to do",
                args.workflow_id
            );
            return 1;
        }
    };
    // Node-only: never track job processes here.
    monitor_config.enabled = false;
    monitor_config.jobs = Some(JobMonitorConfig::default());
    monitor_config.generate_plots = false;

    // The Slurm job id keeps a later allocation on the same host from overwriting
    // this one's summary row.
    let mut label = format!(
        "monitor_wf{}_h{}_r{}",
        args.workflow_id,
        hostname,
        workflow.run_id.unwrap_or(0)
    );
    if let Ok(slurm_job_id) = std::env::var("SLURM_JOB_ID") {
        label.push_str(&format!("_sl{}", slurm_job_id));
    }
    let has_timeseries_db = monitor_config.has_timeseries_db();
    let monitor = match ResourceMonitor::new(monitor_config, args.output_dir.clone(), label.clone())
    {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to start resource monitor: {}", e);
            return 1;
        }
    };
    info!(
        "Monitoring compute node {} for workflow_id={}",
        hostname, args.workflow_id
    );

    wait_for_termination();

    info!("Stopping compute-node monitor on {}", hostname);
    let summary = monitor.shutdown();
    // In time-series mode the monitor thread already stored the summary alongside
    // the samples; in summary mode nothing else persists it.
    if !has_timeseries_db
        && let Some(summary) = summary
        && let Err(e) = write_system_summary(&args.output_dir, &label, &summary)
    {
        error!("Failed to store compute-node summary: {}", e);
        return 1;
    }
    0
}

#[cfg(unix)]
fn wait_for_termination() {
    use signal_hook::consts::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    match Signals::new([SIGTERM, SIGINT]) {
        Ok(mut signals) => {
            signals.forever().next();
        }
        Err(e) => error!("Failed to register signal handlers: {}", e),
    }
}

#[cfg(not(unix))]
fn wait_for_termination() {
    let _ = std::io::stdin().read_line(&mut String::new());
}
