//! Log collection and analysis commands
//!
//! Provides tools for bundling workflow logs and analyzing them for errors.

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::{get_env_user_name, print_error, select_workflow_interactively};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::{Archive, Builder};

lazy_static! {
    static ref INFO_REGEX: Regex = Regex::new(r"(?i)\bINFO\b").unwrap();
}

/// Log subcommands
#[derive(clap::Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # Bundle workflow logs
    torc logs bundle 123 --output-dir ./torc_output

    # Analyze logs for errors
    torc logs analyze wf123.tar.gz
    torc logs analyze ./torc_output --workflow-id 123
")]
pub enum LogCommands {
    /// Bundle all log files for a workflow into a compressed tarball
    #[command(after_long_help = "\
EXAMPLES:
    torc logs bundle 123
    torc logs bundle 123 --output-dir ./torc_output --bundle-dir ./bundles
")]
    Bundle {
        /// Workflow ID to bundle logs for
        #[arg()]
        workflow_id: Option<i64>,
        /// Output directory where logs are stored (the same directory passed to `torc run`)
        #[arg(short, long, default_value = "torc_output")]
        output_dir: PathBuf,
        /// Directory to write the bundle to
        #[arg(long, default_value = ".")]
        bundle_dir: PathBuf,
    },
    /// Analyze logs for errors (from a bundle tarball or log directory)
    #[command(after_long_help = "\
EXAMPLES:
    torc logs analyze wf123.tar.gz
    torc logs analyze ./torc_output --workflow-id 123
")]
    Analyze {
        /// Path to a bundle tarball (.tar.gz) or log directory
        #[arg()]
        path: PathBuf,
        /// Workflow ID to filter logs (required when analyzing a directory with multiple workflows)
        #[arg(short, long)]
        workflow_id: Option<i64>,
    },
}

/// Handle log commands
pub fn handle_log_commands(config: &Configuration, command: &LogCommands) {
    match command {
        LogCommands::Bundle {
            workflow_id,
            output_dir,
            bundle_dir,
        } => {
            collect_bundle(config, *workflow_id, output_dir, bundle_dir);
        }
        LogCommands::Analyze { path, workflow_id } => {
            analyze_path(path, *workflow_id);
        }
    }
}

/// Collect all log files for a workflow into a support bundle
fn collect_bundle(
    config: &Configuration,
    workflow_id: Option<i64>,
    output_dir: &Path,
    bundle_dir: &Path,
) {
    // Get or select workflow ID
    let user = get_env_user_name();
    let wf_id = match workflow_id {
        Some(id) => id,
        None => match select_workflow_interactively(config, &user) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Error selecting workflow: {}", e);
                std::process::exit(1);
            }
        },
    };

    // Create the bundle filename
    let bundle_filename = format!("wf{}.tar.gz", wf_id);
    let bundle_path = bundle_dir.join(&bundle_filename);

    // Get workflow info for metadata
    let workflow = match apis::workflows_api::get_workflow(config, wf_id) {
        Ok(w) => w,
        Err(e) => {
            print_error("getting workflow", &e);
            std::process::exit(1);
        }
    };

    println!("Collecting logs for workflow {} ({})", wf_id, workflow.name);
    println!("Output directory: {}", output_dir.display());
    println!("Bundle path: {}", bundle_path.display());

    // Create the tarball
    let tar_file = match File::create(&bundle_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error creating bundle file: {}", e);
            std::process::exit(1);
        }
    };
    let encoder = GzEncoder::new(tar_file, Compression::default());
    let mut tar_builder = Builder::new(encoder);

    // Collect files matching workflow patterns
    let wf_pattern = format!("wf{}", wf_id);
    let mut files_collected = 0;
    let mut total_size: u64 = 0;

    // Scan output directory for matching files
    if output_dir.exists() {
        // Collect workflow-specific files (job logs, runner logs, etc.)
        files_collected +=
            collect_matching_files(&mut tar_builder, output_dir, &wf_pattern, &mut total_size);

        // Collect Slurm output files (slurm_output_wf*_sl*.o and slurm_output_wf*_sl*.e)
        // These contain important scheduler-level error information
        files_collected +=
            collect_slurm_files(&mut tar_builder, output_dir, &wf_pattern, &mut total_size);

        // Also check job_stdio subdirectory
        let job_stdio_dir = output_dir.join("job_stdio");
        if job_stdio_dir.exists() {
            files_collected += collect_matching_files(
                &mut tar_builder,
                &job_stdio_dir,
                &wf_pattern,
                &mut total_size,
            );
        }
    } else {
        eprintln!(
            "Warning: Output directory does not exist: {}",
            output_dir.display()
        );
    }

    // Write workflow metadata as a JSON file in the bundle
    let metadata = serde_json::json!({
        "workflow_id": wf_id,
        "workflow_name": workflow.name,
        "workflow_description": workflow.description,
        "workflow_user": workflow.user,
        "collected_at": chrono::Utc::now().to_rfc3339(),
        "files_collected": files_collected,
        "total_size_bytes": total_size,
    });
    let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();

    // Add metadata to the tarball
    let mut header = tar::Header::new_gnu();
    header.set_size(metadata_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar_builder
        .append_data(
            &mut header,
            "bundle_metadata.json",
            metadata_json.as_bytes(),
        )
        .unwrap();

    // Finalize the tarball
    match tar_builder.into_inner() {
        Ok(encoder) => {
            if let Err(e) = encoder.finish() {
                eprintln!("Error finishing compression: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error finalizing tarball: {}", e);
            std::process::exit(1);
        }
    }

    println!();
    println!("Log bundle created successfully:");
    println!("  File: {}", bundle_path.display());
    println!("  Files collected: {}", files_collected);
    println!("  Total size: {} bytes", total_size);
    println!();
    println!("To analyze the bundle, run:");
    println!("  torc logs analyze {}", bundle_path.display());
}

/// Collect files matching the workflow pattern from a directory
fn collect_matching_files<W: std::io::Write>(
    tar_builder: &mut Builder<W>,
    dir: &Path,
    wf_pattern: &str,
    total_size: &mut u64,
) -> usize {
    let mut count = 0;

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Warning: Cannot read directory {}: {}", dir.display(), e);
            return 0;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            if filename.contains(wf_pattern) {
                match File::open(&path) {
                    Ok(mut file) => {
                        let metadata = file.metadata().unwrap();
                        *total_size += metadata.len();

                        // Use relative path in the archive
                        let archive_name = if let Some(parent) = path.parent() {
                            if let Some(parent_name) = parent.file_name() {
                                format!("{}/{}", parent_name.to_string_lossy(), filename)
                            } else {
                                filename.to_string()
                            }
                        } else {
                            filename.to_string()
                        };

                        if tar_builder.append_file(&archive_name, &mut file).is_ok() {
                            println!("  Added: {}", archive_name);
                            count += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Cannot read file {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    count
}

/// Collect Slurm output files from a directory
fn collect_slurm_files<W: std::io::Write>(
    tar_builder: &mut Builder<W>,
    dir: &Path,
    wf_pattern: &str,
    total_size: &mut u64,
) -> usize {
    let mut count = 0;

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Warning: Cannot read directory {}: {}", dir.display(), e);
            return 0;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            // Match Slurm files for this workflow: slurm_output_wf*_sl*.o/e,
            // slurm_env_wf*_sl*.log, dmesg_slurm_wf*_sl*.log, job_runner_slurm_wf*_sl*.log
            let is_slurm_file = filename.starts_with("slurm_output_")
                || filename.starts_with("slurm_env_")
                || filename.starts_with("dmesg_slurm_")
                || filename.starts_with("job_runner_slurm_");
            if is_slurm_file && filename.contains(wf_pattern) {
                match File::open(&path) {
                    Ok(mut file) => {
                        let metadata = file.metadata().unwrap();
                        *total_size += metadata.len();

                        // Use relative path in the archive
                        let archive_name = if let Some(parent) = path.parent() {
                            if let Some(parent_name) = parent.file_name() {
                                format!("{}/{}", parent_name.to_string_lossy(), filename)
                            } else {
                                filename.to_string()
                            }
                        } else {
                            filename.to_string()
                        };

                        if tar_builder.append_file(&archive_name, &mut file).is_ok() {
                            println!("  Added: {}", archive_name);
                            count += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Cannot read file {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    count
}

/// Error patterns to search for in log files
struct ErrorPattern {
    name: &'static str,
    pattern: Regex,
    severity: ErrorSeverity,
}

/// Severity level for detected errors
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Warning => write!(f, "WARN"),
            ErrorSeverity::Info => write!(f, "INFO"),
        }
    }
}

/// A detected error in a log file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedError {
    pub file: String,
    pub line_number: usize,
    pub pattern_name: String,
    pub severity: ErrorSeverity,
    pub line_content: String,
}

/// Result of analyzing workflow logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnalysisResult {
    /// Workflow ID that was analyzed
    pub workflow_id: Option<i64>,
    /// Number of log files parsed
    pub files_parsed: usize,
    /// Total number of errors detected
    pub error_count: usize,
    /// Total number of warnings detected
    pub warning_count: usize,
    /// All detected errors
    pub errors: Vec<DetectedError>,
    /// Errors grouped by file
    pub errors_by_file: HashMap<String, Vec<DetectedError>>,
    /// Error counts by pattern type
    pub errors_by_type: HashMap<String, usize>,
}

impl LogAnalysisResult {
    /// Build the result from a list of detected errors
    fn from_errors(
        errors: Vec<DetectedError>,
        files_parsed: usize,
        workflow_id: Option<i64>,
    ) -> Self {
        let error_count = errors
            .iter()
            .filter(|e| e.severity == ErrorSeverity::Error)
            .count();
        let warning_count = errors
            .iter()
            .filter(|e| e.severity == ErrorSeverity::Warning)
            .count();

        // Group errors by file
        let mut errors_by_file: HashMap<String, Vec<DetectedError>> = HashMap::new();
        for error in &errors {
            errors_by_file
                .entry(error.file.clone())
                .or_default()
                .push(error.clone());
        }

        // Count errors by type
        let mut errors_by_type: HashMap<String, usize> = HashMap::new();
        for error in &errors {
            if error.severity == ErrorSeverity::Error {
                *errors_by_type
                    .entry(error.pattern_name.clone())
                    .or_default() += 1;
            }
        }

        Self {
            workflow_id,
            files_parsed,
            error_count,
            warning_count,
            errors,
            errors_by_file,
            errors_by_type,
        }
    }
}

/// Analyze logs for a workflow from an output directory.
///
/// This is the main public API for log analysis, suitable for use by the MCP server.
///
/// # Arguments
/// * `output_dir` - The output directory where logs are stored (same as passed to `torc run`)
/// * `workflow_id` - The workflow ID to analyze logs for
///
/// # Returns
/// A `LogAnalysisResult` containing all detected errors and summary statistics.
pub fn analyze_workflow_logs(
    output_dir: &Path,
    workflow_id: i64,
) -> Result<LogAnalysisResult, String> {
    if !output_dir.exists() {
        return Err(format!(
            "Output directory does not exist: {}",
            output_dir.display()
        ));
    }

    let wf_pattern = format!("wf{}", workflow_id);
    let patterns = get_error_patterns();
    let mut errors: Vec<DetectedError> = Vec::new();
    let mut files_parsed = 0;

    // Scan main directory
    files_parsed += scan_directory_for_logs(output_dir, &wf_pattern, &patterns, &mut errors);

    // Scan job_stdio subdirectory
    let job_stdio_dir = output_dir.join("job_stdio");
    if job_stdio_dir.exists() {
        files_parsed +=
            scan_directory_for_logs(&job_stdio_dir, &wf_pattern, &patterns, &mut errors);
    }

    Ok(LogAnalysisResult::from_errors(
        errors,
        files_parsed,
        Some(workflow_id),
    ))
}

/// Get the error patterns to search for in log files
fn get_error_patterns() -> Vec<ErrorPattern> {
    vec![
        // Torc-specific patterns (more specific, checked first)
        ErrorPattern {
            name: "Missing Output Files",
            pattern: Regex::new(
                r"(?i)(expected output files are missing|Output file validation failed)",
            )
            .unwrap(),
            severity: ErrorSeverity::Error,
        },
        // General system errors
        ErrorPattern {
            name: "Slurm Error",
            pattern: Regex::new(r"(?i)\b(slurmstepd|CANCELLED|TIMEOUT|OUT_OF_MEMORY)\b").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "OOM Killed",
            pattern: Regex::new(r"(?i)\b(out of memory|oom|killed|cannot allocate memory)\b")
                .unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Timeout",
            pattern: Regex::new(r"(?i)\b(timeout|time limit|timed out|walltime)\b").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Segmentation Fault",
            pattern: Regex::new(r"(?i)\b(segmentation fault|segfault|sigsegv)\b").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Permission Denied",
            pattern: Regex::new(r"(?i)\b(permission denied|access denied|EACCES)\b").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "File Not Found",
            pattern: Regex::new(r"(?i)\b(no such file|file not found|ENOENT)\b").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Disk Full",
            pattern: Regex::new(r"(?i)\b(no space left|disk full|quota exceeded|ENOSPC)\b")
                .unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Connection Error",
            pattern: Regex::new(
                r"(?i)\b(connection refused|connection reset|network unreachable|ECONNREFUSED)\b",
            )
            .unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Rust Panic",
            pattern: Regex::new(r"thread .* panicked at").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Python Exception",
            pattern: Regex::new(r"(Traceback \(most recent call last\)|raise \w+Error)").unwrap(),
            severity: ErrorSeverity::Error,
        },
        ErrorPattern {
            name: "Generic Error",
            pattern: Regex::new(r"(?i)\b(error|failed|failure|exception)\b").unwrap(),
            severity: ErrorSeverity::Warning,
        },
    ]
}

/// Scan file content for error patterns
fn scan_content_for_errors(
    filename: &str,
    content: &str,
    patterns: &[ErrorPattern],
    errors: &mut Vec<DetectedError>,
) {
    for (line_number, line) in content.lines().enumerate() {
        // Skip INFO lines for certain patterns to avoid false positives
        let is_info = INFO_REGEX.is_match(line);

        for pattern in patterns {
            if pattern.pattern.is_match(line) {
                // Skip Slurm Error and Generic Error in INFO lines as they are prone to false positives
                // (e.g., module names like 'torc_slurm_job_runner' or informational messages)
                if is_info && (pattern.name == "Slurm Error" || pattern.name == "Generic Error") {
                    continue;
                }

                errors.push(DetectedError {
                    file: filename.to_string(),
                    line_number: line_number + 1,
                    pattern_name: pattern.name.to_string(),
                    severity: pattern.severity,
                    line_content: truncate_line(line, 120),
                });
                break; // Only report first matching pattern per line
            }
        }
    }
}

/// Print parse results (errors and summary)
fn print_parse_results(
    errors: &[DetectedError],
    files_parsed: usize,
    metadata: Option<&serde_json::Value>,
) {
    // Print metadata if available
    if let Some(meta) = metadata {
        println!("Bundle Information:");
        println!(
            "  Workflow ID: {}",
            meta.get("workflow_id").unwrap_or(&serde_json::Value::Null)
        );
        println!(
            "  Workflow Name: {}",
            meta.get("workflow_name")
                .unwrap_or(&serde_json::Value::Null)
        );
        println!(
            "  Collected At: {}",
            meta.get("collected_at").unwrap_or(&serde_json::Value::Null)
        );
        println!();
    }

    println!("Files parsed: {}", files_parsed);
    println!();

    if errors.is_empty() {
        println!("No errors detected in log files.");
        return;
    }

    // Group errors by file
    let mut errors_by_file: HashMap<String, Vec<&DetectedError>> = HashMap::new();
    for error in errors {
        errors_by_file
            .entry(error.file.clone())
            .or_default()
            .push(error);
    }

    // Count errors by severity
    let error_count = errors
        .iter()
        .filter(|e| e.severity == ErrorSeverity::Error)
        .count();
    let warning_count = errors
        .iter()
        .filter(|e| e.severity == ErrorSeverity::Warning)
        .count();

    println!("Detected Issues:");
    println!("  Errors: {}", error_count);
    println!("  Warnings: {}", warning_count);
    println!();

    // Print errors grouped by file
    for (file, file_errors) in &errors_by_file {
        println!("{}:", file);
        for error in file_errors {
            if error.severity == ErrorSeverity::Error {
                println!(
                    "  [{}] Line {}: {} - {}",
                    error.severity, error.line_number, error.pattern_name, error.line_content
                );
            }
        }
        println!();
    }

    // Summary of error types
    println!("Error Type Summary:");
    let mut pattern_counts: HashMap<String, usize> = HashMap::new();
    for error in errors {
        if error.severity == ErrorSeverity::Error {
            *pattern_counts
                .entry(error.pattern_name.clone())
                .or_default() += 1;
        }
    }
    let mut sorted_patterns: Vec<_> = pattern_counts.into_iter().collect();
    sorted_patterns.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

    for (pattern, count) in sorted_patterns {
        println!("  {}: {} occurrence(s)", pattern, count);
    }
}

/// Dispatch to analyze a bundle file or directory
fn analyze_path(path: &Path, workflow_id: Option<i64>) {
    if !path.exists() {
        eprintln!("Error: Path not found: {}", path.display());
        std::process::exit(1);
    }

    if path.is_dir() {
        analyze_directory(path, workflow_id);
    } else {
        analyze_bundle(path);
    }
}

/// Parse a log bundle tarball and extract error information
fn analyze_bundle(bundle_path: &Path) {
    println!("Analyzing log bundle: {}", bundle_path.display());
    println!();

    // Open and decompress the tarball
    let file = match File::open(bundle_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening bundle: {}", e);
            std::process::exit(1);
        }
    };
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let patterns = get_error_patterns();
    let mut errors: Vec<DetectedError> = Vec::new();
    let mut files_parsed = 0;
    let mut metadata: Option<serde_json::Value> = None;

    // Process each file in the archive
    let entries = match archive.entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading archive entries: {}", e);
            std::process::exit(1);
        }
    };

    for entry_result in entries {
        let mut entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: Error reading entry: {}", e);
                continue;
            }
        };

        let path = match entry.path() {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let filename = path.to_string_lossy().to_string();

        // Check if this is the metadata file
        if filename == "bundle_metadata.json" {
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_ok() {
                metadata = serde_json::from_str(&content).ok();
            }
            continue;
        }

        // Parse log files (skip slurm_env files - they contain environment variables, not error logs)
        let is_log_file =
            filename.ends_with(".log") || filename.ends_with(".o") || filename.ends_with(".e");
        let is_env_file = filename.contains("slurm_env_");
        if is_log_file && !is_env_file {
            files_parsed += 1;

            // Read entry content into memory
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_err() {
                continue;
            }

            scan_content_for_errors(&filename, &content, &patterns, &mut errors);
        }
    }

    print_parse_results(&errors, files_parsed, metadata.as_ref());
}

/// Detect workflow IDs present in a directory by scanning filenames
pub(crate) fn detect_workflow_ids(dir: &Path) -> Vec<i64> {
    let wf_pattern = Regex::new(r"wf(\d+)").unwrap();
    let mut workflow_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

    // Scan main directory
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            for cap in wf_pattern.captures_iter(&filename) {
                if let Ok(id) = cap[1].parse::<i64>() {
                    workflow_ids.insert(id);
                }
            }
        }
    }

    // Also scan job_stdio subdirectory
    let job_stdio_dir = dir.join("job_stdio");
    if let Ok(entries) = std::fs::read_dir(&job_stdio_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            for cap in wf_pattern.captures_iter(&filename) {
                if let Ok(id) = cap[1].parse::<i64>() {
                    workflow_ids.insert(id);
                }
            }
        }
    }

    let mut ids: Vec<i64> = workflow_ids.into_iter().collect();
    ids.sort();
    ids
}

/// Parse a log directory and extract error information
fn analyze_directory(dir: &Path, workflow_id: Option<i64>) {
    // Detect workflow IDs in the directory
    let detected_ids = detect_workflow_ids(dir);

    if detected_ids.is_empty() {
        eprintln!(
            "No workflow log files found in directory: {}",
            dir.display()
        );
        std::process::exit(1);
    }

    // Determine which workflow ID to use
    let wf_id = match workflow_id {
        Some(id) => {
            if !detected_ids.contains(&id) {
                eprintln!(
                    "Warning: Workflow {} not found in directory. Detected workflows: {:?}",
                    id, detected_ids
                );
            }
            id
        }
        None => {
            if detected_ids.len() > 1 {
                eprintln!(
                    "Multiple workflows detected in directory: {:?}",
                    detected_ids
                );
                eprintln!("Please specify a workflow ID with --workflow-id");
                std::process::exit(1);
            }
            detected_ids[0]
        }
    };

    let wf_pattern = format!("wf{}", wf_id);
    println!("Parsing log directory: {}", dir.display());
    println!("Workflow ID: {}", wf_id);
    println!();

    let patterns = get_error_patterns();
    let mut errors: Vec<DetectedError> = Vec::new();
    let mut files_parsed = 0;

    // Scan main directory
    files_parsed += scan_directory_for_logs(dir, &wf_pattern, &patterns, &mut errors);

    // Scan job_stdio subdirectory
    let job_stdio_dir = dir.join("job_stdio");
    if job_stdio_dir.exists() {
        files_parsed +=
            scan_directory_for_logs(&job_stdio_dir, &wf_pattern, &patterns, &mut errors);
    }

    print_parse_results(&errors, files_parsed, None);
}

/// Scan a directory for log files matching the workflow pattern
fn scan_directory_for_logs(
    dir: &Path,
    wf_pattern: &str,
    patterns: &[ErrorPattern],
    errors: &mut Vec<DetectedError>,
) -> usize {
    let mut files_parsed = 0;

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Warning: Cannot read directory {}: {}", dir.display(), e);
            return 0;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        // Check if file matches workflow pattern and is a log file
        let is_log_file =
            filename.ends_with(".log") || filename.ends_with(".o") || filename.ends_with(".e");
        // Skip slurm_env files - they contain environment variables, not error logs
        let is_env_file = filename.starts_with("slurm_env_");
        if !is_log_file || !filename.contains(wf_pattern) || is_env_file {
            continue;
        }

        files_parsed += 1;

        // Read and scan file content
        if let Ok(content) = std::fs::read_to_string(&path) {
            let display_name = if let Some(parent) = path.parent() {
                if let Some(parent_name) = parent.file_name() {
                    format!("{}/{}", parent_name.to_string_lossy(), filename)
                } else {
                    filename.to_string()
                }
            } else {
                filename.to_string()
            };
            scan_content_for_errors(&display_name, &content, patterns, errors);
        }
    }

    files_parsed
}

/// Truncate a line to a maximum length
fn truncate_line(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
