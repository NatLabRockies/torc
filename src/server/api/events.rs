//! Event-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use chrono::Utc;
use log::{debug, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateEventResponse, DeleteEventResponse, DeleteEventsResponse, GetEventResponse,
    ListEventsResponse, UpdateEventResponse,
};

use crate::models;

use super::{
    ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg,
    json_parse_error,
};

/// Trait defining event-related API operations
#[async_trait]
pub trait EventsApi<C> {
    /// Store an event.
    async fn create_event(
        &self,
        mut body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError>;

    /// Retrieve an event by ID.
    async fn get_event(&self, id: i64, context: &C) -> Result<GetEventResponse, ApiError>;

    /// Retrieve all events for one workflow.
    async fn list_events(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        category: Option<String>,
        after_timestamp: Option<i64>,
        context: &C,
    ) -> Result<ListEventsResponse, ApiError>;

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError>;

    /// Delete an event.
    async fn delete_event(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventResponse, ApiError>;

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError>;
}

/// Implementation of events API for the server
#[derive(Clone)]
pub struct EventsApiImpl {
    pub context: ApiContext,
}

const EVENT_COLUMNS: &[&str] = &["id", "workflow_id", "timestamp", "data"];

impl EventsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> EventsApi<C> for EventsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store an event.
    async fn create_event(
        &self,
        mut body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError> {
        debug!(
            "create_event({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        // Store timestamp as milliseconds since epoch (UTC)
        let timestamp = Utc::now().timestamp_millis();
        let data = body.data.to_string();

        let result = match sqlx::query(
            r#"
            INSERT INTO event
            (
                workflow_id,
                timestamp,
                data
            )
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(body.workflow_id)
        .bind(timestamp)
        .bind(&data)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to create event"));
            }
        };

        body.id = Some(result.get("id"));
        body.timestamp = timestamp;
        Ok(CreateEventResponse::SuccessfulResponse(body))
    }

    /// Retrieve an event by ID.
    async fn get_event(&self, id: i64, context: &C) -> Result<GetEventResponse, ApiError> {
        debug!(
            "get_event({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, workflow_id, timestamp, data
            FROM event
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(rec)) => rec,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Event not found with ID: {}", id)
                }));
                return Ok(GetEventResponse::NotFoundErrorResponse(error_response));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch event"));
            }
        };

        let data_str: String = record.get("data");
        let data = match serde_json::from_str(&data_str) {
            Ok(json) => json,
            Err(e) => {
                return Err(json_parse_error(e));
            }
        };

        let event = models::EventModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            timestamp: record.get("timestamp"),
            data,
        };

        Ok(GetEventResponse::SuccessfulResponse(event))
    }

    /// Retrieve all events for one workflow.
    async fn list_events(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        category: Option<String>,
        after_timestamp: Option<i64>,
        context: &C,
    ) -> Result<ListEventsResponse, ApiError> {
        debug!(
            "list_events({}, {}, {}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            category,
            after_timestamp,
            context.get().0.clone()
        );

        // Build base query
        let base_query = "SELECT id, workflow_id, timestamp, data FROM event".to_string();

        // Build WHERE clause conditions
        let mut where_conditions = vec!["workflow_id = ?".to_string()];

        // Add timestamp filter if provided (timestamp is stored as INTEGER milliseconds)
        // The after_timestamp parameter is in milliseconds since epoch
        if after_timestamp.is_some() {
            where_conditions.push("timestamp > ?".to_string());
        }

        // Note: Category filtering is not implemented in current schema
        let _category = category; // Acknowledge the parameter to avoid unused warnings

        let where_clause = where_conditions.join(" AND ");

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if EVENT_COLUMNS.contains(&col.as_str()) {
                Some(col.clone())
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        // Build the complete query with pagination and sorting
        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(offset, limit, validated_sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);

        // Bind workflow_id
        sqlx_query = sqlx_query.bind(workflow_id);

        // Bind timestamp if provided (direct integer comparison)
        if let Some(ts) = after_timestamp {
            sqlx_query = sqlx_query.bind(ts);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list events"));
            }
        };

        let mut items: Vec<models::EventModel> = Vec::new();
        for record in records {
            let data_str: String = record.get("data");
            let data = match serde_json::from_str(&data_str) {
                Ok(json) => json,
                Err(e) => {
                    return Err(json_parse_error(e));
                }
            };

            items.push(models::EventModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                timestamp: record.get("timestamp"),
                data,
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query = SqlQueryBuilder::new("SELECT COUNT(*) as total FROM event".to_string())
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);

        // Bind timestamp for count query if provided
        if let Some(ts) = after_timestamp {
            count_sqlx_query = count_sqlx_query.bind(ts);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list events"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_events({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListEventsResponse::SuccessfulResponse(
            models::ListEventsResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError> {
        debug!(
            "update_event({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the existing event to ensure it exists
        match self.get_event(id, context).await? {
            GetEventResponse::SuccessfulResponse(_) => {}
            GetEventResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateEventResponse::ForbiddenErrorResponse(err));
            }
            GetEventResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateEventResponse::NotFoundErrorResponse(err));
            }
            GetEventResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get event".to_string()));
            }
        };

        // Convert body to string for database storage
        let data_str = body.to_string();

        let result = match sqlx::query(
            r#"
            UPDATE event
            SET data = $1
            WHERE id = $2
            "#,
        )
        .bind(data_str)
        .bind(id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update event"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Event not found with ID: {}", id)
            }));
            return Ok(UpdateEventResponse::NotFoundErrorResponse(error_response));
        }

        // Return the updated event by fetching it again
        let updated_event = match self.get_event(id, context).await? {
            GetEventResponse::SuccessfulResponse(event) => event,
            _ => return Err(ApiError("Failed to get updated event".to_string())),
        };

        debug!("Modified event with id: {}", id);
        Ok(UpdateEventResponse::SuccessfulResponse(updated_event))
    }

    /// Delete an event.
    async fn delete_event(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventResponse, ApiError> {
        debug!(
            "delete_event({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the event to ensure it exists and extract the EventModel
        let event = match self.get_event(id, context).await? {
            GetEventResponse::SuccessfulResponse(event) => event,
            GetEventResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteEventResponse::ForbiddenErrorResponse(err));
            }
            GetEventResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteEventResponse::NotFoundErrorResponse(err));
            }
            GetEventResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get event".to_string()));
            }
        };

        match sqlx::query!(r#"DELETE FROM event WHERE id = $1"#, id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() > 1 {
                    Err(ApiError(format!(
                        "Database error: Unexpected number of rows affected: {}",
                        res.rows_affected()
                    )))
                } else if res.rows_affected() == 0 {
                    Err(ApiError("Database error: No rows affected".to_string()))
                } else {
                    info!("Deleted event with id: {}", id);
                    Ok(DeleteEventResponse::SuccessfulResponse(event))
                }
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete event")),
        }
    }

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError> {
        debug!(
            "delete_events(\"{}\", {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );
        Err(ApiError("Api-Error: Operation is NOT implemented".into()))
    }
}
