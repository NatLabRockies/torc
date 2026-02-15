//! User data-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateUserDataResponse, DeleteAllUserDataResponse, DeleteUserDataResponse, GetUserDataResponse,
    ListMissingUserDataResponse, ListUserDataResponse, UpdateUserDataResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, database_error_with_msg};

/// Trait defining user data-related API operations
#[async_trait]
pub trait UserDataApi<C> {
    /// Store user data.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError>;

    /// Delete all user data for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError>;

    /// Retrieve user data by ID.
    async fn get_user_data(&self, id: i64, context: &C) -> Result<GetUserDataResponse, ApiError>;

    /// Return user data that are marked as required but missing.
    async fn list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError>;

    /// Retrieve all user data for one workflow.
    async fn list_user_data(
        &self,
        workflow_id: i64,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        is_ephemeral: Option<bool>,
        context: &C,
    ) -> Result<ListUserDataResponse, ApiError>;

    /// Update user data.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError>;

    /// Delete user data.
    async fn delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError>;
}

/// Implementation of user data API for the server
#[derive(Clone)]
pub struct UserDataApiImpl {
    pub context: ApiContext,
}

const USER_DATA_COLUMNS: &[&str] = &["id", "workflow_id", "name", "is_ephemeral", "data"];

impl UserDataApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> UserDataApi<C> for UserDataApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store user data.
    async fn create_user_data(
        &self,
        mut body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError> {
        debug!(
            "create_user_data({:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            body,
            consumer_job_id,
            producer_job_id,
            context.get().0.clone()
        );

        // Convert boolean to integer for SQLite
        let is_ephemeral_int = if body.is_ephemeral.unwrap_or(false) {
            1
        } else {
            0
        };

        // Serialize data to JSON string if present
        // Treat JSON null as SQL NULL
        let data = match &body.data {
            Some(value) if !value.is_null() => Some(
                serde_json::to_string(value)
                    .map_err(|e| ApiError(format!("Failed to serialize data: {}", e)))?,
            ),
            _ => None,
        };

        // Insert the user_data record
        let user_data_result = match sqlx::query!(
            r#"
            INSERT INTO user_data (workflow_id, name, is_ephemeral, data)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
            body.workflow_id,
            body.name,
            is_ephemeral_int,
            data
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to create user data record",
                ));
            }
        };

        // Set the ID in the response body
        body.id = Some(user_data_result.id);

        // Handle producer job association (output user data)
        if let Some(producer_id) = producer_job_id {
            match sqlx::query!(
                r#"
                INSERT INTO job_output_user_data (job_id, user_data_id)
                VALUES ($1, $2)
                "#,
                producer_id,
                user_data_result.id
            )
            .execute(self.context.pool.as_ref())
            .await
            {
                Ok(_) => {
                    debug!(
                        "Created output association between job {} and user_data {}",
                        producer_id, user_data_result.id
                    );
                }
                Err(e) => {
                    return Err(database_error_with_msg(
                        e,
                        "Failed to create user data association",
                    ));
                }
            }
        }

        // Handle consumer job association (input user data)
        if let Some(consumer_id) = consumer_job_id {
            match sqlx::query!(
                r#"
                INSERT INTO job_input_user_data (job_id, user_data_id)
                VALUES ($1, $2)
                "#,
                consumer_id,
                user_data_result.id
            )
            .execute(self.context.pool.as_ref())
            .await
            {
                Ok(_) => {
                    debug!(
                        "Created input association between job {} and user_data {}",
                        consumer_id, user_data_result.id
                    );
                }
                Err(e) => {
                    return Err(database_error_with_msg(
                        e,
                        "Failed to create user data association",
                    ));
                }
            }
        }

        debug!("User data inserted with id: {:?}", user_data_result.id);
        Ok(CreateUserDataResponse::SuccessfulResponse(body))
    }

    /// Delete all user data for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        debug!(
            "delete_all_user_data({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        // Delete all user_data records for the workflow
        // This will cascade delete associated job input/output relationships
        let result = match sqlx::query!(
            r#"
            DELETE FROM user_data 
            WHERE workflow_id = $1
            "#,
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete user data"));
            }
        };

        let rows_affected = result.rows_affected();
        info!(
            "Deleted {} user_data records for workflow {}",
            rows_affected, workflow_id
        );

        let response_data = serde_json::json!({
            "message": format!("Deleted {} user data records for workflow {}", rows_affected, workflow_id),
            "deleted_count": rows_affected
        });

        Ok(DeleteAllUserDataResponse::SuccessfulResponse(response_data))
    }

    /// Retrieve user data by ID.
    async fn get_user_data(&self, id: i64, context: &C) -> Result<GetUserDataResponse, ApiError> {
        debug!(
            "get_user_data({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Query the user_data table for the specified ID
        let row = match sqlx::query(
            "SELECT id, workflow_id, name, is_ephemeral, data FROM user_data WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&*self.context.pool)
        .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("User data not found with ID: {}", id)
                }));
                return Ok(GetUserDataResponse::NotFoundErrorResponse(error_response));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch user data"));
            }
        };

        // Convert database row to UserDataModel
        // SQLite INTEGER field (0/1) is converted to proper boolean value
        let data_str: Option<String> = row.get("data");
        let data = data_str.and_then(|s| serde_json::from_str(&s).ok());

        let user_data = models::UserDataModel {
            id: Some(row.get("id")),
            workflow_id: row.get("workflow_id"),
            name: row.get("name"),
            is_ephemeral: Some(row.get::<i64, _>("is_ephemeral") != 0), // Convert INTEGER to bool
            data,
        };

        Ok(GetUserDataResponse::SuccessfulResponse(user_data))
    }

    /// Return user data that are marked as required but missing.
    async fn list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError> {
        debug!(
            "list_missing_user_data({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Get missing user data IDs that should have been created by the user
        let user_created_missing = match self.find_missing_user_created_data(id).await {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };

        // Get missing user data IDs that should have been created by jobs
        let job_created_missing = match self.find_missing_job_created_data(id).await {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };

        // Combine both sets of missing IDs
        let mut all_missing_ids = user_created_missing;
        all_missing_ids.extend(job_created_missing);
        all_missing_ids.sort();
        all_missing_ids.dedup();

        let response = models::ListMissingUserDataResponse {
            user_data: all_missing_ids,
        };

        Ok(ListMissingUserDataResponse::SuccessfulResponse(response))
    }

    /// Retrieve all user data for one workflow.
    async fn list_user_data(
        &self,
        workflow_id: i64,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        is_ephemeral: Option<bool>,
        context: &C,
    ) -> Result<ListUserDataResponse, ApiError> {
        debug!(
            "list_user_data({}, {:?}, {:?}, {}, {}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            consumer_job_id,
            producer_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            name,
            is_ephemeral,
            context.get().0.clone()
        );

        // Build base query with potential JOINs
        let mut base_query = "SELECT DISTINCT ud.id, ud.workflow_id, ud.name, ud.is_ephemeral, ud.data FROM user_data ud".to_string();
        let mut joins = Vec::new();
        let mut where_conditions = vec!["ud.workflow_id = ?".to_string()];

        // Add JOINs for job associations if specified
        if consumer_job_id.is_some() {
            joins.push(
                "INNER JOIN job_input_user_data jiud ON ud.id = jiud.user_data_id".to_string(),
            );
            where_conditions.push("jiud.job_id = ?".to_string());
        }

        if producer_job_id.is_some() {
            joins.push(
                "INNER JOIN job_output_user_data joud ON ud.id = joud.user_data_id".to_string(),
            );
            where_conditions.push("joud.job_id = ?".to_string());
        }

        // Add name filter
        if name.is_some() {
            where_conditions.push("ud.name LIKE ?".to_string());
        }

        // Add is_ephemeral filter
        if is_ephemeral.is_some() {
            where_conditions.push("ud.is_ephemeral = ?".to_string());
        }

        // Combine base query with JOINs
        if !joins.is_empty() {
            base_query.push(' ');
            base_query.push_str(&joins.join(" "));
        }

        let where_clause = where_conditions.join(" AND ");

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if USER_DATA_COLUMNS.contains(&col.as_str()) {
                Some(format!("ud.{}", col))
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        // Build the complete query with pagination and sorting
        let query = super::SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(
                offset,
                limit,
                validated_sort_by,
                reverse_sort,
                "ud.id",
                USER_DATA_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);

        // Bind parameters manually in order
        sqlx_query = sqlx_query.bind(workflow_id);
        if let Some(consumer_id) = consumer_job_id {
            sqlx_query = sqlx_query.bind(consumer_id);
        }
        if let Some(producer_id) = producer_job_id {
            sqlx_query = sqlx_query.bind(producer_id);
        }
        if let Some(name_filter) = &name {
            sqlx_query = sqlx_query.bind(format!("%{}%", name_filter));
        }
        if let Some(ephemeral_filter) = is_ephemeral {
            let ephemeral_int = if ephemeral_filter { 1 } else { 0 };
            sqlx_query = sqlx_query.bind(ephemeral_int);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list user data"));
            }
        };

        let mut items: Vec<models::UserDataModel> = Vec::new();
        for record in records {
            items.push(models::UserDataModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                name: record.get("name"),
                is_ephemeral: Some(record.get::<i64, _>("is_ephemeral") != 0), // Convert INTEGER to bool
                data: Some(record.get("data")),
            });
        }

        // Build count query - same as main query but with COUNT(DISTINCT ud.id)
        let mut count_base_query =
            "SELECT COUNT(DISTINCT ud.id) as total FROM user_data ud".to_string();
        if !joins.is_empty() {
            count_base_query.push(' ');
            count_base_query.push_str(&joins.join(" "));
        }

        let count_query = super::SqlQueryBuilder::new(count_base_query)
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(consumer_id) = consumer_job_id {
            count_sqlx_query = count_sqlx_query.bind(consumer_id);
        }
        if let Some(producer_id) = producer_job_id {
            count_sqlx_query = count_sqlx_query.bind(producer_id);
        }
        if let Some(name_filter) = &name {
            count_sqlx_query = count_sqlx_query.bind(format!("%{}%", name_filter));
        }
        if let Some(ephemeral_filter) = is_ephemeral {
            let ephemeral_int = if ephemeral_filter { 1 } else { 0 };
            count_sqlx_query = count_sqlx_query.bind(ephemeral_int);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list user data"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_user_data({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListUserDataResponse::SuccessfulResponse(
            models::ListUserDataResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update user data.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        debug!(
            "update_user_data({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First check if the user data exists
        match self.get_user_data(id, context).await? {
            GetUserDataResponse::SuccessfulResponse(_) => {}
            GetUserDataResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateUserDataResponse::ForbiddenErrorResponse(err));
            }
            GetUserDataResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateUserDataResponse::NotFoundErrorResponse(err));
            }
            GetUserDataResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get user data".to_string()));
            }
        };

        // Convert boolean to integer for SQLite if provided
        let is_ephemeral_int = body.is_ephemeral.map(|val| if val { 1 } else { 0 });

        // Serialize data to JSON string if present
        // Treat JSON null as SQL NULL
        let data = match &body.data {
            Some(value) if !value.is_null() => Some(
                serde_json::to_string(value)
                    .map_err(|e| ApiError(format!("Failed to serialize data: {}", e)))?,
            ),
            _ => None,
        };

        // Update the user_data record using COALESCE to only update non-null fields
        let result = match sqlx::query!(
            r#"
            UPDATE user_data
            SET
                name = COALESCE($1, name),
                is_ephemeral = COALESCE($2, is_ephemeral),
                data = COALESCE($3, data)
            WHERE id = $4
            "#,
            body.name,
            is_ephemeral_int,
            data,
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update user data"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("User data not found with ID: {}", id)
            }));
            return Ok(UpdateUserDataResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Return the updated user data by fetching it again
        let updated_user_data = match self.get_user_data(id, context).await? {
            GetUserDataResponse::SuccessfulResponse(user_data) => user_data,
            _ => return Err(ApiError("Failed to get updated user data".to_string())),
        };

        debug!("Modified user data with id: {}", id);
        Ok(UpdateUserDataResponse::SuccessfulResponse(
            updated_user_data,
        ))
    }

    /// Delete user data.
    async fn delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        debug!(
            "delete_user_data({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First check if the user data exists by trying to fetch it
        let existing_user_data = match self.get_user_data(id, context).await? {
            GetUserDataResponse::SuccessfulResponse(user_data) => user_data,
            GetUserDataResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteUserDataResponse::ForbiddenErrorResponse(err));
            }
            GetUserDataResponse::NotFoundErrorResponse(e) => {
                return Ok(DeleteUserDataResponse::NotFoundErrorResponse(e));
            }
            GetUserDataResponse::DefaultErrorResponse(e) => {
                return Err(ApiError(format!(
                    "Error deleting user data with ID: {}: {:?}",
                    id, e
                )));
            }
        };

        // Delete the user_data record
        // This will cascade delete associated job input/output relationships
        let result = match sqlx::query!(
            r#"
            DELETE FROM user_data 
            WHERE id = $1
            "#,
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete user data"));
            }
        };

        if result.rows_affected() == 0 {
            return Err(ApiError(format!("User data not found with ID: {}", id)));
        }

        info!(
            "Deleted user_data {} (name: {:?}) from workflow {}",
            id, existing_user_data.name, existing_user_data.workflow_id
        );

        Ok(DeleteUserDataResponse::SuccessfulResponse(
            existing_user_data,
        ))
    }
}

impl UserDataApiImpl {
    /// Find user_data record IDs that should have been created by the user but are missing.
    /// This includes user_data records that are referenced in job_input_user_data but are not
    /// present in the job_output_user_data table (meaning they should be user-created) and
    /// don't actually exist in the user_data table.
    async fn find_missing_user_created_data(&self, workflow_id: i64) -> Result<Vec<i64>, ApiError> {
        let rows = match sqlx::query!(
            r#"
            SELECT DISTINCT jiud.user_data_id
            FROM job_input_user_data jiud
            INNER JOIN job j ON jiud.job_id = j.id
            INNER JOIN user_data ud ON jiud.user_data_id = ud.id
            WHERE j.workflow_id = $1
            AND ud.data IS NULL
            AND jiud.user_data_id NOT IN (
                SELECT joud.user_data_id
                FROM job_output_user_data joud
            )
            "#,
            workflow_id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list missing user data",
                ));
            }
        };

        Ok(rows.into_iter().map(|row| row.user_data_id).collect())
    }

    /// Find user_data record IDs that should have been created by a job but are missing.
    /// This includes user_data records that are referenced in job_output_user_data where
    /// the job's status is JobStatus::Completed (meaning the job completed successfully and should have created
    /// the user_data) but the user_data field is NULL.
    async fn find_missing_job_created_data(&self, workflow_id: i64) -> Result<Vec<i64>, ApiError> {
        let completed_status = models::JobStatus::Completed.to_int();

        let rows = match sqlx::query!(
            r#"
            SELECT DISTINCT joud.user_data_id
            FROM job_output_user_data joud
            INNER JOIN job j ON joud.job_id = j.id
            INNER JOIN user_data ud ON joud.user_data_id = ud.id
            WHERE j.workflow_id = $1
            AND j.status = $2
            AND ud.data IS NULL
            "#,
            workflow_id,
            completed_status
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list missing user data",
                ));
            }
        };

        Ok(rows.into_iter().map(|row| row.user_data_id).collect())
    }
}
