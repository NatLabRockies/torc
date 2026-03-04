//! Utilities for automatic RO-Crate entity generation.
//!
//! This module provides helper functions for creating RO-Crate entities for workflow files
//! when `enable_ro_crate` is set on a workflow.

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::models::{FileModel, JobModel, RoCrateEntityModel};
use chrono::{DateTime, Utc};
use log::{debug, warn};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read as IoRead};
use std::path::Path;

/// Compute the SHA256 hash of a file.
///
/// Returns the hash as a lowercase hexadecimal string, or None if the file
/// cannot be read.
pub fn compute_file_sha256(path: &str) -> Option<String> {
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

    Some(format!("{:x}", hasher.finalize()))
}

/// Build an RO-Crate File entity for a workflow file.
///
/// Creates a JSON-LD entity with:
/// - `@id`: file path
/// - `@type`: "File"
/// - `name`: basename from path
/// - `encodingFormat`: MIME type via `mime_guess`
/// - `contentSize`: file size (when available)
/// - `sha256`: SHA256 hash (when available)
/// - `dateModified`: ISO8601 from st_mtime
pub fn build_file_entity(
    workflow_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
    sha256: Option<String>,
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

    // Build metadata JSON object
    let mut metadata = serde_json::json!({
        "@id": file_path,
        "@type": "File",
        "name": basename,
        "encodingFormat": mime_type
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

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: file.id,
        entity_id: file_path.clone(),
        entity_type: "File".to_string(),
        metadata: metadata.to_string(),
    }
}

/// Build an RO-Crate File entity with provenance linking to a CreateAction.
///
/// For output files, includes `wasGeneratedBy` linking to the job's CreateAction entity.
pub fn build_file_entity_with_provenance(
    workflow_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
    sha256: Option<String>,
    job_id: i64,
    attempt_id: i64,
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
        "@type": "File",
        "name": basename,
        "encodingFormat": mime_type,
        "wasGeneratedBy": { "@id": create_action_id }
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

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: file.id,
        entity_id: file_path.clone(),
        entity_type: "File".to_string(),
        metadata: metadata.to_string(),
    }
}

/// Build a CreateAction entity for job provenance.
///
/// Creates a JSON-LD entity representing the job execution:
/// - `@id`: `#job-{job_id}-attempt-{attempt_id}`
/// - `@type`: "CreateAction"
/// - `name`: job name
/// - `instrument`: reference to workflow
/// - `result`: references to output file entities
pub fn build_create_action_entity(
    workflow_id: i64,
    job: &JobModel,
    attempt_id: i64,
    output_file_paths: &[String],
) -> RoCrateEntityModel {
    let action_id = format!("#job-{}-attempt-{}", job.id.unwrap_or(0), attempt_id);

    // Build result references to output files
    let results: Vec<serde_json::Value> = output_file_paths
        .iter()
        .map(|path| serde_json::json!({ "@id": path }))
        .collect();

    let metadata = serde_json::json!({
        "@id": action_id,
        "@type": "CreateAction",
        "name": job.name,
        "instrument": { "@id": format!("#workflow-{}", workflow_id) },
        "result": results
    });

    RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: action_id,
        entity_type: "CreateAction".to_string(),
        metadata: metadata.to_string(),
    }
}

/// Find an existing RO-Crate entity for a file.
///
/// Returns the entity if one with the given file_id already exists, None otherwise.
pub fn find_entity_for_file(
    config: &Configuration,
    workflow_id: i64,
    file_id: i64,
) -> Option<RoCrateEntityModel> {
    match default_api::list_ro_crate_entities(config, workflow_id, None, None) {
        Ok(response) => {
            if let Some(entities) = response.items {
                entities.into_iter().find(|e| e.file_id == Some(file_id))
            } else {
                None
            }
        }
        Err(e) => {
            warn!("Failed to check for existing RO-Crate entities: {}", e);
            None
        }
    }
}

/// Check if an RO-Crate entity already exists for a file.
///
/// Returns true if an entity with the given file_id already exists.
pub fn entity_exists_for_file(config: &Configuration, workflow_id: i64, file_id: i64) -> bool {
    find_entity_for_file(config, workflow_id, file_id).is_some()
}

/// Create an RO-Crate entity for a file if one doesn't already exist.
///
/// This is a non-blocking operation - warnings are logged but errors don't fail
/// the calling operation.
pub fn create_ro_crate_entity_for_file(
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

    // Check if entity already exists
    if entity_exists_for_file(config, workflow_id, file_id) {
        debug!(
            "RO-Crate entity already exists for file_id={}, skipping",
            file_id
        );
        return;
    }

    // Compute SHA256 hash
    let sha256 = compute_file_sha256(&file.path);

    // Build and create the entity
    let entity = build_file_entity(workflow_id, file, content_size, sha256);

    match default_api::create_ro_crate_entity(config, entity) {
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
/// to add the `wasGeneratedBy` provenance field.
///
/// This is a non-blocking operation - warnings are logged but errors don't fail
/// the calling operation.
pub fn create_ro_crate_entity_for_output_file(
    config: &Configuration,
    workflow_id: i64,
    file: &FileModel,
    content_size: Option<u64>,
    job_id: i64,
    attempt_id: i64,
) {
    let file_id = match file.id {
        Some(id) => id,
        None => {
            warn!("Cannot create RO-Crate entity: file has no ID");
            return;
        }
    };

    // Check if entity already exists - if so, update it with provenance
    if let Some(existing) = find_entity_for_file(config, workflow_id, file_id) {
        let entity_id = match existing.id {
            Some(id) => id,
            None => {
                warn!("Existing entity has no ID, cannot update");
                return;
            }
        };

        // Parse existing metadata and add wasGeneratedBy
        let create_action_id = format!("#job-{}-attempt-{}", job_id, attempt_id);
        let mut metadata: serde_json::Value = match serde_json::from_str(&existing.metadata) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to parse existing entity metadata: {}", e);
                return;
            }
        };

        metadata["wasGeneratedBy"] = serde_json::json!({ "@id": create_action_id });

        // Update file size and hash if we have newer data
        if let Some(size) = content_size {
            metadata["contentSize"] = serde_json::json!(size);
        }
        if let Some(hash) = compute_file_sha256(&file.path) {
            metadata["sha256"] = serde_json::json!(hash);
        }

        let updated_entity = RoCrateEntityModel {
            id: Some(entity_id),
            workflow_id,
            file_id: Some(file_id),
            entity_id: existing.entity_id,
            entity_type: existing.entity_type,
            metadata: metadata.to_string(),
        };

        match default_api::update_ro_crate_entity(config, entity_id, updated_entity) {
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

    // No existing entity - create a new one with provenance
    let sha256 = compute_file_sha256(&file.path);

    let entity = build_file_entity_with_provenance(
        workflow_id,
        file,
        content_size,
        sha256,
        job_id,
        attempt_id,
    );

    match default_api::create_ro_crate_entity(config, entity) {
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
pub fn create_create_action_entity(
    config: &Configuration,
    workflow_id: i64,
    job: &JobModel,
    attempt_id: i64,
    output_file_paths: &[String],
) {
    let entity = build_create_action_entity(workflow_id, job, attempt_id, output_file_paths);

    match default_api::create_ro_crate_entity(config, entity) {
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

/// Create RO-Crate entities for input files of a workflow.
///
/// Called during workflow initialization when `enable_ro_crate` is true.
/// Input files are identified as files with `st_mtime` set (they exist before the workflow runs).
pub fn create_entities_for_input_files(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_file_entity_basic() {
        let file = FileModel {
            id: Some(1),
            workflow_id: 100,
            name: "output.csv".to_string(),
            path: "data/output.csv".to_string(),
            st_mtime: Some(1704067200.0), // 2024-01-01T00:00:00Z
        };

        let entity = build_file_entity(100, &file, Some(1024), None);

        assert_eq!(entity.workflow_id, 100);
        assert_eq!(entity.file_id, Some(1));
        assert_eq!(entity.entity_id, "data/output.csv");
        assert_eq!(entity.entity_type, "File");

        let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
        assert_eq!(metadata["@id"], "data/output.csv");
        assert_eq!(metadata["@type"], "File");
        assert_eq!(metadata["name"], "output.csv");
        assert_eq!(metadata["encodingFormat"], "text/csv");
        assert_eq!(metadata["contentSize"], 1024);
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

        let entity = build_file_entity_with_provenance(100, &file, None, None, 42, 1);

        let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
        assert_eq!(metadata["wasGeneratedBy"]["@id"], "#job-42-attempt-1");
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

        let output_files = vec![
            "output/result1.json".to_string(),
            "output/result2.json".to_string(),
        ];

        let entity = build_create_action_entity(100, &job_with_id, 1, &output_files);

        assert_eq!(entity.entity_id, "#job-42-attempt-1");
        assert_eq!(entity.entity_type, "CreateAction");

        let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
        assert_eq!(metadata["@type"], "CreateAction");
        assert_eq!(metadata["name"], "process_data");
        assert_eq!(metadata["instrument"]["@id"], "#workflow-100");
        assert!(metadata["result"].is_array());
        assert_eq!(metadata["result"][0]["@id"], "output/result1.json");
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

            let entity = build_file_entity(1, &file, None, None);
            let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
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

            let entity = build_file_entity(1, &file, None, None);
            let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
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
        let json =
            r#"{"workflow_id":1,"entity_id":"test.txt","entity_type":"File","metadata":"{}"}"#;
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
            metadata: r#"{"name":"Test"}"#.to_string(),
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
    fn test_build_file_entity_with_sha256() {
        let file = FileModel {
            id: Some(1),
            workflow_id: 100,
            name: "output.csv".to_string(),
            path: "data/output.csv".to_string(),
            st_mtime: Some(1704067200.0),
        };

        let sha256 = Some("abc123def456".to_string());
        let entity = build_file_entity(100, &file, Some(1024), sha256);

        let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
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
        let entity = build_file_entity_with_provenance(100, &file, None, sha256, 42, 1);

        let metadata: serde_json::Value = serde_json::from_str(&entity.metadata).unwrap();
        assert_eq!(metadata["sha256"], "deadbeef");
        assert_eq!(metadata["wasGeneratedBy"]["@id"], "#job-42-attempt-1");
    }
}
