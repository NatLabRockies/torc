//! Logging configuration for torc-server
//!
//! This module provides structured logging using the `tracing` ecosystem with support for:
//! - Console output
//! - File output with automatic size-based rotation
//! - Configurable log levels
//! - Both human-readable and JSON formats

use anyhow::Result;
use file_rotate::{ContentLimit, FileRotate, compression::Compression, suffix::AppendCount};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Size-based rotating file writer for logging
/// Rotates when file reaches 10 MiB and keeps 5 files
struct RotatingWriter {
    file_rotate: Arc<Mutex<FileRotate<AppendCount>>>,
}

impl RotatingWriter {
    fn new(log_dir: &Path) -> Result<Self> {
        let log_path = log_dir.join("torc-server.log");

        let file_rotate = FileRotate::new(
            log_path,
            AppendCount::new(5),                   // Keep 5 rotated files
            ContentLimit::Bytes(10 * 1024 * 1024), // 10 MiB
            Compression::None,
            #[cfg(unix)]
            None,
        );

        Ok(Self {
            file_rotate: Arc::new(Mutex::new(file_rotate)),
        })
    }
}

impl Write for RotatingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file_rotate.lock().unwrap();
        file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file_rotate.lock().unwrap();
        file.flush()
    }
}

impl Clone for RotatingWriter {
    fn clone(&self) -> Self {
        Self {
            file_rotate: Arc::clone(&self.file_rotate),
        }
    }
}

/// Initialize the logging system
///
/// # Arguments
///
/// * `log_dir` - Optional directory for log files. If None, only console logging is enabled
/// * `log_level` - Log level filter (e.g., "info", "debug", "warn")
/// * `json_format` - If true, use JSON format for file logs (useful for log aggregation)
///
/// # Examples
///
/// ```ignore
/// // Console only
/// init_logging(None, "info", false)?;
///
/// // Console + size-based rotating file logs
/// init_logging(Some(Path::new("/var/log/torc")), "debug", false)?;
///
/// // Console + JSON file logs for structured logging
/// init_logging(Some(Path::new("/var/log/torc")), "info", true)?;
/// ```
pub fn init_logging(
    log_dir: Option<&Path>,
    log_level: &str,
    json_format: bool,
) -> Result<Option<WorkerGuard>> {
    // Create environment filter from log level
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // If RUST_LOG is not set, use the provided log_level
        EnvFilter::new(log_level)
    });

    if let Some(dir) = log_dir {
        // Ensure log directory exists
        std::fs::create_dir_all(dir)?;

        // Create rotating file writer (10 MiB per file, keep 5 files)
        let file_writer = RotatingWriter::new(dir)?;

        // Use non-blocking writer to avoid blocking the server on log writes
        let (non_blocking, guard) = tracing_appender::non_blocking(file_writer);

        if json_format {
            // JSON format for file, human-readable for console
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(std::io::stderr)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_line_number(true),
                )
                .with(
                    fmt::layer()
                        .json()
                        .with_writer(non_blocking)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_line_number(true),
                )
                .init();
        } else {
            // Human-readable format for both console and file
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(std::io::stderr)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_line_number(true),
                )
                .with(
                    fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false) // No ANSI colors in file
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_line_number(true),
                )
                .init();
        }

        tracing::info!(
            log_dir = %dir.display(),
            log_level = %log_level,
            json_format = %json_format,
            max_file_size = "10 MiB",
            max_files = 5,
            "Logging initialized with file output and size-based rotation"
        );

        // Return the guard so the caller keeps it alive for proper log flushing
        Ok(Some(guard))
    } else {
        // Console-only logging
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_line_number(true),
            )
            .init();

        tracing::info!(
            log_level = %log_level,
            "Logging initialized (console only)"
        );

        Ok(None)
    }
}

/// Create a rotating file writer for use with tracing timing instrumentation
///
/// This is a helper function for the timing instrumentation code in main.rs
/// that needs to set up file rotation manually.
pub fn create_rotating_writer(log_dir: &Path) -> Result<impl Write + Clone + Send + 'static> {
    std::fs::create_dir_all(log_dir)?;
    RotatingWriter::new(log_dir)
}
