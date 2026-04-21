use anyhow::{Context, Result};
use clap::Parser;
use plotly::common::{AxisSide, Mode};
use plotly::layout::{Axis, Layout};
use plotly::{Plot, Scatter};
use rusqlite::{Connection, Result as SqliteResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tool for generating interactive HTML plots from Torc resource monitoring data
#[derive(Parser, Debug)]
#[command(about = "Generate interactive HTML plots from resource monitoring data", long_about = None)]
pub struct Args {
    /// Path to the resource metrics database file(s)
    #[arg(required = true)]
    pub db_paths: Vec<PathBuf>,

    /// Output directory for generated plots (default: current directory)
    #[arg(short, long, default_value = ".")]
    pub output_dir: PathBuf,

    /// Only plot specific job IDs (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub job_ids: Vec<i64>,

    /// Optional prefix for output filenames. When empty, files are named e.g. `job_4.html`,
    /// `summary.html`, `system_timeline.html`.
    #[arg(short = 'p', long, default_value = "")]
    pub prefix: String,

    /// Output format: html or json
    #[arg(short = 'f', long, default_value = "html")]
    pub format: String,
}

#[derive(Debug, Clone)]
struct ResourceSample {
    job_id: i64,
    timestamp: i64,
    cpu_percent: f64,
    memory_bytes: i64,
    num_processes: i64,
}

#[derive(Debug, Clone)]
struct SystemResourceSample {
    timestamp: i64,
    cpu_percent: f64,
    memory_bytes: i64,
    total_memory_bytes: i64,
}

#[derive(Debug)]
struct JobMetrics {
    job_id: i64,
    job_name: Option<String>,
    samples: Vec<ResourceSample>,
    peak_cpu: f64,
    avg_cpu: f64,
    peak_memory_gb: f64,
    avg_memory_gb: f64,
    duration_seconds: f64,
}

#[derive(Debug, Clone)]
struct SystemSummary {
    sample_count: i64,
    peak_cpu_percent: f64,
    avg_cpu_percent: f64,
    peak_memory_bytes: i64,
    avg_memory_bytes: i64,
}

#[derive(Debug)]
struct SystemMetrics {
    samples: Vec<SystemResourceSample>,
    summary: Option<SystemSummary>,
    peak_cpu: f64,
    avg_cpu: f64,
    peak_memory_gb: f64,
    avg_memory_gb: f64,
    duration_seconds: f64,
}

pub fn run(args: &Args) -> Result<()> {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&args.output_dir).context("Failed to create output directory")?;

    // Load data from all database files
    let mut all_jobs: HashMap<i64, Vec<ResourceSample>> = HashMap::new();
    let mut job_names: HashMap<i64, String> = HashMap::new();
    let mut system_samples: Vec<SystemResourceSample> = Vec::new();
    let mut system_summaries: Vec<SystemSummary> = Vec::new();

    for db_path in &args.db_paths {
        println!("Loading data from: {}", db_path.display());
        let samples = load_samples(db_path)?;
        let names = load_job_names(db_path)?;
        let loaded_system_samples = load_system_samples(db_path)?;
        let loaded_system_summary = load_system_summary(db_path)?;
        println!(
            "  Loaded {} job samples, {} job names, {} system samples",
            samples.len(),
            names.len(),
            loaded_system_samples.len()
        );

        for sample in samples {
            all_jobs.entry(sample.job_id).or_default().push(sample);
        }

        // Merge job names
        job_names.extend(names);
        system_samples.extend(loaded_system_samples);
        if let Some(summary) = loaded_system_summary {
            system_summaries.push(summary);
        }
    }

    // Filter by job IDs if specified
    let jobs_to_plot: Vec<i64> = if args.job_ids.is_empty() {
        all_jobs.keys().copied().collect()
    } else {
        args.job_ids.clone()
    };

    let system_metrics = calculate_system_metrics(system_samples, system_summaries);

    if jobs_to_plot.is_empty() && system_metrics.is_none() {
        println!("No resource data found to plot");
        return Ok(());
    }

    // Calculate metrics for each job
    let mut job_metrics: Vec<JobMetrics> = Vec::new();
    for job_id in &jobs_to_plot {
        if let Some(samples) = all_jobs.get(job_id)
            && !samples.is_empty()
        {
            let job_name = job_names.get(job_id).cloned();
            let metrics = calculate_metrics(*job_id, job_name, samples);

            let job_display = if let Some(ref name) = metrics.job_name {
                format!("Job {} ({})", metrics.job_id, name)
            } else {
                format!("Job {}", metrics.job_id)
            };

            println!(
                "{}: {} samples, {:.1}s duration, peak CPU: {:.1}%, peak mem: {:.2} GB",
                job_display,
                samples.len(),
                metrics.duration_seconds,
                metrics.peak_cpu,
                metrics.peak_memory_gb
            );
            job_metrics.push(metrics);
        }
    }

    job_metrics.sort_by_key(|m| m.job_id);

    // Determine file extension based on format
    let extension = match args.format.as_str() {
        "json" => "json",
        _ => "html",
    };

    // Generate plots
    println!("\nGenerating plots...");
    let mut total_plots = 0;
    let filename = |stem: &str| -> String {
        if args.prefix.is_empty() {
            format!("{}.{}", stem, extension)
        } else {
            format!("{}_{}.{}", args.prefix, stem, extension)
        }
    };

    // 1. Individual job plots
    for metrics in &job_metrics {
        let output_path = args
            .output_dir
            .join(filename(&format!("job_{}", metrics.job_id)));
        plot_job_timeline(metrics, &output_path, &args.format)?;
        println!("  Created: {}", output_path.display());
        total_plots += 1;
    }

    // 2. Overview plots with all jobs
    if job_metrics.len() > 1 {
        let cpu_output_path = args.output_dir.join(filename("cpu_all_jobs"));
        plot_all_jobs_cpu_overview(&job_metrics, &cpu_output_path, &args.format)?;
        println!("  Created: {}", cpu_output_path.display());
        total_plots += 1;

        let memory_output_path = args.output_dir.join(filename("memory_all_jobs"));
        plot_all_jobs_memory_overview(&job_metrics, &memory_output_path, &args.format)?;
        println!("  Created: {}", memory_output_path.display());
        total_plots += 1;
    }

    // 3. Job summary dashboard
    if !job_metrics.is_empty() {
        let output_path = args.output_dir.join(filename("summary"));
        plot_summary_dashboard(&job_metrics, &output_path, &args.format)?;
        println!("  Created: {}", output_path.display());
        total_plots += 1;
    }

    // 4. System resource plots
    if let Some(metrics) = &system_metrics {
        if !metrics.samples.is_empty() {
            let output_path = args.output_dir.join(filename("system_timeline"));
            plot_system_timeline(metrics, &output_path, &args.format)?;
            println!("  Created: {}", output_path.display());
            total_plots += 1;
        }

        if metrics.summary.is_some() {
            let output_path = args.output_dir.join(filename("system_summary"));
            plot_system_summary(metrics, &output_path, &args.format)?;
            println!("  Created: {}", output_path.display());
            total_plots += 1;
        }
    }

    println!("\nDone! Generated {} plot(s)", total_plots);

    Ok(())
}

fn load_samples(db_path: &Path) -> Result<Vec<ResourceSample>> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    let mut stmt = conn.prepare(
        "SELECT job_id, timestamp, cpu_percent, memory_bytes, num_processes
         FROM job_resource_samples
         ORDER BY job_id, timestamp",
    )?;

    let samples: SqliteResult<Vec<ResourceSample>> = stmt
        .query_map([], |row| {
            Ok(ResourceSample {
                job_id: row.get(0)?,
                timestamp: row.get(1)?,
                cpu_percent: row.get(2)?,
                memory_bytes: row.get(3)?,
                num_processes: row.get(4)?,
            })
        })?
        .collect();

    Ok(samples?)
}

fn load_job_names(db_path: &Path) -> Result<HashMap<i64, String>> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    // Check if job_metadata table exists
    let table_exists: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='job_metadata'")?
        .exists([])?;

    if !table_exists {
        return Ok(HashMap::new());
    }

    let mut stmt = conn.prepare("SELECT job_id, job_name FROM job_metadata")?;
    let names: SqliteResult<HashMap<i64, String>> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect();

    Ok(names?)
}

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool> {
    Ok(conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?1")?
        .exists([table_name])?)
}

fn load_system_samples(db_path: &Path) -> Result<Vec<SystemResourceSample>> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    if !table_exists(&conn, "system_resource_samples")? {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT timestamp, cpu_percent, memory_bytes, total_memory_bytes
         FROM system_resource_samples
         ORDER BY timestamp, rowid",
    )?;

    let samples: SqliteResult<Vec<SystemResourceSample>> = stmt
        .query_map([], |row| {
            Ok(SystemResourceSample {
                timestamp: row.get(0)?,
                cpu_percent: row.get(1)?,
                memory_bytes: row.get(2)?,
                total_memory_bytes: row.get(3)?,
            })
        })?
        .collect();

    Ok(samples?)
}

fn load_system_summary(db_path: &Path) -> Result<Option<SystemSummary>> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    if !table_exists(&conn, "system_resource_summary")? {
        return Ok(None);
    }

    let mut stmt = conn.prepare(
        "SELECT sample_count, peak_cpu_percent, avg_cpu_percent, peak_memory_bytes, avg_memory_bytes
         FROM system_resource_summary
         WHERE id = 1",
    )?;

    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    Ok(Some(SystemSummary {
        sample_count: row.get(0)?,
        peak_cpu_percent: row.get(1)?,
        avg_cpu_percent: row.get(2)?,
        peak_memory_bytes: row.get(3)?,
        avg_memory_bytes: row.get(4)?,
    }))
}

fn calculate_metrics(
    job_id: i64,
    job_name: Option<String>,
    samples: &[ResourceSample],
) -> JobMetrics {
    let peak_cpu = samples.iter().map(|s| s.cpu_percent).fold(0.0, f64::max);
    let avg_cpu = samples.iter().map(|s| s.cpu_percent).sum::<f64>() / samples.len() as f64;

    let peak_memory_bytes = samples.iter().map(|s| s.memory_bytes).max().unwrap_or(0);
    let avg_memory_bytes =
        samples.iter().map(|s| s.memory_bytes).sum::<i64>() / samples.len() as i64;

    let peak_memory_gb = peak_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let avg_memory_gb = avg_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    let start_time = samples.first().unwrap().timestamp;
    let end_time = samples.last().unwrap().timestamp;
    let duration_seconds = (end_time - start_time) as f64;

    JobMetrics {
        job_id,
        job_name,
        samples: samples.to_vec(),
        peak_cpu,
        avg_cpu,
        peak_memory_gb,
        avg_memory_gb,
        duration_seconds,
    }
}

fn calculate_system_metrics(
    mut samples: Vec<SystemResourceSample>,
    summaries: Vec<SystemSummary>,
) -> Option<SystemMetrics> {
    samples.sort_by_key(|s| s.timestamp);

    let summary = merge_system_summaries(summaries);

    let (peak_cpu, avg_cpu, peak_memory_gb, avg_memory_gb, duration_seconds) =
        if !samples.is_empty() {
            let peak_cpu = samples.iter().map(|s| s.cpu_percent).fold(0.0, f64::max);
            let avg_cpu = samples.iter().map(|s| s.cpu_percent).sum::<f64>() / samples.len() as f64;

            let peak_memory_bytes = samples.iter().map(|s| s.memory_bytes).max().unwrap_or(0);
            let avg_memory_bytes =
                samples.iter().map(|s| s.memory_bytes).sum::<i64>() / samples.len() as i64;

            let start_time = samples.first().unwrap().timestamp;
            let end_time = samples.last().unwrap().timestamp;

            (
                peak_cpu,
                avg_cpu,
                peak_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                avg_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                (end_time - start_time) as f64,
            )
        } else if let Some(summary) = &summary {
            (
                summary.peak_cpu_percent,
                summary.avg_cpu_percent,
                summary.peak_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                summary.avg_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                0.0,
            )
        } else {
            return None;
        };

    Some(SystemMetrics {
        samples,
        summary,
        peak_cpu,
        avg_cpu,
        peak_memory_gb,
        avg_memory_gb,
        duration_seconds,
    })
}

fn merge_system_summaries(summaries: Vec<SystemSummary>) -> Option<SystemSummary> {
    if summaries.is_empty() {
        return None;
    }

    let sample_count: i64 = summaries.iter().map(|s| s.sample_count).sum();
    let peak_cpu_percent = summaries
        .iter()
        .map(|s| s.peak_cpu_percent)
        .fold(0.0, f64::max);
    let peak_memory_bytes = summaries
        .iter()
        .map(|s| s.peak_memory_bytes)
        .max()
        .unwrap_or(0);

    let avg_cpu_percent = weighted_avg_f64(
        summaries
            .iter()
            .map(|s| (s.avg_cpu_percent, s.sample_count)),
    );
    let avg_memory_bytes = weighted_avg_i64(
        summaries
            .iter()
            .map(|s| (s.avg_memory_bytes, s.sample_count)),
    );

    Some(SystemSummary {
        sample_count,
        peak_cpu_percent,
        avg_cpu_percent,
        peak_memory_bytes,
        avg_memory_bytes,
    })
}

fn weighted_avg_f64(values: impl Iterator<Item = (f64, i64)>) -> f64 {
    let mut weighted_sum = 0.0;
    let mut total_weight = 0;
    for (value, weight) in values {
        weighted_sum += value * weight as f64;
        total_weight += weight;
    }
    if total_weight == 0 {
        0.0
    } else {
        weighted_sum / total_weight as f64
    }
}

fn weighted_avg_i64(values: impl Iterator<Item = (i64, i64)>) -> i64 {
    let mut weighted_sum = 0;
    let mut total_weight = 0;
    for (value, weight) in values {
        weighted_sum += value * weight;
        total_weight += weight;
    }
    if total_weight == 0 {
        0
    } else {
        weighted_sum / total_weight
    }
}

fn write_plot(plot: &Plot, output_path: &Path, format: &str) -> Result<()> {
    match format {
        "json" => {
            let json_str = plot.to_json();
            std::fs::write(output_path, json_str)
                .with_context(|| format!("Failed to write JSON to {}", output_path.display()))?;
        }
        _ => {
            plot.write_html(output_path);
        }
    }
    Ok(())
}

fn plot_job_timeline(metrics: &JobMetrics, output_path: &Path, format: &str) -> Result<()> {
    let mut plot = Plot::new();

    // Convert timestamps to relative seconds
    let start_time = metrics.samples.first().unwrap().timestamp;
    let times: Vec<f64> = metrics
        .samples
        .iter()
        .map(|s| (s.timestamp - start_time) as f64)
        .collect();

    let cpu_values: Vec<f64> = metrics.samples.iter().map(|s| s.cpu_percent).collect();
    let memory_values: Vec<f64> = metrics
        .samples
        .iter()
        .map(|s| s.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        .collect();
    let process_counts: Vec<i64> = metrics.samples.iter().map(|s| s.num_processes).collect();

    // CPU trace
    let cpu_trace = Scatter::new(times.clone(), cpu_values)
        .name("CPU %")
        .mode(Mode::Lines)
        .y_axis("y1");

    // Memory trace
    let memory_trace = Scatter::new(times.clone(), memory_values)
        .name("Memory (GB)")
        .mode(Mode::Lines)
        .y_axis("y2");

    // Process count trace
    let process_trace = Scatter::new(times, process_counts)
        .name("# Processes")
        .mode(Mode::Lines)
        .y_axis("y3");

    plot.add_trace(cpu_trace);
    plot.add_trace(memory_trace);
    plot.add_trace(process_trace);

    let job_display = if let Some(ref name) = metrics.job_name {
        format!("Job {} ({})", metrics.job_id, name)
    } else {
        format!("Job {}", metrics.job_id)
    };

    let title = format!(
        "{} Resource Usage Timeline<br><sub>Peak: {:.1}% CPU, {:.2} GB Memory | Avg: {:.1}% CPU, {:.2} GB Memory</sub>",
        job_display,
        metrics.peak_cpu,
        metrics.peak_memory_gb,
        metrics.avg_cpu,
        metrics.avg_memory_gb
    );

    let layout = Layout::new()
        .title(&title)
        .x_axis(Axis::new().title("Time (seconds)"))
        .y_axis(Axis::new().title("CPU %"))
        .y_axis2(
            Axis::new()
                .title("Memory (GB)")
                .overlaying("y")
                .side(AxisSide::Right),
        )
        .y_axis3(
            Axis::new()
                .title("Processes")
                .overlaying("y")
                .side(AxisSide::Right)
                .anchor("free")
                .position(0.95),
        );

    plot.set_layout(layout);
    write_plot(&plot, output_path, format)?;

    Ok(())
}

fn plot_all_jobs_cpu_overview(
    metrics: &[JobMetrics],
    output_path: &Path,
    format: &str,
) -> Result<()> {
    let mut plot = Plot::new();

    for job_metrics in metrics {
        let start_time = job_metrics.samples.first().unwrap().timestamp;
        let times: Vec<f64> = job_metrics
            .samples
            .iter()
            .map(|s| (s.timestamp - start_time) as f64)
            .collect();

        let cpu_values: Vec<f64> = job_metrics.samples.iter().map(|s| s.cpu_percent).collect();

        let trace_name = if let Some(ref name) = job_metrics.job_name {
            format!("Job {} ({})", job_metrics.job_id, name)
        } else {
            format!("Job {}", job_metrics.job_id)
        };

        let trace = Scatter::new(times, cpu_values)
            .name(&trace_name)
            .mode(Mode::Lines);

        plot.add_trace(trace);
    }

    let layout = Layout::new()
        .title("CPU Usage - All Jobs")
        .x_axis(Axis::new().title("Time (seconds)"))
        .y_axis(Axis::new().title("CPU %"));

    plot.set_layout(layout);
    write_plot(&plot, output_path, format)?;

    Ok(())
}

fn plot_all_jobs_memory_overview(
    metrics: &[JobMetrics],
    output_path: &Path,
    format: &str,
) -> Result<()> {
    let mut plot = Plot::new();

    for job_metrics in metrics {
        let start_time = job_metrics.samples.first().unwrap().timestamp;
        let times: Vec<f64> = job_metrics
            .samples
            .iter()
            .map(|s| (s.timestamp - start_time) as f64)
            .collect();

        let memory_values: Vec<f64> = job_metrics
            .samples
            .iter()
            .map(|s| s.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
            .collect();

        let trace_name = if let Some(ref name) = job_metrics.job_name {
            format!("Job {} ({})", job_metrics.job_id, name)
        } else {
            format!("Job {}", job_metrics.job_id)
        };

        let trace = Scatter::new(times, memory_values)
            .name(&trace_name)
            .mode(Mode::Lines);

        plot.add_trace(trace);
    }

    let layout = Layout::new()
        .title("Memory Usage - All Jobs")
        .x_axis(Axis::new().title("Time (seconds)"))
        .y_axis(Axis::new().title("Memory (GB)"));

    plot.set_layout(layout);
    write_plot(&plot, output_path, format)?;

    Ok(())
}

fn plot_summary_dashboard(metrics: &[JobMetrics], output_path: &Path, format: &str) -> Result<()> {
    use plotly::Bar;

    let mut plot = Plot::new();

    let job_ids: Vec<String> = metrics.iter().map(|m| m.job_id.to_string()).collect();
    let peak_cpus: Vec<f64> = metrics.iter().map(|m| m.peak_cpu).collect();
    let avg_cpus: Vec<f64> = metrics.iter().map(|m| m.avg_cpu).collect();
    let peak_mems: Vec<f64> = metrics.iter().map(|m| m.peak_memory_gb).collect();
    let avg_mems: Vec<f64> = metrics.iter().map(|m| m.avg_memory_gb).collect();

    // CPU bar chart
    let peak_cpu_trace = Bar::new(job_ids.clone(), peak_cpus)
        .name("Peak CPU %")
        .y_axis("y1");
    let avg_cpu_trace = Bar::new(job_ids.clone(), avg_cpus)
        .name("Avg CPU %")
        .y_axis("y1");

    // Memory bar chart
    let peak_mem_trace = Bar::new(job_ids.clone(), peak_mems)
        .name("Peak Memory (GB)")
        .x_axis("x2")
        .y_axis("y2");
    let avg_mem_trace = Bar::new(job_ids, avg_mems)
        .name("Avg Memory (GB)")
        .x_axis("x2")
        .y_axis("y2");

    plot.add_trace(peak_cpu_trace);
    plot.add_trace(avg_cpu_trace);
    plot.add_trace(peak_mem_trace);
    plot.add_trace(avg_mem_trace);

    let layout = Layout::new()
        .title("Resource Usage Summary - All Jobs")
        .x_axis(Axis::new().title("Job ID").domain(&[0.0, 0.45]))
        .y_axis(Axis::new().title("CPU %"))
        .x_axis2(Axis::new().title("Job ID").domain(&[0.55, 1.0]))
        .y_axis2(
            Axis::new()
                .title("Memory (GB)")
                .anchor("x2")
                .side(AxisSide::Left),
        )
        .bar_mode(plotly::layout::BarMode::Group);

    plot.set_layout(layout);
    write_plot(&plot, output_path, format)?;

    Ok(())
}

fn plot_system_timeline(metrics: &SystemMetrics, output_path: &Path, format: &str) -> Result<()> {
    let mut plot = Plot::new();

    let start_time = metrics.samples.first().unwrap().timestamp;
    let times: Vec<f64> = metrics
        .samples
        .iter()
        .map(|s| (s.timestamp - start_time) as f64)
        .collect();
    let cpu_values: Vec<f64> = metrics.samples.iter().map(|s| s.cpu_percent).collect();
    let memory_values: Vec<f64> = metrics
        .samples
        .iter()
        .map(|s| s.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        .collect();
    let total_memory_values: Vec<f64> = metrics
        .samples
        .iter()
        .map(|s| s.total_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        .collect();

    let cpu_trace = Scatter::new(times.clone(), cpu_values)
        .name("System CPU %")
        .mode(Mode::Lines)
        .y_axis("y1");
    let memory_trace = Scatter::new(times.clone(), memory_values)
        .name("Used Memory (GB)")
        .mode(Mode::Lines)
        .y_axis("y2");
    let total_memory_trace = Scatter::new(times, total_memory_values)
        .name("Total Memory (GB)")
        .mode(Mode::Lines)
        .y_axis("y2");

    plot.add_trace(cpu_trace);
    plot.add_trace(memory_trace);
    plot.add_trace(total_memory_trace);

    let title = format!(
        "System Resource Usage Timeline<br><sub>Peak: {:.1}% CPU, {:.2} GB Memory | Avg: {:.1}% CPU, {:.2} GB Memory | Duration: {:.1}s</sub>",
        metrics.peak_cpu,
        metrics.peak_memory_gb,
        metrics.avg_cpu,
        metrics.avg_memory_gb,
        metrics.duration_seconds
    );

    let layout = Layout::new()
        .title(&title)
        .x_axis(Axis::new().title("Time (seconds)"))
        .y_axis(Axis::new().title("CPU %"))
        .y_axis2(
            Axis::new()
                .title("Memory (GB)")
                .overlaying("y")
                .side(AxisSide::Right),
        );

    plot.set_layout(layout);
    write_plot(&plot, output_path, format)?;

    Ok(())
}

fn plot_system_summary(metrics: &SystemMetrics, output_path: &Path, format: &str) -> Result<()> {
    use plotly::Bar;

    let mut plot = Plot::new();

    let labels = vec!["System".to_string()];
    let peak_cpu_trace = Bar::new(labels.clone(), vec![metrics.peak_cpu])
        .name("Peak CPU %")
        .y_axis("y1");
    let avg_cpu_trace = Bar::new(labels.clone(), vec![metrics.avg_cpu])
        .name("Avg CPU %")
        .y_axis("y1");
    let peak_mem_trace = Bar::new(labels.clone(), vec![metrics.peak_memory_gb])
        .name("Peak Memory (GB)")
        .x_axis("x2")
        .y_axis("y2");
    let avg_mem_trace = Bar::new(labels, vec![metrics.avg_memory_gb])
        .name("Avg Memory (GB)")
        .x_axis("x2")
        .y_axis("y2");

    plot.add_trace(peak_cpu_trace);
    plot.add_trace(avg_cpu_trace);
    plot.add_trace(peak_mem_trace);
    plot.add_trace(avg_mem_trace);

    let sample_count = metrics
        .summary
        .as_ref()
        .map(|s| s.sample_count)
        .unwrap_or(metrics.samples.len() as i64);
    let title = format!(
        "System Resource Usage Summary<br><sub>{} samples</sub>",
        sample_count
    );

    let layout = Layout::new()
        .title(&title)
        .x_axis(Axis::new().title("Scope").domain(&[0.0, 0.45]))
        .y_axis(Axis::new().title("CPU %"))
        .x_axis2(Axis::new().title("Scope").domain(&[0.55, 1.0]))
        .y_axis2(
            Axis::new()
                .title("Memory (GB)")
                .anchor("x2")
                .side(AxisSide::Left),
        )
        .bar_mode(plotly::layout::BarMode::Group);

    plot.set_layout(layout);
    write_plot(&plot, output_path, format)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_system_plots_without_job_samples() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("resource_metrics_test.db");
        let output_dir = temp_dir.path().join("plots");
        let conn = Connection::open(&db_path).unwrap();

        conn.execute(
            "CREATE TABLE job_resource_samples (
                job_id INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                cpu_percent REAL NOT NULL,
                memory_bytes INTEGER NOT NULL,
                num_processes INTEGER NOT NULL,
                PRIMARY KEY (job_id, timestamp)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE job_metadata (
                job_id INTEGER PRIMARY KEY,
                job_name TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE system_resource_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                cpu_percent REAL NOT NULL,
                memory_bytes INTEGER NOT NULL,
                total_memory_bytes INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE system_resource_summary (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                sample_count INTEGER NOT NULL,
                peak_cpu_percent REAL NOT NULL,
                avg_cpu_percent REAL NOT NULL,
                peak_memory_bytes INTEGER NOT NULL,
                avg_memory_bytes INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO system_resource_samples
                (timestamp, cpu_percent, memory_bytes, total_memory_bytes)
             VALUES
                (100, 10.0, 1024, 4096),
                (101, 20.0, 2048, 4096)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO system_resource_summary
                (id, sample_count, peak_cpu_percent, avg_cpu_percent,
                 peak_memory_bytes, avg_memory_bytes)
             VALUES (1, 2, 20.0, 15.0, 2048, 1536)",
            [],
        )
        .unwrap();

        let args = Args {
            db_paths: vec![db_path],
            output_dir: output_dir.clone(),
            job_ids: Vec::new(),
            prefix: "resource_plot".to_string(),
            format: "json".to_string(),
        };

        run(&args).unwrap();

        assert!(
            output_dir
                .join("resource_plot_system_timeline.json")
                .exists()
        );
        assert!(
            output_dir
                .join("resource_plot_system_summary.json")
                .exists()
        );
        assert!(!output_dir.join("resource_plot_summary.json").exists());

        let summary_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(output_dir.join("resource_plot_system_summary.json")).unwrap(),
        )
        .unwrap();
        assert_bar_summary_uses_split_axes(&summary_json);
    }

    #[test]
    fn job_summary_uses_split_axes_for_cpu_and_memory_bars() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("job_summary.json");
        let metrics = vec![JobMetrics {
            job_id: 1,
            job_name: Some("job".to_string()),
            samples: Vec::new(),
            peak_cpu: 80.0,
            avg_cpu: 40.0,
            peak_memory_gb: 2.0,
            avg_memory_gb: 1.0,
            duration_seconds: 0.0,
        }];

        plot_summary_dashboard(&metrics, &output_path, "json").unwrap();

        let summary_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(output_path).unwrap()).unwrap();
        assert_bar_summary_uses_split_axes(&summary_json);
    }

    fn assert_bar_summary_uses_split_axes(plot_json: &serde_json::Value) {
        let traces = plot_json["data"].as_array().unwrap();
        assert!(traces.len() >= 4);

        for trace in traces {
            let name = trace["name"].as_str().unwrap();
            if name.contains("Memory") {
                assert_eq!(trace["xaxis"], "x2");
                assert_eq!(trace["yaxis"], "y2");
            } else {
                assert!(trace.get("xaxis").is_none());
                assert_eq!(trace["yaxis"], "y1");
            }
        }

        let layout = &plot_json["layout"];
        assert_eq!(layout["xaxis"]["domain"][0], 0.0);
        assert_eq!(layout["xaxis"]["domain"][1], 0.45);
        assert_eq!(layout["xaxis2"]["domain"][0], 0.55);
        assert_eq!(layout["xaxis2"]["domain"][1], 1.0);
        assert_eq!(layout["yaxis2"]["anchor"], "x2");
        assert!(layout["yaxis2"].get("overlaying").is_none());
    }
}
