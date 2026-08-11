//! Utilities for automatic RO-Crate entity generation.
//!
//! This module provides helper functions for creating RO-Crate entities for workflow files
//! when `enable_ro_crate` is set on a workflow.

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::version_check;
use crate::models::{FileModel, JobModel, RoCrateEntityModel};
use crate::ro_crate_json_ld::typed_entity;
use chrono::{DateTime, Utc};
use log::{debug, warn};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read as IoRead};
use std::path::Path;

/// Convert a `serde_json::Value` (expected to be an object) into the
/// `HashMap<String, Value>` representation used by `RoCrateEntityModel::metadata`.
fn metadata_value_to_map(value: &serde_json::Value) -> HashMap<String, serde_json::Value> {
    value
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

/// `@id` prefixes Torc itself synthesizes for provenance entities. User-supplied
/// `FileSpec::identifier` values starting with one of these would collide in the
/// `(workflow_id, entity_id)` uniqueness index or shadow synthetic entities
/// emitted at export time. Validator and exporter share this list so adding a
/// new synthetic prefix in one place doesn't drift from the other.
const RESERVED_ENTITY_ID_PREFIXES: &[&str] = &["#torc-", "#software-", "#job-"];

/// Exact `@id` values reserved for Torc's synthetic export-root entities. Same
/// rationale as [`RESERVED_ENTITY_ID_PREFIXES`].
const RESERVED_ENTITY_IDS: &[&str] = &["ro-crate-metadata.json", "./"];

/// True when `id` matches a reserved exact value or starts with a reserved prefix.
pub(crate) fn is_reserved_entity_id(id: &str) -> bool {
    RESERVED_ENTITY_IDS.contains(&id)
        || RESERVED_ENTITY_ID_PREFIXES
            .iter()
            .any(|p| id.starts_with(p))
}

fn id_ref(id: impl AsRef<str>) -> serde_json::Value {
    serde_json::json!({ "@id": id.as_ref() })
}

fn refs_value(ids: &[String]) -> Option<serde_json::Value> {
    match ids {
        [] => None,
        [id] => Some(id_ref(id)),
        ids => Some(serde_json::Value::Array(
            ids.iter().map(id_ref).collect::<Vec<_>>(),
        )),
    }
}

/// Compute the SHA256 hash of a file.
///
/// Returns the hash as a lowercase hexadecimal string, or None if the file
/// cannot be read.
fn compute_file_sha256(path: &str) -> Option<String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            debug!("Cannot open file for SHA256 computation '{}': {}", path, e);
            return None;
        }
    };

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(e) => {
                debug!("Error reading file for SHA256 '{}': {}", path, e);
                return None;
            }
        }
    }

    Some(hex::encode(hasher.finalize()))
}

/// Build an RO-Crate File entity for a workflow file.
///
/// Creates a JSON-LD entity with:
/// - `@id`: `identifier_override` when set, otherwise the file path
/// - `@type`: "File"
/// - `name`: basename from path
/// - `encodingFormat`: MIME type via `mime_guess`
/// - `contentSize`: file size (when available)
/// - `sha256`: SHA256 hash (when available)
/// - `dateModified`: ISO8601 from st_mtime
/// - `sameAs`: the file path, when `identifier_override` differs from it
/// - `@type` is emitted as `["File", "prov:Entity"]`
///
/// `identifier_override` carries a user-supplied stable identifier (DOI, PURL, URN,
/// ...) for input files; see [`FileSpec::identifier`]. When None, the @id falls back
/// to the file path to preserve the original behaviour.
fn build_file_entity(
    workflow_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
    sha256: Option<String>,
    identifier_override: Option<&str>,
) -> RoCrateEntityModel {
    let file_path = &file.path;
    let basename = Path::new(file_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());

    // Infer MIME type from file extension
    let mime_type = mime_guess::from_path(file_path)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let entity_id = identifier_override
        .unwrap_or(file_path.as_str())
        .to_string();

    // Build metadata JSON object
    let mut metadata = serde_json::json!({
        "@id": entity_id,
        "@type": typed_entity("File", "prov:Entity"),
        "name": basename,
        "encodingFormat": mime_type
    });

    // Preserve the local path under sameAs whenever a stable identifier was supplied,
    // so consumers can still locate the bytes on disk.
    if identifier_override.is_some_and(|id| id != file_path.as_str()) {
        metadata["sameAs"] = serde_json::json!({ "@id": file_path });
    }

    // Add content size if available
    if let Some(size) = content_size {
        metadata["contentSize"] = serde_json::json!(size);
    }

    // Add SHA256 hash if available
    if let Some(hash) = sha256 {
        metadata["sha256"] = serde_json::json!(hash);
    }

    // Add date modified from st_mtime if available
    if let Some(st_mtime) = file.st_mtime {
        let datetime = DateTime::<Utc>::from_timestamp(st_mtime as i64, 0).unwrap_or_else(Utc::now);
        metadata["dateModified"] = serde_json::json!(datetime.to_rfc3339());
    }

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: file.id,
        entity_id,
        entity_type: "File".to_string(),
        metadata: metadata_value_to_map(&metadata),
    }
}

/// Build an RO-Crate File entity with provenance linking to a CreateAction.
///
/// For output files, includes `prov:wasGeneratedBy` linking to the job's CreateAction entity.
#[allow(clippy::too_many_arguments)]
fn build_file_entity_with_provenance(
    workflow_id: i64,
    run_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
    sha256: Option<String>,
    job_id: i64,
    attempt_id: i64,
    derived_from_paths: &[String],
) -> RoCrateEntityModel {
    let file_path = &file.path;
    let basename = Path::new(file_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());

    // Infer MIME type from file extension
    let mime_type = mime_guess::from_path(file_path)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    // Create action reference for provenance
    let create_action_id = format!("#job-{}-attempt-{}", job_id, attempt_id);

    // Build metadata JSON object with provenance
    let mut metadata = serde_json::json!({
        "@id": file_path,
        "@type": typed_entity("File", "prov:Entity"),
        "name": basename,
        "encodingFormat": mime_type,
        "prov:wasGeneratedBy": { "@id": create_action_id },
        "prov:wasAttributedTo": id_ref(format!("#torc-run-id-{}", run_id))
    });

    // Add content size if available
    if let Some(size) = content_size {
        metadata["contentSize"] = serde_json::json!(size);
    }

    // Add SHA256 hash if available
    if let Some(hash) = sha256 {
        metadata["sha256"] = serde_json::json!(hash);
    }

    // Add date modified from st_mtime if available
    if let Some(st_mtime) = file.st_mtime {
        let datetime = DateTime::<Utc>::from_timestamp(st_mtime as i64, 0).unwrap_or_else(Utc::now);
        metadata["dateModified"] = serde_json::json!(datetime.to_rfc3339());
    }

    if let Some(derived_from) = refs_value(derived_from_paths) {
        metadata["prov:wasDerivedFrom"] = derived_from;
    }

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: file.id,
        entity_id: file_path.clone(),
        entity_type: "File".to_string(),
        metadata: metadata_value_to_map(&metadata),
    }
}

/// Build a CreateAction entity for job provenance.
///
/// Creates a JSON-LD entity representing the job execution:
/// - `@id`: `#job-{job_id}-attempt-{attempt_id}`
/// - `@type`: `["CreateAction", "prov:Activity"]`
/// - `name`: job name
/// - `prov:hadPlan`: reference to the workflow plan entity
/// - `instrument`: reference to the run-specific software agent
/// - `result`: references to output file entities
fn build_create_action_entity(
    workflow_id: i64,
    run_id: i64,
    job: &JobModel,
    attempt_id: i64,
    input_file_paths: &[String],
    output_file_paths: &[String],
) -> RoCrateEntityModel {
    let action_id = format!("#job-{}-attempt-{}", job.id.unwrap_or(0), attempt_id);

    // Build result references to output files
    let results: Vec<serde_json::Value> = output_file_paths
        .iter()
        .map(|path| serde_json::json!({ "@id": path }))
        .collect();

    let mut metadata = serde_json::json!({
        "@id": action_id,
        "@type": typed_entity("CreateAction", "prov:Activity"),
        "name": job.name,
        "prov:hadPlan": id_ref("#torc-workflow"),
        "isPartOf": id_ref(format!("#torc-run-id-{}", run_id)),
        "instrument": id_ref(format!("#software-torc-run-id-{}", run_id)),
        "result": results,
        "prov:wasAssociatedWith": [
            id_ref(format!("#software-torc-run-id-{}", run_id)),
            id_ref(format!("#software-torc-server-run-id-{}", run_id))
        ]
    });

    if let Some(inputs) = refs_value(input_file_paths) {
        metadata["object"] = inputs.clone();
        metadata["prov:used"] = inputs;
    }

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: action_id,
        entity_type: "CreateAction".to_string(),
        metadata: metadata_value_to_map(&metadata),
    }
}

fn build_workflow_plan_entity(workflow_id: i64, workflow_name: &str) -> RoCrateEntityModel {
    let metadata = serde_json::json!({
        "@id": "#torc-workflow",
        "@type": typed_entity("SoftwareApplication", "prov:Plan"),
        "name": workflow_name
    });

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: "#torc-workflow".to_string(),
        entity_type: "SoftwareApplication".to_string(),
        metadata: metadata_value_to_map(&metadata),
    }
}

fn build_workflow_run_entity_base(
    workflow_id: i64,
    run_id: i64,
    workflow_name: &str,
) -> RoCrateEntityModel {
    let metadata = serde_json::json!({
        "@id": format!("#torc-run-id-{}", run_id),
        "@type": typed_entity("CreateAction", "prov:Activity"),
        "name": format!("{} Run {}", workflow_name, run_id),
        "prov:hadPlan": id_ref("#torc-workflow"),
        "instrument": id_ref(format!("#software-torc-run-id-{}", run_id)),
        "prov:wasAssociatedWith": [
            id_ref(format!("#software-torc-run-id-{}", run_id)),
            id_ref(format!("#software-torc-server-run-id-{}", run_id))
        ]
    });

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: format!("#torc-run-id-{}", run_id),
        entity_type: "CreateAction".to_string(),
        metadata: metadata_value_to_map(&metadata),
    }
}

fn apply_workflow_run_entity_times(
    entity: &mut RoCrateEntityModel,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
) -> bool {
    entity.metadata.insert(
        "startTime".to_string(),
        serde_json::json!(start_time.to_rfc3339()),
    );
    if let Some(end_time) = end_time {
        entity.metadata.insert(
            "endTime".to_string(),
            serde_json::json!(end_time.to_rfc3339()),
        );
    } else {
        entity.metadata.remove("endTime");
    }

    true
}

fn parse_entity_datetime(entity: &RoCrateEntityModel, field: &str) -> Option<DateTime<Utc>> {
    let value = entity.metadata.get(field)?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Find an existing RO-Crate entity for a file.
///
/// Returns the entity if one with the given file_id already exists, None otherwise.
fn find_entity_for_file(
    config: &Configuration,
    workflow_id: i64,
    file_id: i64,
) -> Option<RoCrateEntityModel> {
    match apis::ro_crate_api::find_ro_crate_entity_by_file_id(config, workflow_id, file_id) {
        Ok(entity) => entity,
        Err(e) => {
            warn!("Failed to check for existing RO-Crate entities: {}", e);
            None
        }
    }
}

fn find_entity_by_entity_id(
    config: &Configuration,
    workflow_id: i64,
    entity_id: &str,
) -> Option<RoCrateEntityModel> {
    match apis::ro_crate_api::find_ro_crate_entity_by_entity_id(config, workflow_id, entity_id) {
        Ok(entity) => entity,
        Err(e) => {
            warn!("Failed to check for existing RO-Crate entities: {}", e);
            None
        }
    }
}

fn create_or_update_entity_by_entity_id(
    config: &Configuration,
    workflow_id: i64,
    entity: RoCrateEntityModel,
) {
    if let Some(existing) = find_entity_by_entity_id(config, workflow_id, &entity.entity_id) {
        let entity_db_id = match existing.id {
            Some(id) => id,
            None => {
                warn!("Existing entity has no ID, cannot update");
                return;
            }
        };

        let updated_entity = RoCrateEntityModel {
            id: Some(entity_db_id),
            ..entity
        };

        if let Err(e) = apis::ro_crate_entities_api::update_ro_crate_entity(
            config,
            entity_db_id,
            updated_entity,
        ) {
            warn!(
                "Failed to update RO-Crate entity '{}' (entity_id={}): {}",
                existing.entity_type, existing.entity_id, e
            );
        }
        return;
    }

    if let Err(e) = apis::ro_crate_entities_api::create_ro_crate_entity(config, entity) {
        warn!("Failed to create RO-Crate entity: {}", e);
    }
}

fn create_or_update_run_entity(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    workflow_name: &str,
) {
    let run_entity_id = format!("#torc-run-id-{}", run_id);
    if let Some(existing_run_entity) = find_entity_by_entity_id(config, workflow_id, &run_entity_id)
    {
        let entity_db_id = match existing_run_entity.id {
            Some(id) => id,
            None => {
                warn!("Existing run entity has no ID, cannot update");
                return;
            }
        };

        let start_time =
            parse_entity_datetime(&existing_run_entity, "startTime").unwrap_or_else(Utc::now);
        let end_time = parse_entity_datetime(&existing_run_entity, "endTime");
        let mut updated_run_entity = RoCrateEntityModel {
            id: Some(entity_db_id),
            ..build_workflow_run_entity_base(workflow_id, run_id, workflow_name)
        };
        if !apply_workflow_run_entity_times(&mut updated_run_entity, start_time, end_time) {
            return;
        }

        if let Err(e) = apis::ro_crate_entities_api::update_ro_crate_entity(
            config,
            entity_db_id,
            updated_run_entity,
        ) {
            warn!(
                "Failed to update RO-Crate run entity '{}' (entity_id={}): {}",
                existing_run_entity.entity_type, existing_run_entity.entity_id, e
            );
        }
        return;
    }

    let mut new_run_entity = build_workflow_run_entity_base(workflow_id, run_id, workflow_name);
    if !apply_workflow_run_entity_times(&mut new_run_entity, Utc::now(), None) {
        return;
    }
    if let Err(e) = apis::ro_crate_entities_api::create_ro_crate_entity(config, new_run_entity) {
        warn!(
            "Failed to create RO-Crate run entity '{}': {}",
            run_entity_id, e
        );
    }
}

pub(crate) fn create_workflow_provenance_entities(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    workflow_name: &str,
) {
    let plan_entity = build_workflow_plan_entity(workflow_id, workflow_name);
    create_or_update_entity_by_entity_id(config, workflow_id, plan_entity);

    create_or_update_run_entity(config, workflow_id, run_id, workflow_name);
}

/// Create or replace an RO-Crate entity for a file.
///
/// If an entity already exists for this file, it is updated with fresh metadata
/// (hash, size, timestamps). Otherwise a new entity is created.
///
/// This is a non-blocking operation - warnings are logged but errors don't fail
/// the calling operation.
fn create_ro_crate_entity_for_file(
    config: &Configuration,
    workflow_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
) {
    let file_id = match file.id {
        Some(id) => id,
        None => {
            warn!("Cannot create RO-Crate entity: file has no ID");
            return;
        }
    };

    // Compute SHA256 hash
    let sha256 = compute_file_sha256(&file.path);

    // An entity may already exist for this file: the workflow-creation step
    // pre-populates one whenever the user supplied a stable `identifier` in
    // FileSpec, and the server's `create_entities_for_input_files` upserts a
    // path-keyed row at init time. When a pre-existing entity_id differs from
    // the file path, treat it as the user's stable identifier and preserve it
    // here -- otherwise the rebuilt metadata would silently revert @id back to
    // the path.
    let existing = find_entity_for_file(config, workflow_id, file_id);
    let identifier_override = existing
        .as_ref()
        .map(|e| e.entity_id.as_str())
        .filter(|id| *id != file.path.as_str());

    // Build the entity
    let entity = build_file_entity(workflow_id, file, content_size, sha256, identifier_override);

    // Check if entity already exists - if so, update it
    if let Some(existing) = existing {
        let entity_id = match existing.id {
            Some(id) => id,
            None => {
                warn!("Existing entity has no ID, cannot update");
                return;
            }
        };

        let updated_entity = RoCrateEntityModel {
            id: Some(entity_id),
            ..entity
        };

        match apis::ro_crate_entities_api::update_ro_crate_entity(config, entity_id, updated_entity)
        {
            Ok(_) => {
                debug!(
                    "Updated RO-Crate entity for file '{}' (entity_id={})",
                    file.path, entity_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to update RO-Crate entity for file '{}': {}",
                    file.path, e
                );
            }
        }
        return;
    }

    match apis::ro_crate_entities_api::create_ro_crate_entity(config, entity) {
        Ok(created) => {
            debug!(
                "Created RO-Crate entity for file '{}' (entity_id={})",
                file.path,
                created.id.unwrap_or(0)
            );
        }
        Err(e) => {
            warn!(
                "Failed to create RO-Crate entity for file '{}': {}",
                file.path, e
            );
        }
    }
}

/// Create an RO-Crate entity for an output file with provenance.
///
/// Creates the File entity and links it to the job's CreateAction. If an entity
/// already exists for this file (e.g., created during initialization), updates it
/// to add the `prov:wasGeneratedBy` provenance field.
///
/// This is a non-blocking operation - warnings are logged but errors don't fail
/// the calling operation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_ro_crate_entity_for_output_file(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
    job_id: i64,
    attempt_id: i64,
    derived_from_paths: &[String],
) {
    let file_id = match file.id {
        Some(id) => id,
        None => {
            warn!("Cannot create RO-Crate entity: file has no ID");
            return;
        }
    };

    // Compute SHA256 hash
    let sha256 = compute_file_sha256(&file.path);

    // Build the entity with provenance
    let entity = build_file_entity_with_provenance(
        workflow_id,
        run_id,
        file,
        content_size,
        sha256,
        job_id,
        attempt_id,
        derived_from_paths,
    );

    // Check if entity already exists - if so, replace it
    if let Some(existing) = find_entity_for_file(config, workflow_id, file_id) {
        let entity_id = match existing.id {
            Some(id) => id,
            None => {
                warn!("Existing entity has no ID, cannot update");
                return;
            }
        };

        let updated_entity = RoCrateEntityModel {
            id: Some(entity_id),
            ..entity
        };

        match apis::ro_crate_entities_api::update_ro_crate_entity(config, entity_id, updated_entity)
        {
            Ok(_) => {
                debug!(
                    "Updated RO-Crate entity for output file '{}' with provenance (entity_id={})",
                    file.path, entity_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to update RO-Crate entity for output file '{}': {}",
                    file.path, e
                );
            }
        }
        return;
    }

    // No existing entity - create a new one

    match apis::ro_crate_entities_api::create_ro_crate_entity(config, entity) {
        Ok(created) => {
            debug!(
                "Created RO-Crate entity for output file '{}' (entity_id={})",
                file.path,
                created.id.unwrap_or(0)
            );
        }
        Err(e) => {
            warn!(
                "Failed to create RO-Crate entity for output file '{}': {}",
                file.path, e
            );
        }
    }
}

/// Create a CreateAction entity for a job.
///
/// This is a non-blocking operation - warnings are logged but errors don't fail
/// the calling operation.
pub(crate) fn create_create_action_entity(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    job: &JobModel,
    attempt_id: i64,
    input_file_paths: &[String],
    output_file_paths: &[String],
) {
    let entity = build_create_action_entity(
        workflow_id,
        run_id,
        job,
        attempt_id,
        input_file_paths,
        output_file_paths,
    );

    match apis::ro_crate_entities_api::create_ro_crate_entity(config, entity) {
        Ok(created) => {
            debug!(
                "Created RO-Crate CreateAction entity for job '{}' (entity_id={})",
                job.name,
                created.id.unwrap_or(0)
            );
        }
        Err(e) => {
            warn!(
                "Failed to create RO-Crate CreateAction entity for job '{}': {}",
                job.name, e
            );
        }
    }
}

/// Append `{ "@id": id }` to the entity's `result` array if not already present.
///
/// Normalizes a missing or non-array `result` value into an array so the dataset
/// reference can be added without clobbering existing tracked results. Returns
/// `false` when `metadata` is not a JSON object, so the caller can treat the
/// link as failed rather than reporting a spurious success.
fn append_result_ref(metadata: &mut serde_json::Value, id: &str) -> bool {
    let Some(obj) = metadata.as_object_mut() else {
        return false;
    };
    let entry = obj
        .entry("result".to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !entry.is_array() {
        let prev = entry.clone();
        *entry = serde_json::json!([prev]);
    }
    let arr = entry
        .as_array_mut()
        .expect("result was normalized to an array");
    let already = arr
        .iter()
        .any(|v| v.get("@id").and_then(|x| x.as_str()) == Some(id));
    if !already {
        arr.push(id_ref(id));
    }
    true
}

/// Create or update a job's CreateAction entity to record that it produced a
/// dataset, then return the CreateAction's entity id.
///
/// If a CreateAction already exists for the job/attempt (e.g. created during a
/// prior run with tracked file I/O), the dataset is appended to its existing
/// `result` array so runtime-tracked outputs are preserved. Otherwise a new
/// minimal CreateAction is built (plan/software/run refs only, no tracked
/// `object`/`result`) and the dataset is added as its sole result. The
/// minimal shape is intentional: the runtime only auto-creates a CreateAction
/// for jobs with tracked outputs, so a dataset-producing job (which torc
/// doesn't track) reaches this branch precisely when there are no tracked
/// File entities to reference.
///
/// Returns `Some("#job-{id}-attempt-{attempt}")` on success so the caller can
/// wire the dataset's `prov:wasGeneratedBy`. This is a non-blocking operation:
/// warnings are logged and `None` is returned on failure.
pub(crate) fn link_dataset_to_job_create_action(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    job: &JobModel,
    attempt_id: i64,
    dataset_entity_id: &str,
) -> Option<String> {
    let job_id = match job.id {
        Some(id) => id,
        None => {
            warn!(
                "job_name={} has no job_id, cannot wire CreateAction provenance",
                job.name
            );
            return None;
        }
    };
    let action_id = format!("#job-{}-attempt-{}", job_id, attempt_id);

    if let Some(existing) = find_entity_by_entity_id(config, workflow_id, &action_id) {
        let entity_db_id = match existing.id {
            Some(id) => id,
            None => {
                warn!(
                    "Existing CreateAction entity has no entity_db_id, cannot update action_id={}",
                    action_id
                );
                return None;
            }
        };
        let updated_metadata = append_dataset_result(
            &existing.metadata,
            &action_id,
            dataset_entity_id,
            "existing",
        )?;
        let updated = RoCrateEntityModel {
            id: Some(entity_db_id),
            metadata: updated_metadata,
            ..existing
        };
        if let Err(e) =
            apis::ro_crate_entities_api::update_ro_crate_entity(config, entity_db_id, updated)
        {
            warn!(
                "Failed to update CreateAction entity action_id={}: {}",
                action_id, e
            );
            return None;
        }
        return Some(action_id);
    }

    let mut entity = build_create_action_entity(workflow_id, run_id, job, attempt_id, &[], &[]);
    entity.metadata =
        append_dataset_result(&entity.metadata, &action_id, dataset_entity_id, "built")?;

    match apis::ro_crate_entities_api::create_ro_crate_entity(config, entity) {
        Ok(_) => Some(action_id),
        Err(e) => {
            warn!(
                "Failed to create CreateAction entity action_id={}: {}",
                action_id, e
            );
            None
        }
    }
}

/// Append a dataset reference to a CreateAction's `result` array.
/// Returns `None` on non-object metadata so callers can short-circuit.
///
/// `source` distinguishes whether the metadata came from a stored entity
/// ("existing") or a freshly built one ("built"), purely for diagnostics.
fn append_dataset_result(
    metadata_map: &HashMap<String, serde_json::Value>,
    action_id: &str,
    dataset_entity_id: &str,
    source: &str,
) -> Option<HashMap<String, serde_json::Value>> {
    let mut metadata = serde_json::Value::Object(metadata_map.clone().into_iter().collect());
    if !append_result_ref(&mut metadata, dataset_entity_id) {
        warn!(
            "{} CreateAction metadata for action_id={} is not a JSON object; cannot record dataset result",
            source, action_id
        );
        return None;
    }
    Some(metadata_value_to_map(&metadata))
}

/// Pre-create an RO-Crate File entity that carries a user-supplied stable identifier.
///
/// Called from `create_files` at workflow-creation time for every input file that
/// has a `FileSpec::identifier` set. The entity is created with `entity_id` (and
/// `metadata["@id"]`) equal to the user-supplied identifier; the file path is
/// preserved as `sameAs`.
///
/// Persistence matters: the server's `create_entities_for_input_files` (run at
/// init time) upserts metadata for the same `file_id` and forcefully resets
/// `metadata["@id"]` to the file path. It does NOT touch the `entity_id` column,
/// so the user identifier survives in `entity_id`. The client-side
/// `create_ro_crate_entity_for_file` then reads `entity_id` back and uses it as
/// the override when rebuilding metadata, restoring `@id`. Pre-creating here is
/// what makes that round-trip possible without a server change.
///
/// Returns an error if the entity cannot be created -- callers should roll back
/// the workflow on failure, the same way they do for other creation steps.
pub(crate) fn create_input_file_entity_with_identifier(
    config: &Configuration,
    workflow_id: i64,
    file: &FileModel,
    identifier: &str,
) -> Result<(), String> {
    let file_id = file
        .id
        .ok_or_else(|| "Cannot pre-create RO-Crate entity: file has no ID".to_string())?;

    // Content size is best-effort; SHA256 is intentionally skipped here because
    // hashing happens again at init time and we don't want to pay the cost twice.
    let content_size = std::fs::metadata(&file.path).ok().map(|m| m.len());
    let entity = build_file_entity(workflow_id, file, content_size, None, Some(identifier));

    apis::ro_crate_entities_api::create_ro_crate_entity(config, entity).map_err(|e| {
        format!(
            "Failed to create RO-Crate entity with identifier '{}' for file '{}' \
             (file_id={}): {}",
            identifier, file.path, file_id, e
        )
    })?;

    debug!(
        "Pre-created RO-Crate entity for input file '{}' (file_id={}) with identifier '{}'",
        file.path, file_id, identifier
    );
    Ok(())
}

/// Create RO-Crate entities for input files of a workflow.
///
/// Called during workflow initialization when `enable_ro_crate` is true.
/// Input files are identified as files with `st_mtime` set (they exist before the workflow runs).
pub(crate) fn create_entities_for_input_files(
    config: &Configuration,
    workflow_id: i64,
    files: &[FileModel],
) {
    for file in files {
        // Input files have st_mtime set (they exist before workflow runs)
        if file.st_mtime.is_some() {
            // Get file size if the file exists
            let content_size = std::fs::metadata(&file.path).ok().map(|m| m.len());

            create_ro_crate_entity_for_file(config, workflow_id, file, content_size);
        }
    }
}

/// Find the path to a binary by name.
///
/// Looks for the binary in the same directory as the current executable first,
/// then falls back to searching PATH. Returns None if the binary is not found.
fn find_binary_path(name: &str) -> Option<String> {
    // First, look in the same directory as the current executable
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|p| p.is_file())
        .or_else(|| {
            // Fall back to searching PATH
            std::env::var_os("PATH").and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(name))
                    .find(|p| p.is_file())
            })
        });

    path.map(|p| p.display().to_string())
}

/// Build a SoftwareApplication RO-Crate entity for a torc binary.
///
/// Uses compile-time version and git hash instead of runtime SHA256 computation.
/// The git hash uniquely identifies the build without the overhead of hashing
/// large binaries at runtime.
fn build_software_entity(
    workflow_id: i64,
    run_id: i64,
    name: &str,
    binary_path: &str,
) -> RoCrateEntityModel {
    let entity_id = format!("#software-{}-run-id-{}", name, run_id);

    // Use compile-time constants for version identification
    let version = version_check::full_version();
    let git_hash = version_check::GIT_HASH;

    let metadata = serde_json::json!({
        "@id": entity_id,
        "@type": typed_entity("SoftwareApplication", "prov:SoftwareAgent"),
        "name": name,
        "version": version,
        "url": binary_path,
        "torc:git_hash": git_hash,
    });

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id,
        entity_type: "SoftwareApplication".to_string(),
        metadata: metadata_value_to_map(&metadata),
    }
}

/// Create RO-Crate SoftwareApplication entities for torc binaries.
///
/// Attempts to create entities for `torc` and (on Linux) `torc-slurm-job-runner`.
/// Binaries that are not found on the system are silently skipped.
/// The `torc-server` entity is created server-side (see `RoCrateApiImpl`).
///
/// This is called during workflow initialization regardless of `enable_ro_crate`.
/// The `run_id` is included in each entity to distinguish software records across runs.
pub(crate) fn create_software_entities(config: &Configuration, workflow_id: i64, run_id: i64) {
    let mut binary_names: Vec<&str> = vec!["torc"];

    // torc-slurm-job-runner is only available on Linux
    if cfg!(target_os = "linux") {
        binary_names.push("torc-slurm-job-runner");
    }

    // Check existing entities to avoid duplicates
    let existing_ids: std::collections::HashSet<String> =
        match apis::ro_crate_entities_api::list_ro_crate_entities(
            config,
            workflow_id,
            None,
            None,
            None,
            None,
            None,
            None,
        ) {
            Ok(response) => response.items.into_iter().map(|e| e.entity_id).collect(),
            Err(e) => {
                warn!(
                    "Failed to list existing RO-Crate entities for workflow {}: {}",
                    workflow_id, e
                );
                std::collections::HashSet::new()
            }
        };

    for name in binary_names {
        let entity_id = format!("#software-{}-run-id-{}", name, run_id);
        if existing_ids.contains(&entity_id) {
            debug!(
                "SoftwareApplication entity '{}' already exists, skipping",
                entity_id
            );
            continue;
        }

        // Only create entity if binary is found
        let binary_path = match find_binary_path(name) {
            Some(path) => path,
            None => {
                debug!("Binary '{}' not found, skipping RO-Crate entity", name);
                continue;
            }
        };

        let entity = build_software_entity(workflow_id, run_id, name, &binary_path);
        match apis::ro_crate_entities_api::create_ro_crate_entity(config, entity) {
            Ok(created) => {
                debug!(
                    "Created SoftwareApplication entity for '{}' version='{}' (entity_id={})",
                    name,
                    version_check::full_version(),
                    created.id.unwrap_or(0)
                );
            }
            Err(e) => {
                warn!(
                    "Failed to create SoftwareApplication entity for '{}': {}",
                    name, e
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: every synthetic `@id` Torc actually emits must be
    /// classified as reserved by [`is_reserved_entity_id`]. If a builder gains
    /// a new prefix and the constants aren't updated, this test fails and
    /// keeps the validator from silently letting users collide with it.
    #[test]
    fn reserved_id_constants_cover_synthetic_builders() {
        let plan = build_workflow_plan_entity(1, "wf");
        assert!(
            is_reserved_entity_id(&plan.entity_id),
            "workflow plan id {} not reserved",
            plan.entity_id
        );

        let run = build_workflow_run_entity_base(1, 7, "wf");
        assert!(
            is_reserved_entity_id(&run.entity_id),
            "workflow run id {} not reserved",
            run.entity_id
        );

        let job = JobModel::new(1, "j".to_string(), "echo".to_string());
        let mut job_with_id = job;
        job_with_id.id = Some(42);
        let action = build_create_action_entity(1, 7, &job_with_id, 1, &[], &[]);
        assert!(
            is_reserved_entity_id(&action.entity_id),
            "create action id {} not reserved",
            action.entity_id
        );

        let software = build_software_entity(1, 7, "torc", "/usr/bin/torc");
        assert!(
            is_reserved_entity_id(&software.entity_id),
            "software id {} not reserved",
            software.entity_id
        );

        // Synthetic export-root ids that the exporter writes directly.
        assert!(is_reserved_entity_id("ro-crate-metadata.json"));
        assert!(is_reserved_entity_id("./"));

        // Negative cases: typical user identifiers must NOT be classified as reserved.
        assert!(!is_reserved_entity_id("data/input.csv"));
        assert!(!is_reserved_entity_id("https://doi.org/10.1234/abc"));
        assert!(!is_reserved_entity_id("urn:dataset:abc"));
    }

    #[test]
    fn test_build_file_entity_basic() {
        let file = FileModel {
            id: Some(1),
            workflow_id: 100,
            name: "output.csv".to_string(),
            path: "data/output.csv".to_string(),
            st_mtime: Some(1704067200.0), // 2024-01-01T00:00:00Z
        };

        let entity = build_file_entity(100, &file, Some(1024), None, None);

        assert_eq!(entity.workflow_id, 100);
        assert_eq!(entity.file_id, Some(1));
        assert_eq!(entity.entity_id, "data/output.csv");
        assert_eq!(entity.entity_type, "File");

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["@id"], "data/output.csv");
        assert_eq!(metadata["@type"][0], "File");
        assert_eq!(metadata["@type"][1], "prov:Entity");
        assert_eq!(metadata["name"], "output.csv");
        assert_eq!(metadata["encodingFormat"], "text/csv");
        assert_eq!(metadata["contentSize"], 1024);
        assert!(metadata.get("prov:wasAttributedTo").is_none());
    }

    #[test]
    fn test_build_file_entity_with_provenance() {
        let file = FileModel {
            id: Some(2),
            workflow_id: 100,
            name: "result.json".to_string(),
            path: "output/result.json".to_string(),
            st_mtime: Some(1704067200.0),
        };

        let entity = build_file_entity_with_provenance(100, 1, &file, None, None, 42, 1, &[]);

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["prov:wasGeneratedBy"]["@id"], "#job-42-attempt-1");
        assert_eq!(metadata["prov:wasAttributedTo"]["@id"], "#torc-run-id-1");
    }

    #[test]
    fn test_append_result_ref() {
        // Appends into an existing array, deduplicating.
        let mut meta = serde_json::json!({ "result": [{ "@id": "out/a" }] });
        append_result_ref(&mut meta, "data/ds/");
        append_result_ref(&mut meta, "data/ds/"); // duplicate is a no-op
        let arr = meta["result"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["@id"], "out/a");
        assert_eq!(arr[1]["@id"], "data/ds/");

        // Missing `result` is created as an array.
        let mut meta = serde_json::json!({ "name": "x" });
        append_result_ref(&mut meta, "data/ds/");
        assert_eq!(meta["result"][0]["@id"], "data/ds/");

        // A non-array `result` is normalized into an array first.
        let mut meta = serde_json::json!({ "result": { "@id": "out/a" } });
        append_result_ref(&mut meta, "data/ds/");
        let arr = meta["result"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["@id"], "out/a");
        assert_eq!(arr[1]["@id"], "data/ds/");
    }

    #[test]
    fn test_build_create_action_entity() {
        let job = JobModel::new(
            100,
            "process_data".to_string(),
            "python process.py".to_string(),
        );
        let mut job_with_id = job;
        job_with_id.id = Some(42);

        let input_files = vec!["input/source.csv".to_string()];
        let output_files = vec![
            "output/result1.json".to_string(),
            "output/result2.json".to_string(),
        ];

        let entity =
            build_create_action_entity(100, 1, &job_with_id, 1, &input_files, &output_files);

        assert_eq!(entity.entity_id, "#job-42-attempt-1");
        assert_eq!(entity.entity_type, "CreateAction");

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["@type"][0], "CreateAction");
        assert_eq!(metadata["@type"][1], "prov:Activity");
        assert_eq!(metadata["name"], "process_data");
        assert_eq!(metadata["instrument"]["@id"], "#software-torc-run-id-1");
        assert_eq!(metadata["prov:hadPlan"]["@id"], "#torc-workflow");
        assert_eq!(metadata["prov:used"]["@id"], "input/source.csv");
        assert!(metadata["result"].is_array());
        assert_eq!(metadata["result"][0]["@id"], "output/result1.json");
        assert_eq!(metadata["isPartOf"]["@id"], "#torc-run-id-1");
    }

    #[test]
    fn test_mime_type_inference() {
        // Test that known file types get appropriate MIME types (not the default)
        let known_types = ["file.json", "file.csv", "file.txt", "file.py", "file.rs"];

        for path in known_types {
            let file = FileModel {
                id: Some(1),
                workflow_id: 1,
                name: path.to_string(),
                path: path.to_string(),
                st_mtime: None,
            };

            let entity = build_file_entity(1, &file, None, None, None);
            let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
            let mime = metadata["encodingFormat"].as_str().unwrap();

            // Known file types should not fall back to the default
            assert_ne!(
                mime, "application/octet-stream",
                "Expected known file type '{}' to have a specific MIME type, not the default",
                path
            );
        }

        // Test that unknown file types get the default
        let unknown_types = ["file", "file.xyz123"];

        for path in unknown_types {
            let file = FileModel {
                id: Some(1),
                workflow_id: 1,
                name: path.to_string(),
                path: path.to_string(),
                st_mtime: None,
            };

            let entity = build_file_entity(1, &file, None, None, None);
            let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
            let mime = metadata["encodingFormat"].as_str().unwrap();

            assert_eq!(
                mime, "application/octet-stream",
                "Expected unknown file type '{}' to have the default MIME type",
                path
            );
        }
    }

    #[test]
    fn test_serde_json_deserialize_ro_crate() {
        // Test that standard serde_json deserialization works
        let json = r#"{"workflow_id":1,"entity_id":"test.txt","entity_type":"File","metadata":{}}"#;
        let model: crate::models::RoCrateEntityModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.workflow_id, 1);
        assert_eq!(model.entity_id, "test.txt");
        assert_eq!(model.entity_type, "File");
    }

    #[test]
    fn test_ro_crate_entity_model_roundtrip() {
        // Test serialization and deserialization roundtrip
        let model = crate::models::RoCrateEntityModel {
            id: None,
            workflow_id: 1,
            file_id: None,
            entity_id: "data/output.parquet".to_string(),
            entity_type: "File".to_string(),
            metadata: serde_json::from_str(r#"{"name":"Test"}"#).unwrap(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&model).unwrap();
        println!("Serialized JSON: {}", json);

        // Deserialize back
        let parsed: crate::models::RoCrateEntityModel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workflow_id, 1);
        assert_eq!(parsed.entity_id, "data/output.parquet");
        assert_eq!(parsed.entity_type, "File");
    }

    #[test]
    fn test_compute_file_sha256() {
        use std::io::Write;

        // Create a temporary file with known content
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_sha256.txt");
        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(b"hello world").unwrap();
        drop(file);

        // Compute hash - "hello world" has a well-known SHA256
        let hash = compute_file_sha256(temp_file.to_str().unwrap());
        assert!(hash.is_some());
        // SHA256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            hash.unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );

        // Clean up
        std::fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_compute_file_sha256_nonexistent() {
        let hash = compute_file_sha256("/nonexistent/path/to/file.txt");
        assert!(hash.is_none());
    }

    #[test]
    fn test_build_file_entity_with_identifier_override() {
        // When the user supplies a stable identifier (DOI/PURL/URN), both the @id
        // and the entity_id column must use it. The local path is preserved under
        // sameAs so consumers can still locate the bytes.
        let file = FileModel {
            id: Some(7),
            workflow_id: 100,
            name: "reference.csv".to_string(),
            path: "data/reference.csv".to_string(),
            st_mtime: Some(1704067200.0),
        };

        let entity = build_file_entity(
            100,
            &file,
            Some(2048),
            None,
            Some("https://doi.org/10.1234/abc"),
        );

        assert_eq!(entity.entity_id, "https://doi.org/10.1234/abc");
        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["@id"], "https://doi.org/10.1234/abc");
        assert_eq!(metadata["sameAs"]["@id"], "data/reference.csv");
        // Basename and other derived fields stay tied to the local path.
        assert_eq!(metadata["name"], "reference.csv");
        assert_eq!(metadata["encodingFormat"], "text/csv");
        assert_eq!(metadata["contentSize"], 2048);
    }

    #[test]
    fn test_build_file_entity_override_equal_to_path_omits_same_as() {
        // If the caller's "override" happens to equal the file path (e.g. legacy
        // entities), there's no extra information in sameAs -- it would be a
        // self-reference. Keep it out to avoid confusing consumers.
        let file = FileModel {
            id: Some(8),
            workflow_id: 100,
            name: "noop.txt".to_string(),
            path: "data/noop.txt".to_string(),
            st_mtime: None,
        };

        let entity = build_file_entity(100, &file, None, None, Some("data/noop.txt"));
        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["@id"], "data/noop.txt");
        assert!(metadata.get("sameAs").is_none());
    }

    #[test]
    fn test_build_file_entity_with_sha256() {
        let file = FileModel {
            id: Some(1),
            workflow_id: 100,
            name: "output.csv".to_string(),
            path: "data/output.csv".to_string(),
            st_mtime: Some(1704067200.0),
        };

        let sha256 = Some("abc123def456".to_string());
        let entity = build_file_entity(100, &file, Some(1024), sha256, None);

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["sha256"], "abc123def456");
    }

    #[test]
    fn test_build_file_entity_with_provenance_and_sha256() {
        let file = FileModel {
            id: Some(2),
            workflow_id: 100,
            name: "result.json".to_string(),
            path: "output/result.json".to_string(),
            st_mtime: Some(1704067200.0),
        };

        let sha256 = Some("deadbeef".to_string());
        let entity = build_file_entity_with_provenance(100, 1, &file, None, sha256, 42, 1, &[]);

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["sha256"], "deadbeef");
        assert_eq!(metadata["prov:wasGeneratedBy"]["@id"], "#job-42-attempt-1");
    }

    #[test]
    fn test_build_software_entity() {
        let entity = build_software_entity(100, 3, "torc", "/usr/local/bin/torc");

        assert_eq!(entity.workflow_id, 100);
        assert_eq!(entity.file_id, None);
        assert_eq!(entity.entity_id, "#software-torc-run-id-3");
        assert_eq!(entity.entity_type, "SoftwareApplication");

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["@id"], "#software-torc-run-id-3");
        assert_eq!(metadata["@type"][0], "SoftwareApplication");
        assert_eq!(metadata["@type"][1], "prov:SoftwareAgent");
        assert_eq!(metadata["name"], "torc");
        assert_eq!(metadata["url"], "/usr/local/bin/torc");
        // Version and git_hash are compile-time constants
        assert!(metadata.get("version").is_some());
        assert!(metadata.get("torc:git_hash").is_some());
    }

    #[test]
    fn test_build_software_entity_different_binary() {
        let entity = build_software_entity(42, 1, "torc-server", "/opt/torc/torc-server");

        assert_eq!(entity.entity_id, "#software-torc-server-run-id-1");
        assert_eq!(entity.entity_type, "SoftwareApplication");

        let metadata: serde_json::Value = serde_json::to_value(&entity.metadata).unwrap();
        assert_eq!(metadata["name"], "torc-server");
        assert_eq!(metadata["url"], "/opt/torc/torc-server");
        assert_eq!(metadata["@type"][0], "SoftwareApplication");
        assert_eq!(metadata["@type"][1], "prov:SoftwareAgent");
        assert!(metadata.get("version").is_some());
        assert!(metadata.get("torc:git_hash").is_some());
    }

    #[test]
    fn test_parse_entity_datetime() {
        let entity = crate::models::RoCrateEntityModel {
            id: Some(1),
            workflow_id: 100,
            file_id: None,
            entity_id: "#torc-run-id-7".to_string(),
            entity_type: "CreateAction".to_string(),
            metadata: serde_json::from_value(serde_json::json!({
                "startTime": "2024-01-01T00:00:00Z"
            }))
            .unwrap(),
        };

        let parsed = parse_entity_datetime(&entity, "startTime").unwrap();
        assert_eq!(parsed.to_rfc3339(), "2024-01-01T00:00:00+00:00");
        assert!(parse_entity_datetime(&entity, "endTime").is_none());
    }
}
