use clap::Subcommand;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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
    let initial_task = match default_api::get_task(config, task_id) {
        Ok(task) => task,
        Err(e) => {
            eprintln!("Error getting task {}: {}", task_id, e);
            std::process::exit(1);
        }
    };

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

    let start = Instant::now();
    loop {
        if let Some(limit) = timeout_secs
            && start.elapsed() >= Duration::from_secs(limit)
        {
            eprintln!("Timeout waiting for task {}", task_id);
            std::process::exit(1);
        }

        // If SSE notifies us, refresh the task state immediately.
        if rx.try_recv().is_ok() {
            match default_api::get_task(config, task_id) {
                Ok(task) => {
                    print_task_result(&task, format);
                    exit_for_task(&task);
                }
                Err(e) => {
                    eprintln!("Error getting task {}: {}", task_id, e);
                    std::process::exit(1);
                }
            }
        }

        match default_api::get_task(config, task_id) {
            Ok(task) => {
                if is_task_terminal(&task) {
                    print_task_result(&task, format);
                    exit_for_task(&task);
                }
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
