//! JSON-LD helpers and RO-Crate entity builders shared by the client and the server.
//!
//! The builders use only database state plus a file's recorded `size` and `sha256`, so the
//! server can create output-file and CreateAction entities in the job completion transaction.

#[cfg(any(feature = "client", feature = "server"))]
use crate::models::{FileModel, RoCrateEntityModel};
#[cfg(any(feature = "client", feature = "server"))]
use chrono::{DateTime, Utc};
#[cfg(any(feature = "client", feature = "server"))]
use std::collections::HashMap;
#[cfg(any(feature = "client", feature = "server"))]
use std::path::Path;

pub fn typed_entity(primary_type: &str, prov_type: &str) -> serde_json::Value {
    serde_json::json!([primary_type, prov_type])
}

/// Convert a `serde_json::Value` (expected to be an object) into the
/// `HashMap<String, Value>` representation used by `RoCrateEntityModel::metadata`.
#[cfg(any(feature = "client", feature = "server"))]
pub fn metadata_value_to_map(value: &serde_json::Value) -> HashMap<String, serde_json::Value> {
    value
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

#[cfg(any(feature = "client", feature = "server"))]
pub fn id_ref(id: impl AsRef<str>) -> serde_json::Value {
    serde_json::json!({ "@id": id.as_ref() })
}

#[cfg(any(feature = "client", feature = "server"))]
pub fn refs_value(ids: &[String]) -> Option<serde_json::Value> {
    match ids {
        [] => None,
        [id] => Some(id_ref(id)),
        ids => Some(serde_json::Value::Array(
            ids.iter().map(id_ref).collect::<Vec<_>>(),
        )),
    }
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
#[cfg(any(feature = "client", feature = "server"))]
pub fn build_file_entity(
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
#[cfg(any(feature = "client", feature = "server"))]
pub fn build_file_entity_with_provenance(
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
#[cfg(any(feature = "client", feature = "server"))]
pub fn build_create_action_entity(
    workflow_id: i64,
    run_id: i64,
    job_id: i64,
    job_name: &str,
    attempt_id: i64,
    input_file_paths: &[String],
    output_file_paths: &[String],
) -> RoCrateEntityModel {
    let action_id = format!("#job-{}-attempt-{}", job_id, attempt_id);

    // Build result references to output files
    let results: Vec<serde_json::Value> = output_file_paths
        .iter()
        .map(|path| serde_json::json!({ "@id": path }))
        .collect();

    let mut metadata = serde_json::json!({
        "@id": action_id,
        "@type": typed_entity("CreateAction", "prov:Activity"),
        "name": job_name,
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

/// CreateAction keys that `merge_create_action` overwrites so the action points at the
/// current run.
#[cfg(any(feature = "client", feature = "server"))]
const RUN_SCOPED_KEYS: &[&str] = &["isPartOf", "instrument", "prov:wasAssociatedWith"];

/// Fill in a CreateAction that already exists (for example one `ro-crate add-dataset -j`
/// pre-created, or one from an earlier run: the `@id` carries no run id) from a freshly built
/// one. The run-scoped keys are overwritten so the action points at the current run, `result`
/// gains the built references it lacks, and every other built key is added only where
/// missing, so dataset links and user metadata are kept. Returns whether `existing` changed.
#[cfg(any(feature = "client", feature = "server"))]
pub fn merge_create_action(
    existing: &mut HashMap<String, serde_json::Value>,
    built: &HashMap<String, serde_json::Value>,
) -> bool {
    let mut changed = false;
    for (key, value) in built {
        if RUN_SCOPED_KEYS.contains(&key.as_str()) {
            if existing.get(key) != Some(value) {
                existing.insert(key.clone(), value.clone());
                changed = true;
            }
            continue;
        }
        if key != "result" {
            if !existing.contains_key(key) {
                existing.insert(key.clone(), value.clone());
                changed = true;
            }
            continue;
        }
        let result = existing
            .entry(key.clone())
            .or_insert_with(|| serde_json::json!([]));
        if !result.is_array() {
            *result = serde_json::json!([result.clone()]);
        }
        let refs = result.as_array_mut().expect("result is an array");
        for r in value.as_array().into_iter().flatten() {
            if !refs.contains(r) {
                refs.push(r.clone());
                changed = true;
            }
        }
    }
    changed
}

#[cfg(all(test, any(feature = "client", feature = "server")))]
mod tests {
    use super::*;

    #[test]
    fn merge_create_action_keeps_dataset_and_adds_tracked_refs() {
        let built = build_create_action_entity(
            1,
            2,
            3,
            "job",
            1,
            &["in.csv".to_string()],
            &["out.csv".to_string()],
        )
        .metadata;
        let mut existing = metadata_value_to_map(&serde_json::json!({
            "@id": "#job-3-attempt-1",
            "name": "user name",
            "result": [{ "@id": "dataset/" }],
        }));

        assert!(merge_create_action(&mut existing, &built));
        assert_eq!(
            existing["result"],
            serde_json::json!([{ "@id": "dataset/" }, { "@id": "out.csv" }])
        );
        assert_eq!(existing["object"], serde_json::json!({ "@id": "in.csv" }));
        assert_eq!(existing["prov:used"], existing["object"]);
        assert_eq!(existing["name"], "user name");
        assert!(!merge_create_action(&mut existing, &built));

        // A rerun (same @id, new run) repoints the run-scoped keys.
        let rerun = build_create_action_entity(1, 5, 3, "job", 1, &[], &[]).metadata;
        assert!(merge_create_action(&mut existing, &rerun));
        assert_eq!(existing["isPartOf"]["@id"], "#torc-run-id-5");
        assert_eq!(existing["instrument"]["@id"], "#software-torc-run-id-5");
        assert_eq!(
            existing["prov:wasAssociatedWith"][1]["@id"],
            "#software-torc-server-run-id-5"
        );
    }
}
