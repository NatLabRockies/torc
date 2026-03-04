use std::io::Read;
use std::path::{Path, PathBuf};

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_wrapped_if_json};
use crate::client::commands::pagination::{RoCrateEntityListParams, paginate_ro_crate_entities};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use tabled::Tabled;

#[derive(Tabled)]
struct RoCrateEntityTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Entity ID")]
    entity_id: String,
    #[tabled(rename = "Type")]
    entity_type: String,
    #[tabled(rename = "File ID")]
    file_id: String,
}

#[derive(clap::Subcommand)]
pub enum RoCrateCommands {
    /// Create an RO-Crate entity for a workflow
    #[command(name = "create")]
    Create {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// JSON-LD @id for this entity (e.g., "data/output.parquet")
        #[arg(long)]
        entity_id: String,
        /// Schema.org @type (e.g., "File", "Dataset", "SoftwareApplication")
        #[arg(long, name = "type")]
        entity_type: String,
        /// JSON-LD metadata as a JSON string, or "-" to read from stdin
        #[arg(long)]
        metadata: String,
        /// Optional file ID to link this entity to
        #[arg(long)]
        file_id: Option<i64>,
    },
    /// List RO-Crate entities for a workflow
    List {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of entities to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
    /// Get a specific RO-Crate entity by ID
    Get {
        /// ID of the RO-Crate entity to get
        #[arg()]
        id: i64,
    },
    /// Update an RO-Crate entity
    #[command(name = "update")]
    Update {
        /// ID of the RO-Crate entity to update
        #[arg()]
        id: i64,
        /// New JSON-LD @id
        #[arg(long)]
        entity_id: Option<String>,
        /// New Schema.org @type
        #[arg(long, name = "type")]
        entity_type: Option<String>,
        /// New JSON-LD metadata as a JSON string, or "-" to read from stdin
        #[arg(long)]
        metadata: Option<String>,
        /// New file ID to link (use 0 to unlink)
        #[arg(long)]
        file_id: Option<i64>,
    },
    /// Delete an RO-Crate entity
    Delete {
        /// ID of the RO-Crate entity to delete
        #[arg()]
        id: i64,
    },
    /// Export all RO-Crate entities as an ro-crate-metadata.json document
    Export {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Add a Dataset entity for a directory
    ///
    /// Creates an RO-Crate Dataset entity representing a directory of files,
    /// such as a hive-partitioned Parquet dataset. Computes file count, total
    /// size, and optionally a manifest or content hash.
    #[command(name = "add-dataset")]
    AddDataset {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Logical name for the dataset (e.g., "training_output")
        #[arg(long)]
        name: String,
        /// Path to the directory
        #[arg(long)]
        path: String,
        /// Hash mode: "manifest" (default), "content", or "none"
        #[arg(long, default_value = "manifest")]
        hash_mode: String,
        /// Optional description for the dataset
        #[arg(long)]
        description: Option<String>,
        /// Optional encoding format (e.g., "application/vnd.apache.parquet")
        #[arg(long)]
        encoding_format: Option<String>,
        /// Number of threads for parallel processing (default: number of CPUs)
        #[arg(long, short = 't')]
        threads: Option<usize>,
    },
}

pub fn handle_ro_crate_commands(config: &Configuration, command: &RoCrateCommands, format: &str) {
    match command {
        RoCrateCommands::Create {
            workflow_id,
            entity_id,
            entity_type,
            metadata,
            file_id,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };
            let metadata_str = read_metadata_input(metadata);
            let mut entity = models::RoCrateEntityModel::new(
                selected_workflow_id,
                entity_id.clone(),
                entity_type.clone(),
                metadata_str,
            );
            entity.file_id = *file_id;

            match default_api::create_ro_crate_entity(config, entity) {
                Ok(created) => {
                    if print_if_json(format, &created, "RO-Crate entity") {
                        // JSON printed
                    } else {
                        println!(
                            "Created RO-Crate entity with ID: {}",
                            created.id.unwrap_or(-1)
                        );
                    }
                }
                Err(e) => {
                    print_error("creating RO-Crate entity", &e);
                    std::process::exit(1);
                }
            }
        }
        RoCrateCommands::List {
            workflow_id,
            limit,
            offset,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            // Use pagination to fetch all entities (or up to limit if specified)
            let params = if let Some(lim) = limit {
                RoCrateEntityListParams::new()
                    .with_offset(*offset)
                    .with_limit(*lim)
            } else {
                RoCrateEntityListParams::new().with_offset(*offset)
            };

            match paginate_ro_crate_entities(config, selected_workflow_id, params) {
                Ok(entities) => {
                    if print_wrapped_if_json(
                        format,
                        "ro_crate_entities",
                        &entities,
                        "RO-Crate entities",
                    ) {
                        // JSON printed
                    } else if entities.is_empty() {
                        println!(
                            "No RO-Crate entities found for workflow ID: {}",
                            selected_workflow_id
                        );
                    } else {
                        println!(
                            "RO-Crate entities for workflow ID {}:",
                            selected_workflow_id
                        );
                        let rows: Vec<RoCrateEntityTableRow> = entities
                            .iter()
                            .map(|e| RoCrateEntityTableRow {
                                id: e.id.unwrap_or(-1),
                                entity_id: e.entity_id.clone(),
                                entity_type: e.entity_type.clone(),
                                file_id: e
                                    .file_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                            })
                            .collect();
                        display_table_with_count(&rows, "RO-Crate entities");
                    }
                }
                Err(e) => {
                    print_error("listing RO-Crate entities", &e);
                    std::process::exit(1);
                }
            }
        }
        RoCrateCommands::Get { id } => {
            match default_api::get_ro_crate_entity(config, *id) {
                Ok(entity) => {
                    if print_if_json(format, &entity, "RO-Crate entity") {
                        // JSON printed
                    } else {
                        println!("RO-Crate entity ID {}:", id);
                        println!("  Workflow ID: {}", entity.workflow_id);
                        println!("  Entity ID: {}", entity.entity_id);
                        println!("  Type: {}", entity.entity_type);
                        if let Some(file_id) = entity.file_id {
                            println!("  File ID: {}", file_id);
                        }
                        println!("  Metadata:");
                        // Pretty-print the metadata JSON
                        if let Ok(parsed) =
                            serde_json::from_str::<serde_json::Value>(&entity.metadata)
                        {
                            if let Ok(pretty) = serde_json::to_string_pretty(&parsed) {
                                for line in pretty.lines() {
                                    println!("    {}", line);
                                }
                            } else {
                                println!("    {}", entity.metadata);
                            }
                        } else {
                            println!("    {}", entity.metadata);
                        }
                    }
                }
                Err(e) => {
                    print_error("getting RO-Crate entity", &e);
                    std::process::exit(1);
                }
            }
        }
        RoCrateCommands::Update {
            id,
            entity_id,
            entity_type,
            metadata,
            file_id,
        } => {
            // First fetch the existing entity
            let existing = match default_api::get_ro_crate_entity(config, *id) {
                Ok(entity) => entity,
                Err(e) => {
                    print_error("getting RO-Crate entity for update", &e);
                    std::process::exit(1);
                }
            };

            let updated_metadata = metadata
                .as_ref()
                .map(|m| read_metadata_input(m))
                .unwrap_or(existing.metadata);

            let updated = models::RoCrateEntityModel {
                id: existing.id,
                workflow_id: existing.workflow_id,
                file_id: file_id
                    .map(|fid| if fid == 0 { None } else { Some(fid) })
                    .unwrap_or(existing.file_id),
                entity_id: entity_id.clone().unwrap_or(existing.entity_id),
                entity_type: entity_type.clone().unwrap_or(existing.entity_type),
                metadata: updated_metadata,
            };

            match default_api::update_ro_crate_entity(config, *id, updated) {
                Ok(result) => {
                    if print_if_json(format, &result, "RO-Crate entity") {
                        // JSON printed
                    } else {
                        println!("Updated RO-Crate entity ID: {}", id);
                    }
                }
                Err(e) => {
                    print_error("updating RO-Crate entity", &e);
                    std::process::exit(1);
                }
            }
        }
        RoCrateCommands::Delete { id } => match default_api::delete_ro_crate_entity(config, *id) {
            Ok(_) => {
                println!("Deleted RO-Crate entity ID: {}", id);
            }
            Err(e) => {
                print_error("deleting RO-Crate entity", &e);
                std::process::exit(1);
            }
        },
        RoCrateCommands::Export {
            workflow_id,
            output,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };
            handle_export(config, selected_workflow_id, output.as_deref(), format);
        }
        RoCrateCommands::AddDataset {
            workflow_id,
            name,
            path,
            hash_mode,
            description,
            encoding_format,
            threads,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };
            handle_add_dataset(
                config,
                selected_workflow_id,
                name,
                path,
                hash_mode,
                description.as_deref(),
                encoding_format.as_deref(),
                *threads,
                format,
            );
        }
    }
}

fn read_metadata_input(metadata: &str) -> String {
    if metadata == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .expect("Failed to read metadata from stdin");
        buf
    } else {
        metadata.to_string()
    }
}

fn handle_export(
    config: &Configuration,
    workflow_id: i64,
    output_path: Option<&str>,
    format: &str,
) {
    // Fetch the workflow name for the root dataset
    let workflow_name = match default_api::get_workflow(config, workflow_id) {
        Ok(w) => w.name,
        Err(e) => {
            print_error("getting workflow", &e);
            std::process::exit(1);
        }
    };

    // Fetch all RO-Crate entities using pagination
    let entities =
        match paginate_ro_crate_entities(config, workflow_id, RoCrateEntityListParams::new()) {
            Ok(entities) => entities,
            Err(e) => {
                print_error("listing RO-Crate entities", &e);
                std::process::exit(1);
            }
        };

    if format == "json" {
        // In JSON format mode, just output the raw entities list
        if let Ok(json) = serde_json::to_string_pretty(&entities) {
            println!("{}", json);
        }
        return;
    }

    // Build the RO-Crate metadata document
    let mut graph: Vec<serde_json::Value> = Vec::new();

    // 1. The metadata descriptor entity
    graph.push(serde_json::json!({
        "@id": "ro-crate-metadata.json",
        "@type": "CreativeWork",
        "about": {"@id": "./"},
        "conformsTo": {"@id": "https://w3id.org/ro/crate/1.1"}
    }));

    // 2. Collect hasPart references from user entities
    let has_part: Vec<serde_json::Value> = entities
        .iter()
        .map(|e| serde_json::json!({"@id": e.entity_id}))
        .collect();

    // 3. The root dataset entity
    graph.push(serde_json::json!({
        "@id": "./",
        "@type": "Dataset",
        "name": workflow_name,
        "hasPart": has_part
    }));

    // 4. User entities
    for entity in &entities {
        if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&entity.metadata) {
            // Ensure @id and @type are set from the entity record
            if let Some(obj) = parsed.as_object_mut() {
                obj.insert("@id".to_string(), serde_json::json!(entity.entity_id));
                obj.insert("@type".to_string(), serde_json::json!(entity.entity_type));
            }
            graph.push(parsed);
        } else {
            // Fallback: create a minimal entity
            graph.push(serde_json::json!({
                "@id": entity.entity_id,
                "@type": entity.entity_type
            }));
        }
    }

    let ro_crate = serde_json::json!({
        "@context": "https://w3id.org/ro/crate/1.1/context",
        "@graph": graph
    });

    let pretty = serde_json::to_string_pretty(&ro_crate).expect("Failed to serialize RO-Crate");

    match output_path {
        Some(path) => {
            std::fs::write(path, &pretty).unwrap_or_else(|e| {
                eprintln!("Failed to write to {}: {}", path, e);
                std::process::exit(1);
            });
            println!("Exported RO-Crate metadata to: {}", path);
        }
        None => {
            println!("{}", pretty);
        }
    }
}

/// Statistics about a directory for Dataset metadata.
#[derive(Debug)]
struct DatasetStats {
    file_count: u64,
    total_size_bytes: u64,
    hash: Option<String>,
}

/// Information about a single file for parallel processing.
struct FileInfo {
    path: PathBuf,
    rel_path: String,
    size: u64,
    mtime: f64,
}

/// Collect all file paths in a directory recursively.
fn collect_files(dir_path: &Path) -> std::io::Result<Vec<FileInfo>> {
    let mut files = Vec::new();
    collect_files_recursive(dir_path, dir_path, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    dir: &Path,
    base: &Path,
    files: &mut Vec<FileInfo>,
) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path is not a directory: {}", dir.display()),
        ));
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, base, files)?;
        } else if path.is_file() {
            let metadata = std::fs::metadata(&path)?;
            let size = metadata.len();
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            let rel_path = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            files.push(FileInfo {
                path,
                rel_path,
                size,
                mtime,
            });
        }
    }
    Ok(())
}

/// Compute SHA256 hash of a single file.
fn hash_file(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536]; // 64KB buffer for better I/O performance

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute statistics for a directory, including optional hash.
/// Uses parallel processing when num_threads > 1.
///
/// Hash modes:
/// - "manifest": Hash a sorted list of (relative_path, size, mtime) entries
/// - "content": Hash all file contents (slow for large datasets)
/// - "none": No hash computed
fn compute_dataset_stats(
    dir_path: &Path,
    hash_mode: &str,
    num_threads: usize,
) -> std::io::Result<DatasetStats> {
    // First, collect all files (single-threaded directory walk)
    let files = collect_files(dir_path)?;

    let file_count = files.len() as u64;
    let total_size_bytes: u64 = files.iter().map(|f| f.size).sum();

    let hash = match hash_mode {
        "manifest" => {
            // Build manifest entries (already have metadata from collection)
            let mut manifest_entries: Vec<String> = files
                .iter()
                .map(|f| format!("{}|{}|{:.6}", f.rel_path, f.size, f.mtime))
                .collect();

            // Sort entries for deterministic hash
            manifest_entries.sort();
            let manifest = manifest_entries.join("\n");
            let hash = Sha256::digest(manifest.as_bytes());
            Some(format!("{:x}", hash))
        }
        "content" => {
            // Hash all file contents in parallel
            Some(compute_content_hash_parallel(&files, num_threads)?)
        }
        _ => None,
    };

    Ok(DatasetStats {
        file_count,
        total_size_bytes,
        hash,
    })
}

/// Compute a hash of all file contents in parallel (Merkle-tree style).
fn compute_content_hash_parallel(
    files: &[FileInfo],
    num_threads: usize,
) -> std::io::Result<String> {
    // Configure thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    // Hash files in parallel
    let file_hashes: Result<Vec<(String, String)>, std::io::Error> = pool.install(|| {
        files
            .par_iter()
            .map(|f| {
                let hash = hash_file(&f.path)?;
                Ok((f.rel_path.clone(), hash))
            })
            .collect()
    });

    let mut file_hashes = file_hashes?;

    // Sort by path for determinism
    file_hashes.sort_by(|a, b| a.0.cmp(&b.0));

    // Combine all file hashes into a final hash
    let mut final_hasher = Sha256::new();
    for (path, hash) in &file_hashes {
        final_hasher.update(format!("{}:{}\n", path, hash).as_bytes());
    }

    Ok(format!("{:x}", final_hasher.finalize()))
}

#[allow(clippy::too_many_arguments)]
fn handle_add_dataset(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    path: &str,
    hash_mode: &str,
    description: Option<&str>,
    encoding_format: Option<&str>,
    threads: Option<usize>,
    format: &str,
) {
    // Validate hash mode
    if !["manifest", "content", "none"].contains(&hash_mode) {
        eprintln!(
            "Error: Invalid hash mode '{}'. Must be one of: manifest, content, none",
            hash_mode
        );
        std::process::exit(1);
    }

    let dir_path = Path::new(path);

    // Check if directory exists
    if !dir_path.exists() {
        eprintln!("Error: Directory does not exist: {}", path);
        std::process::exit(1);
    }

    if !dir_path.is_dir() {
        eprintln!("Error: Path is not a directory: {}", path);
        std::process::exit(1);
    }

    // Determine number of threads (default to number of CPUs)
    let num_threads = threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
    });

    // Compute statistics
    println!(
        "Computing dataset statistics for: {} (using {} threads)",
        path, num_threads
    );
    let stats = match compute_dataset_stats(dir_path, hash_mode, num_threads) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error computing dataset statistics: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "  Files: {}, Size: {} bytes",
        stats.file_count, stats.total_size_bytes
    );
    if let Some(ref hash) = stats.hash {
        println!("  Hash ({}): {}", hash_mode, hash);
    }

    // Ensure path ends with / for directory convention
    let entity_id = if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{}/", path)
    };

    // Build the metadata JSON
    let mut metadata = serde_json::json!({
        "@id": entity_id,
        "@type": "Dataset",
        "name": name,
        "contentSize": stats.total_size_bytes,
        "fileCount": stats.file_count,
        "hashMode": hash_mode
    });

    if let Some(hash) = stats.hash {
        metadata["sha256"] = serde_json::json!(hash);
    }

    if let Some(desc) = description {
        metadata["description"] = serde_json::json!(desc);
    }

    if let Some(enc) = encoding_format {
        metadata["encodingFormat"] = serde_json::json!(enc);
    }

    // Create the RO-Crate entity
    let entity = models::RoCrateEntityModel::new(
        workflow_id,
        entity_id.clone(),
        "Dataset".to_string(),
        metadata.to_string(),
    );

    match default_api::create_ro_crate_entity(config, entity) {
        Ok(created) => {
            if print_if_json(format, &created, "RO-Crate Dataset entity") {
                // JSON printed
            } else {
                println!(
                    "Created RO-Crate Dataset entity with ID: {}",
                    created.id.unwrap_or(-1)
                );
            }
        }
        Err(e) => {
            print_error("creating RO-Crate Dataset entity", &e);
            std::process::exit(1);
        }
    }
}
