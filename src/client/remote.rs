//! Remote worker execution module.
//!
//! This module provides functionality for running torc workers on remote machines via SSH.
//! It enables distributed workflow execution without requiring a scheduler like Slurm.
//!
//! # Usage
//!
//! Create a worker file listing remote machines:
//!
//! ```text
//! # workers.txt
//! worker1.example.com
//! alice@worker2.example.com:2222
//! 192.168.1.10
//! ```
//!
//! Then run workers remotely:
//!
//! ```bash
//! torc remote run workers.txt <workflow-id>
//! torc remote status workers.txt <workflow-id>
//! torc remote stop workers.txt <workflow-id>
//! torc remote collect-logs workers.txt <workflow-id>
//! ```

pub mod shell;
pub mod ssh;
pub mod types;
pub mod worker_file;

pub use shell::{RemoteShell, detect_remote_shell};
pub use ssh::{
    check_all_connectivity, check_ssh_connectivity, get_remote_torc_version, parallel_execute,
    scp_download, ssh_execute, ssh_execute_capture, ssh_execute_checked, verify_all_versions,
    verify_version,
};
pub use types::{RemoteOperationResult, RemoteWorkerState, WorkerEntry};
pub use worker_file::{parse_worker_content, parse_worker_file};
