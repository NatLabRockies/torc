use clap::Subcommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant};

use crate::client::apis::Error as ApiError;
use crate::client::apis::configuration::Configuration;
use crate::client::apis::tasks_api;
use crate::client::sse_client::{SseConnection, SseError};
use crate::models::{EventSeverity, TaskModel, TaskStatus};

#[derive(Subcommand, Debug, Clone)]
pub enum TasksCommands {
    /// Wait for a task to complete.
    ///
    /// This uses the workflow SSE stream to wake early on completion, with periodic
    /// polling as a fallback.
    Wait {
        /// Task ID
        id: i64,
        /// Timeout in seconds (default: wait forever)
        #[arg(long)]
        timeout: Option<u64>,
        /// Poll interval in seconds used as an SSE fallback
        #[arg(long, default_value_t = 10)]
        poll_fallback_interval: u64,
    },
}

pub fn handle_tasks_commands(config: &Configuration, command: &TasksCommands, format: &str) {
    match command {
        TasksCommands::Wait {
            id,
            timeout,
            poll_fallback_interval,
        } => handle_wait(config, *id, *timeout, *poll_fallback_interval, format),
    }
}

fn handle_wait(
    config: &Configuration,
    task_id: i64,
    timeout_secs: Option<u64>,
    poll_interval_secs: u64,
    format: &str,
) {
    match wait_for_task(config, task_id, timeout_secs, poll_interval_secs) {
        Ok(task) => {
            print_task_result(&task, format);
            exit_for_task(&task);
        }
        Err(WaitError::Timeout) => {
            if format == "json" {
                let payload = serde_json::json!({
                    "status": "error",
                    "error": "Timeout",
                    "message": format!("Timeout waiting for task {}", task_id),
                    "task_id": task_id,
                });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            } else {
                eprintln!("Timeout waiting for task {}", task_id);
            }
            std::process::exit(1);
        }
        Err(WaitError::Api(msg)) => {
            if format == "json" {
                let payload = serde_json::json!({
                    "status": "error",
                    "error": "ApiError",
                    "message": format!("Error getting task {}: {}", task_id, msg),
                    "task_id": task_id,
                });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            } else {
                eprintln!("Error getting task {}: {}", task_id, msg);
            }
            std::process::exit(1);
        }
    }
}

/// Wait for an async task to reach a terminal state.
///
/// Uses the workflow SSE stream to wake early on completion, with periodic polling as a
/// fallback. Prints transient retry warnings to stderr but does not exit the process.
pub fn wait_for_task(
    config: &Configuration,
    task_id: i64,
    timeout_secs: Option<u64>,
    poll_interval_secs: u64,
) -> Result<TaskModel, WaitError> {
    let start = Instant::now();
    let deadline = timeout_secs.map(|limit| start + Duration::from_secs(limit));

    let initial_task = get_task_with_retry(config, task_id, deadline)?;

    if is_task_terminal(&initial_task) {
        return Ok(initial_task);
    }

    let (tx, rx) = mpsc::channel::<()>();
    let sse_config = config.clone();
    let workflow_id = initial_task.workflow_id;

    // Shutdown flag so the SSE listener can exit when `wait_for_task` returns. The thread
    // only observes it between events (the inner `read_line` blocks), so this is best-effort:
    // it prevents thread accumulation across many `wait_for_task` calls in a long-lived
    // process, but a thread stuck on a silent connection will persist until the next event
    // or connection close. For CLI users that's fine (the process exits either way).
    let shutdown = Arc::new(AtomicBool::new(false));
    let sse_shutdown = Arc::clone(&shutdown);
    let _guard = ShutdownGuard(Arc::clone(&shutdown));

    thread::spawn(move || {
        let mut conn =
            match SseConnection::connect(&sse_config, workflow_id, Some(EventSeverity::Info)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to connect to SSE stream for workflow {}: {}",
                        workflow_id, e
                    );
                    return;
                }
            };

        loop {
            if sse_shutdown.load(Ordering::Relaxed) {
                return;
            }
            match conn.next_event() {
                Ok(Some(event)) => {
                    if sse_shutdown.load(Ordering::Relaxed) {
                        return;
                    }
                    if event.event_type == "task_completed"
                        && event.data.get("task_id").and_then(|v| v.as_i64()) == Some(task_id)
                    {
                        let _ = tx.send(());
                        return;
                    }
                }
                Ok(None) => return,
                Err(SseError::ConnectionClosed) => return,
                Err(SseError::Io(_)) => return,
                Err(SseError::Request(_)) => return,
                Err(SseError::Parse(_)) => return,
            }
        }
    });

    let mut backoff = Backoff::new(Duration::from_millis(200), Duration::from_secs(5));
    loop {
        if let Some(deadline) = deadline
            && Instant::now() >= deadline
        {
            return Err(WaitError::Timeout);
        }

        // If SSE notifies us, refresh the task state immediately.
        if rx.try_recv().is_ok() {
            match tasks_api::get_task(config, task_id) {
                Ok(task) => {
                    backoff.reset();
                    return Ok(task);
                }
                Err(e) if is_retryable_get_task_error(&e) => {
                    let delay = backoff.next_delay();
                    eprintln!(
                        "Warning: transient error getting task {} (SSE wake): {}; retrying in {:?}",
                        task_id, e, delay
                    );
                    sleep_or_wake(&rx, delay);
                    continue;
                }
                Err(e) => return Err(WaitError::Api(e.to_string())),
            }
        }

        match tasks_api::get_task(config, task_id) {
            Ok(task) => {
                backoff.reset();
                if is_task_terminal(&task) {
                    return Ok(task);
                }
            }
            Err(e) if is_retryable_get_task_error(&e) => {
                let delay = backoff.next_delay();
                eprintln!(
                    "Warning: transient error getting task {}: {}; retrying in {:?}",
                    task_id, e, delay
                );
                sleep_or_wake(&rx, delay);
                continue;
            }
            Err(e) => return Err(WaitError::Api(e.to_string())),
        }

        let wait = Duration::from_secs(poll_interval_secs.max(1));
        // If we can receive SSE completion within the interval, we wake early.
        sleep_or_wake(&rx, wait);
    }
}

/// Block for up to `wait` or until the SSE thread notifies. If the SSE thread has already
/// exited (`Disconnected`), fall back to `thread::sleep` so we don't spin.
fn sleep_or_wake(rx: &mpsc::Receiver<()>, wait: Duration) {
    match rx.recv_timeout(wait) {
        Ok(()) | Err(RecvTimeoutError::Timeout) => {}
        Err(RecvTimeoutError::Disconnected) => thread::sleep(wait),
    }
}

#[derive(Debug)]
pub enum WaitError {
    Timeout,
    Api(String),
}

/// Sets the shutdown flag on drop so the SSE listener thread can exit cleanly when
/// `wait_for_task` returns (either via Ok, Err, or panic).
struct ShutdownGuard(Arc<AtomicBool>);

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
struct Backoff {
    initial: Duration,
    max: Duration,
    current: Duration,
}

impl Backoff {
    fn new(initial: Duration, max: Duration) -> Self {
        Self {
            initial,
            max,
            current: initial,
        }
    }

    fn reset(&mut self) {
        self.current = self.initial;
    }

    fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = std::cmp::min(self.max, self.current.saturating_mul(2));
        delay
    }
}

fn get_task_with_retry(
    config: &Configuration,
    task_id: i64,
    deadline: Option<Instant>,
) -> Result<TaskModel, WaitError> {
    let mut backoff = Backoff::new(Duration::from_millis(200), Duration::from_secs(5));
    loop {
        if let Some(deadline) = deadline
            && Instant::now() >= deadline
        {
            return Err(WaitError::Timeout);
        }

        match tasks_api::get_task(config, task_id) {
            Ok(task) => return Ok(task),
            Err(e) if is_retryable_get_task_error(&e) => {
                let delay = backoff.next_delay();
                eprintln!(
                    "Warning: transient error getting task {}: {}; retrying in {:?}",
                    task_id, e, delay
                );
                thread::sleep(delay);
            }
            Err(e) => return Err(WaitError::Api(e.to_string())),
        }
    }
}

fn is_retryable_get_task_error(err: &ApiError<tasks_api::GetTaskError>) -> bool {
    match err {
        ApiError::Reqwest(_) | ApiError::Io(_) => true,
        ApiError::Serde(_) => false,
        ApiError::ResponseError(resp) => {
            let status = resp.status;
            status.is_server_error()
                || status == reqwest::StatusCode::REQUEST_TIMEOUT
                || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        }
    }
}

fn is_task_terminal(task: &TaskModel) -> bool {
    matches!(task.status, TaskStatus::Succeeded | TaskStatus::Failed)
}

fn exit_for_task(task: &TaskModel) -> ! {
    if task.status == TaskStatus::Succeeded {
        std::process::exit(0);
    }
    std::process::exit(1);
}

fn print_task_result(task: &TaskModel, format: &str) {
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(task).unwrap());
    } else {
        println!("Task {}", task.id);
        println!("  Workflow ID: {}", task.workflow_id);
        println!("  Operation: {}", task.operation);
        println!("  Status: {}", task.status);
        if let Some(err) = &task.error {
            println!("  Error: {}", err);
        }
    }
}
