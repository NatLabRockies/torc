//! CLI commands for managing datasets (first-class directory outputs).

use std::path::Path;

use chrono::DateTime;
use clap::Subcommand;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::{
    get_env_user_name,
    output::{print_json, print_json_wrapped},
    select_workflow_interactively,
    table_format::display_table_with_count,
};
use crate::models::{DatasetFinalizationRequest, HashMode};
use tabled::Tabled;

/// Format Unix timestamp to human-readable string
fn format_timestamp(timestamp: Option<f64>) -> String {
    match timestamp {
        Some(ts) => {
            let dt = DateTime::from_timestamp(ts as i64, 0).unwrap_or_default();
            dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        }
        None => "N/A".to_string(),
    }
}

/// Format bytes to human-readable size
fn format_size(bytes: Option<i64>) -> String {
    match bytes {
        Some(b) => {
            if b < 1024 {
                format!("{} B", b)
            } else if b < 1024 * 1024 {
                format!("{:.1} KB", b as f64 / 1024.0)
            } else if b < 1024 * 1024 * 1024 {
                format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.2} GB", b as f64 / (1024.0 * 1024.0 * 1024.0))
            }
        }
        None => "N/A".to_string(),
    }
}

#[derive(Tabled)]
struct DatasetTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Path")]
    path: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Hash Mode")]
    hash_mode: String,
    #[tabled(rename = "Files")]
    file_count: String,
    #[tabled(rename = "Size")]
    total_size: String,
}

#[derive(Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # List datasets for a workflow
    torc datasets list 123

    # Get JSON output
    torc -f json datasets list 123

    # Get dataset details
    torc datasets get 456

    # Manual finalization (for crashed runners)
    torc datasets finalize 123 --reclaim-stale-after 10m
")]
pub enum DatasetCommands {
    /// List datasets for a workflow
    #[command(after_long_help = "\
EXAMPLES:
    # List all datasets for a workflow
    torc datasets list 123

    # Filter by status
    torc datasets list 123 --status pending

    # Get JSON output
    torc -f json datasets list 123
")]
    List {
        /// List datasets for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Filter by status (pending, finalizing, finalized)
        #[arg(long)]
        status: Option<String>,
        /// Maximum number of datasets to return
        #[arg(short, long, default_value = "1000")]
        limit: i64,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
    /// Get dataset details
    #[command(after_long_help = "\
EXAMPLES:
    # Get dataset by ID
    torc datasets get 456

    # Get JSON output
    torc -f json datasets get 456
")]
    Get {
        /// Dataset ID
        #[arg()]
        dataset_id: i64,
    },
    /// Manually finalize datasets (for crashed runners or manual recovery)
    #[command(after_long_help = "\
EXAMPLES:
    # Finalize all pending datasets for a workflow
    torc datasets finalize 123

    # Reclaim stale finalization claims older than 10 minutes
    torc datasets finalize 123 --reclaim-stale-after 10m

    # Finalize a specific dataset
    torc datasets finalize --dataset-id 456
")]
    Finalize {
        /// Workflow ID (required unless --dataset-id is provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Finalize a specific dataset by ID
        #[arg(long)]
        dataset_id: Option<i64>,
        /// Reclaim stale finalization claims older than this duration (e.g., 10m, 1h)
        #[arg(long)]
        reclaim_stale_after: Option<String>,
        /// Preview what would be finalized without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

/// Handle dataset commands
pub fn handle_datasets(
    config: &Configuration,
    command: &DatasetCommands,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        DatasetCommands::List {
            workflow_id,
            status,
            limit,
            offset,
        } => handle_list(
            config,
            *workflow_id,
            status.clone(),
            *limit,
            *offset,
            json_output,
        ),
        DatasetCommands::Get { dataset_id } => handle_get(config, *dataset_id, json_output),
        DatasetCommands::Finalize {
            workflow_id,
            dataset_id,
            reclaim_stale_after,
            dry_run,
        } => handle_finalize(
            config,
            *workflow_id,
            *dataset_id,
            reclaim_stale_after.clone(),
            *dry_run,
            json_output,
        ),
    }
}

fn handle_list(
    config: &Configuration,
    workflow_id: Option<i64>,
    status: Option<String>,
    limit: i64,
    offset: i64,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let workflow_id = match workflow_id {
        Some(id) => id,
        None => select_workflow_interactively(config, &get_env_user_name())?,
    };

    match default_api::list_datasets(config, workflow_id, offset, limit, status) {
        Ok(response) => {
            if json_output {
                let items = response.items.as_deref().unwrap_or(&[]);
                print_json_wrapped("datasets", items, "datasets");
            } else {
                let datasets = response.items.unwrap_or_default();
                let rows: Vec<DatasetTableRow> = datasets
                    .iter()
                    .map(|d| DatasetTableRow {
                        id: d.id.unwrap_or(0),
                        name: d.name.clone(),
                        path: d.path.clone(),
                        status: d.status.to_string(),
                        hash_mode: d.hash_mode.to_string(),
                        file_count: d
                            .file_count
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        total_size: format_size(d.total_size_bytes),
                    })
                    .collect();
                display_table_with_count(&rows, "datasets");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to list datasets: {}", e);
            Err(e.into())
        }
    }
}

fn handle_get(
    config: &Configuration,
    dataset_id: i64,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match default_api::get_dataset(config, dataset_id) {
        Ok(dataset) => {
            if json_output {
                print_json(&dataset, "dataset");
            } else {
                println!("Dataset Details:");
                println!("  ID:          {}", dataset.id.unwrap_or(0));
                println!("  Name:        {}", dataset.name);
                println!("  Path:        {}", dataset.path);
                println!("  Workflow ID: {}", dataset.workflow_id);
                println!("  Status:      {}", dataset.status);
                println!("  Hash Mode:   {}", dataset.hash_mode);
                if let Some(desc) = &dataset.description {
                    println!("  Description: {}", desc);
                }
                if let Some(count) = dataset.file_count {
                    println!("  File Count:  {}", count);
                }
                if let Some(size) = dataset.total_size_bytes {
                    println!(
                        "  Total Size:  {} ({} bytes)",
                        format_size(Some(size)),
                        size
                    );
                }
                if let Some(hash) = &dataset.manifest_hash {
                    println!("  Hash:        {}", hash);
                }
                if dataset.claimed_by_node_id.is_some() || dataset.claimed_at.is_some() {
                    println!(
                        "  Claimed By:  Node {}",
                        dataset.claimed_by_node_id.unwrap_or(0)
                    );
                    println!("  Claimed At:  {}", format_timestamp(dataset.claimed_at));
                }
                if dataset.finalized_at.is_some() {
                    println!("  Finalized:   {}", format_timestamp(dataset.finalized_at));
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to get dataset: {}", e);
            Err(e.into())
        }
    }
}

fn handle_finalize(
    config: &Configuration,
    workflow_id: Option<i64>,
    dataset_id: Option<i64>,
    reclaim_stale_after: Option<String>,
    dry_run: bool,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // If specific dataset ID provided, finalize just that one
    if let Some(id) = dataset_id {
        let dataset = default_api::get_dataset(config, id)?;

        if dry_run {
            println!(
                "Would finalize dataset {} ({}) at {}",
                id, dataset.name, dataset.path
            );
            return Ok(());
        }

        // Compute stats for this dataset
        let path = Path::new(&dataset.path);
        let hash_mode = dataset.hash_mode;
        let (file_count, total_size_bytes, manifest_hash) =
            compute_dataset_stats(path, &hash_mode)?;

        let request = DatasetFinalizationRequest {
            file_count,
            total_size_bytes,
            manifest_hash,
        };

        match default_api::finalize_dataset(config, id, request) {
            Ok(finalized) => {
                if json_output {
                    print_json(&finalized, "dataset");
                } else {
                    println!(
                        "Finalized dataset {} ({}) - {} files, {}",
                        id,
                        finalized.name,
                        finalized.file_count.unwrap_or(0),
                        format_size(finalized.total_size_bytes)
                    );
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to finalize dataset: {}", e);
                Err(e.into())
            }
        }
    } else {
        // Finalize all pending datasets for a workflow
        let workflow_id = match workflow_id {
            Some(id) => id,
            None => select_workflow_interactively(config, &get_env_user_name())?,
        };

        // Handle stale claim reclamation if requested
        if let Some(duration_str) = &reclaim_stale_after {
            let _duration = parse_duration(duration_str)?;
            // Note: Reclaiming stale claims would require a server-side API
            // For now, we just list finalizing datasets and warn
            let response = default_api::list_datasets(
                config,
                workflow_id,
                0,
                1000,
                Some("finalizing".to_string()),
            )?;
            let finalizing = response.items.unwrap_or_default();
            if !finalizing.is_empty() {
                println!(
                    "Warning: {} dataset(s) are in 'finalizing' status. Server-side reclaim not yet implemented.",
                    finalizing.len()
                );
                for d in &finalizing {
                    println!(
                        "  - {} (ID: {}) claimed at {}",
                        d.name,
                        d.id.unwrap_or(0),
                        format_timestamp(d.claimed_at)
                    );
                }
            }
        }

        // Get pending datasets
        let response =
            default_api::list_datasets(config, workflow_id, 0, 1000, Some("pending".to_string()))?;
        let pending = response.items.unwrap_or_default();

        if pending.is_empty() {
            if !json_output {
                println!(
                    "No pending datasets to finalize for workflow {}",
                    workflow_id
                );
            }
            return Ok(());
        }

        if dry_run {
            println!("Would finalize {} dataset(s):", pending.len());
            for d in &pending {
                println!("  - {} (ID: {}) at {}", d.name, d.id.unwrap_or(0), d.path);
            }
            return Ok(());
        }

        let mut finalized_count = 0;
        let mut errors = Vec::new();

        for dataset in pending {
            let id = dataset.id.unwrap_or(0);
            let path = Path::new(&dataset.path);
            let hash_mode = dataset.hash_mode;

            match compute_dataset_stats(path, &hash_mode) {
                Ok((file_count, total_size_bytes, manifest_hash)) => {
                    let request = DatasetFinalizationRequest {
                        file_count,
                        total_size_bytes,
                        manifest_hash,
                    };

                    match default_api::finalize_dataset(config, id, request) {
                        Ok(finalized) => {
                            if !json_output {
                                println!(
                                    "Finalized {} - {} files, {}",
                                    finalized.name,
                                    finalized.file_count.unwrap_or(0),
                                    format_size(finalized.total_size_bytes)
                                );
                            }
                            finalized_count += 1;
                        }
                        Err(e) => {
                            errors.push(format!("Failed to finalize {}: {}", dataset.name, e));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to compute stats for {} at {}: {}",
                        dataset.name, dataset.path, e
                    ));
                }
            }
        }

        if !json_output {
            println!("\nFinalized {} dataset(s)", finalized_count);
            if !errors.is_empty() {
                println!("\nErrors ({}):", errors.len());
                for err in &errors {
                    println!("  - {}", err);
                }
            }
        }

        if !errors.is_empty() {
            Err(format!("{} finalization error(s)", errors.len()).into())
        } else {
            Ok(())
        }
    }
}

/// Parse a duration string like "10m", "1h", "30s"
fn parse_duration(s: &str) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty duration string".into());
    }

    let (num_str, multiplier) = if let Some(n) = s.strip_suffix('s') {
        (n, 1)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3600)
    } else {
        return Err(format!(
            "Invalid duration format: {}. Use format like 10m, 1h, 30s",
            s
        )
        .into());
    };

    let num: u64 = num_str.parse()?;
    let secs = num * multiplier;

    Ok(std::time::Duration::from_secs(secs))
}

/// Compute file count, total size, and optional hash for a dataset directory.
fn compute_dataset_stats(
    path: &Path,
    hash_mode: &HashMode,
) -> Result<(i64, i64, Option<String>), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Read;

    if !path.exists() {
        return Err(format!("Dataset path does not exist: {}", path.display()).into());
    }

    if !path.is_dir() {
        return Err(format!("Dataset path is not a directory: {}", path.display()).into());
    }

    let mut file_count: i64 = 0;
    let mut total_size_bytes: i64 = 0;
    let mut manifest_entries: Vec<String> = Vec::new();
    let mut content_hasher = Sha256::new();

    // Walk the directory recursively
    fn walk_dir(
        dir: &Path,
        base: &Path,
        file_count: &mut i64,
        total_size_bytes: &mut i64,
        manifest_entries: &mut Vec<String>,
        content_hasher: &mut sha2::Sha256,
        hash_mode: &HashMode,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                walk_dir(
                    &path,
                    base,
                    file_count,
                    total_size_bytes,
                    manifest_entries,
                    content_hasher,
                    hash_mode,
                )?;
            } else if metadata.is_file() {
                *file_count += 1;
                let size = metadata.len() as i64;
                *total_size_bytes += size;

                // Compute relative path for manifest
                let rel_path = path
                    .strip_prefix(base)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());

                match hash_mode {
                    HashMode::Manifest => {
                        // Hash of (path, size, mtime)
                        let mtime = metadata
                            .modified()
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0)
                            })
                            .unwrap_or(0);
                        manifest_entries.push(format!("{}:{}:{}", rel_path, size, mtime));
                    }
                    HashMode::Content => {
                        // Read file content and add to hash
                        let mut file = fs::File::open(&path)?;
                        let mut buffer = [0u8; 8192];
                        loop {
                            let bytes_read = file.read(&mut buffer)?;
                            if bytes_read == 0 {
                                break;
                            }
                            content_hasher.update(&buffer[..bytes_read]);
                        }
                    }
                    HashMode::None => {
                        // No hashing needed
                    }
                }
            }
        }
        Ok(())
    }

    walk_dir(
        path,
        path,
        &mut file_count,
        &mut total_size_bytes,
        &mut manifest_entries,
        &mut content_hasher,
        hash_mode,
    )?;

    let manifest_hash = match hash_mode {
        HashMode::Manifest => {
            // Sort entries for deterministic hash
            manifest_entries.sort();
            let manifest_content = manifest_entries.join("\n");
            let hash = Sha256::digest(manifest_content.as_bytes());
            Some(format!("{:x}", hash))
        }
        HashMode::Content => {
            let hash = content_hasher.finalize();
            Some(format!("{:x}", hash))
        }
        HashMode::None => None,
    };

    Ok((file_count, total_size_bytes, manifest_hash))
}
