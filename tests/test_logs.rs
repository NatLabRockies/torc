use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use torc::client::commands::logs::analyze_workflow_logs;

#[test]
fn test_analyze_workflow_logs_false_positives() {
    let dir = tempdir().unwrap();
    let log_path = dir
        .path()
        .join("job_runner_slurm_wf137_sl12672684_n0_pid4104879.log");

    let mut file = File::create(&log_path).unwrap();
    writeln!(
        file,
        "[2026-02-13T21:46:32Z INFO  torc_slurm_job_runner::unix_main] Starting Slurm job runner"
    )
    .unwrap();
    writeln!(
        file,
        "[2026-02-13T21:46:32Z INFO  torc_slurm_job_runner::unix_main] Job ID: 12672684"
    )
    .unwrap();
    writeln!(file, "[2026-02-13T21:46:32Z INFO  torc::client::utils] Capturing environment variables containing 'SLURM' to: output/slurm_env_wf137_sl12672684.log").unwrap();
    writeln!(
        file,
        "slurmstepd: error: *** JOB 12672684 CANCELLED AT 2026-02-13T21:47:42 DUE TO TIME LIMIT ***"
    )
    .unwrap();

    let result = analyze_workflow_logs(dir.path(), 137).unwrap();

    // Should only have 1 error (the CANCELLED one)
    // The INFO logs should be ignored due to word boundaries and INFO skipping
    assert_eq!(result.error_count, 1);
    assert_eq!(result.errors[0].pattern_name, "Slurm Error");
    assert!(result.errors[0].line_content.contains("CANCELLED"));
}

#[test]
fn test_analyze_workflow_logs_info_skipping() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("wf137_runner.log");

    let mut file = File::create(&log_path).unwrap();
    writeln!(file, "[INFO] This is an error that should be ignored").unwrap();
    writeln!(file, "[ERROR] This is a real error that should be caught").unwrap();

    let result = analyze_workflow_logs(dir.path(), 137).unwrap();

    assert_eq!(result.error_count, 0); // [ERROR] matches Generic Error which is a Warning
    assert_eq!(result.warning_count, 1);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].pattern_name, "Generic Error");
}

#[test]
fn test_analyze_workflow_logs_word_boundaries() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("wf137_boundaries.log");

    let mut file = File::create(&log_path).unwrap();
    // Should NOT match (substrings)
    writeln!(file, "This is some groomed output").unwrap(); // groom contains oom
    writeln!(file, "The timeout_value is 10").unwrap(); // timeout_value is not \btimeout\b
    writeln!(file, "segmented is small").unwrap(); // contains 'seg' as a substring but should not match segmentation fault

    // Should match
    writeln!(file, "Out of memory: killed process 123").unwrap();
    writeln!(file, "Job timed out after 10m").unwrap();
    writeln!(file, "Segmentation fault (core dumped)").unwrap();

    let result = analyze_workflow_logs(dir.path(), 137).unwrap();

    assert_eq!(result.error_count, 3);

    let patterns: Vec<String> = result
        .errors
        .iter()
        .map(|e| e.pattern_name.clone())
        .collect();
    assert!(patterns.contains(&"OOM Killed".to_string()));
    assert!(patterns.contains(&"Timeout".to_string()));
    assert!(patterns.contains(&"Segmentation Fault".to_string()));
}
