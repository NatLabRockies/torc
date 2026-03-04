//! HPC system profile commands
//!
//! Commands for listing, detecting, and querying HPC system profiles.

use clap::Subcommand;
use std::collections::HashMap;
use tabled::Tabled;

use super::output::print_json;

use crate::client::hpc::{HpcPartition, HpcProfile, HpcProfileRegistry};
use crate::config::{ClientHpcConfig, TorcConfig};

use super::table_format::display_table_with_count;

/// Create an HPC profile registry with built-in profiles and user-defined profiles from config
///
/// This is a public version for use by other modules (e.g., main.rs for submit command)
pub fn create_registry_with_config_public(hpc_config: &ClientHpcConfig) -> HpcProfileRegistry {
    create_registry_with_config(hpc_config)
}

/// Resolve an HPC profile by name or auto-detection, with dynamic Slurm fallback.
///
/// Resolution order:
/// 1. If `name` is provided → look up in registry (supports "slurm" for dynamic detection)
/// 2. Else → try auto-detection via registry (built-in profiles, then dynamic Slurm fallback)
/// 3. If nothing found → return Err with helpful message
///
/// When the profile is resolved via dynamic Slurm detection (not a pre-configured profile),
/// an informational message is printed to stderr.
pub fn resolve_hpc_profile(
    registry: &HpcProfileRegistry,
    name: Option<&str>,
) -> Result<HpcProfile, String> {
    if let Some(name) = name {
        if let Some(profile) = registry.get(name) {
            return Ok(profile);
        }
        return Err(format!("Unknown HPC profile: {}", name));
    }

    // Try built-in/custom profile detection first (env vars, hostname patterns)
    if let Some(profile) = registry.profiles().iter().find(|p| p.detect()) {
        return Ok(profile.clone());
    }

    // Fall back to dynamic Slurm detection via registry.detect()
    if let Some(profile) = registry.detect() {
        eprintln!(
            "No pre-configured HPC profile found. Using dynamically detected Slurm cluster: {}",
            profile.display_name
        );
        return Ok(profile);
    }

    Err("No HPC profile specified and no system detected.\n\
         Use --hpc-profile <name> to specify a profile, or run on a Slurm cluster."
        .to_string())
}

/// Create an HPC profile registry with built-in profiles and user-defined profiles from config
fn create_registry_with_config(hpc_config: &ClientHpcConfig) -> HpcProfileRegistry {
    let mut registry = HpcProfileRegistry::with_builtin_profiles();

    // Apply profile overrides to built-in profiles
    // Note: This modifies the default_account for profiles
    // In a real implementation, we'd need mutable access to profiles

    // Add custom profiles from config
    for (name, profile_config) in &hpc_config.custom_profiles {
        let profile = config_to_profile(name, profile_config);
        registry.register(profile);
    }

    registry
}

/// Convert a config profile to an HpcProfile
fn config_to_profile(name: &str, config: &crate::config::HpcProfileConfig) -> HpcProfile {
    let mut detection = Vec::new();

    // Parse detect_env_var (format: "NAME=value")
    if let Some(env_var) = &config.detect_env_var
        && let Some((var_name, var_value)) = env_var.split_once('=')
    {
        detection.push(crate::client::hpc::HpcDetection::EnvVar {
            name: var_name.to_string(),
            value: var_value.to_string(),
        });
    }

    // Parse hostname pattern
    if let Some(pattern) = &config.detect_hostname {
        detection.push(crate::client::hpc::HpcDetection::HostnamePattern {
            pattern: pattern.clone(),
        });
    }

    // Convert partitions
    let partitions: Vec<HpcPartition> = config.partitions.iter().map(config_to_partition).collect();

    HpcProfile {
        name: name.to_string(),
        display_name: config.display_name.clone(),
        description: config.description.clone(),
        detection,
        default_account: config.default_account.clone(),
        partitions,
        charge_factor_cpu: config.charge_factor_cpu,
        charge_factor_gpu: config.charge_factor_gpu,
        metadata: HashMap::new(),
    }
}

/// Convert a config partition to an HpcPartition
fn config_to_partition(config: &crate::config::HpcPartitionConfig) -> HpcPartition {
    HpcPartition {
        name: config.name.clone(),
        description: config.description.clone(),
        cpus_per_node: config.cpus_per_node,
        memory_mb: config.memory_mb,
        max_walltime_secs: config.max_walltime_secs,
        max_nodes: None,
        max_nodes_per_user: None,
        min_nodes: None,
        gpus_per_node: config.gpus_per_node,
        gpu_type: config.gpu_type.clone(),
        gpu_memory_gb: config.gpu_memory_gb,
        local_disk_gb: None,
        shared: config.shared,
        requires_explicit_request: config.requires_explicit_request,
        default_qos: None,
        features: vec![],
    }
}

/// HPC system profile commands
#[derive(Debug, Subcommand)]
pub enum HpcCommands {
    /// List available HPC profiles
    List,
    /// Detect the current HPC system
    Detect,
    /// Show details of an HPC profile
    Show {
        /// Profile name
        name: String,
    },
    /// List partitions for an HPC profile
    Partitions {
        /// Profile name (e.g., "kestrel"). If not specified, tries to detect current system.
        name: Option<String>,
        /// Filter for GPU partitions
        #[arg(long)]
        gpu: bool,
        /// Filter for shared partitions
        #[arg(long)]
        shared: bool,
    },
    /// Find the best partition for given requirements
    Match {
        /// Profile name (e.g., "kestrel"). If not specified, tries to detect current system.
        name: Option<String>,
        /// Required CPUs
        #[arg(long)]
        cpus: u32,
        /// Required memory (e.g., "16g", "512m")
        #[arg(long)]
        memory: String,
        /// Required wall time (e.g., "4:00:00", "30:00")
        #[arg(long)]
        walltime: String,
        /// Required GPUs
        #[arg(long)]
        gpus: Option<u32>,
    },
    /// Generate an HPC profile from the current Slurm cluster
    Generate {
        /// Profile name (e.g., "kestrel")
        #[arg(short, long)]
        name: Option<String>,
        /// Human-readable display name
        #[arg(short, long)]
        display_name: Option<String>,
        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Skip standby partitions (ones ending in -stdby)
        #[arg(long)]
        skip_stdby: bool,
    },
}

use serde::Serialize;

#[derive(Tabled, Serialize)]
struct ProfileRow {
    name: String,
    display: String,
    partitions: usize,
    detected: bool,
}

#[derive(Tabled, Serialize)]
struct PartitionRow {
    name: String,
    cpus: u32,
    #[serde(skip_serializing)]
    memory: String,
    #[serde(skip_serializing)]
    walltime: String,
    #[serde(skip_serializing)]
    gpus: String,

    #[tabled(skip)]
    memory_mb: u64,
    #[tabled(skip)]
    max_walltime_secs: u64,
    #[tabled(skip)]
    gpus_per_node: Option<u32>,
    #[tabled(skip)]
    gpu_type: Option<String>,

    shared: bool,
    explicit: bool,
}

#[derive(Serialize)]
struct MatchResult {
    requirements: MatchRequirements,
    matching_partitions: Vec<HpcPartition>,
    best_partition: Option<HpcPartition>,
}

#[derive(Serialize)]
struct MatchRequirements {
    cpus: u32,
    memory_mb: u64,
    walltime_secs: u64,
    gpus: Option<u32>,
}

pub fn handle_hpc_commands(command: &HpcCommands, format: &str) {
    let config = TorcConfig::load().unwrap_or_default();
    let registry = create_registry_with_config(&config.client.hpc);

    match command {
        HpcCommands::List => {
            let mut rows = Vec::new();
            for profile in registry.profiles() {
                rows.push(ProfileRow {
                    name: profile.name.clone(),
                    display: profile.display_name.clone(),
                    partitions: profile.partitions.len(),
                    detected: profile.detect(),
                });
            }

            if format == "json" {
                print_json(&rows, "hpc profiles");
            } else {
                display_table_with_count(&rows, "HPC profiles");
            }
        }

        HpcCommands::Detect => {
            if let Some(profile) = registry.detect() {
                if format == "json" {
                    print_json(&profile, "detected hpc profile");
                } else {
                    println!(
                        "Detected HPC system: {} ({})",
                        profile.display_name, profile.name
                    );
                }
            } else if format == "json" {
                print_json(&Option::<HpcProfile>::None, "detected hpc profile");
            } else {
                println!("No known HPC system detected.");
            }
        }

        HpcCommands::Show { name } => {
            if let Some(profile) = registry.get(name) {
                if format == "json" {
                    print_json(&profile, "hpc profile");
                } else {
                    println!("HPC Profile: {}", profile.display_name);
                    println!("Identifier:  {}", profile.name);
                    if !profile.description.is_empty() {
                        println!("Description: {}", profile.description);
                    }
                    println!("Partitions:  {}", profile.partitions.len());

                    if !profile.metadata.is_empty() {
                        println!("\nMetadata:");
                        for (k, v) in &profile.metadata {
                            println!("  {}: {}", k, v);
                        }
                    }
                }
            } else {
                eprintln!("Unknown HPC profile: {}", name);
                std::process::exit(1);
            }
        }

        HpcCommands::Partitions { name, gpu, shared } => {
            let profile = if let Some(n) = name {
                registry.get(n)
            } else {
                registry.detect()
            };

            if let Some(profile) = profile {
                let mut rows = Vec::new();
                for p in &profile.partitions {
                    if *gpu && p.gpus_per_node.is_none() {
                        continue;
                    }
                    if *shared && !p.shared {
                        continue;
                    }

                    rows.push(PartitionRow {
                        name: p.name.clone(),
                        cpus: p.cpus_per_node,
                        memory: format!("{:.1} GB", p.memory_gb()),
                        walltime: p.max_walltime_str(),
                        gpus: p
                            .gpus_per_node
                            .map(|n| {
                                format!("{}x {}", n, p.gpu_type.as_deref().unwrap_or("unknown"))
                            })
                            .unwrap_or_else(|| "-".to_string()),
                        memory_mb: p.memory_mb,
                        max_walltime_secs: p.max_walltime_secs,
                        gpus_per_node: p.gpus_per_node,
                        gpu_type: p.gpu_type.clone(),
                        shared: p.shared,
                        explicit: p.requires_explicit_request,
                    });
                }

                if format == "json" {
                    print_json(&rows, &format!("partitions for {}", profile.name));
                } else {
                    display_table_with_count(&rows, &format!("partitions for {}", profile.name));
                }
            } else {
                eprintln!("HPC profile not found or detected.");
                std::process::exit(1);
            }
        }

        HpcCommands::Match {
            name,
            cpus,
            memory,
            walltime,
            gpus,
        } => {
            let profile = if let Some(n) = name {
                registry.get(n)
            } else {
                registry.detect()
            };

            if let Some(profile) = profile {
                let mem_mb = match crate::client::commands::slurm::parse_memory_mb(memory) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Error parsing memory: {}", e);
                        std::process::exit(1);
                    }
                };
                let walltime_secs =
                    match crate::client::commands::slurm::parse_walltime_secs(walltime) {
                        Ok(w) => w,
                        Err(e) => {
                            eprintln!("Error parsing walltime: {}", e);
                            std::process::exit(1);
                        }
                    };

                let matching =
                    profile.find_matching_partitions(*cpus, mem_mb, walltime_secs, *gpus);
                let best = profile.find_best_partition(*cpus, mem_mb, walltime_secs, *gpus);

                if format == "json" {
                    let result = MatchResult {
                        requirements: MatchRequirements {
                            cpus: *cpus,
                            memory_mb: mem_mb,
                            walltime_secs,
                            gpus: *gpus,
                        },
                        matching_partitions: matching.iter().map(|&p| p.clone()).collect(),
                        best_partition: best.cloned(),
                    };
                    print_json(&result, "match result");
                } else {
                    let rows: Vec<_> = matching
                        .iter()
                        .map(|p| {
                            let mut name_display = p.name.clone();
                            if Some(*p) == best {
                                name_display = format!("{} (BEST)", name_display);
                            }

                            PartitionRow {
                                name: name_display,
                                cpus: p.cpus_per_node,
                                memory: format!("{:.1} GB", p.memory_gb()),
                                walltime: p.max_walltime_str(),
                                gpus: p
                                    .gpus_per_node
                                    .map(|n| format!("{}x", n))
                                    .unwrap_or_else(|| "-".to_string()),
                                memory_mb: p.memory_mb,
                                max_walltime_secs: p.max_walltime_secs,
                                gpus_per_node: p.gpus_per_node,
                                gpu_type: p.gpu_type.clone(),
                                shared: p.shared,
                                explicit: p.requires_explicit_request,
                            }
                        })
                        .collect();

                    display_table_with_count(&rows, "matching partitions");

                    if let Some(best) = best {
                        println!();
                        println!("Recommended: {} partition", best.name);
                        if best.requires_explicit_request {
                            println!("  Use: --partition={}", best.name);
                        } else {
                            println!("  (Auto-routed based on requirements)");
                        }
                    }
                }
            } else {
                eprintln!("HPC profile not found or detected.");
                std::process::exit(1);
            }
        }

        HpcCommands::Generate {
            name,
            display_name,
            output,
            skip_stdby,
        } => match generate_profile_from_slurm(name.clone(), display_name.clone(), *skip_stdby) {
            Ok(toml_output) => {
                if let Some(path) = output {
                    if let Err(e) = std::fs::write(path, &toml_output) {
                        eprintln!("Failed to write output file: {}", e);
                        std::process::exit(1);
                    }
                    eprintln!("Profile written to: {}", path.display());
                } else {
                    println!("{}", toml_output);
                }
            }
            Err(e) => {
                eprintln!("Failed to generate profile: {}", e);
                std::process::exit(1);
            }
        },
    }
}

/// Generate an HPC profile from the current Slurm cluster
fn generate_profile_from_slurm(
    name: Option<String>,
    display_name: Option<String>,
    skip_stdby: bool,
) -> Result<String, String> {
    let profile =
        crate::client::hpc::slurm::generate_dynamic_slurm_profile(name, display_name, skip_stdby)?;

    // Generate TOML output
    generate_toml_profile(&profile.name, &profile.display_name, &profile.partitions)
}

/// Generate TOML configuration for the profile
fn generate_toml_profile(
    name: &str,
    display_name: &str,
    partitions: &[HpcPartition],
) -> Result<String, String> {
    let mut output = String::new();

    // Header comment
    output.push_str(&format!(
        "# HPC profile for {} generated from Slurm\n",
        display_name
    ));
    output.push_str(&format!(
        "# Generated: {}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    output.push_str("#\n");
    output.push_str("# To use this profile, add it to your torc config file:\n");
    output.push_str("#   ~/.config/torc/config.toml (Linux/macOS)\n");
    output.push_str("#   %APPDATA%\\torc\\config.toml (Windows)\n");
    output.push_str("#\n");
    output.push_str("# You may want to review and adjust:\n");
    output.push_str(
        "#   - requires_explicit_request: set to true for partitions that shouldn't auto-route\n",
    );
    output.push_str("#   - gpu_memory_gb: add GPU memory if known\n");
    output.push_str("#   - description: add human-readable descriptions\n");
    output.push('\n');

    // Profile header
    output.push_str(&format!("[client.hpc.custom_profiles.\"{}\"]\n", name));
    output.push_str(&format!("display_name = \"{}\"\n", display_name));

    // Try to generate hostname detection pattern
    if let Ok(hostname) = hostname::get() {
        let hostname = hostname.to_string_lossy();
        // Extract domain pattern (e.g., "node01.cluster.edu" -> ".*\\.cluster\\.edu")
        if let Some(dot_pos) = hostname.find('.') {
            let domain = &hostname[dot_pos + 1..];
            let pattern = format!(".*\\\\.{}", domain.replace('.', "\\\\."));
            output.push_str(&format!("detect_hostname = \"{}\"\n", pattern));
        }
    }

    output.push('\n');

    // Partitions
    for partition in partitions {
        output.push_str(&format!(
            "[[client.hpc.custom_profiles.\"{}\".partitions]]\n",
            name
        ));
        output.push_str(&format!("name = \"{}\"\n", partition.name));
        if !partition.description.is_empty() {
            output.push_str(&format!("description = \"{}\"\n", partition.description));
        } else {
            output.push_str("# description = \"\"\n");
        }
        output.push_str(&format!("cpus_per_node = {}\n", partition.cpus_per_node));
        output.push_str(&format!("memory_mb = {}\n", partition.memory_mb));
        output.push_str(&format!(
            "max_walltime_secs = {}\n",
            partition.max_walltime_secs
        ));

        if let Some(gpus) = partition.gpus_per_node {
            output.push_str(&format!("gpus_per_node = {}\n", gpus));
        }
        if let Some(ref gpu_type) = partition.gpu_type {
            output.push_str(&format!("gpu_type = \"{}\"\n", gpu_type));
        }
        if let Some(gpu_mem) = partition.gpu_memory_gb {
            output.push_str(&format!("gpu_memory_gb = {}\n", gpu_mem));
        } else {
            output.push_str("# gpu_memory_gb = 0\n");
        }
        if partition.shared {
            output.push_str("shared = true\n");
        } else {
            output.push_str("shared = false\n");
        }
        if partition.requires_explicit_request {
            output.push_str("requires_explicit_request = true\n");
        } else {
            output.push_str("requires_explicit_request = false\n");
        }

        output.push('\n');
    }

    Ok(output)
}
