//! Server configuration for torc-server

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the torc-server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Log level (error, warn, info, debug, trace)
    pub log_level: String,

    /// Whether to use HTTPS
    pub https: bool,

    /// Hostname or IP address to bind to
    #[serde(alias = "url")]
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Number of worker threads
    pub threads: u32,

    /// Path to the SQLite database file
    pub database: Option<String>,

    /// Path to htpasswd file for basic authentication
    pub auth_file: Option<String>,

    /// Require authentication for all requests
    pub require_auth: bool,

    /// TTL in seconds for credential cache (0 to disable).
    /// Caching avoids repeated bcrypt verification for the same credentials.
    pub credential_cache_ttl_secs: u64,

    /// Enforce access control based on workflow ownership and group membership
    pub enforce_access_control: bool,

    /// Interval in seconds for background job completion processing
    pub completion_check_interval_secs: f64,

    /// Logging configuration
    pub logging: ServerLoggingConfig,

    /// List of admin users (members of the system admin group)
    /// These users can create and manage access groups
    pub admin_users: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            https: false,
            host: "0.0.0.0".to_string(),
            port: 8080,
            threads: 1,
            database: None,
            auth_file: None,
            require_auth: false,
            credential_cache_ttl_secs: 60,
            enforce_access_control: false,
            completion_check_interval_secs: 30.0,
            logging: ServerLoggingConfig::default(),
            admin_users: Vec::new(),
        }
    }
}

/// Logging configuration for the server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ServerLoggingConfig {
    /// Directory for log files (enables file logging with rotation)
    pub log_dir: Option<PathBuf>,

    /// Use JSON format for log files
    pub json_logs: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.log_level, "info");
        assert!(!config.https);
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.threads, 1);
        assert!(config.database.is_none());
        assert!(!config.require_auth);
        assert!(!config.enforce_access_control);
        assert_eq!(config.completion_check_interval_secs, 30.0);
    }

    #[test]
    fn test_logging_config_defaults() {
        let config = ServerLoggingConfig::default();
        assert!(config.log_dir.is_none());
        assert!(!config.json_logs);
    }
}
