use clap::Subcommand;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::client::apis::Error as ApiError;
use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
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
    let start = Instant::now();
    let deadline = timeout_secs.map(|limit| start + Duration::from_secs(limit));

    let initial_task = get_task_with_retry(config, task_id, deadline);

    if is_task_terminal(&initial_task) {
        print_task_result(&initial_task, format);
        exit_for_task(&initial_task);
    }

    let (tx, rx) = mpsc::channel::<()>();
    let sse_config = config.clone();
    let workflow_id = initial_task.workflow_id;

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
            match conn.next_event() {
                Ok(Some(event)) => {
                    if event.event_type == "task_completed"
                        && event.data.get("task_id").and_then(|v| v.as_i64()) == Some(task_id)
                    {
                        let _ = tx.send(());
                        return;
                    }
                }
                Ok(None) => {
                    // Connection closed
                    return;
                }
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
            eprintln!("Timeout waiting for task {}", task_id);
            std::process::exit(1);
        }

        // If SSE notifies us, refresh the task state immediately.
        if rx.try_recv().is_ok() {
            match default_api::get_task(config, task_id) {
                Ok(task) => {
                    backoff.reset();
                    print_task_result(&task, format);
                    exit_for_task(&task);
                }
                Err(e) if is_retryable_get_task_error(&e) => {
                    let delay = backoff.next_delay();
                    eprintln!(
                        "Warning: transient error getting task {} (SSE wake): {}; retrying in {:?}",
                        task_id, e, delay
                    );
                    let _ = rx.recv_timeout(delay);
                    continue;
                }
                Err(e) => {
                    eprintln!("Error getting task {}: {}", task_id, e);
                    std::process::exit(1);
                }
            }
        }

        match default_api::get_task(config, task_id) {
            Ok(task) => {
                backoff.reset();
                if is_task_terminal(&task) {
                    print_task_result(&task, format);
                    exit_for_task(&task);
                }
            }
            Err(e) if is_retryable_get_task_error(&e) => {
                let delay = backoff.next_delay();
                eprintln!(
                    "Warning: transient error getting task {}: {}; retrying in {:?}",
                    task_id, e, delay
                );
                let _ = rx.recv_timeout(delay);
                continue;
            }
            Err(e) => {
                eprintln!("Error getting task {}: {}", task_id, e);
                std::process::exit(1);
            }
        }

        let wait = Duration::from_secs(poll_interval_secs.max(1));
        // If we can receive SSE completion within the interval, we wake early.
        let _ = rx.recv_timeout(wait);
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
) -> TaskModel {
    let mut backoff = Backoff::new(Duration::from_millis(200), Duration::from_secs(5));
    loop {
        if let Some(deadline) = deadline
            && Instant::now() >= deadline
        {
            eprintln!("Timeout waiting for task {}", task_id);
            std::process::exit(1);
        }

        match default_api::get_task(config, task_id) {
            Ok(task) => return task,
            Err(e) if is_retryable_get_task_error(&e) => {
                let delay = backoff.next_delay();
                eprintln!(
                    "Warning: transient error getting task {}: {}; retrying in {:?}",
                    task_id, e, delay
                );
                thread::sleep(delay);
            }
            Err(e) => {
                eprintln!("Error getting task {}: {}", task_id, e);
                std::process::exit(1);
            }
        }
    }
}

fn is_retryable_get_task_error(err: &ApiError<default_api::GetTaskError>) -> bool {
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
