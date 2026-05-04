//! Common utilities for the Torc client
//!
//! This module contains utility functions that are used across multiple
//! client modules.
//!
//! # Example
//!
//! ```rust
//! use torc::client::{Configuration, apis, utils::send_with_retries};
//!
//! # fn example(config: &Configuration) -> Result<(), Box<dyn std::error::Error>> {
//! // Retry API calls with automatic network error handling
//! let response = send_with_retries(
//!     config,
//!     || apis::system_api::ping(config),
//!     5, // Wait up to 5 minutes for server recovery
//! )?;
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use log::{debug, error, info, warn};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::models;

const PING_INTERVAL_SECONDS: u64 = 30;

/// Creates a cross-platform shell command for executing shell scripts/commands.
///
/// On Unix systems, uses `bash -c` for shell execution.
/// On Windows, uses `cmd /C` for shell execution.
///
/// # Returns
///
/// A `Command` configured with the appropriate shell interpreter and argument flag.
/// The caller should add the actual command string using `.arg(command_str)`.
///
/// # Example
///
/// ```ignore
/// use torc::client::utils::shell_command;
///
/// let output = shell_command()
///     .arg("echo hello")
///     .output()?;
/// ```
pub fn shell_command() -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C");
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c");
        cmd
    }
}

/// Execute an API call with automatic retries for network errors
///
/// This function will immediately return non-network errors, but will retry
/// network-related errors by periodically pinging the server until it comes
/// back online or the timeout is reached.
///
/// # Arguments
/// * `config` - The API configuration to use for server pings
/// * `api_call` - The API call function to execute and potentially retry
/// * `wait_for_healthy_database_minutes` - Maximum time to wait for the server to recover
///
/// # Returns
/// The result of the API call, or the original error if retries are exhausted
/// Check whether an error string indicates a transient failure that should be retried.
///
/// Retryable errors include:
/// - Most `apis::Error::Reqwest` variants (matched via the "error in reqwest" substring
///   produced by `apis::Error::Display`). This covers connect, send-request, and
///   response-read failures whose inner cause is not exposed by reqwest's top-level
///   `Display`. Reqwest *builder* errors (invalid URL, misconfigured client) are
///   excluded — they are deterministic and retrying will not help.
/// - Network-level failures (connection refused, DNS, timeout, unreachable)
/// - HTTP 5xx responses (server crash, gateway error, overloaded)
/// - Database contention ("database is locked", "busy")
fn is_retryable_error(error_str: &str) -> bool {
    let s = error_str.to_ascii_lowercase();

    // reqwest::Error with Kind::Builder (e.g. invalid URL, bad base_path) is
    // deterministic. It surfaces as "error in reqwest: builder error: ..." and
    // must not enter the retry loop.
    if s.contains("builder error") {
        return false;
    }

    // Any other reqwest-layer failure is transient: connect, send, body read, TLS, etc.
    // apis::Error::Display emits "error in reqwest: ..." for every Error::Reqwest.
    s.contains("error in reqwest")
        // Network errors
        || s.contains("connection")
        || s.contains("timeout")
        || s.contains("network")
        || s.contains("dns")
        || s.contains("resolve")
        || s.contains("unreachable")
        // HTTP server errors (formatted as "status code NNN" by the generated client)
        || is_5xx_response_error(&s)
        // Database contention (server may surface this via database_lock_aware_error)
        || is_database_lock_error_lowercased(&s)
}

/// Check whether a lowercased error string indicates SQLite lock contention.
/// Lock errors typically clear in milliseconds, so the caller can retry quickly
/// instead of falling back to the slow ping-and-wait path used for outages.
///
/// Caller must lowercase the input first. Naming `_lowercased` documents the
/// precondition at call sites and avoids the repeated allocation that a
/// case-insensitive helper would force on `is_retryable_error`.
fn is_database_lock_error_lowercased(s: &str) -> bool {
    s.contains("database is locked") || s.contains("database is busy") || s.contains("sqlite_busy")
}

/// Tunable parameters for the fast-retry phase. Production callers should use
/// `FastRetryConfig::production()`; tests can substitute zero delays so the
/// fast path runs in microseconds.
#[derive(Clone, Copy)]
struct FastRetryConfig {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
}

impl FastRetryConfig {
    const fn production() -> Self {
        Self {
            max_attempts: 6,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(2),
        }
    }
}

/// Retry an API call quickly while it keeps failing with SQLite lock errors.
///
/// Returns `Ok` on the first successful call. Returns `Err` with the latest
/// error if all attempts are exhausted, or if the call returns a non-lock
/// error (in which case the caller can decide whether to fall through to a
/// slower retry path). The first failure is passed in as `initial_err` so the
/// helper does not double-call the api on entry.
fn fast_retry_for_lock_errors<T, E, F>(
    initial_err: E,
    api_call: &mut F,
    config: FastRetryConfig,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
{
    let mut e = initial_err;
    let mut delay = config.initial_delay;

    for attempt in 1..=config.max_attempts {
        thread::sleep(delay);
        match api_call() {
            Ok(result) => {
                debug!(
                    "Recovered from database lock after {} fast retries",
                    attempt
                );
                return Ok(result);
            }
            Err(retry_err) => {
                let lower = retry_err.to_string().to_ascii_lowercase();
                let is_lock = is_database_lock_error_lowercased(&lower);
                e = retry_err;
                if !is_lock {
                    // Different error class; stop fast-retrying so the caller
                    // can route it through the generic retry path.
                    break;
                }
                delay = delay.saturating_mul(2).min(config.max_delay);
            }
        }
    }
    Err(e)
}

fn is_5xx_response_error(s: &str) -> bool {
    let Some(start) = s.find("status code ") else {
        return false;
    };
    let status_start = start + "status code ".len();
    let digits: String = s[status_start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();

    digits
        .parse::<u16>()
        .is_ok_and(|code| (500..600).contains(&code))
}

pub fn send_with_retries<T, E, F>(
    config: &Configuration,
    mut api_call: F,
    wait_for_healthy_database_minutes: u64,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
{
    // Fast-retry phase for SQLite lock contention. Lock errors typically clear in
    // milliseconds; the slow ping-and-wait loop below assumes the server is down
    // and would waste throughput by sleeping 30s before each retry.
    let mut e = match api_call() {
        Ok(result) => return Ok(result),
        Err(e) => e,
    };

    if is_database_lock_error_lowercased(&e.to_string().to_ascii_lowercase()) {
        match fast_retry_for_lock_errors(e, &mut api_call, FastRetryConfig::production()) {
            Ok(result) => return Ok(result),
            Err(latest_err) => e = latest_err,
        }
    }

    let error_str = e.to_string();
    if !is_retryable_error(&error_str) {
        return Err(e);
    }

    warn!(
        "Transient error detected: {}. Entering retry loop for up to {} minutes.",
        e, wait_for_healthy_database_minutes
    );

    let start_time = Instant::now();
    let timeout_duration = Duration::from_secs(wait_for_healthy_database_minutes * 60);

    loop {
        if start_time.elapsed() >= timeout_duration {
            error!(
                "Retry timeout exceeded ({} minutes). Giving up.",
                wait_for_healthy_database_minutes
            );
            return Err(e);
        }

        thread::sleep(Duration::from_secs(PING_INTERVAL_SECONDS));

        // Try to ping the server first to confirm it's reachable
        match apis::system_api::ping(config) {
            Ok(_) => {
                info!("Server is responding. Retrying original API call.");
                match api_call() {
                    Ok(result) => return Ok(result),
                    Err(retry_err) => {
                        let retry_str = retry_err.to_string();
                        if is_retryable_error(&retry_str) {
                            warn!(
                                "Retry attempt failed with transient error: {}. \
                                 Will keep retrying.",
                                retry_err
                            );
                            continue;
                        }
                        return Err(retry_err);
                    }
                }
            }
            Err(ping_error) => {
                debug!(
                    "Server still unreachable: {}. Continuing to wait...",
                    ping_error
                );
                continue;
            }
        }
    }
}

/// Atomically claim a workflow action for execution
///
/// This function attempts to claim an action so that only one compute node
/// executes it. Uses automatic retries for network errors.
///
/// # Arguments
/// * `config` - The API configuration
/// * `workflow_id` - The workflow ID
/// * `action_id` - The action ID to claim
/// * `compute_node_id` - The compute node ID claiming the action
/// * `wait_for_healthy_database_minutes` - Maximum time to wait for server recovery
///
/// # Returns
/// * `Ok(true)` - Successfully claimed the action
/// * `Ok(false)` - Action was already claimed by another compute node
/// * `Err(_)` - An error occurred during the claim attempt
pub fn claim_action(
    config: &Configuration,
    workflow_id: i64,
    action_id: i64,
    compute_node_id: Option<i64>,
    wait_for_healthy_database_minutes: u64,
) -> Result<bool, Box<dyn std::error::Error>> {
    let claimed = send_with_retries(
        config,
        || -> Result<bool, Box<dyn std::error::Error>> {
            let body = models::ClaimActionRequest { compute_node_id };

            match apis::workflow_actions_api::claim_action(config, workflow_id, action_id, body) {
                Ok(result) => Ok(result.success),
                Err(err) => {
                    // Check if it's a Conflict (already claimed by another compute node)
                    if let crate::client::apis::Error::ResponseError(ref response_content) = err
                        && response_content.status == reqwest::StatusCode::CONFLICT
                    {
                        return Ok(false);
                    }
                    Err(Box::new(err))
                }
            }
        },
        wait_for_healthy_database_minutes,
    )?;

    Ok(claimed)
}

/// Detect the number of NVIDIA GPUs available on the system.
///
/// Uses NVML (NVIDIA Management Library) to query the number of GPU devices.
/// Returns 0 if NVML fails to initialize (e.g., no NVIDIA drivers installed,
/// no NVIDIA GPUs present, or NVML library not available).
///
/// # Returns
/// The number of NVIDIA GPUs detected, or 0 if detection fails.
///
/// # Example
/// ```ignore
/// let num_gpus = detect_nvidia_gpus();
/// println!("Detected {} NVIDIA GPU(s)", num_gpus);
/// ```
pub fn detect_nvidia_gpus() -> i64 {
    match nvml_wrapper::Nvml::init() {
        Ok(nvml) => match nvml.device_count() {
            Ok(count) => {
                info!("Detected {} NVIDIA GPU(s)", count);
                count as i64
            }
            Err(e) => {
                debug!("Failed to get NVIDIA GPU count: {}", e);
                0
            }
        },
        Err(e) => {
            debug!(
                "NVML initialization failed (no NVIDIA GPUs or drivers): {}",
                e
            );
            0
        }
    }
}

/// Capture environment variables containing a substring and save them to a file.
///
/// This is useful for debugging job runner environment, especially for capturing
/// all SLURM-related environment variables.
///
/// # Arguments
/// * `file_path` - Path where the environment variables will be written
/// * `substring` - Only environment variables whose names contain this substring will be captured
///
/// # Note
/// Errors are logged but do not cause the function to fail, since environment capture
/// is informational and should not block process exit.
pub fn capture_env_vars(file_path: &Path, substring: &str) {
    info!(
        "Capturing environment variables containing '{}' to: {}",
        substring,
        file_path.display()
    );

    let mut env_vars: Vec<(String, String)> = std::env::vars()
        .filter(|(key, _)| key.contains(substring))
        .collect();

    // Sort for consistent output
    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    match File::create(file_path) {
        Ok(mut file) => {
            for (key, value) in &env_vars {
                if let Err(e) = writeln!(file, "{}={}", key, value) {
                    error!("Error writing environment variable to file: {}", e);
                    return;
                }
            }
            info!(
                "Successfully captured {} environment variables",
                env_vars.len()
            );
        }
        Err(e) => {
            error!(
                "Error creating environment variables file {}: {}",
                file_path.display(),
                e
            );
        }
    }
}

/// Capture dmesg output and save it to a file.
///
/// This may contain useful debug information if any job failed (e.g., OOM killer,
/// hardware errors, kernel panics).
///
/// # Arguments
/// * `file_path` - Path where the dmesg output will be written
/// * `filter_after` - If provided, only include log lines with timestamps after this time
///
/// # Note
/// Errors are logged but do not cause the function to fail, since dmesg capture
/// is informational and should not block process exit.
pub fn capture_dmesg(file_path: &Path, filter_after: Option<DateTime<Local>>) {
    info!("Capturing dmesg output to: {}", file_path.display());
    if let Some(cutoff) = filter_after {
        info!(
            "Filtering dmesg to only include messages after: {}",
            cutoff.format("%Y-%m-%d %H:%M:%S")
        );
    }

    match Command::new("dmesg").arg("--ctime").output() {
        Ok(output) => match File::create(file_path) {
            Ok(mut file) => {
                let stdout_str = String::from_utf8_lossy(&output.stdout);

                // Filter lines if a cutoff time is provided
                let filtered_output = if let Some(cutoff) = filter_after {
                    filter_dmesg_by_time(&stdout_str, cutoff)
                } else {
                    stdout_str.to_string()
                };

                if let Err(e) = file.write_all(filtered_output.as_bytes()) {
                    error!("Error writing dmesg stdout to file: {}", e);
                }
                if !output.stderr.is_empty() {
                    if let Err(e) = file.write_all(b"\n--- stderr ---\n") {
                        error!("Error writing dmesg separator: {}", e);
                    }
                    if let Err(e) = file.write_all(&output.stderr) {
                        error!("Error writing dmesg stderr to file: {}", e);
                    }
                }
                info!("Successfully captured dmesg output");
            }
            Err(e) => {
                error!("Error creating dmesg file {}: {}", file_path.display(), e);
            }
        },
        Err(e) => {
            error!("Error running dmesg command: {}", e);
        }
    }
}

/// Filter dmesg output to only include lines after a given cutoff time.
///
/// Parses timestamps in the format `[Day Mon DD HH:MM:SS YYYY]` from dmesg --ctime output.
/// Lines without parseable timestamps are included (they may be continuation lines).
fn filter_dmesg_by_time(dmesg_output: &str, cutoff: DateTime<Local>) -> String {
    let mut filtered_lines = Vec::new();
    let mut include_following = false;

    for line in dmesg_output.lines() {
        // Try to parse the timestamp from the line
        // Format: [Day Mon DD HH:MM:SS YYYY] message
        // Example: [Tue Nov 25 10:11:08 2025] BIOS-e820: ...
        if let Some(timestamp) = parse_dmesg_timestamp(line) {
            include_following = timestamp >= cutoff;
        }

        // Include line if it's after the cutoff (or if we couldn't parse a timestamp,
        // include it if the previous timestamped line was included)
        if include_following {
            filtered_lines.push(line);
        }
    }

    if filtered_lines.is_empty() {
        format!(
            "# No dmesg messages found after {}\n",
            cutoff.format("%Y-%m-%d %H:%M:%S")
        )
    } else {
        filtered_lines.join("\n") + "\n"
    }
}

/// Parse a timestamp from a dmesg --ctime output line.
///
/// Expected format: `[Day Mon DD HH:MM:SS YYYY] message`
/// Example: `[Tue Nov 25 10:11:08 2025] BIOS-e820: ...`
fn parse_dmesg_timestamp(line: &str) -> Option<DateTime<Local>> {
    // Find the timestamp between [ and ]
    let start = line.find('[')?;
    let end = line.find(']')?;
    if start >= end {
        return None;
    }

    let timestamp_str = &line[start + 1..end];

    // Parse format: "Day Mon DD HH:MM:SS YYYY"
    // Example: "Tue Nov 25 10:11:08 2025"
    let naive = NaiveDateTime::parse_from_str(timestamp_str, "%a %b %e %H:%M:%S %Y").ok()?;

    // Convert to local timezone
    Local.from_local_datetime(&naive).single()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dmesg_timestamp() {
        // Standard format
        let line = "[Tue Nov 25 10:11:08 2025] BIOS-e820: some message";
        let ts = parse_dmesg_timestamp(line);
        assert!(ts.is_some());

        // Single digit day (with space padding)
        let line = "[Mon Dec  1 09:05:00 2025] kernel: message";
        let ts = parse_dmesg_timestamp(line);
        assert!(ts.is_some());

        // Invalid format
        let line = "No timestamp here";
        let ts = parse_dmesg_timestamp(line);
        assert!(ts.is_none());

        // Malformed timestamp
        let line = "[invalid timestamp] message";
        let ts = parse_dmesg_timestamp(line);
        assert!(ts.is_none());
    }

    #[test]
    fn test_is_retryable_error() {
        // Network errors
        assert!(is_retryable_error("connection refused"));
        assert!(is_retryable_error("Connection reset by peer"));
        assert!(is_retryable_error("DNS lookup failed"));
        assert!(is_retryable_error("request timeout"));
        assert!(is_retryable_error("network is unreachable"));

        // Any apis::Error::Reqwest variant (surfaces as "error in reqwest: ...")
        assert!(is_retryable_error(
            "error in reqwest: error sending request for url \
             (http://127.0.0.1:39195/torc-service/v1/workflows/1/is_complete)"
        ));
        assert!(is_retryable_error(
            "error in reqwest: error decoding response body"
        ));

        // HTTP 5xx from generated client
        assert!(is_retryable_error(
            "error in response: status code 500: internal error"
        ));
        assert!(is_retryable_error("error in response: status code 502"));
        assert!(is_retryable_error(
            "error in response: status code 503: service unavailable"
        ));
        assert!(is_retryable_error(
            "error in response: status code 504: gateway timeout"
        ));
        assert!(is_retryable_error(
            "error in response: status code 599: network connect timeout"
        ));

        // Database contention
        assert!(is_retryable_error("database is locked"));
        assert!(is_retryable_error("database is busy"));
        assert!(is_retryable_error(
            "Failed to create result record: database is locked"
        ));
        assert!(is_retryable_error("SQLITE_BUSY: snapshot conflict"));

        // Non-retryable errors
        // reqwest builder errors are deterministic (invalid URL, etc.)
        assert!(!is_retryable_error(
            "error in reqwest: builder error: relative URL without a base"
        ));
        assert!(!is_retryable_error(
            "error in response: status code 404: not found"
        ));
        assert!(!is_retryable_error(
            "error in response: status code 422: validation error"
        ));
        assert!(!is_retryable_error("serde: missing field `id`"));
        assert!(!is_retryable_error(
            "error in response: status code 403: forbidden"
        ));
    }

    #[test]
    fn test_is_database_lock_error_lowercased() {
        // Caller is responsible for lowercasing first.
        assert!(is_database_lock_error_lowercased("database is locked"));
        assert!(is_database_lock_error_lowercased("database is busy"));
        assert!(is_database_lock_error_lowercased(
            "sqlite_busy: snapshot conflict"
        ));
        assert!(is_database_lock_error_lowercased(
            "failed to create result record: database is locked"
        ));
        assert!(!is_database_lock_error_lowercased(
            "error in response: status code 500: internal error"
        ));
        assert!(!is_database_lock_error_lowercased("connection refused"));
        assert!(!is_database_lock_error_lowercased("timeout"));
        // Contract: uppercase input does not match (must be pre-lowercased).
        assert!(!is_database_lock_error_lowercased("Database Is Locked"));
    }

    #[derive(Debug)]
    struct MockError(String);

    impl std::fmt::Display for MockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }

    fn test_config(max_attempts: u32) -> FastRetryConfig {
        FastRetryConfig {
            max_attempts,
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        }
    }

    #[test]
    fn fast_retry_recovers_after_transient_lock_errors() {
        let mut count = 0u32;
        let mut api_call = || -> Result<i32, MockError> {
            count += 1;
            if count < 3 {
                Err(MockError("database is locked".to_string()))
            } else {
                Ok(42)
            }
        };

        let result = fast_retry_for_lock_errors(
            MockError("database is locked".to_string()),
            &mut api_call,
            test_config(6),
        );

        assert_eq!(result.unwrap(), 42);
        assert_eq!(count, 3, "api_call should be invoked until it succeeds");
    }

    #[test]
    fn fast_retry_exhausts_attempts_on_persistent_lock_errors() {
        let mut count = 0u32;
        let mut api_call = || -> Result<(), MockError> {
            count += 1;
            Err(MockError(format!("database is locked (call {count})")))
        };

        let result = fast_retry_for_lock_errors(
            MockError("database is locked (initial)".to_string()),
            &mut api_call,
            test_config(4),
        );

        let err = result.expect_err("all retries should fail");
        assert_eq!(count, 4, "api_call should be invoked max_attempts times");
        assert_eq!(
            err.0, "database is locked (call 4)",
            "should return the most recent error, not the initial one"
        );
    }

    #[test]
    fn fast_retry_stops_when_error_class_changes() {
        let mut count = 0u32;
        let mut api_call = || -> Result<(), MockError> {
            count += 1;
            if count < 3 {
                Err(MockError("database is locked".to_string()))
            } else {
                Err(MockError(
                    "error in response: status code 500: internal error".to_string(),
                ))
            }
        };

        let result = fast_retry_for_lock_errors(
            MockError("database is locked".to_string()),
            &mut api_call,
            test_config(6),
        );

        let err = result.expect_err("non-lock error should propagate");
        assert_eq!(count, 3, "should stop after the first non-lock error");
        assert_eq!(err.0, "error in response: status code 500: internal error");
    }

    #[test]
    fn test_filter_dmesg_by_time() {
        let dmesg = "\
[Tue Nov 25 08:00:00 2025] old message 1
[Tue Nov 25 09:00:00 2025] old message 2
[Tue Nov 25 10:00:00 2025] new message 1
[Tue Nov 25 11:00:00 2025] new message 2
";
        // Create a cutoff at 9:30
        let naive =
            NaiveDateTime::parse_from_str("Tue Nov 25 09:30:00 2025", "%a %b %e %H:%M:%S %Y")
                .unwrap();
        let cutoff = Local.from_local_datetime(&naive).single().unwrap();

        let filtered = filter_dmesg_by_time(dmesg, cutoff);

        // Should only include messages at 10:00 and 11:00
        assert!(filtered.contains("new message 1"));
        assert!(filtered.contains("new message 2"));
        assert!(!filtered.contains("old message 1"));
        assert!(!filtered.contains("old message 2"));
    }
}
