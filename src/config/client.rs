//! Client configuration for the torc CLI

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for the torc CLI client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    /// URL of the torc-server API
    pub api_url: String,

    /// Output format (table or json)
    pub format: String,

    /// Log level (error, warn, info, debug, trace)
    pub log_level: String,

    /// Run command configuration
    pub run: ClientRunConfig,

    /// Slurm scheduler configuration
    pub slurm: ClientSlurmConfig,

    /// HPC profile configuration
    pub hpc: ClientHpcConfig,

    /// Watch command configuration
    pub watch: ClientWatchConfig,

    /// TLS configuration
    pub tls: ClientTlsConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8080/torc-service/v1".to_string(),
            format: "table".to_string(),
            log_level: "info".to_string(),
            run: ClientRunConfig::default(),
            slurm: ClientSlurmConfig::default(),
            hpc: ClientHpcConfig::default(),
            watch: ClientWatchConfig::default(),
            tls: ClientTlsConfig::default(),
        }
    }
}

/// TLS configuration for client connections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientTlsConfig {
    /// Path to a PEM-encoded CA certificate to trust
    pub ca_cert: Option<String>,

    /// Skip certificate verification (for testing only)
    pub insecure: bool,
}

/// Configuration for the `torc run` command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientRunConfig {
    /// Job completion poll interval in seconds
    pub poll_interval: f64,

    /// Maximum number of parallel jobs to run concurrently
    pub max_parallel_jobs: Option<i64>,

    /// Output directory for jobs
    pub output_dir: PathBuf,

    /// Number of CPUs available
    pub num_cpus: Option<i64>,

    /// Memory in GB
    pub memory_gb: Option<f64>,

    /// Number of GPUs available
    pub num_gpus: Option<i64>,
}

impl Default for ClientRunConfig {
    fn default() -> Self {
        Self {
            poll_interval: 5.0,
            max_parallel_jobs: None,
            output_dir: PathBuf::from("output"),
            num_cpus: None,
            memory_gb: None,
            num_gpus: None,
        }
    }
}

/// Configuration for Slurm scheduler integration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientSlurmConfig {
    /// Poll interval in seconds for Slurm job runners
    pub poll_interval: i32,

    /// Keep submission scripts after job submission (useful for debugging)
    pub keep_submission_scripts: bool,

    /// If true, only claim jobs that match the scheduler_id of the worker.
    /// If false (default), jobs with a scheduler_id mismatch will be claimed
    /// if no matching jobs are available.
    pub strict_scheduler_match: bool,
}

impl Default for ClientSlurmConfig {
    fn default() -> Self {
        Self {
            poll_interval: 30,
            keep_submission_scripts: false,
            strict_scheduler_match: false,
        }
    }
}

/// Configuration for the `torc watch` command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientWatchConfig {
    /// Poll interval in seconds for checking workflow status
    pub poll_interval: u64,

    /// Maximum recovery attempts per job
    pub max_retries: u32,

    /// Cooldown period between retries in seconds
    pub retry_cooldown: u64,

    /// Claude model to use for diagnosis
    pub model: String,

    /// Path to failure pattern cache database
    pub cache_path: Option<PathBuf>,

    /// Rate limit: max API calls per minute
    pub rate_limit_per_minute: u32,

    /// Path to audit log file
    pub audit_log_path: Option<PathBuf>,

    /// Anthropic API key (fallback if ANTHROPIC_API_KEY env var not set)
    pub api_key: Option<String>,
}

impl Default for ClientWatchConfig {
    fn default() -> Self {
        Self {
            poll_interval: 30,
            max_retries: 3,
            retry_cooldown: 60,
            model: "claude-sonnet-4-20250514".to_string(),
            cache_path: None,
            rate_limit_per_minute: 10,
            audit_log_path: None,
            api_key: None,
        }
    }
}

/// Configuration for HPC profiles
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientHpcConfig {
    /// Default account to use for HPC jobs
    pub default_account: Option<String>,

    /// Profile overrides - allows customizing built-in profiles
    /// Key is the profile name (e.g., "kestrel")
    pub profile_overrides: HashMap<String, HpcProfileOverride>,

    /// Custom profiles defined by the user
    pub custom_profiles: HashMap<String, HpcProfileConfig>,
}

/// Override settings for a built-in HPC profile
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HpcProfileOverride {
    /// Override the default account for this profile
    pub default_account: Option<String>,
}

/// Configuration for a custom HPC profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpcProfileConfig {
    /// Display name for the profile
    pub display_name: String,

    /// Description of the HPC system
    #[serde(default)]
    pub description: String,

    /// Detection via environment variable (name=value)
    #[serde(default)]
    pub detect_env_var: Option<String>,

    /// Detection via hostname pattern (regex)
    #[serde(default)]
    pub detect_hostname: Option<String>,

    /// Default account for this profile
    #[serde(default)]
    pub default_account: Option<String>,

    /// Charge factor for CPU jobs
    #[serde(default = "default_charge_factor")]
    pub charge_factor_cpu: f64,

    /// Charge factor for GPU jobs
    #[serde(default = "default_charge_factor_gpu")]
    pub charge_factor_gpu: f64,

    /// Partition configurations
    #[serde(default)]
    pub partitions: Vec<HpcPartitionConfig>,
}

fn default_charge_factor() -> f64 {
    1.0
}

fn default_charge_factor_gpu() -> f64 {
    10.0
}

/// Configuration for an HPC partition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpcPartitionConfig {
    /// Partition name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// CPUs per node
    pub cpus_per_node: u32,

    /// Memory per node in MB
    pub memory_mb: u64,

    /// Maximum wall time in seconds
    pub max_walltime_secs: u64,

    /// GPUs per node (if any)
    #[serde(default)]
    pub gpus_per_node: Option<u32>,

    /// GPU type (e.g., "h100", "a100")
    #[serde(default)]
    pub gpu_type: Option<String>,

    /// GPU memory in GB
    #[serde(default)]
    pub gpu_memory_gb: Option<u32>,

    /// Whether the partition supports shared access
    #[serde(default)]
    pub shared: bool,

    /// Whether partition must be explicitly requested
    #[serde(default)]
    pub requires_explicit_request: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_defaults() {
        let config = ClientConfig::default();
        assert_eq!(
            config.api_url,
            "http://localhost:8080/torc-service/v1".to_string()
        );
        assert_eq!(config.format, "table");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_run_config_defaults() {
        let config = ClientRunConfig::default();
        assert_eq!(config.poll_interval, 5.0);
        assert!(config.max_parallel_jobs.is_none());
        assert_eq!(config.output_dir, PathBuf::from("output"));
    }
}
