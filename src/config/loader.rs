//! Configuration loader with layered configuration support
//!
//! Loads configuration from multiple sources with the following priority:
//! 1. Built-in defaults (lowest)
//! 2. System config (`/etc/torc/config.toml`)
//! 3. User config (`~/.config/torc/config.toml`)
//! 4. Project-local config (`./torc.toml`)
//! 5. Environment variables (`TORC_*`)
//! 6. CLI arguments (highest, handled externally)

use config::{Config, ConfigError, Environment, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{ClientConfig, DashConfig, ServerConfig};

/// Complete Torc configuration containing all component settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TorcConfig {
    /// Client (CLI) configuration
    pub client: ClientConfig,

    /// Server configuration
    pub server: ServerConfig,

    /// Dashboard configuration
    pub dash: DashConfig,
}

/// Configuration file paths and their sources
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// System-wide config path
    pub system: PathBuf,

    /// User config path
    pub user: Option<PathBuf>,

    /// Project-local config path
    pub local: PathBuf,
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigPaths {
    /// Create new config paths with platform-appropriate defaults
    pub fn new() -> Self {
        let user = dirs::config_dir().map(|p| p.join("torc").join("config.toml"));

        Self {
            system: PathBuf::from("/etc/torc/config.toml"),
            user,
            local: PathBuf::from("torc.toml"),
        }
    }

    /// Get all paths that exist
    pub fn existing_paths(&self) -> Vec<&PathBuf> {
        let mut paths = Vec::new();
        if self.system.exists() {
            paths.push(&self.system);
        }
        if let Some(user) = &self.user
            && user.exists()
        {
            paths.push(user);
        }
        if self.local.exists() {
            paths.push(&self.local);
        }
        paths
    }

    /// Get the user config directory (creates parent dirs if needed)
    pub fn user_config_dir(&self) -> Option<PathBuf> {
        self.user
            .as_ref()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    }
}

impl TorcConfig {
    /// Load configuration from all sources
    ///
    /// Sources are loaded in this order (later sources override earlier):
    /// 1. Built-in defaults
    /// 2. System config (`/etc/torc/config.toml`)
    /// 3. User config (`~/.config/torc/config.toml`)
    /// 4. Project-local config (`./torc.toml`)
    /// 5. Environment variables (`TORC_*`)
    pub fn load() -> Result<Self, ConfigError> {
        let paths = ConfigPaths::new();
        Self::load_with_paths(&paths)
    }

    /// Load configuration with custom paths
    pub fn load_with_paths(paths: &ConfigPaths) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        // 1. System config (optional)
        if paths.system.exists() {
            builder = builder.add_source(
                File::from(paths.system.clone())
                    .format(FileFormat::Toml)
                    .required(false),
            );
        }

        // 2. User config (optional)
        if let Some(user_path) = &paths.user
            && user_path.exists()
        {
            builder = builder.add_source(
                File::from(user_path.clone())
                    .format(FileFormat::Toml)
                    .required(false),
            );
        }

        // 3. Project-local config (optional)
        if paths.local.exists() {
            builder = builder.add_source(
                File::from(paths.local.clone())
                    .format(FileFormat::Toml)
                    .required(false),
            );
        }

        // 4. Environment variables
        // Use double underscore for nesting to avoid conflicts with field names:
        //   TORC_CLIENT__API_URL -> client.api_url
        //   TORC_SERVER__PORT -> server.port
        //   TORC_DASH__HOST -> dash.host
        // Single underscore is preserved in field names.
        builder = builder.add_source(
            Environment::with_prefix("TORC")
                .prefix_separator("_")
                .separator("__") // Double underscore for nesting
                .try_parsing(true)
                .keep_prefix(false),
        );

        // Build and deserialize with defaults for missing fields
        let config = builder.build()?;
        config.try_deserialize().or_else(|_| Ok(Self::default()))
    }

    /// Load configuration from specific file paths
    pub fn load_from_files(paths: &[PathBuf]) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        for path in paths {
            if path.exists() {
                builder = builder.add_source(
                    File::from(path.clone())
                        .format(FileFormat::Toml)
                        .required(false),
                );
            }
        }

        // Add environment variables
        builder = builder.add_source(
            Environment::with_prefix("TORC")
                .separator("_")
                .try_parsing(true),
        );

        let config = builder.build()?;
        config.try_deserialize().or_else(|_| Ok(Self::default()))
    }

    /// Generate a default configuration file content
    pub fn generate_default_config() -> String {
        r#"# Torc Configuration File
# Place in ~/.config/torc/config.toml (user) or /etc/torc/config.toml (system)
# Or ./torc.toml for project-specific settings

[client]
# URL of the torc-server API
api_url = "http://localhost:8080/torc-service/v1"

# Output format: "table" or "json"
format = "table"

# Log level: error, warn, info, debug, trace
log_level = "info"

[client.run]
# Job completion poll interval in seconds
poll_interval = 5.0

# Output directory for job logs and artifacts
output_dir = "torc_output"

# Maximum number of parallel jobs (optional, uses resource-based if not set)
# max_parallel_jobs = 4

# Resource limits for local execution (optional)
# num_cpus = 8
# memory_gb = 32.0
# num_gpus = 1

[client.tls]
# Path to a PEM-encoded CA certificate to trust
# ca_cert = "/path/to/ca.pem"

# Skip certificate verification (for testing only)
insecure = false

[client.slurm]
# Poll interval in seconds for Slurm job runners
poll_interval = 30

# Keep submission scripts after job submission (useful for debugging)
keep_submission_scripts = false

[client.hpc]
# Default account to use for HPC jobs (applies to all profiles)
# default_account = "my_project"

# Override settings for built-in profiles
# [client.hpc.profile_overrides.kestrel]
# default_account = "my_kestrel_account"

# Define custom HPC profiles
# [[client.hpc.custom_profiles]]
# name = "my_cluster"
# display_name = "My Custom Cluster"
# description = "Our department's HPC cluster"
# detect_env_var = "MY_CLUSTER=prod"
# default_account = "dept_account"
# charge_factor_cpu = 1.0
# charge_factor_gpu = 10.0
#
# [[client.hpc.custom_profiles.my_cluster.partitions]]
# name = "compute"
# cpus_per_node = 64
# memory_mb = 256000
# max_walltime_secs = 172800  # 2 days

[server]
# Hostname/IP to bind to
url = "localhost"

# Port to listen on
port = 8080

# Number of worker threads
threads = 1

# Use HTTPS
https = false

# Path to SQLite database (optional, uses DATABASE_URL env var if not set)
# database = "/path/to/torc.db"

# Path to htpasswd file for authentication (optional)
# auth_file = "/path/to/htpasswd"

# Require authentication for all requests
require_auth = false

# Interval for background job completion processing (seconds)
completion_check_interval_secs = 30.0

# Log level: error, warn, info, debug, trace
log_level = "info"

[server.logging]
# Directory for log files (enables file logging)
# log_dir = "/var/log/torc"

# Use JSON format for logs
json_logs = false

# Admin users (can create and manage access groups)
# These users are automatically added to the system "admin" group on startup
# admin_users = ["alice", "bob"]

[dash]
# Host to bind to
host = "127.0.0.1"

# Port to listen on
port = 8090

# URL of the torc-server API
api_url = "http://localhost:8080/torc-service/v1"

# Path to torc CLI binary
torc_bin = "torc"

# Path to torc-server binary
torc_server_bin = "torc-server"

# Run in standalone mode (auto-start torc-server)
standalone = false

# Server port for standalone mode (0 = auto-detect)
server_port = 0

# Job completion check interval for standalone mode (seconds)
completion_check_interval_secs = 5
"#
        .to_string()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate client config
        if !["table", "json"].contains(&self.client.format.as_str()) {
            errors.push(format!(
                "client.format must be 'table' or 'json', got '{}'",
                self.client.format
            ));
        }

        if self.client.run.poll_interval <= 0.0 {
            errors.push("client.run.poll_interval must be positive".to_string());
        }

        // Validate server config
        if self.server.port == 0 {
            errors.push("server.port cannot be 0".to_string());
        }

        if self.server.threads == 0 {
            errors.push("server.threads must be at least 1".to_string());
        }

        if self.server.completion_check_interval_secs <= 0.0 {
            errors.push("server.completion_check_interval_secs must be positive".to_string());
        }

        // Validate dash config
        if self.dash.port == 0 {
            errors.push("dash.port cannot be 0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get the configuration paths
    pub fn paths() -> ConfigPaths {
        ConfigPaths::new()
    }

    /// Convert to TOML string
    pub fn to_toml(&self) -> Result<String, ::toml::ser::Error> {
        ::toml::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TorcConfig::default();
        assert_eq!(
            config.client.api_url,
            "http://localhost:8080/torc-service/v1"
        );
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.dash.port, 8090);
    }

    #[test]
    fn test_config_paths() {
        let paths = ConfigPaths::new();
        assert_eq!(paths.system, PathBuf::from("/etc/torc/config.toml"));
        assert!(paths.user.is_some());
        assert_eq!(paths.local, PathBuf::from("torc.toml"));
    }

    #[test]
    fn test_validate_valid_config() {
        let config = TorcConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_format() {
        let mut config = TorcConfig::default();
        config.client.format = "invalid".to_string();
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("format")));
    }

    #[test]
    fn test_generate_default_config() {
        let config = TorcConfig::generate_default_config();
        assert!(config.contains("[client]"));
        assert!(config.contains("[server]"));
        assert!(config.contains("[dash]"));
        assert!(config.contains("api_url"));
    }
}
