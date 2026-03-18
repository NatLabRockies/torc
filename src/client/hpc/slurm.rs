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
pub fn parse_slurm_timelimit(s: &str) -> u64 {
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

// ============================================================================
// Live cluster state queries
// ============================================================================

/// Node availability counts for a partition
#[derive(Debug, Clone)]
pub struct PartitionAvailability {
    pub partition: String,
    pub idle: u32,
    pub mixed: u32,
    pub allocated: u32,
    pub down: u32,
    pub total: u32,
}

/// Queue depth information for a partition
#[derive(Debug, Clone)]
pub struct QueueDepthInfo {
    pub partition: String,
    pub pending_jobs: u32,
    pub pending_nodes: u32,
    pub running_jobs: u32,
}

/// Query sinfo for node availability per partition.
///
/// If `partition` is Some, only queries that partition. Otherwise queries all.
pub fn query_partition_availability(
    partition: Option<&str>,
) -> Result<Vec<PartitionAvailability>, String> {
    let mut args = vec!["-e", "-o", "%P|%T|%D", "--noheader"];
    let partition_arg;
    if let Some(p) = partition {
        partition_arg = p.to_string();
        args.push("-p");
        args.push(&partition_arg);
    }

    let output = Command::new(get_sinfo_exec())
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run sinfo: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "sinfo failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_partition_availability(&stdout)
}

/// Parse sinfo output for node availability.
/// Format: "%P|%T|%D" (partition|state|node_count)
pub fn parse_partition_availability(input: &str) -> Result<Vec<PartitionAvailability>, String> {
    let mut map: HashMap<String, PartitionAvailability> = HashMap::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 3 {
            continue;
        }

        let name = parts[0].trim_end_matches('*').to_string();
        let state = parts[1].to_lowercase();
        let count: u32 = parts[2].parse().unwrap_or(0);

        let entry = map.entry(name.clone()).or_insert(PartitionAvailability {
            partition: name,
            idle: 0,
            mixed: 0,
            allocated: 0,
            down: 0,
            total: 0,
        });

        entry.total += count;

        if state.starts_with("idle") {
            entry.idle += count;
        } else if state.starts_with("mix") {
            entry.mixed += count;
        } else if state.starts_with("alloc") {
            entry.allocated += count;
        } else if state.starts_with("down")
            || state.starts_with("drain")
            || state.starts_with("not_responding")
        {
            entry.down += count;
        }
        // Other states (completing, reserved, etc.) count toward total only
    }

    Ok(map.into_values().collect())
}

/// Query squeue for queue depth on a partition.
///
/// If `partition` is Some, only queries that partition. Otherwise queries all.
pub fn query_queue_depth(partition: Option<&str>) -> Result<Vec<QueueDepthInfo>, String> {
    let squeue_exec = if cfg!(any(test, debug_assertions)) {
        std::env::var("TORC_FAKE_SQUEUE").unwrap_or_else(|_| "squeue".to_string())
    } else {
        "squeue".to_string()
    };

    let mut args = vec!["--noheader", "-o", "%P|%T|%D"];
    let partition_arg;
    if let Some(p) = partition {
        partition_arg = p.to_string();
        args.push("-p");
        args.push(&partition_arg);
    }

    let output = Command::new(&squeue_exec)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run squeue: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "squeue failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_queue_depth(&stdout)
}

/// Parse squeue output for queue depth.
/// Format: "%P|%T|%D" (partition|state|nodes)
pub fn parse_queue_depth(input: &str) -> Result<Vec<QueueDepthInfo>, String> {
    let mut map: HashMap<String, QueueDepthInfo> = HashMap::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 3 {
            continue;
        }

        let name = parts[0].trim_end_matches('*').to_string();
        let state = parts[1].to_uppercase();
        let nodes: u32 = parts[2].parse().unwrap_or(0);

        let entry = map.entry(name.clone()).or_insert(QueueDepthInfo {
            partition: name,
            pending_jobs: 0,
            pending_nodes: 0,
            running_jobs: 0,
        });

        if state == "PENDING" || state == "CONFIGURING" {
            entry.pending_jobs += 1;
            entry.pending_nodes += nodes;
        } else if state == "RUNNING" || state == "COMPLETING" {
            entry.running_jobs += 1;
        }
    }

    Ok(map.into_values().collect())
}

// ============================================================================
// sbatch --test-only probes
// ============================================================================

/// Result of an `sbatch --test-only` probe
#[derive(Debug, Clone)]
pub struct SbatchTestResult {
    /// Estimated start time from Slurm scheduler
    pub estimated_start: Option<chrono::NaiveDateTime>,
    /// Whether the probe succeeded
    pub success: bool,
    /// Error message if the probe failed
    pub error_message: Option<String>,
    /// Raw output from sbatch (for debugging)
    pub raw_output: String,
}

/// Get the sbatch executable path (allows for testing with fake binary in dev/test builds)
fn get_sbatch_exec() -> String {
    if cfg!(any(test, debug_assertions)) {
        std::env::var("TORC_FAKE_SBATCH").unwrap_or_else(|_| "sbatch".to_string())
    } else {
        "sbatch".to_string()
    }
}

/// Run `sbatch --test-only` to get an estimated start time from Slurm.
///
/// This does NOT submit a job. It asks the scheduler when a job with the given
/// parameters would start, without actually queuing it.
pub fn run_sbatch_test_only(
    account: &str,
    partition: Option<&str>,
    nodes: u32,
    walltime: &str,
    qos: Option<&str>,
    gres: Option<&str>,
) -> SbatchTestResult {
    let sbatch = get_sbatch_exec();
    let mut cmd = Command::new(&sbatch);

    cmd.args([
        "--test-only",
        "--account",
        account,
        "--nodes",
        &nodes.to_string(),
        "--time",
        walltime,
        "--wrap",
        "hostname",
    ]);

    if let Some(p) = partition {
        cmd.args(["--partition", p]);
    }
    if let Some(q) = qos {
        cmd.args(["--qos", q]);
    }
    if let Some(g) = gres {
        cmd.args(["--gres", g]);
    }

    debug!(
        "Running sbatch --test-only: account={} partition={} nodes={} walltime={}",
        account,
        partition.unwrap_or("<default>"),
        nodes,
        walltime
    );

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return SbatchTestResult {
                estimated_start: None,
                success: false,
                error_message: Some(format!("Failed to run sbatch: {}", e)),
                raw_output: String::new(),
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);

    parse_sbatch_test_only(&combined)
}

/// Parse the output of `sbatch --test-only`.
///
/// Slurm outputs the estimated start time in stderr, in a format like:
/// `sbatch: Job 12345 to start at 2026-03-17T14:30:00 using 167 processors on nodes ...`
///
/// Some Slurm versions use slightly different formats:
/// `sbatch: Job 12345 to start at 2026-03-17T14:30:00 on nodes ...`
pub fn parse_sbatch_test_only(output: &str) -> SbatchTestResult {
    // Look for the estimated start time pattern
    // Various Slurm versions may use slightly different formats
    for line in output.lines() {
        let line = line.trim();

        // Match: "sbatch: Job NNNNN to start at YYYY-MM-DDTHH:MM:SS"
        if let Some(idx) = line.find("to start at ") {
            let after = &line[idx + "to start at ".len()..];
            // Take the datetime portion (19 chars: YYYY-MM-DDTHH:MM:SS)
            if after.len() >= 19 {
                let datetime_str = &after[..19];
                if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M:%S")
                {
                    return SbatchTestResult {
                        estimated_start: Some(dt),
                        success: true,
                        error_message: None,
                        raw_output: output.to_string(),
                    };
                }
            }
        }

        // Check for error messages
        if line.contains("error:") || line.contains("Unable to allocate") {
            return SbatchTestResult {
                estimated_start: None,
                success: false,
                error_message: Some(line.to_string()),
                raw_output: output.to_string(),
            };
        }
    }

    SbatchTestResult {
        estimated_start: None,
        success: false,
        error_message: Some("Could not parse sbatch --test-only output".to_string()),
        raw_output: output.to_string(),
    }
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

    #[test]
    fn test_parse_partition_availability() {
        let input = "\
standard|idle|45
standard|mixed|12
standard|allocated|180
standard|down|3
gpu-h100|idle|2
gpu-h100|mixed|1
gpu-h100|allocated|15
";
        let result = parse_partition_availability(input).unwrap();
        assert_eq!(result.len(), 2);

        let std_part = result.iter().find(|p| p.partition == "standard").unwrap();
        assert_eq!(std_part.idle, 45);
        assert_eq!(std_part.mixed, 12);
        assert_eq!(std_part.allocated, 180);
        assert_eq!(std_part.down, 3);
        assert_eq!(std_part.total, 240);

        let gpu_part = result.iter().find(|p| p.partition == "gpu-h100").unwrap();
        assert_eq!(gpu_part.idle, 2);
        assert_eq!(gpu_part.mixed, 1);
        assert_eq!(gpu_part.allocated, 15);
        assert_eq!(gpu_part.total, 18);
    }

    #[test]
    fn test_parse_partition_availability_with_default_marker() {
        let input = "standard*|idle|10\nstandard*|allocated|50\n";
        let result = parse_partition_availability(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].partition, "standard");
        assert_eq!(result[0].idle, 10);
    }

    #[test]
    fn test_parse_queue_depth() {
        let input = "\
standard|PENDING|4
standard|PENDING|8
standard|RUNNING|1
standard|RUNNING|1
gpu-h100|PENDING|2
gpu-h100|RUNNING|1
";
        let result = parse_queue_depth(input).unwrap();
        assert_eq!(result.len(), 2);

        let std_q = result.iter().find(|q| q.partition == "standard").unwrap();
        assert_eq!(std_q.pending_jobs, 2);
        assert_eq!(std_q.pending_nodes, 12);
        assert_eq!(std_q.running_jobs, 2);

        let gpu_q = result.iter().find(|q| q.partition == "gpu-h100").unwrap();
        assert_eq!(gpu_q.pending_jobs, 1);
        assert_eq!(gpu_q.pending_nodes, 2);
        assert_eq!(gpu_q.running_jobs, 1);
    }

    #[test]
    fn test_parse_queue_depth_empty() {
        let result = parse_queue_depth("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_sbatch_test_only_success() {
        let output = "sbatch: Job 12345 to start at 2026-03-17T14:30:00 using 167 processors on nodes node[001-167]";
        let result = parse_sbatch_test_only(output);
        assert!(result.success);
        assert!(result.estimated_start.is_some());
        let dt = result.estimated_start.unwrap();
        assert_eq!(dt.to_string(), "2026-03-17 14:30:00");
    }

    #[test]
    fn test_parse_sbatch_test_only_simple_format() {
        // Some Slurm versions use a simpler format
        let output = "sbatch: Job 99999 to start at 2026-04-01T08:00:00 on nodes compute-001";
        let result = parse_sbatch_test_only(output);
        assert!(result.success);
        let dt = result.estimated_start.unwrap();
        assert_eq!(dt.to_string(), "2026-04-01 08:00:00");
    }

    #[test]
    fn test_parse_sbatch_test_only_error() {
        let output = "sbatch: error: Batch job submission failed: Invalid account or account/partition combination specified";
        let result = parse_sbatch_test_only(output);
        assert!(!result.success);
        assert!(result.estimated_start.is_none());
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_parse_sbatch_test_only_unparseable() {
        let output = "some unexpected output";
        let result = parse_sbatch_test_only(output);
        assert!(!result.success);
        assert!(result.estimated_start.is_none());
    }

    #[test]
    fn test_parse_sbatch_test_only_multiline() {
        // sbatch often writes to stderr with informational lines first
        let output = "\
sbatch: Pending job allocation 0
sbatch: Job 54321 to start at 2026-03-18T09:15:00 using 4 processors on nodes node[100-103]
";
        let result = parse_sbatch_test_only(output);
        assert!(result.success);
        let dt = result.estimated_start.unwrap();
        assert_eq!(dt.to_string(), "2026-03-18 09:15:00");
    }
}
