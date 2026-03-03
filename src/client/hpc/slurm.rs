//! Dynamic Slurm HPC profile generation
//!
//! This module provides functionality to detect the current Slurm cluster
//! and dynamically generate an HPC profile based on sinfo and scontrol output.
use log::debug;
use std::collections::HashMap;
use std::process::Command;

use super::profiles::{HpcPartition, HpcProfile};
/// Information about a partition gathered from sinfo
#[derive(Debug)]
pub struct SinfoPartition {
    pub name: String,
    pub cpus: u32,
    pub memory_mb: u64,
    pub timelimit_secs: u64,
    pub gres: Option<String>,
}

/// Additional partition info from scontrol
#[derive(Debug, Default)]
struct ScontrolPartitionInfo {
    min_nodes: Option<u32>,
    max_nodes: Option<u32>,
    oversubscribe: Option<String>,
    default_qos: Option<String>,
}

/// Get the sinfo executable path (allows for testing with fake binary in dev/test builds)
fn get_sinfo_exec() -> String {
    if cfg!(any(test, debug_assertions)) {
        std::env::var("TORC_FAKE_SINFO").unwrap_or_else(|_| "sinfo".to_string())
    } else {
        "sinfo".to_string()
    }
}

/// Get the scontrol executable path (allows for testing with fake binary in dev/test builds)
fn get_scontrol_exec() -> String {
    if cfg!(any(test, debug_assertions)) {
        std::env::var("TORC_FAKE_SCONTROL").unwrap_or_else(|_| "scontrol".to_string())
    } else {
        "scontrol".to_string()
    }
}

/// Detect if Slurm is available and return a dynamic profile
pub fn detect_slurm_profile() -> Option<HpcProfile> {
    // Check if sinfo is available
    if Command::new(get_sinfo_exec())
        .arg("--version")
        .output()
        .is_err()
    {
        return None;
    }

    match generate_dynamic_slurm_profile(None, None, false) {
        Ok(profile) => Some(profile),
        Err(e) => {
            debug!("Failed to generate dynamic Slurm profile: {}", e);
            None
        }
    }
}

/// Generate an HPC profile from the current Slurm cluster
pub fn generate_dynamic_slurm_profile(
    name: Option<String>,
    display_name: Option<String>,
    skip_stdby: bool,
) -> Result<HpcProfile, String> {
    // Get cluster name
    let cluster_name = name.unwrap_or_else(|| {
        std::env::var("SLURM_CLUSTER_NAME")
            .ok()
            .or_else(|| {
                // Try to get from scontrol
                Command::new(get_scontrol_exec())
                    .args(["show", "config"])
                    .output()
                    .ok()
                    .and_then(|out| {
                        String::from_utf8(out.stdout).ok().and_then(|s| {
                            s.lines()
                                .find(|l| l.starts_with("ClusterName"))
                                .and_then(|l| l.split('=').nth(1))
                                .map(|s| s.trim().to_string())
                        })
                    })
            })
            .unwrap_or_else(|| {
                // Fall back to hostname
                hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            })
    });

    let final_display_name = if let Some(d) = display_name {
        d
    } else {
        // Capitalize first letter of cluster_name and add suffix
        let mut chars = cluster_name.chars();
        let capitalized = match chars.next() {
            None => cluster_name.clone(),
            Some(c) => c.to_uppercase().chain(chars).collect(),
        };
        format!("{} (Slurm)", capitalized)
    };

    // Get partition info from sinfo
    let sinfo_partitions = parse_sinfo_output()?;

    if sinfo_partitions.is_empty() {
        return Err("No partitions found. Is Slurm available on this system?".to_string());
    }

    // Group partitions by name (Slurm reports each node type separately)
    let mut partition_map: HashMap<String, Vec<&SinfoPartition>> = HashMap::new();
    for sp in &sinfo_partitions {
        partition_map.entry(sp.name.clone()).or_default().push(sp);
    }

    // Deduplicate and merge partition info
    let mut partitions = Vec::new();
    let mut seen_names: Vec<String> = partition_map.keys().cloned().collect();
    seen_names.sort(); // Consistent ordering

    for name in seen_names {
        // Skip standby partitions if requested
        if skip_stdby && name.ends_with("-stdby") {
            continue;
        }
        let group = partition_map.get(&name).unwrap();

        // Get scontrol info (same for all nodes in partition)
        let scontrol_info = parse_scontrol_partition(&name).unwrap_or_default();

        // Merge partition info from all node types:
        // - CPUs: use minimum (guaranteed on all nodes)
        // - Memory: use minimum (guaranteed on all nodes)
        // - Walltime: should be same, use max to be safe
        // - GPUs: if any node has GPUs, capture that info
        let mut min_cpus = u32::MAX;
        let mut min_memory = u64::MAX;
        let mut max_walltime = 0u64;
        let mut gpus_per_node: Option<u32> = None;
        let mut gpu_type: Option<String> = None;

        for sp in group {
            min_cpus = min_cpus.min(sp.cpus);
            min_memory = min_memory.min(sp.memory_mb);
            max_walltime = max_walltime.max(sp.timelimit_secs);

            // Capture GPU info if present, using minimum count across node types
            let (gp, gt) = parse_gres(&sp.gres);
            if let Some(count) = gp {
                gpus_per_node = Some(gpus_per_node.map_or(count, |prev| prev.min(count)));
                match (&gpu_type, &gt) {
                    (None, _) => gpu_type = gt,
                    (Some(existing), Some(new)) if existing != new => gpu_type = None,
                    _ => {}
                }
            }
        }

        // Fallback: infer GPU info from partition name if GRES wasn't reported
        if gpus_per_node.is_none()
            && let Some((inferred_count, inferred_type)) = infer_gpu_from_name(&name)
        {
            gpus_per_node = Some(inferred_count);
            gpu_type = Some(inferred_type);
        }

        // Determine if shared based on OverSubscribe setting or partition name
        let shared = scontrol_info.oversubscribe.as_ref().is_some_and(|o| {
            o.to_lowercase().contains("yes") || o.to_lowercase().contains("force")
        }) || name.to_lowercase().contains("shared");

        let partition = HpcPartition {
            name,
            description: String::new(),
            cpus_per_node: min_cpus,
            memory_mb: min_memory,
            max_walltime_secs: max_walltime,
            max_nodes: scontrol_info.max_nodes,
            max_nodes_per_user: None,
            min_nodes: scontrol_info.min_nodes,
            gpus_per_node,
            gpu_type,
            gpu_memory_gb: None,
            local_disk_gb: None,
            shared,
            requires_explicit_request: false,
            default_qos: scontrol_info.default_qos.filter(|q| q != "N/A"),
            features: vec![],
        };

        partitions.push(partition);
    }

    Ok(HpcProfile {
        name: cluster_name,
        display_name: final_display_name,
        description: "Dynamically detected Slurm cluster".to_string(),
        detection: vec![], // Not used for dynamic profiles
        default_account: None,
        partitions,
        charge_factor_cpu: 1.0,
        charge_factor_gpu: 10.0,
        metadata: HashMap::new(),
    })
}

/// Parse output from sinfo command
fn parse_sinfo_output() -> Result<Vec<SinfoPartition>, String> {
    // Run sinfo with specific format
    // %P = partition, %c = cpus, %m = memory, %l = timelimit, %G = gres, %D = nodes
    let output = Command::new(get_sinfo_exec())
        .args(["-e", "-o", "%P|%c|%m|%l|%G|%D", "--noheader"])
        .output()
        .map_err(|e| format!("Failed to run sinfo: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "sinfo failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_sinfo_string(&stdout)
}

/// Parse sinfo output string into partition info
/// Format: "%P|%c|%m|%l|%G|%D" (partition|cpus|memory|timelimit|gres|nodes)
pub fn parse_sinfo_string(input: &str) -> Result<Vec<SinfoPartition>, String> {
    let mut partitions = Vec::new();

    for line in input.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 6 {
            continue;
        }

        // Remove trailing * from default partition name
        let name = parts[0].trim_end_matches('*').to_string();

        let cpus: u32 = parts[1].parse().unwrap_or(1);

        // Memory is in MB
        let memory_mb: u64 = parts[2].parse().unwrap_or(1024);

        // Parse timelimit (formats: "infinite", "1-00:00:00", "4:00:00", "30:00")
        let timelimit_secs = parse_slurm_timelimit(parts[3]);

        let gres = if parts[4] == "(null)" || parts[4].is_empty() {
            None
        } else {
            Some(parts[4].to_string())
        };

        partitions.push(SinfoPartition {
            name,
            cpus,
            memory_mb,
            timelimit_secs,
            gres,
        });
    }

    Ok(partitions)
}

/// Parse timelimit string from Slurm format to seconds
fn parse_slurm_timelimit(s: &str) -> u64 {
    let s = s.trim();

    if s == "infinite" || s == "UNLIMITED" {
        return 365 * 24 * 3600; // 1 year as "infinite"
    }

    // Formats: "days-hours:minutes:seconds", "hours:minutes:seconds", "minutes:seconds"
    let mut days = 0;
    let time_part;

    if let Some((d_str, t_str)) = s.split_once('-') {
        days = d_str.parse().unwrap_or(0);
        time_part = t_str;
    } else {
        time_part = s;
    }

    let parts: Vec<&str> = time_part.split(':').collect();
    let mut hours = 0;
    let mut minutes = 0;
    let mut seconds = 0;

    match parts.len() {
        3 => {
            hours = parts[0].parse().unwrap_or(0);
            minutes = parts[1].parse().unwrap_or(0);
            seconds = parts[2].parse().unwrap_or(0);
        }
        2 => {
            minutes = parts[0].parse().unwrap_or(0);
            seconds = parts[1].parse().unwrap_or(0);
        }
        1 => {
            minutes = parts[0].parse().unwrap_or(0);
        }
        _ => {}
    }

    (days * 24 * 3600) + (hours * 3600) + (minutes * 60) + seconds
}

/// Parse output from scontrol show partition <name>
fn parse_scontrol_partition(name: &str) -> Option<ScontrolPartitionInfo> {
    let output = Command::new(get_scontrol_exec())
        .args(["show", "partition", name])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut info = ScontrolPartitionInfo::default();

    for word in stdout.split_whitespace() {
        if let Some((key, value)) = word.split_once('=') {
            match key {
                "MinNodes" => info.min_nodes = value.parse().ok(),
                "MaxNodes" => info.max_nodes = value.parse().ok(),
                "OverSubscribe" => info.oversubscribe = Some(value.to_string()),
                "QOS" | "QoS" => info.default_qos = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Some(info)
}

/// Infer GPU information from partition name if not explicitly provided in GRES
fn infer_gpu_from_name(name: &str) -> Option<(u32, String)> {
    let name_lower = name.to_lowercase();
    if !name_lower.contains("gpu") {
        return None;
    }

    // Common GPU node configurations
    // Default count of 4 is a heuristic - actual GPU counts vary by cluster
    let gpu_types = [
        ("h100", "h100", 4),
        ("a100", "a100", 4),
        ("v100", "v100", 4),
        ("a40", "a40", 4),
        ("a30", "a30", 4),
        ("l40", "l40", 4),
    ];

    for (pattern, gpu_type, default_count) in gpu_types {
        if name_lower.contains(pattern) {
            return Some((default_count, gpu_type.to_string()));
        }
    }

    // Generic GPU partition without specific type
    // Default to 4 GPUs - this is a heuristic; verify against actual cluster config
    Some((4, "gpu".to_string()))
}

/// Parse GRES string to extract GPU count and type
fn parse_gres(gres: &Option<String>) -> (Option<u32>, Option<String>) {
    let gres = match gres {
        Some(g) => g,
        None => return (None, None),
    };

    // Find gpu entry (might be multiple GRES separated by comma)
    for entry in gres.split(',') {
        // Strip socket info like "(S:0-3)" before parsing
        let entry = entry.split('(').next().unwrap_or(entry);

        let parts: Vec<&str> = entry.split(':').collect();
        if parts.first() != Some(&"gpu") {
            continue;
        }

        match parts.len() {
            2 => {
                // gpu:COUNT
                let count: u32 = parts[1].parse().unwrap_or(0);
                if count > 0 {
                    return (Some(count), None);
                }
            }
            3 => {
                // gpu:TYPE:COUNT
                let gpu_type = parts[1].to_string();
                let count: u32 = parts[2].parse().unwrap_or(0);
                if count > 0 {
                    return (Some(count), Some(gpu_type));
                }
            }
            _ => {}
        }
    }

    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_timelimit() {
        assert_eq!(parse_slurm_timelimit("infinite"), 365 * 24 * 3600);
        assert_eq!(parse_slurm_timelimit("UNLIMITED"), 365 * 24 * 3600);
        assert_eq!(parse_slurm_timelimit("1-00:00:00"), 24 * 3600);
        assert_eq!(parse_slurm_timelimit("04:30:00"), 4 * 3600 + 30 * 60);
        assert_eq!(parse_slurm_timelimit("30:00"), 30 * 60);
        assert_eq!(parse_slurm_timelimit("45"), 45 * 60);
    }

    #[test]
    fn test_parse_gres_simple() {
        // gpu:4
        let (count, gpu_type) = parse_gres(&Some("gpu:4".to_string()));
        assert_eq!(count, Some(4));
        assert_eq!(gpu_type, None);
    }

    #[test]
    fn test_parse_gres_with_type() {
        // gpu:a100:2
        let (count, gpu_type) = parse_gres(&Some("gpu:a100:2".to_string()));
        assert_eq!(count, Some(2));
        assert_eq!(gpu_type, Some("a100".to_string()));
    }

    #[test]
    fn test_parse_gres_with_socket_info() {
        // gpu:h100:2(S:0-3)
        let (count, gpu_type) = parse_gres(&Some("gpu:h100:2(S:0-3)".to_string()));
        assert_eq!(count, Some(2));
        assert_eq!(gpu_type, Some("h100".to_string()));
    }

    #[test]
    fn test_parse_gres_multiple() {
        // nvme:1,gpu:4
        let (count, gpu_type) = parse_gres(&Some("nvme:1,gpu:4".to_string()));
        assert_eq!(count, Some(4));
        assert_eq!(gpu_type, None);
    }

    #[test]
    fn test_parse_gres_none() {
        let (count, gpu_type) = parse_gres(&None);
        assert_eq!(count, None);
        assert_eq!(gpu_type, None);
    }

    #[test]
    fn test_infer_gpu_from_name() {
        assert_eq!(
            infer_gpu_from_name("gpu-h100"),
            Some((4, "h100".to_string()))
        );
        assert_eq!(
            infer_gpu_from_name("standard-gpu"),
            Some((4, "gpu".to_string()))
        );
        assert_eq!(infer_gpu_from_name("compute"), None);
    }
}
