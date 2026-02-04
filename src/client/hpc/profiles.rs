//! HPC profile definitions for known HPC systems
//!
//! This module provides data structures for defining HPC system profiles,
//! including partition configurations, resource limits, and auto-detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Duration;

/// How to detect if we're running on a particular HPC system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HpcDetection {
    /// Detect by environment variable value
    EnvVar {
        /// Environment variable name
        name: String,
        /// Expected value
        value: String,
    },
    /// Detect by hostname pattern (regex)
    HostnamePattern {
        /// Regex pattern to match hostname
        pattern: String,
    },
    /// Detect by existence of a file
    FileExists {
        /// Path to check
        path: String,
    },
}

impl HpcDetection {
    /// Check if this detection method matches the current environment
    pub fn matches(&self) -> bool {
        match self {
            HpcDetection::EnvVar { name, value } => {
                env::var(name).map(|v| v == *value).unwrap_or(false)
            }
            HpcDetection::HostnamePattern { pattern } => {
                if let Ok(hostname) = hostname::get()
                    && let Some(hostname_str) = hostname.to_str()
                    && let Ok(re) = regex::Regex::new(pattern)
                {
                    return re.is_match(hostname_str);
                }
                false
            }
            HpcDetection::FileExists { path } => std::path::Path::new(path).exists(),
        }
    }
}

/// A partition on an HPC system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpcPartition {
    /// Partition name (as used with --partition)
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Number of CPUs per node
    pub cpus_per_node: u32,

    /// Memory per node in MB
    pub memory_mb: u64,

    /// Maximum wall time in seconds
    pub max_walltime_secs: u64,

    /// Total nodes in partition (optional)
    #[serde(default)]
    pub max_nodes: Option<u32>,

    /// Maximum nodes per user (optional)
    #[serde(default)]
    pub max_nodes_per_user: Option<u32>,

    /// Minimum nodes per job (e.g., for high-bandwidth partitions)
    #[serde(default)]
    pub min_nodes: Option<u32>,

    /// Number of GPUs per node (if any)
    #[serde(default)]
    pub gpus_per_node: Option<u32>,

    /// GPU type (e.g., "h100", "a100")
    #[serde(default)]
    pub gpu_type: Option<String>,

    /// GPU memory in GB per GPU
    #[serde(default)]
    pub gpu_memory_gb: Option<u32>,

    /// Local disk storage in GB (if any)
    #[serde(default)]
    pub local_disk_gb: Option<u64>,

    /// Whether the partition supports shared node access
    #[serde(default)]
    pub shared: bool,

    /// Whether partition must be explicitly requested (vs auto-routed)
    #[serde(default)]
    pub requires_explicit_request: bool,

    /// Default QOS for this partition
    #[serde(default)]
    pub default_qos: Option<String>,

    /// Additional constraints or features
    #[serde(default)]
    pub features: Vec<String>,
}

impl HpcPartition {
    /// Get the maximum wall time as a Duration
    pub fn max_walltime(&self) -> Duration {
        Duration::from_secs(self.max_walltime_secs)
    }

    /// Format wall time as HH:MM:SS string
    pub fn max_walltime_str(&self) -> String {
        let secs = self.max_walltime_secs;
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let s = secs % 60;

        if hours >= 24 {
            let days = hours / 24;
            let h = hours % 24;
            format!("{}-{:02}:{:02}:{:02}", days, h, mins, s)
        } else {
            format!("{:02}:{:02}:{:02}", hours, mins, s)
        }
    }

    /// Get memory in GB
    pub fn memory_gb(&self) -> f64 {
        self.memory_mb as f64 / 1024.0
    }

    /// Check if this partition can satisfy the given requirements
    pub fn can_satisfy(
        &self,
        cpus: u32,
        memory_mb: u64,
        walltime_secs: u64,
        gpus: Option<u32>,
    ) -> bool {
        // Check CPU
        if cpus > self.cpus_per_node {
            return false;
        }

        // Check memory
        if memory_mb > self.memory_mb {
            return false;
        }

        // Check wall time
        if walltime_secs > self.max_walltime_secs {
            return false;
        }

        // Check GPUs if requested
        if let Some(requested_gpus) = gpus
            && requested_gpus > 0
        {
            match self.gpus_per_node {
                Some(available) if requested_gpus <= available => {}
                _ => return false,
            }
        }

        true
    }
}

/// An HPC system profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpcProfile {
    /// System identifier (e.g., "kestrel", "perlmutter")
    pub name: String,

    /// Human-readable display name
    pub display_name: String,

    /// Optional description
    #[serde(default)]
    pub description: String,

    /// Detection methods (any match triggers detection)
    pub detection: Vec<HpcDetection>,

    /// Default account (can be overridden in config)
    #[serde(default)]
    pub default_account: Option<String>,

    /// Available partitions
    pub partitions: Vec<HpcPartition>,

    /// Charge factor for CPU jobs (AU per node-hour)
    #[serde(default = "default_charge_factor")]
    pub charge_factor_cpu: f64,

    /// Charge factor for GPU jobs (AU per node-hour)
    #[serde(default = "default_charge_factor_gpu")]
    pub charge_factor_gpu: f64,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_charge_factor() -> f64 {
    1.0
}

fn default_charge_factor_gpu() -> f64 {
    10.0
}

impl HpcProfile {
    /// Check if this profile matches the current environment
    pub fn detect(&self) -> bool {
        self.detection.iter().any(|d| d.matches())
    }

    /// Get a partition by name
    pub fn get_partition(&self, name: &str) -> Option<&HpcPartition> {
        self.partitions.iter().find(|p| p.name == name)
    }

    /// Find partitions that can satisfy the given requirements
    pub fn find_matching_partitions(
        &self,
        cpus: u32,
        memory_mb: u64,
        walltime_secs: u64,
        gpus: Option<u32>,
    ) -> Vec<&HpcPartition> {
        self.partitions
            .iter()
            .filter(|p| p.can_satisfy(cpus, memory_mb, walltime_secs, gpus))
            .collect()
    }

    /// Find the best partition for the given requirements
    /// Prefers: GPU partitions if GPUs requested, shared if small job, otherwise standard
    /// Avoids: debug partitions (they're for development, not production)
    pub fn find_best_partition(
        &self,
        cpus: u32,
        memory_mb: u64,
        walltime_secs: u64,
        gpus: Option<u32>,
    ) -> Option<&HpcPartition> {
        let matching = self.find_matching_partitions(cpus, memory_mb, walltime_secs, gpus);

        if matching.is_empty() {
            return None;
        }

        // Filter out debug partitions for automatic selection
        let non_debug: Vec<_> = matching
            .iter()
            .filter(|p| !p.name.to_lowercase().contains("debug"))
            .copied()
            .collect();

        // Use non-debug partitions if available, otherwise fall back to all matching
        let candidates = if non_debug.is_empty() {
            &matching
        } else {
            &non_debug
        };

        // If GPUs requested, prefer GPU partitions that don't require explicit request
        if gpus.map(|g| g > 0).unwrap_or(false) {
            // First try auto-routed GPU partitions
            if let Some(gpu_partition) = candidates
                .iter()
                .find(|p| p.gpus_per_node.is_some() && !p.requires_explicit_request)
            {
                return Some(gpu_partition);
            }
            // Fall back to any GPU partition
            if let Some(gpu_partition) = candidates.iter().find(|p| p.gpus_per_node.is_some()) {
                return Some(gpu_partition);
            }
        }

        // For small jobs, prefer shared partitions that don't require explicit request
        let is_small_job = cpus <= 26 && memory_mb <= 60_000; // ~1/4 of standard node
        if is_small_job {
            // First try auto-routed shared partitions (prefer non-GPU)
            if let Some(shared_partition) = candidates
                .iter()
                .find(|p| p.shared && !p.requires_explicit_request && p.gpus_per_node.is_none())
            {
                return Some(shared_partition);
            }
        }

        // If GPUs not requested, prefer non-GPU partitions that don't require explicit request
        if gpus.map(|g| g == 0).unwrap_or(true)
            && let Some(cpu_partition) = candidates
                .iter()
                .find(|p| !p.requires_explicit_request && p.gpus_per_node.is_none())
        {
            return Some(cpu_partition);
        }

        // Prefer partitions that don't require explicit request (auto-routed)
        if let Some(auto_partition) = candidates.iter().find(|p| !p.requires_explicit_request) {
            return Some(auto_partition);
        }

        // Return first matching from candidates
        candidates.first().copied()
    }

    /// Get all GPU partitions
    pub fn gpu_partitions(&self) -> Vec<&HpcPartition> {
        self.partitions
            .iter()
            .filter(|p| p.gpus_per_node.is_some())
            .collect()
    }

    /// Get all CPU-only partitions
    pub fn cpu_partitions(&self) -> Vec<&HpcPartition> {
        self.partitions
            .iter()
            .filter(|p| p.gpus_per_node.is_none())
            .collect()
    }
}

/// Registry of known HPC profiles
#[derive(Debug, Clone, Default)]
pub struct HpcProfileRegistry {
    profiles: Vec<HpcProfile>,
}

impl HpcProfileRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// Create a registry with all built-in profiles
    pub fn with_builtin_profiles() -> Self {
        let mut registry = Self::new();
        registry.register(super::dane::dane_profile());
        registry.register(super::kestrel::kestrel_profile());
        registry
    }

    /// Register a profile
    pub fn register(&mut self, profile: HpcProfile) {
        // Remove existing profile with same name
        self.profiles.retain(|p| p.name != profile.name);
        self.profiles.push(profile);
    }

    /// Get all registered profiles
    pub fn profiles(&self) -> &[HpcProfile] {
        &self.profiles
    }

    /// Get a profile by name
    pub fn get(&self, name: &str) -> Option<&HpcProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// Detect the current HPC system
    pub fn detect(&self) -> Option<&HpcProfile> {
        self.profiles.iter().find(|p| p.detect())
    }

    /// Get profile names
    pub fn names(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.name.as_str()).collect()
    }
}
