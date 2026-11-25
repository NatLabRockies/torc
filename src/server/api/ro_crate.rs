//! RO-Crate entity-related API endpoints

#![allow(clippy::too_many_arguments)]

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, info, warn};
use sqlx::Row;

use crate::server::api_responses::{
    CreateRoCrateEntityResponse, DeleteRoCrateEntitiesResponse, DeleteRoCrateEntityResponse,
    GetRoCrateEntityResponse, ListRoCrateEntitiesResponse, UpdateRoCrateEntityResponse,
};

use crate::models;
use crate::ro_crate_json_ld::{
    build_create_action_entity, build_file_entity, build_file_entity_with_provenance, typed_entity,
};

use super::{
    ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg,
    message_error_response, resource_not_found_response,
};

const RO_CRATE_ENTITY_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "file_id",
    "entity_id",
    "entity_type",
    "metadata",
];

/// Parse a JSON-encoded metadata column into a typed `HashMap`.
/// Malformed/non-object payloads degrade to an empty map so a single bad row
/// doesn't break list endpoints.
fn parse_metadata(raw: &str) -> std::collections::HashMap<String, serde_json::Value> {
    serde_json::from_str(raw).unwrap_or_default()
}

/// Serialize the typed metadata into the JSON-blob column.
fn serialize_metadata(
    m: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<String, ApiError> {
    serde_json::to_string(m).map_err(|e| ApiError(format!("Failed to serialize metadata: {}", e)))
}

/// The current version of this binary, set at compile time.
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The git commit hash of this binary, set at compile time via build.rs.
const GIT_HASH: &str = env!("GIT_HASH");

/// Returns the full version string including git hash (e.g., "0.8.0 (abc1234)")
fn full_version() -> String {
    format!("{} ({})", SERVER_VERSION, GIT_HASH)
}

fn ro_crate_duplicate_details(error_string: &str) -> Option<(&'static str, &'static str)> {
    if !error_string.contains("UNIQUE constraint failed") {
        return None;
    }

    if error_string.contains("ro_crate_entity.workflow_id, ro_crate_entity.file_id") {
        Some((
            "RO-Crate entity already exists for this workflow/file link",
            "file_id",
        ))
    } else if error_string.contains("ro_crate_entity.workflow_id, ro_crate_entity.entity_id") {
        Some((
            "RO-Crate entity already exists for this workflow/entity_id link",
            "entity_id",
        ))
    } else {
        Some(("RO-Crate entity already exists", "unknown"))
    }
}

fn log_duplicate_ro_crate_entity(
    operation: &str,
    workflow_id: i64,
    duplicate_field: &str,
    file_id: Option<i64>,
    entity_id: &str,
) {
    if let Some(file_id) = file_id {
        warn!(
            "Duplicate RO-Crate entity {} for workflow_id={} duplicate_field={} file_id={} has_file_id=true entity_id={}",
            operation, workflow_id, duplicate_field, file_id, entity_id
        );
    } else {
        warn!(
            "Duplicate RO-Crate entity {} for workflow_id={} duplicate_field={} has_file_id=false entity_id={}",
            operation, workflow_id, duplicate_field, entity_id
        );
    }
}

/// Trait defining RO-Crate entity-related API operations
#[async_trait]
pub trait RoCrateApi<C> {
    /// Store one RO-Crate entity record.
    async fn create_ro_crate_entity(
        &self,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError>;

    /// Retrieve an RO-Crate entity record by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError>;

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        file_id: Option<i64>,
        entity_id: Option<String>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError>;

    /// Update an RO-Crate entity record.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError>;

    /// Delete an RO-Crate entity record.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError>;

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError>;
}

/// Create the RO-Crate entities for a job's accepted output files inside the completion
/// transaction: one File entity per accepted output (with `prov:wasGeneratedBy` provenance)
/// and the job attempt's CreateAction, whose `result` lists all of the job's outputs. Does nothing unless the workflow has `enable_ro_crate`.
///
/// Idempotent, so replaying a completion converges: a File entity is replaced by `file_id`,
/// and an existing CreateAction for the same job attempt is kept. A uniqueness conflict with
/// an unrelated entity is logged and skipped so it never costs the completion; other database
/// errors are returned.
pub(crate) async fn create_output_entities_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    workflow_id: i64,
    run_id: i64,
    job_id: i64,
    job_name: &str,
    attempt_id: i64,
    output_file_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let enabled: Option<i64> =
        sqlx::query_scalar("SELECT enable_ro_crate FROM workflow WHERE id = ?")
            .bind(workflow_id)
            .fetch_one(&mut **tx)
            .await?;
    if enabled != Some(1) {
        return Ok(());
    }

    // The CreateAction lists every output of the job, including any whose stats were not
    // recorded with this completion, so a later restore never leaves it incomplete.
    let output_paths: Vec<String> = sqlx::query_scalar(
        "SELECT f.path FROM job_output_file jof JOIN file f ON f.id = jof.file_id \
         WHERE jof.job_id = ? ORDER BY f.id",
    )
    .bind(job_id)
    .fetch_all(&mut **tx)
    .await?;
    if output_paths.is_empty() {
        return Ok(());
    }

    let ids_json = serde_json::to_string(output_file_ids).expect("serialize ids");
    let outputs: Vec<models::FileModel> = sqlx::query(
        "SELECT f.id, f.workflow_id, f.name, f.path, f.st_mtime, f.size, f.sha256 \
         FROM json_each(?1) j CROSS JOIN file f ON f.id = j.value AND f.workflow_id = ?2 \
         ORDER BY f.id",
    )
    .bind(&ids_json)
    .bind(workflow_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|r| models::FileModel {
        id: Some(r.get("id")),
        workflow_id: r.get("workflow_id"),
        name: r.get("name"),
        path: r.get("path"),
        st_mtime: r.get("st_mtime"),
        size: r.get("size"),
        sha256: r.get("sha256"),
    })
    .collect();
    let input_paths: Vec<String> = sqlx::query_scalar(
        "SELECT f.path FROM job_input_file jif JOIN file f ON f.id = jif.file_id \
         WHERE jif.job_id = ? ORDER BY f.id",
    )
    .bind(job_id)
    .fetch_all(&mut **tx)
    .await?;

    let action = build_create_action_entity(
        workflow_id,
        run_id,
        job_id,
        job_name,
        attempt_id,
        &input_paths,
        &output_paths,
    );
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO ro_crate_entity \
         (workflow_id, file_id, entity_id, entity_type, metadata) VALUES (?, NULL, ?, ?, ?)",
    )
    .bind(workflow_id)
    .bind(&action.entity_id)
    .bind(&action.entity_type)
    .bind(entity_metadata_json(&action))
    .execute(&mut **tx)
    .await?;
    if inserted.rows_affected() == 0 {
        // An existing action (a replay, or one `ro-crate add-dataset -j` pre-created) gains
        // the tracked references it lacks. Any other entity that happens to use this ID is
        // left untouched.
        let existing: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, metadata FROM ro_crate_entity WHERE workflow_id = ? AND entity_id = ? \
             AND file_id IS NULL AND entity_type = 'CreateAction'",
        )
        .bind(workflow_id)
        .bind(&action.entity_id)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some((id, metadata)) = existing {
            let mut merged = serde_json::from_str::<serde_json::Value>(&metadata)
                .map(|v| crate::ro_crate_json_ld::metadata_value_to_map(&v))
                .unwrap_or_default();
            if crate::ro_crate_json_ld::merge_create_action(&mut merged, &action.metadata) {
                let merged = serde_json::Value::Object(merged.into_iter().collect());
                sqlx::query("UPDATE ro_crate_entity SET metadata = ? WHERE id = ?")
                    .bind(merged.to_string())
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            }
        }
    }

    for file in &outputs {
        let entity = build_file_entity_with_provenance(
            workflow_id,
            run_id,
            file,
            file.size.and_then(|s| u64::try_from(s).ok()),
            file.sha256.clone(),
            job_id,
            attempt_id,
            &input_paths,
        );
        let metadata = entity_metadata_json(&entity);
        let updated = sqlx::query(
            "UPDATE OR IGNORE ro_crate_entity SET entity_id = ?, entity_type = ?, metadata = ? \
             WHERE workflow_id = ? AND file_id = ?",
        )
        .bind(&entity.entity_id)
        .bind(&entity.entity_type)
        .bind(&metadata)
        .bind(workflow_id)
        .bind(file.id)
        .execute(&mut **tx)
        .await?;
        if updated.rows_affected() > 0 {
            continue;
        }
        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO ro_crate_entity \
             (workflow_id, file_id, entity_id, entity_type, metadata) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(workflow_id)
        .bind(file.id)
        .bind(&entity.entity_id)
        .bind(&entity.entity_type)
        .bind(&metadata)
        .execute(&mut **tx)
        .await?;
        if inserted.rows_affected() == 0 {
            warn!(
                "Skipping RO-Crate entity for output file_id={}: entity_id={} conflicts with an \
                 existing entity workflow_id={} job_id={}",
                file.id.unwrap_or_default(),
                entity.entity_id,
                workflow_id,
                job_id
            );
        }
    }
    Ok(())
}

fn entity_metadata_json(entity: &models::RoCrateEntityModel) -> String {
    serde_json::Value::Object(entity.metadata.clone().into_iter().collect()).to_string()
}

/// Implementation of RO-Crate entity API for the server
#[derive(Clone)]
pub struct RoCrateApiImpl {
    pub context: ApiContext,
}

impl RoCrateApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }

    /// Create RO-Crate File entities for input files of a workflow.
    ///
    /// Input files are identified as files with `st_mtime` set. During workflow creation,
    /// the client auto-detects files that exist on disk and records their modification time.
    /// Updates metadata for files that already have RO-Crate entities;
    /// creates new entities otherwise.
    ///
    /// This is called during `initialize_jobs` when `enable_ro_crate` is true.
    pub async fn create_entities_for_input_files(&self, workflow_id: i64) -> Result<i64, ApiError> {
        // Get all files with st_mtime set (input files)
        let input_files = match sqlx::query!(
            r#"
            SELECT id, workflow_id, name, path, st_mtime, size, sha256
            FROM file
            WHERE workflow_id = $1 AND st_mtime IS NOT NULL
            "#,
            workflow_id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(files) => files,
            Err(e) => {
                return Err(super::database_error_with_msg(
                    e,
                    "Failed to list input files for RO-Crate",
                ));
            }
        };

        // st_mtime is also set on output files once their job completes, so exclude any
        // file a job produces; otherwise re-initializing after a run records outputs as inputs.
        let output_file_ids: std::collections::HashSet<i64> =
            sqlx::query_scalar("SELECT file_id FROM job_output_file WHERE workflow_id = $1")
                .bind(workflow_id)
                .fetch_all(self.context.pool.as_ref())
                .await
                .map_err(|e| {
                    super::database_error_with_msg(e, "Failed to list output files for RO-Crate")
                })?
                .into_iter()
                .collect();
        let input_files: Vec<_> = input_files
            .into_iter()
            .filter(|f| !output_file_ids.contains(&f.id))
            .collect();

        // Get existing RO-Crate entities by file_id for upsert
        let existing_entities: std::collections::HashMap<i64, (i64, String, String)> =
            match sqlx::query!(
                r#"
            SELECT id, file_id, entity_id, metadata
            FROM ro_crate_entity
            WHERE workflow_id = $1 AND file_id IS NOT NULL
            "#,
                workflow_id
            )
            .fetch_all(self.context.pool.as_ref())
            .await
            {
                Ok(rows) => rows
                    .into_iter()
                    .filter_map(|r| r.file_id.map(|fid| (fid, (r.id, r.entity_id, r.metadata))))
                    .collect(),
                Err(e) => {
                    return Err(super::database_error_with_msg(
                        e,
                        "Failed to check existing RO-Crate entities",
                    ));
                }
            };

        let mut upserted_count = 0i64;
        for file in input_files {
            let existing = existing_entities.get(&file.id);
            let file = models::FileModel {
                id: Some(file.id),
                workflow_id: file.workflow_id,
                name: file.name,
                path: file.path,
                st_mtime: file.st_mtime,
                size: file.size,
                sha256: file.sha256,
            };
            // An entity_id that differs from the path is the user's stable identifier
            // (FileSpec::identifier, pre-created at workflow creation). Keep it as @id.
            let identifier = existing
                .map(|(_, entity_id, _)| entity_id.as_str())
                .filter(|id| *id != file.path.as_str());
            let built = build_file_entity(
                workflow_id,
                &file,
                file.size.and_then(|s| u64::try_from(s).ok()),
                file.sha256.clone(),
                identifier,
            );

            // Overlay the built fields on any stored metadata, so fields added by other
            // tools are kept.
            let mut metadata = existing
                .and_then(|(_, _, existing_metadata)| {
                    serde_json::from_str::<serde_json::Value>(existing_metadata).ok()
                })
                .filter(serde_json::Value::is_object)
                .unwrap_or_else(|| serde_json::json!({}));
            // The builder omits stats it does not have, so drop the server-owned ones first;
            // otherwise a cleared size/hash would leave the previous run's values in place.
            if let Some(obj) = metadata.as_object_mut() {
                for key in ["contentSize", "sha256", "dateModified"] {
                    obj.remove(key);
                }
            }
            for (key, value) in built.metadata {
                metadata[key] = value;
            }

            let metadata_str = metadata.to_string();

            // Update existing entity or create new one
            let result = if let Some(&(entity_db_id, _, _)) = existing {
                sqlx::query!(
                    r#"
                    UPDATE ro_crate_entity SET metadata = $1 WHERE id = $2
                    "#,
                    metadata_str,
                    entity_db_id,
                )
                .execute(self.context.pool.as_ref())
                .await
            } else {
                sqlx::query!(
                    r#"
                    INSERT INTO ro_crate_entity
                        (workflow_id, file_id, entity_id, entity_type, metadata)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    workflow_id,
                    file.id,
                    file.path,
                    "File",
                    metadata_str,
                )
                .execute(self.context.pool.as_ref())
                .await
            };

            match result {
                Ok(_) => {
                    debug!(
                        "Upserted RO-Crate entity for input file '{}' (file_id={})",
                        file.path,
                        file.id.unwrap_or_default()
                    );
                    upserted_count += 1;
                }
                Err(e) => {
                    // Log warning but don't fail - RO-Crate is non-blocking
                    log::warn!(
                        "Failed to upsert RO-Crate entity for file '{}': {}",
                        file.path,
                        e
                    );
                }
            }
        }

        debug!(
            "Upserted {} RO-Crate entities for input files in workflow_id={}",
            upserted_count, workflow_id
        );
        Ok(upserted_count)
    }

    /// Create a SoftwareApplication RO-Crate entity for the torc-server binary.
    ///
    /// Records the server's version, binary path, and SHA256 hash. Skips if an
    /// entity with `#software-torc-server-run-id-{run_id}` already exists for this workflow.
    ///
    /// Called during `initialize_jobs` regardless of `enable_ro_crate`.
    pub async fn create_server_software_entity(&self, workflow_id: i64) -> Result<(), ApiError> {
        // Get the current run_id from workflow
        let run_id: i64 =
            sqlx::query_scalar!("SELECT run_id FROM workflow WHERE id = $1", workflow_id,)
                .fetch_optional(self.context.pool.as_ref())
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to get workflow run_id"))?
                .unwrap_or(0);

        let entity_id = format!("#software-torc-server-run-id-{}", run_id);

        // Check if entity already exists
        let exists = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM ro_crate_entity WHERE workflow_id = $1 AND entity_id = $2",
            workflow_id,
            entity_id,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to check existing software entity"))?;

        if exists > 0 {
            debug!(
                "SoftwareApplication entity '{}' already exists for workflow_id={}, skipping",
                entity_id, workflow_id
            );
            return Ok(());
        }

        // Use compile-time constants for version identification
        let version = full_version();
        let exe_path = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let metadata = serde_json::json!({
            "@id": entity_id,
            "@type": typed_entity("SoftwareApplication", "prov:SoftwareAgent"),
            "name": "torc-server",
            "version": version,
            "url": exe_path,
            "torc:git_hash": GIT_HASH,
        });

        let metadata_str = metadata.to_string();
        let entity_type = "SoftwareApplication";

        match sqlx::query!(
            r#"
            INSERT INTO ro_crate_entity (workflow_id, file_id, entity_id, entity_type, metadata)
            VALUES ($1, NULL, $2, $3, $4)
            "#,
            workflow_id,
            entity_id,
            entity_type,
            metadata_str,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "Created SoftwareApplication entity for torc-server version={} \
                     (workflow_id={})",
                    version, workflow_id
                );
            }
            Err(e) => {
                log::warn!(
                    "Failed to create SoftwareApplication entity for torc-server: {}",
                    e
                );
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<C> RoCrateApi<C> for RoCrateApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store one RO-Crate entity record.
    async fn create_ro_crate_entity(
        &self,
        mut body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError> {
        debug!(
            "create_ro_crate_entity - X-Span-ID: {:?}",
            context.get().0.clone()
        );

        let metadata_str = serialize_metadata(&body.metadata)?;
        let result = match sqlx::query!(
            r#"
            INSERT INTO ro_crate_entity (workflow_id, file_id, entity_id, entity_type, metadata)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id AS "id!"
            "#,
            body.workflow_id,
            body.file_id,
            body.entity_id,
            body.entity_type,
            metadata_str,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                let error_string = e.to_string();
                if let Some((message, duplicate_field)) = ro_crate_duplicate_details(&error_string)
                {
                    log_duplicate_ro_crate_entity(
                        "rejected on create",
                        body.workflow_id,
                        duplicate_field,
                        body.file_id,
                        &body.entity_id,
                    );
                    return Ok(
                        CreateRoCrateEntityResponse::UnprocessableContentErrorResponse(
                            message_error_response(message),
                        ),
                    );
                }
                return Err(database_error_with_msg(
                    e,
                    "Failed to create RO-Crate entity record",
                ));
            }
        };
        body.id = Some(result.id);
        debug!(
            "Created RO-Crate entity with ID: {} for workflow_id={}",
            result.id, body.workflow_id
        );
        Ok(CreateRoCrateEntityResponse::SuccessfulResponse(body))
    }

    /// Retrieve an RO-Crate entity record by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError> {
        debug!(
            "get_ro_crate_entity({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query!(
            r#"
            SELECT id, workflow_id, file_id, entity_id, entity_type, metadata
            FROM ro_crate_entity
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                return Ok(GetRoCrateEntityResponse::NotFoundErrorResponse(
                    resource_not_found_response("RO-Crate entity", id),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch RO-Crate entity",
                ));
            }
        };

        let model = models::RoCrateEntityModel {
            id: Some(record.id),
            workflow_id: record.workflow_id,
            file_id: record.file_id,
            entity_id: record.entity_id,
            entity_type: record.entity_type,
            metadata: parse_metadata(&record.metadata),
        };

        Ok(GetRoCrateEntityResponse::SuccessfulResponse(model))
    }

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        file_id: Option<i64>,
        entity_id: Option<String>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError> {
        debug!(
            "list_ro_crate_entities({}, {}, {}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            file_id,
            entity_id,
            sort_by,
            reverse_sort,
            context.get().0.clone()
        );

        let limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let mut where_clauses = vec!["workflow_id = ?".to_string()];
        if file_id.is_some() {
            where_clauses.push("file_id = ?".to_string());
        }
        if entity_id.is_some() {
            where_clauses.push("entity_id = ?".to_string());
        }

        let query = SqlQueryBuilder::new(
            "SELECT id, workflow_id, file_id, entity_id, entity_type, metadata FROM ro_crate_entity"
                .to_string(),
        )
        .with_where(where_clauses.join(" AND "))
        .with_pagination_and_sorting(
            offset,
            limit,
            sort_by,
            reverse_sort,
            "id",
            RO_CRATE_ENTITY_COLUMNS,
        )
        .build();

        let mut query_builder = sqlx::query(sqlx::AssertSqlSafe(&*query)).bind(workflow_id);
        if let Some(file_id) = file_id {
            query_builder = query_builder.bind(file_id);
        }
        if let Some(ref entity_id) = entity_id {
            query_builder = query_builder.bind(entity_id);
        }

        let records = match query_builder.fetch_all(self.context.pool.as_ref()).await {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list RO-Crate entities",
                ));
            }
        };

        let items: Vec<models::RoCrateEntityModel> = records
            .into_iter()
            .map(|record| {
                let metadata_str: String = record.get("metadata");
                models::RoCrateEntityModel {
                    id: Some(record.get("id")),
                    workflow_id: record.get("workflow_id"),
                    file_id: record.get("file_id"),
                    entity_id: record.get("entity_id"),
                    entity_type: record.get("entity_type"),
                    metadata: parse_metadata(&metadata_str),
                }
            })
            .collect();

        // Get total count with the same filters but without pagination.
        let count_query = format!(
            "SELECT COUNT(*) as total FROM ro_crate_entity WHERE {}",
            where_clauses.join(" AND ")
        );
        let mut count_query_builder =
            sqlx::query(sqlx::AssertSqlSafe(&*count_query)).bind(workflow_id);
        if let Some(file_id) = file_id {
            count_query_builder = count_query_builder.bind(file_id);
        }
        if let Some(ref entity_id) = entity_id {
            count_query_builder = count_query_builder.bind(entity_id);
        }

        let total_count = match count_query_builder
            .fetch_one(self.context.pool.as_ref())
            .await
        {
            Ok(row) => row.get("total"),
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to count RO-Crate entities",
                ));
            }
        };

        Ok(ListRoCrateEntitiesResponse::SuccessfulResponse(
            crate::paginated_list_response!(
                models::ListRoCrateEntitiesResponse,
                items,
                offset,
                total_count
            ),
        ))
    }

    /// Update an RO-Crate entity record.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        mut body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError> {
        debug!(
            "update_ro_crate_entity({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let pool = self.context.pool.as_ref();
        // Reject a workflow_id change before checking file_id, so the file check only ever runs
        // against the stored (authorized) workflow and cannot probe another workflow's files.
        if super::ref_outside_workflow(pool, "ro_crate_entity", "id", id, body.workflow_id)
            .await?
            .is_some()
        {
            return Ok(
                if super::workflow_id_mismatch(pool, "ro_crate_entity", id).await? {
                    UpdateRoCrateEntityResponse::UnprocessableContentErrorResponse(
                        super::workflow_id_immutable_response(),
                    )
                } else {
                    UpdateRoCrateEntityResponse::NotFoundErrorResponse(resource_not_found_response(
                        "RO-Crate entity",
                        id,
                    ))
                },
            );
        }

        // body.workflow_id now equals the stored workflow_id.
        if let Some(file_id) = body.file_id
            && let Some(err) =
                super::ref_outside_workflow(pool, "file", "file_id", file_id, body.workflow_id)
                    .await?
        {
            return Ok(UpdateRoCrateEntityResponse::UnprocessableContentErrorResponse(err));
        }

        let metadata_str = serialize_metadata(&body.metadata)?;
        let result = match sqlx::query!(
            r#"
            UPDATE ro_crate_entity
            SET file_id = $1, entity_id = $2, entity_type = $3, metadata = $4
            WHERE id = $5 AND workflow_id = $6
            "#,
            body.file_id,
            body.entity_id,
            body.entity_type,
            metadata_str,
            id,
            body.workflow_id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                let error_string = e.to_string();
                if let Some((message, duplicate_field)) = ro_crate_duplicate_details(&error_string)
                {
                    log_duplicate_ro_crate_entity(
                        "rejected on update",
                        body.workflow_id,
                        duplicate_field,
                        body.file_id,
                        &body.entity_id,
                    );
                    return Ok(
                        UpdateRoCrateEntityResponse::UnprocessableContentErrorResponse(
                            message_error_response(message),
                        ),
                    );
                }
                return Err(database_error_with_msg(
                    e,
                    "Failed to update RO-Crate entity",
                ));
            }
        };

        if result.rows_affected() == 0 {
            return Ok(
                if super::workflow_id_mismatch(pool, "ro_crate_entity", id).await? {
                    UpdateRoCrateEntityResponse::UnprocessableContentErrorResponse(
                        super::workflow_id_immutable_response(),
                    )
                } else {
                    UpdateRoCrateEntityResponse::NotFoundErrorResponse(resource_not_found_response(
                        "RO-Crate entity",
                        id,
                    ))
                },
            );
        }

        body.id = Some(id);
        debug!("Updated RO-Crate entity with ID: {}", id);
        Ok(UpdateRoCrateEntityResponse::SuccessfulResponse(body))
    }

    /// Delete an RO-Crate entity record.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError> {
        debug!(
            "delete_ro_crate_entity({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM ro_crate_entity WHERE id = $1", id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete RO-Crate entity",
                ));
            }
        };

        if result.rows_affected() == 0 {
            return Ok(DeleteRoCrateEntityResponse::NotFoundErrorResponse(
                resource_not_found_response("RO-Crate entity", id),
            ));
        }

        info!("Deleted RO-Crate entity with ID: {}", id);
        Ok(DeleteRoCrateEntityResponse::SuccessfulResponse(
            serde_json::json!({"message": "RO-Crate entity deleted successfully"}),
        ))
    }

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError> {
        debug!(
            "delete_ro_crate_entities(workflow_id={}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM ro_crate_entity WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete RO-Crate entities",
                ));
            }
        };

        let deleted_count = result.rows_affected();
        info!(
            "Deleted {} RO-Crate entities for workflow_id={}",
            deleted_count, workflow_id
        );
        Ok(DeleteRoCrateEntitiesResponse::SuccessfulResponse(
            serde_json::json!({
                "message": format!("Deleted {} RO-Crate entities", deleted_count),
                "deleted_count": deleted_count
            }),
        ))
    }
}
