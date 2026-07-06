//! SSH utilities for remote worker execution.

use log::{debug, info};
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;

use super::types::WorkerEntry;

/// Default SSH connection timeout in seconds.
const SSH_CONNECT_TIMEOUT: u64 = 30;

/// Execute a command on a remote host via SSH.
///
/// Returns the raw Output from the SSH command.
pub fn ssh_execute(
    worker: &WorkerEntry,
    command: &str,
    timeout_secs: Option<u64>,
) -> Result<Output, String> {
    let timeout = timeout_secs.unwrap_or(SSH_CONNECT_TIMEOUT);

    let mut cmd = Command::new("ssh");

    // Add SSH options
    cmd.arg("-o")
        .arg(format!("ConnectTimeout={}", timeout))
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new");

    // Add port if specified
    if let Some(port) = worker.port {
        cmd.arg("-p").arg(port.to_string());
    }

    // Add target and command
    cmd.arg(worker.ssh_target()).arg(command);

    debug!(
        "Executing SSH command on {}: {}",
        worker.display_name(),
        command
    );

    cmd.output()
        .map_err(|e| format!("SSH execution failed for {}: {}", worker.display_name(), e))
}

/// Execute a command on a remote host, failing if the remote command exits
/// non-zero.
///
/// Plain [`ssh_execute`] returns `Ok` whenever SSH itself connects, even if the
/// remote command failed (e.g. a PowerShell `throw`, a missing executable, or a
/// permission error). Callers that treat that `Ok` as success silently swallow
/// the real failure, which then resurfaces later as a confusing downstream error
/// (an unreadable PID file, a missing archive) with the original stderr lost.
/// This wrapper checks `status.success()` and surfaces the remote stderr at the
/// call site instead.
pub fn ssh_execute_checked(
    worker: &WorkerEntry,
    command: &str,
    timeout_secs: Option<u64>,
) -> Result<Output, String> {
    let output = ssh_execute(worker, command, timeout_secs)?;

    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Prefer stderr, but fall back to stdout since some shells (notably
        // cmd.exe/PowerShell) write diagnostics there.
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        Err(format!(
            "Command failed on {} (exit {}): {}",
            worker.display_name(),
            code,
            detail
        ))
    }
}

/// Execute a command on a remote host and return stdout as a string.
///
/// Returns an error if the command fails (non-zero exit code).
pub fn ssh_execute_capture(worker: &WorkerEntry, command: &str) -> Result<String, String> {
    let output = ssh_execute_checked(worker, command, None)?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Check if SSH connection to a worker is possible.
///
/// This probes the remote shell family (POSIX vs Windows), which both verifies
/// connectivity and confirms the host runs a shell that `torc remote` supports.
/// A previous implementation ran the POSIX `true` builtin, which fails on
/// Windows hosts and caused them to be rejected before any work was attempted.
pub fn check_ssh_connectivity(worker: &WorkerEntry) -> Result<(), String> {
    debug!("Checking SSH connectivity to {}", worker.display_name());

    super::shell::detect_remote_shell(worker).map(|shell| {
        debug!(
            "SSH connectivity OK for {} (shell={:?})",
            worker.display_name(),
            shell
        );
    })
}

/// Get the torc version on a remote host.
///
/// Returns the version string (e.g., "torc 0.7.0").
pub fn get_remote_torc_version(worker: &WorkerEntry) -> Result<String, String> {
    debug!("Getting torc version from {}", worker.display_name());

    let output = ssh_execute_capture(worker, "torc --version")?;
    Ok(output.trim().to_string())
}

/// Parse the version from a torc version string.
///
/// Handles formats like "torc 0.7.0" or just "0.7.0".
pub fn parse_torc_version(version_str: &str) -> String {
    let trimmed = version_str.trim();
    trimmed.strip_prefix("torc ").unwrap_or(trimmed).to_string()
}

/// Verify that a remote worker has the same torc version as local.
pub fn verify_version(worker: &WorkerEntry, local_version: &str) -> Result<(), String> {
    let remote_version_str = get_remote_torc_version(worker)?;
    let remote_version = parse_torc_version(&remote_version_str);

    if remote_version.as_str() == local_version {
        debug!(
            "Version match for {}: {}",
            worker.display_name(),
            local_version
        );
        Ok(())
    } else {
        Err(format!(
            "Version mismatch: local={}, {}={}",
            local_version,
            worker.display_name(),
            remote_version
        ))
    }
}

/// Execute a command on a remote host via SCP.
///
/// Returns the raw Output from the SCP command.
pub fn scp_download(
    worker: &WorkerEntry,
    remote_path: &str,
    local_path: &str,
    timeout_secs: Option<u64>,
) -> Result<Output, String> {
    let timeout = timeout_secs.unwrap_or(300); // Default 5 minutes for file transfers

    let mut cmd = Command::new("scp");

    // Add SSH options via -o
    cmd.arg("-o")
        .arg(format!("ConnectTimeout={}", timeout))
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new");

    // Add port if specified
    if let Some(port) = worker.port {
        cmd.arg("-P").arg(port.to_string());
    }

    // Add source and destination
    let remote_spec = format!("{}:{}", worker.ssh_target(), remote_path);
    cmd.arg(&remote_spec).arg(local_path);

    debug!(
        "SCP download from {}: {} -> {}",
        worker.display_name(),
        remote_path,
        local_path
    );

    cmd.output()
        .map_err(|e| format!("SCP failed for {}: {}", worker.display_name(), e))
}

/// Execute operations in parallel across multiple workers.
///
/// Returns results in the same order as the input workers.
pub fn parallel_execute<F, R>(workers: &[WorkerEntry], operation: F, max_parallel: usize) -> Vec<R>
where
    F: Fn(&WorkerEntry) -> R + Send + Sync + Clone + 'static,
    R: Send + 'static,
{
    if workers.is_empty() {
        return Vec::new();
    }

    // Limit parallelism
    let max_parallel = max_parallel.min(workers.len());

    // Create channels for work distribution
    let (tx, rx) = mpsc::channel::<(usize, WorkerEntry)>();
    let (result_tx, result_rx) = mpsc::channel::<(usize, R)>();

    // Spawn worker threads
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let mut handles = Vec::with_capacity(max_parallel);

    for _ in 0..max_parallel {
        let rx = rx.clone();
        let result_tx = result_tx.clone();
        let op = operation.clone();

        let handle = thread::spawn(move || {
            loop {
                let work = {
                    let rx = rx.lock().unwrap();
                    rx.recv()
                };

                match work {
                    Ok((idx, worker)) => {
                        let result = op(&worker);
                        if result_tx.send((idx, result)).is_err() {
                            break;
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }
        });

        handles.push(handle);
    }

    // Send work to threads
    for (idx, worker) in workers.iter().enumerate() {
        if tx.send((idx, worker.clone())).is_err() {
            break;
        }
    }

    // Close the send channel to signal threads to finish
    drop(tx);

    // Collect results
    let mut results: Vec<Option<R>> = (0..workers.len()).map(|_| None).collect();
    for (idx, result) in result_rx.iter().take(workers.len()) {
        results[idx] = Some(result);
    }

    // Wait for all threads to complete
    for handle in handles {
        let _ = handle.join();
    }

    // Unwrap results (all should be Some at this point)
    results.into_iter().map(|r| r.unwrap()).collect()
}

/// Verify that all workers have matching torc versions.
///
/// Returns Ok if all versions match, or an error with details about mismatches.
pub fn verify_all_versions(
    workers: &[WorkerEntry],
    local_version: &str,
    max_parallel: usize,
) -> Result<(), String> {
    info!("Verifying torc versions on {} worker(s)...", workers.len());

    let local_ver = local_version.to_string();
    let results: Vec<Result<(), String>> = parallel_execute(
        workers,
        move |worker| verify_version(worker, &local_ver),
        max_parallel,
    );

    let errors: Vec<String> = results.into_iter().filter_map(|r| r.err()).collect();

    if errors.is_empty() {
        info!("All workers have matching torc version: {}", local_version);
        Ok(())
    } else {
        Err(format!(
            "Version check failed on {} worker(s):\n  {}",
            errors.len(),
            errors.join("\n  ")
        ))
    }
}

/// Check SSH connectivity to all workers.
///
/// Returns Ok if all workers are reachable, or an error with details.
pub fn check_all_connectivity(workers: &[WorkerEntry], max_parallel: usize) -> Result<(), String> {
    info!(
        "Checking SSH connectivity to {} worker(s)...",
        workers.len()
    );

    let results: Vec<Result<(), String>> =
        parallel_execute(workers, check_ssh_connectivity, max_parallel);

    let errors: Vec<(String, String)> = workers
        .iter()
        .zip(results)
        .filter_map(|(w, r)| r.err().map(|e| (w.display_name().to_string(), e)))
        .collect();

    if errors.is_empty() {
        info!("All workers are reachable via SSH");
        Ok(())
    } else {
        Err(format!(
            "SSH connectivity check failed for {} worker(s):\n  {}",
            errors.len(),
            errors
                .iter()
                .map(|(h, e)| format!("{}: {}", h, e))
                .collect::<Vec<_>>()
                .join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_torc_version() {
        assert_eq!(parse_torc_version("torc 0.7.0"), "0.7.0");
        assert_eq!(parse_torc_version("0.7.0"), "0.7.0");
        assert_eq!(parse_torc_version("torc 1.2.3-beta"), "1.2.3-beta");
        assert_eq!(parse_torc_version("  torc 0.7.0  "), "0.7.0");
    }

    #[test]
    fn test_parallel_execute_empty() {
        let workers: Vec<WorkerEntry> = vec![];
        let results: Vec<i32> = parallel_execute(&workers, |_| 42, 4);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parallel_execute_ordering() {
        let workers: Vec<WorkerEntry> = (0..10)
            .map(|i| WorkerEntry::new(format!("host{}", i)))
            .collect();

        let results: Vec<String> = parallel_execute(&workers, |w| w.host.clone(), 4);

        // Results should be in the same order as input
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result, &format!("host{}", i));
        }
    }
}
