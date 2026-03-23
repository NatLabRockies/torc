#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::models::{DeleteCountResponse, EventModel, ListEventsResponse};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct EventsListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub category: Option<String>,
    #[param(nullable = true)]
    pub after_timestamp: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteEventsQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/events",
    operation_id = "create_event",
    request_body = EventModel,
    responses((status = 200, description = "Successful response", body = EventModel))
)]
pub async fn create_event(Json(mut body): Json<EventModel>) -> Json<EventModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/events",
    operation_id = "delete_events",
    params(DeleteEventsQuery),
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_events(Query(_query): Query<DeleteEventsQuery>) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/events",
    operation_id = "list_events",
    params(EventsListQuery),
    responses((status = 200, description = "Successful response", body = ListEventsResponse))
)]
pub async fn list_events(Query(query): Query<EventsListQuery>) -> Json<ListEventsResponse> {
    Json(ListEventsResponse {
        items: vec![],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 0,
        total_count: 0,
        has_more: false,
    })
}

#[utoipa::path(
    delete,
    path = "/events/{id}",
    operation_id = "delete_event",
    params(("id" = i64, Path, description = "ID of the event record.")),
    responses((status = 200, description = "Successful response", body = EventModel))
)]
pub async fn delete_event(Path(id): Path<i64>) -> Json<EventModel> {
    Json(example_event(Some(id)))
}

#[utoipa::path(
    get,
    path = "/events/{id}",
    operation_id = "get_event",
    params(("id" = i64, Path, description = "ID of the event record.")),
    responses((status = 200, description = "Successful response", body = EventModel))
)]
pub async fn get_event(Path(id): Path<i64>) -> Json<EventModel> {
    Json(example_event(Some(id)))
}

#[utoipa::path(
    put,
    path = "/events/{id}",
    operation_id = "update_event",
    params(("id" = i64, Path, description = "ID of the event.")),
    request_body = EventModel,
    responses((status = 200, description = "Successful response", body = EventModel))
)]
pub async fn update_event(
    Path(id): Path<i64>,
    Json(mut body): Json<EventModel>,
) -> Json<EventModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_event(id: Option<i64>) -> EventModel {
    EventModel {
        id,
        workflow_id: 6,
        timestamp: 1_742_500_000_000,
        data: serde_json::json!({"event": "created"}),
    }
}
