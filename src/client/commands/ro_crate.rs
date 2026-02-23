use std::io::Read;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_wrapped_if_json};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
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
        /// Workflow ID
        #[arg()]
        workflow_id: i64,
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
        /// Workflow ID
        #[arg()]
        workflow_id: i64,
        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
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
            let metadata_str = read_metadata_input(metadata);
            let mut entity = models::RoCrateEntityModel::new(
                *workflow_id,
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

            match default_api::list_ro_crate_entities(
                config,
                selected_workflow_id,
                Some(*offset),
                *limit,
            ) {
                Ok(response) => {
                    let entities = response.items.unwrap_or_default();
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
            handle_export(config, *workflow_id, output.as_deref(), format);
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

    // Fetch all RO-Crate entities
    let entities = match default_api::list_ro_crate_entities(config, workflow_id, None, None) {
        Ok(response) => response.items.unwrap_or_default(),
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
