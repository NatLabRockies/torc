//! LLNL Dane HPC profile
//!
//! Dane is a cluster at Lawrence Livermore National Laboratory (LLNL) featuring:
//! - CPU nodes with 112 cores and ~251GB RAM each
//! - Various partitions for different walltime requirements
//!
//! Detection: Environment variable CLUSTER=dane

use super::profiles::{HpcDetection, HpcPartition, HpcProfile};

/// Create the Dane HPC profile
pub fn dane_profile() -> HpcProfile {
    HpcProfile {
        name: "dane".to_string(),
        display_name: "LLNL Dane".to_string(),
        description: "Lawrence Livermore National Laboratory's Dane cluster".to_string(),
        detection: vec![HpcDetection::EnvVar {
            name: "CLUSTER".to_string(),
            value: "dane".to_string(),
        }],
        default_account: None,
        partitions: dane_partitions(),
        charge_factor_cpu: 1.0,
        charge_factor_gpu: 1.0, // No GPUs on this system
        metadata: [
            (
                "documentation".to_string(),
                "https://hpc.llnl.gov/hardware/compute-platforms/dane".to_string(),
            ),
            ("organization".to_string(), "LLNL".to_string()),
        ]
        .into_iter()
        .collect(),
    }
}

fn dane_partitions() -> Vec<HpcPartition> {
    vec![
        // Debug partition - short walltime for testing
        HpcPartition {
            name: "pdebug".to_string(),
            description: "Debug partition for developing and testing jobs".to_string(),
            cpus_per_node: 112,
            memory_mb: 257_054,
            max_walltime_secs: 3600, // 1 hour
            max_nodes: None,
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["debug".to_string()],
        },
        // Default batch partition (1 day max)
        HpcPartition {
            name: "pbatch".to_string(),
            description: "Default batch partition for standard jobs (max 1 day)".to_string(),
            cpus_per_node: 112,
            memory_mb: 257_054,
            max_walltime_secs: 24 * 3600, // 1 day
            max_nodes: None,
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: false, // Default partition
            default_qos: None,
            features: vec![],
        },
        // CI partition (1 day max)
        HpcPartition {
            name: "pci".to_string(),
            description: "CI partition for continuous integration jobs".to_string(),
            cpus_per_node: 112,
            memory_mb: 257_054,
            max_walltime_secs: 24 * 3600, // 1 day
            max_nodes: None,
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["ci".to_string()],
        },
        // Serial partition - long walltime for serial jobs
        HpcPartition {
            name: "pserial".to_string(),
            description: "Serial partition for long-running jobs (max 7 days)".to_string(),
            cpus_per_node: 112,
            memory_mb: 257_054,
            max_walltime_secs: 7 * 24 * 3600, // 7 days
            max_nodes: None,
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["serial".to_string()],
        },
        // Jupyter partition
        HpcPartition {
            name: "pjupyter".to_string(),
            description: "Partition for Jupyter notebook sessions".to_string(),
            cpus_per_node: 112,
            memory_mb: 257_054,
            max_walltime_secs: 24 * 3600, // 1 day
            max_nodes: None,
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: true, // Jupyter sessions typically share nodes
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["jupyter".to_string()],
        },
        // All partition - special access, infinite walltime
        // Note: This partition likely requires special permissions
        HpcPartition {
            name: "pall".to_string(),
            description: "Special partition with unlimited walltime (requires special access)"
                .to_string(),
            cpus_per_node: 112,
            memory_mb: 257_054,
            max_walltime_secs: 365 * 24 * 3600, // Effectively infinite (1 year)
            max_nodes: None,
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["special".to_string()],
        },
    ]
}
