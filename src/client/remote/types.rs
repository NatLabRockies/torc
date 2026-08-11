//! Types for remote worker execution.

use std::fmt;

/// Represents a parsed entry from the worker file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerEntry {
    /// Original line from file (for error messages)
    pub(crate) original: String,
    /// Username (optional, defaults to current user if not specified)
    pub user: Option<String>,
    /// Hostname or IP address
    pub host: String,
    /// SSH port (optional, defaults to 22)
    pub port: Option<u16>,
}

impl WorkerEntry {
    /// Create a new WorkerEntry with just a hostname.
    pub fn new(host: impl Into<String>) -> Self {
        let host = host.into();
        Self {
            original: host.clone(),
            user: None,
            host,
            port: None,
        }
    }

    /// Set the user for this entry.
    fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the port for this entry.
    fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Returns the SSH target string: [user@]host
    pub(crate) fn ssh_target(&self) -> String {
        match &self.user {
            Some(user) => format!("{}@{}", user, self.host),
            None => self.host.clone(),
        }
    }

    /// Returns the display name for this worker (used in output).
    pub(crate) fn display_name(&self) -> &str {
        &self.host
    }
}

impl fmt::Display for WorkerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.user, self.port) {
            (Some(user), Some(port)) => write!(f, "{}@{}:{}", user, self.host, port),
            (Some(user), None) => write!(f, "{}@{}", user, self.host),
            (None, Some(port)) => write!(f, "{}:{}", self.host, port),
            (None, None) => write!(f, "{}", self.host),
        }
    }
}

/// State of a remote worker.
#[derive(Debug, Clone)]
pub enum RemoteWorkerState {
    /// Worker is running
    Running {
        /// Process ID on the remote machine
        pid: u32,
    },
    /// Worker is not running (process exited or never started)
    NotRunning,
    /// Could not determine state (SSH error, etc.)
    Unknown {
        /// Error message describing what went wrong
        error: String,
    },
}

impl fmt::Display for RemoteWorkerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteWorkerState::Running { pid } => write!(f, "Running (PID {})", pid),
            RemoteWorkerState::NotRunning => write!(f, "Not Running"),
            RemoteWorkerState::Unknown { error } => write!(f, "Unknown: {}", error),
        }
    }
}

/// Result of a remote operation on a single worker.
#[derive(Debug)]
pub struct RemoteOperationResult {
    /// The worker this result is for
    pub(crate) worker: WorkerEntry,
    /// Whether the operation succeeded
    pub(crate) success: bool,
    /// Human-readable message about the result
    pub(crate) message: String,
}

impl RemoteOperationResult {
    /// Create a successful result.
    pub(crate) fn success(worker: WorkerEntry, message: impl Into<String>) -> Self {
        Self {
            worker,
            success: true,
            message: message.into(),
        }
    }

    /// Create a failed result.
    pub(crate) fn failure(worker: WorkerEntry, message: impl Into<String>) -> Self {
        Self {
            worker,
            success: false,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_entry_simple() {
        let entry = WorkerEntry::new("example.com");
        assert_eq!(entry.host, "example.com");
        assert_eq!(entry.user, None);
        assert_eq!(entry.port, None);
        assert_eq!(entry.ssh_target(), "example.com");
        assert_eq!(entry.to_string(), "example.com");
    }

    #[test]
    fn test_worker_entry_with_user() {
        let entry = WorkerEntry::new("example.com").with_user("alice");
        assert_eq!(entry.host, "example.com");
        assert_eq!(entry.user, Some("alice".to_string()));
        assert_eq!(entry.ssh_target(), "alice@example.com");
        assert_eq!(entry.to_string(), "alice@example.com");
    }

    #[test]
    fn test_worker_entry_with_port() {
        let entry = WorkerEntry::new("example.com").with_port(2222);
        assert_eq!(entry.host, "example.com");
        assert_eq!(entry.port, Some(2222));
        assert_eq!(entry.ssh_target(), "example.com");
        assert_eq!(entry.to_string(), "example.com:2222");
    }

    #[test]
    fn test_worker_entry_full() {
        let entry = WorkerEntry::new("example.com")
            .with_user("alice")
            .with_port(2222);
        assert_eq!(entry.ssh_target(), "alice@example.com");
        assert_eq!(entry.to_string(), "alice@example.com:2222");
    }

    #[test]
    fn test_remote_worker_state_display() {
        assert_eq!(
            RemoteWorkerState::Running { pid: 1234 }.to_string(),
            "Running (PID 1234)"
        );
        assert_eq!(RemoteWorkerState::NotRunning.to_string(), "Not Running");
        assert_eq!(
            RemoteWorkerState::Unknown {
                error: "connection failed".to_string()
            }
            .to_string(),
            "Unknown: connection failed"
        );
    }
}
