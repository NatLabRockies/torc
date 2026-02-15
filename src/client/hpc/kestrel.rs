//! NLR Kestrel HPC profile
//!
//! Kestrel is NLR's flagship HPC system featuring:
//! - 2,240 standard CPU nodes (104 cores, 240GB RAM each)
//! - 156 GPU nodes with 4x NVIDIA H100 GPUs (80GB each)
//! - Various specialized partitions for different workload types
//!
//! Detection: Environment variable NREL_CLUSTER=kestrel

use super::profiles::{HpcDetection, HpcPartition, HpcProfile};

/// Create the Kestrel HPC profile
pub fn kestrel_profile() -> HpcProfile {
    HpcProfile {
        name: "kestrel".to_string(),
        display_name: "NLR Kestrel".to_string(),
        description: "NLR's flagship HPC system with CPU and GPU nodes".to_string(),
        detection: vec![HpcDetection::EnvVar {
            name: "NREL_CLUSTER".to_string(),
            value: "kestrel".to_string(),
        }],
        default_account: None,
        partitions: kestrel_partitions(),
        charge_factor_cpu: 10.0,
        charge_factor_gpu: 100.0,
        metadata: [
            (
                "documentation".to_string(),
                "https://nrel.github.io/HPC/Documentation/Systems/Kestrel/Running/".to_string(),
            ),
            ("support_email".to_string(), "HPC-Help@nrel.gov".to_string()),
        ]
        .into_iter()
        .collect(),
    }
}

fn kestrel_partitions() -> Vec<HpcPartition> {
    vec![
        // Debug partition
        HpcPartition {
            name: "debug".to_string(),
            description: "Nodes dedicated to developing and troubleshooting jobs".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 3600, // 1 hour
            max_nodes: Some(2),
            max_nodes_per_user: Some(2),
            min_nodes: None,
            gpus_per_node: Some(2), // Max 2 GPUs per user in debug
            gpu_type: Some("h100".to_string()),
            gpu_memory_gb: Some(80),
            local_disk_gb: None,
            shared: true,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["debug".to_string()],
        },
        // Short partition (<=4 hours)
        HpcPartition {
            name: "short".to_string(),
            description: "Nodes that prefer jobs with walltimes <= 4 hours".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000, // ~240G usable (984256M total but we use practical limit)
            max_walltime_secs: 4 * 3600, // 4 hours
            max_nodes: Some(2240),
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: false, // Auto-routed based on walltime
            default_qos: None,
            features: vec![],
        },
        // Standard partition (<=2 days)
        HpcPartition {
            name: "standard".to_string(),
            description: "Nodes that prefer jobs with walltimes <= 2 days".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 2 * 24 * 3600, // 2 days
            max_nodes: Some(2240),
            max_nodes_per_user: Some(1050),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: false,
            default_qos: None,
            features: vec![],
        },
        // Long partition (>2 days, up to 10 days)
        HpcPartition {
            name: "long".to_string(),
            description: "Nodes that prefer jobs with walltimes > 2 days (max 10 days)".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 10 * 24 * 3600, // 10 days
            max_nodes: Some(430),
            max_nodes_per_user: Some(215),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: false,
            default_qos: None,
            features: vec![],
        },
        // Medium memory partition (1TB RAM)
        HpcPartition {
            name: "medmem".to_string(),
            description: "Nodes with 1TB of RAM".to_string(),
            cpus_per_node: 104,
            memory_mb: 1_000_000,              // ~1TB
            max_walltime_secs: 10 * 24 * 3600, // 10 days
            max_nodes: Some(64),
            max_nodes_per_user: Some(32),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: false, // Auto-routed based on memory
            default_qos: None,
            features: vec!["highmem".to_string()],
        },
        // Big memory partition (2TB RAM, short walltime)
        HpcPartition {
            name: "bigmem".to_string(),
            description: "Nodes with 2TB RAM and 5.6TB NVMe local disk".to_string(),
            cpus_per_node: 104,
            memory_mb: 2_000_000,             // ~2TB
            max_walltime_secs: 2 * 24 * 3600, // 2 days
            max_nodes: Some(10),
            max_nodes_per_user: Some(4),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: Some(5600),
            shared: false,
            requires_explicit_request: false, // Auto-routed based on memory
            default_qos: None,
            features: vec!["bigmem".to_string(), "nvme".to_string()],
        },
        // Big memory long partition
        HpcPartition {
            name: "bigmeml".to_string(),
            description: "Bigmem nodes for jobs > 2 days (max 10 days)".to_string(),
            cpus_per_node: 104,
            memory_mb: 2_000_000,
            max_walltime_secs: 10 * 24 * 3600, // 10 days
            max_nodes: Some(4),
            max_nodes_per_user: Some(2),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: Some(5600),
            shared: false,
            requires_explicit_request: false,
            default_qos: None,
            features: vec!["bigmem".to_string(), "nvme".to_string()],
        },
        // High bandwidth partition (dual NIC)
        HpcPartition {
            name: "hbw".to_string(),
            description: "CPU nodes with dual network interface cards for multi-node jobs"
                .to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 2 * 24 * 3600, // 2 days
            max_nodes: Some(512),
            max_nodes_per_user: Some(256),
            min_nodes: Some(2), // Minimum 2 nodes required
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: true, // Must specify -p hbw
            default_qos: None,
            features: vec!["dual-nic".to_string()],
        },
        // High bandwidth long partition
        HpcPartition {
            name: "hbwl".to_string(),
            description: "HBW nodes for jobs > 2 days (max 10 days)".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 10 * 24 * 3600, // 10 days
            max_nodes: Some(128),
            max_nodes_per_user: Some(64),
            min_nodes: Some(2),
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: false,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["dual-nic".to_string()],
        },
        // NVMe partition
        HpcPartition {
            name: "nvme".to_string(),
            description: "CPU nodes with 1.7TB NVMe local drives".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 2 * 24 * 3600, // 2 days
            max_nodes: Some(256),
            max_nodes_per_user: Some(128),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: Some(1700),
            shared: false,
            requires_explicit_request: true, // Must specify -p nvme
            default_qos: None,
            features: vec!["nvme".to_string()],
        },
        // Shared partition
        HpcPartition {
            name: "shared".to_string(),
            description: "Nodes that can be shared by multiple users and jobs".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 2 * 24 * 3600, // 2 days
            max_nodes: Some(128),
            max_nodes_per_user: Some(64),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: true,
            requires_explicit_request: true, // Must specify -p shared
            default_qos: None,
            features: vec!["shared".to_string()],
        },
        // Shared long partition
        HpcPartition {
            name: "sharedl".to_string(),
            description: "Shared nodes for jobs > 2 days".to_string(),
            cpus_per_node: 104,
            memory_mb: 240_000,
            max_walltime_secs: 10 * 24 * 3600, // Docs say 2 days but listing says 10 days pattern
            max_nodes: Some(32),
            max_nodes_per_user: Some(16),
            min_nodes: None,
            gpus_per_node: None,
            gpu_type: None,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared: true,
            requires_explicit_request: true,
            default_qos: None,
            features: vec!["shared".to_string()],
        },
        // GPU H100 partition (short walltime, <= 4 hours preferred)
        HpcPartition {
            name: "gpu-h100s".to_string(),
            description: "GPU nodes preferring jobs <= 4 hours".to_string(),
            cpus_per_node: 128,
            memory_mb: 360_000,          // ~384G base, some have more
            max_walltime_secs: 4 * 3600, // 4 hours
            max_nodes: Some(156),
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: Some(4),
            gpu_type: Some("h100".to_string()),
            gpu_memory_gb: Some(80),
            local_disk_gb: Some(3400), // 3.4TB
            shared: true,              // GPU nodes are always shared
            requires_explicit_request: false,
            default_qos: None,
            features: vec!["gpu".to_string(), "h100".to_string()],
        },
        // GPU H100 partition (standard, <= 2 days)
        HpcPartition {
            name: "gpu-h100".to_string(),
            description: "GPU nodes with 4x NVIDIA H100 SXM 80GB".to_string(),
            cpus_per_node: 128,
            memory_mb: 360_000,
            max_walltime_secs: 2 * 24 * 3600, // 2 days
            max_nodes: Some(156),
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: Some(4),
            gpu_type: Some("h100".to_string()),
            gpu_memory_gb: Some(80),
            local_disk_gb: Some(3400),
            shared: true,
            requires_explicit_request: false,
            default_qos: None,
            features: vec!["gpu".to_string(), "h100".to_string()],
        },
        // GPU H100 long partition (> 2 days)
        HpcPartition {
            name: "gpu-h100l".to_string(),
            description: "GPU nodes for jobs > 2 days".to_string(),
            cpus_per_node: 128,
            memory_mb: 360_000,
            max_walltime_secs: 10 * 24 * 3600, // 10 days (assumed from pattern)
            max_nodes: Some(39),
            max_nodes_per_user: None,
            min_nodes: None,
            gpus_per_node: Some(4),
            gpu_type: Some("h100".to_string()),
            gpu_memory_gb: Some(80),
            local_disk_gb: Some(3400),
            shared: true,
            requires_explicit_request: false,
            default_qos: None,
            features: vec!["gpu".to_string(), "h100".to_string()],
        },
    ]
}
