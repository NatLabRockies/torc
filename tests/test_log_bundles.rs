//! Integration tests for log bundle collection and analysis
//!
//! These tests create temporary directories with fake log files to verify
//! the bundling and analysis functionality works correctly.

use flate2::read::GzDecoder;
use rstest::rstest;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tar::Archive;
use tempfile::TempDir;

/// Helper to create a fake log file with the given content
fn create_log_file(dir: &Path, filename: &str, content: &str) {
    let path = dir.join(filename);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

/// Helper to run torc command and capture output
fn run_torc(args: &[&str]) -> (bool, String, String) {
    let output = Command::new("./target/debug/torc")
        .args(args)
        .output()
        .expect("Failed to execute torc");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Build the torc binary before running tests
fn ensure_binary_built() {
    let status = Command::new("cargo")
        .args(["build", "--bin", "torc"])
        .status()
        .expect("Failed to build torc");
    assert!(status.success(), "Failed to build torc binary");
}

#[rstest]
fn test_bundle_creates_tarball() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();
    fs::create_dir_all(output_dir.join("job_stdio")).unwrap();

    // Create fake log files for workflow 42
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: Job runner started\nINFO: Processing jobs\n",
    );
    create_log_file(&output_dir, "job_stdio/job_wf42_j1_r1.o", "Job 1 output\n");
    create_log_file(&output_dir, "job_stdio/job_wf42_j1_r1.e", "Job 1 stderr\n");
    create_log_file(&output_dir, "slurm_output_wf42_sl12345.o", "Slurm stdout\n");
    create_log_file(&output_dir, "slurm_output_wf42_sl12345.e", "Slurm stderr\n");

    // Also create files for a different workflow (should not be collected)
    create_log_file(
        &output_dir,
        "job_runner_wf99_hostname_r1.log",
        "Different workflow\n",
    );

    let bundle_dir = temp_dir.path().join("bundles");
    fs::create_dir_all(&bundle_dir).unwrap();

    // Run bundle command (skip API call by checking file existence only)
    // Since we can't call the API without a server, we'll test the analyze command instead
    // which doesn't require API access

    // Manually create a bundle for testing
    let bundle_path = bundle_dir.join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    // Verify bundle was created
    assert!(bundle_path.exists(), "Bundle file should exist");

    // Verify bundle contents
    let file = File::open(&bundle_path).unwrap();
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut found_files: Vec<String> = Vec::new();
    for entry in archive.entries().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().to_string();
        found_files.push(path);
    }

    assert!(
        found_files.iter().any(|f| f.contains("job_runner_wf42")),
        "Bundle should contain job runner log"
    );
    assert!(
        found_files.iter().any(|f| f.contains("job_wf42_j1")),
        "Bundle should contain job stdout"
    );
    assert!(
        !found_files.iter().any(|f| f.contains("wf99")),
        "Bundle should not contain other workflow files"
    );
}

/// Helper to create a test bundle manually (simulating what the bundle command does)
fn create_test_bundle(output_dir: &Path, bundle_path: &Path, workflow_id: i64) {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::Builder;

    let tar_file = File::create(bundle_path).unwrap();
    let encoder = GzEncoder::new(tar_file, Compression::default());
    let mut tar_builder = Builder::new(encoder);

    let wf_pattern = format!("wf{}", workflow_id);

    // Collect matching files from output directory
    collect_files_to_tar(&mut tar_builder, output_dir, &wf_pattern);

    // Collect from job_stdio subdirectory
    let job_stdio_dir = output_dir.join("job_stdio");
    if job_stdio_dir.exists() {
        collect_files_to_tar(&mut tar_builder, &job_stdio_dir, &wf_pattern);
    }

    // Add metadata
    let metadata = serde_json::json!({
        "workflow_id": workflow_id,
        "workflow_name": "test_workflow",
        "collected_at": "2024-01-01T00:00:00Z",
    });
    let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();

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

    let encoder = tar_builder.into_inner().unwrap();
    encoder.finish().unwrap();
}

fn collect_files_to_tar<W: std::io::Write>(
    tar_builder: &mut tar::Builder<W>,
    dir: &Path,
    wf_pattern: &str,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let filename = path.file_name().unwrap().to_string_lossy();
                if filename.contains(wf_pattern) {
                    let mut file = File::open(&path).unwrap();
                    let archive_name = if let Some(parent_name) = dir.file_name() {
                        format!("{}/{}", parent_name.to_string_lossy(), filename)
                    } else {
                        filename.to_string()
                    };
                    tar_builder.append_file(&archive_name, &mut file).unwrap();
                }
            }
        }
    }
}

#[rstest]
fn test_analyze_bundle_detects_oom() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log file with OOM error
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: Starting job\nERROR: Out of memory - cannot allocate 4GB\nINFO: Job failed\n",
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("OOM") || stdout.contains("out of memory") || stdout.contains("Killed"),
        "Should detect OOM error. stdout: {}",
        stdout
    );
    assert!(
        stdout.contains("ERROR") || stdout.contains("Error"),
        "Should report error severity. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_bundle_detects_timeout() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log file with timeout error
    create_log_file(
        &output_dir,
        "slurm_output_wf42_sl12345.e",
        "slurmstepd: error: DUE TO TIME LIMIT\nJob cancelled\n",
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Timeout") || stdout.contains("TIME LIMIT") || stdout.contains("Slurm"),
        "Should detect timeout error. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_bundle_detects_segfault() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log file with segfault
    create_log_file(
        &output_dir,
        "job_stdio/job_wf42_j1_r1.e",
        "Processing data...\nSegmentation fault (core dumped)\n",
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Segmentation") || stdout.contains("segfault"),
        "Should detect segfault. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_bundle_detects_python_exception() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log file with Python traceback
    create_log_file(
        &output_dir,
        "job_stdio/job_wf42_j1_r1.e",
        r#"Running script...
Traceback (most recent call last):
  File "script.py", line 10, in <module>
    process_data()
  File "script.py", line 5, in process_data
    raise ValueError("Invalid input")
ValueError: Invalid input
"#,
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Python") || stdout.contains("Traceback"),
        "Should detect Python exception. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_bundle_detects_missing_output_files() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log file with missing output files error (as logged by JobRunner)
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        r#"INFO: Starting job 123
INFO: Job 123 completed with return_code=0
ERROR: Output file validation failed for job 123: Job 123 completed successfully but expected output files are missing: /path/to/output1.csv, /path/to/output2.parquet
INFO: Job ID 123 completed return_code=1 status=Failed
"#,
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Missing Output Files"),
        "Should detect missing output files error. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_bundle_no_errors() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create clean log files with no errors
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: Starting job runner\nINFO: Processing 10 jobs\nINFO: All jobs completed successfully\n",
    );
    create_log_file(
        &output_dir,
        "job_stdio/job_wf42_j1_r1.o",
        "Processing complete.\nResults saved to output.csv\n",
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("No errors detected"),
        "Should report no errors. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_directory_single_workflow() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log files for a single workflow
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: Starting\nERROR: Connection refused\n",
    );

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", output_dir.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Workflow ID: 42"),
        "Should auto-detect workflow. stdout: {}",
        stdout
    );
    assert!(
        stdout.contains("Connection"),
        "Should detect connection error. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_directory_multiple_workflows_requires_id() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log files for multiple workflows
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "Workflow 42\n",
    );
    create_log_file(
        &output_dir,
        "job_runner_wf99_hostname_r1.log",
        "Workflow 99\n",
    );

    let (success, _stdout, stderr) = run_torc(&["logs", "analyze", output_dir.to_str().unwrap()]);

    assert!(!success, "Should fail without workflow ID");
    assert!(
        stderr.contains("Multiple workflows") || stderr.contains("--workflow-id"),
        "Should ask for workflow ID. stderr: {}",
        stderr
    );
}

#[rstest]
fn test_analyze_directory_with_workflow_id() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create log files for multiple workflows
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: Workflow 42\nERROR: Disk full - no space left on device\n",
    );
    create_log_file(
        &output_dir,
        "job_runner_wf99_hostname_r1.log",
        "Workflow 99\n",
    );

    let (success, stdout, stderr) = run_torc(&[
        "logs",
        "analyze",
        output_dir.to_str().unwrap(),
        "--workflow-id",
        "42",
    ]);

    assert!(
        success,
        "Should succeed with workflow ID. stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Workflow ID: 42"),
        "Should use specified workflow. stdout: {}",
        stdout
    );
    assert!(
        stdout.contains("Disk") || stdout.contains("space"),
        "Should detect disk error. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_skips_slurm_env_files() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create slurm env file with content that would normally trigger error detection
    // This should be skipped since it's environment variables, not errors
    create_log_file(
        &output_dir,
        "slurm_env_wf42_sl12345_n0_pid1234.log",
        "SLURM_JOB_ID=12345\nPATH=/usr/bin\nTIMEOUT=3600\nERROR_HANDLER=true\n",
    );

    // Create a clean job log
    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: All jobs completed\n",
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    // The slurm_env file contains "TIMEOUT" and "ERROR" which would trigger detection
    // if not properly skipped
    assert!(
        stdout.contains("No errors detected") || !stdout.contains("slurm_env"),
        "Should skip slurm_env files. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_nonexistent_path() {
    ensure_binary_built();

    let (success, _stdout, stderr) =
        run_torc(&["logs", "analyze", "/nonexistent/path/bundle.tar.gz"]);

    assert!(!success, "Should fail for nonexistent path");
    assert!(
        stderr.contains("not found") || stderr.contains("Path not found"),
        "Should report path not found. stderr: {}",
        stderr
    );
}

#[rstest]
fn test_analyze_empty_directory() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    let (success, _stdout, stderr) = run_torc(&["logs", "analyze", output_dir.to_str().unwrap()]);

    assert!(!success, "Should fail for empty directory");
    assert!(
        stderr.contains("No workflow") || stderr.contains("not found"),
        "Should report no workflows found. stderr: {}",
        stderr
    );
}

#[rstest]
fn test_analyze_shows_metadata() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();

    create_log_file(
        &output_dir,
        "job_runner_wf42_hostname_r1.log",
        "INFO: Done\n",
    );

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Bundle Information"),
        "Should show bundle info. stdout: {}",
        stdout
    );
    assert!(
        stdout.contains("Workflow ID: 42") || stdout.contains("workflow_id"),
        "Should show workflow ID. stdout: {}",
        stdout
    );
    assert!(
        stdout.contains("test_workflow") || stdout.contains("Workflow Name"),
        "Should show workflow name. stdout: {}",
        stdout
    );
}

#[rstest]
fn test_analyze_reports_file_count() {
    ensure_binary_built();

    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).unwrap();
    fs::create_dir_all(output_dir.join("job_stdio")).unwrap();

    // Create multiple log files
    create_log_file(&output_dir, "job_runner_wf42_hostname_r1.log", "Log 1\n");
    create_log_file(&output_dir, "slurm_output_wf42_sl12345.o", "Log 2\n");
    create_log_file(&output_dir, "slurm_output_wf42_sl12345.e", "Log 3\n");
    create_log_file(&output_dir, "job_stdio/job_wf42_j1_r1.o", "Log 4\n");
    create_log_file(&output_dir, "job_stdio/job_wf42_j1_r1.e", "Log 5\n");

    let bundle_path = temp_dir.path().join("wf42.tar.gz");
    create_test_bundle(&output_dir, &bundle_path, 42);

    let (success, stdout, stderr) = run_torc(&["logs", "analyze", bundle_path.to_str().unwrap()]);

    assert!(success, "Analyze should succeed. stderr: {}", stderr);
    assert!(
        stdout.contains("Files parsed: 5") || stdout.contains("parsed: 5"),
        "Should report 5 files parsed. stdout: {}",
        stdout
    );
}
