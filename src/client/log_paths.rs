use std::path::{Path, PathBuf};

/// Return the name of the job runner log file for the local runner.
pub fn get_job_runner_log_file(
    output_dir: PathBuf,
    hostname: &str,
    workflow_id: i64,
    run_id: i64,
) -> String {
    format!(
        "{}/job_runner_{}_wf{}_r{}.log",
        output_dir.display(),
        hostname,
        workflow_id,
        run_id,
    )
}

/// Return the name of the job runner log file for Slurm schedulers.
pub fn get_slurm_job_runner_log_file(
    output_dir: PathBuf,
    workflow_id: i64,
    slurm_job_id: &str,
    node_id: &str,
    task_pid: usize,
) -> String {
    format!(
        "{}/job_runner_slurm_wf{}_sl{}_n{}_pid{}.log",
        output_dir.display(),
        workflow_id,
        slurm_job_id,
        node_id,
        task_pid
    )
}

/// Get the path to a job's stdout log file
pub fn get_job_stdout_path(
    output_dir: &Path,
    workflow_id: i64,
    job_id: i64,
    run_id: i64,
    attempt_id: i64,
) -> String {
    format!(
        "{}/job_stdio/job_wf{}_j{}_r{}_a{}.o",
        output_dir.display(),
        workflow_id,
        job_id,
        run_id,
        attempt_id
    )
}

/// Get the path to a job's stderr log file
pub fn get_job_stderr_path(
    output_dir: &Path,
    workflow_id: i64,
    job_id: i64,
    run_id: i64,
    attempt_id: i64,
) -> String {
    format!(
        "{}/job_stdio/job_wf{}_j{}_r{}_a{}.e",
        output_dir.display(),
        workflow_id,
        job_id,
        run_id,
        attempt_id
    )
}

/// Get the path to a job's combined stdout+stderr log file
pub fn get_job_combined_path(
    output_dir: &Path,
    workflow_id: i64,
    job_id: i64,
    run_id: i64,
    attempt_id: i64,
) -> String {
    format!(
        "{}/job_stdio/job_wf{}_j{}_r{}_a{}.log",
        output_dir.display(),
        workflow_id,
        job_id,
        run_id,
        attempt_id
    )
}

/// Get the path to Slurm's stdout log file
pub fn get_slurm_stdout_path(output_dir: &Path, workflow_id: i64, slurm_job_id: &str) -> String {
    format!(
        "{}/slurm_output_wf{}_sl{}.o",
        output_dir.display(),
        workflow_id,
        slurm_job_id
    )
}

/// Get the path to Slurm's stderr log file
pub fn get_slurm_stderr_path(output_dir: &Path, workflow_id: i64, slurm_job_id: &str) -> String {
    format!(
        "{}/slurm_output_wf{}_sl{}.e",
        output_dir.display(),
        workflow_id,
        slurm_job_id
    )
}

/// Return the path for the dmesg log file captured by the Slurm job runner.
/// Uses the same identifiers as the job runner log for consistency and easy correlation.
pub fn get_slurm_dmesg_log_file(
    output_dir: PathBuf,
    workflow_id: i64,
    slurm_job_id: &str,
    node_id: &str,
    task_pid: usize,
) -> String {
    format!(
        "{}/dmesg_slurm_wf{}_sl{}_n{}_pid{}.log",
        output_dir.display(),
        workflow_id,
        slurm_job_id,
        node_id,
        task_pid
    )
}

/// Return the path for the Slurm environment variables log file.
/// Uses the same identifiers as the job runner log for consistency and easy correlation.
pub fn get_slurm_env_log_file(
    output_dir: PathBuf,
    workflow_id: i64,
    slurm_job_id: &str,
    node_id: &str,
    task_pid: usize,
) -> String {
    format!(
        "{}/slurm_env_wf{}_sl{}_n{}_pid{}.log",
        output_dir.display(),
        workflow_id,
        slurm_job_id,
        node_id,
        task_pid
    )
}

/// Return the name of the watch log file.
pub fn get_watch_log_file(output_dir: PathBuf, hostname: &str, workflow_id: i64) -> String {
    format!(
        "{}/watch_{}_wf{}.log",
        output_dir.display(),
        hostname,
        workflow_id
    )
}
