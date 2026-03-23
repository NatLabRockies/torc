#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::models::{DeleteCountResponse, ListSlurmSchedulersResponse, SlurmSchedulerModel};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SlurmSchedulersListQuery {
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
    pub name: Option<String>,
    #[param(nullable = true)]
    pub account: Option<String>,
    #[param(nullable = true)]
    pub gres: Option<String>,
    #[param(nullable = true)]
    pub mem: Option<String>,
    #[param(nullable = true)]
    pub nodes: Option<i64>,
    #[param(nullable = true)]
    pub partition: Option<String>,
    #[param(nullable = true)]
    pub qos: Option<String>,
    #[param(nullable = true)]
    pub tmp: Option<String>,
    #[param(nullable = true)]
    pub walltime: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteSlurmSchedulersQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/slurm_schedulers",
    operation_id = "create_slurm_scheduler",
    request_body = SlurmSchedulerModel,
    responses((status = 200, description = "Successful response", body = SlurmSchedulerModel))
)]
pub async fn create_slurm_scheduler(
    Json(mut body): Json<SlurmSchedulerModel>,
) -> Json<SlurmSchedulerModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/slurm_schedulers",
    operation_id = "delete_slurm_schedulers",
    params(DeleteSlurmSchedulersQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_slurm_schedulers(
    Query(_query): Query<DeleteSlurmSchedulersQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/slurm_schedulers",
    operation_id = "list_slurm_schedulers",
    params(SlurmSchedulersListQuery),
    responses((status = 200, description = "Successful response", body = ListSlurmSchedulersResponse))
)]
pub async fn list_slurm_schedulers(
    Query(query): Query<SlurmSchedulersListQuery>,
) -> Json<ListSlurmSchedulersResponse> {
    Json(ListSlurmSchedulersResponse {
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
    path = "/slurm_schedulers/{id}",
    operation_id = "delete_slurm_scheduler",
    params(("id" = i64, Path, description = "Slurm compute node configuration ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = SlurmSchedulerModel))
)]
pub async fn delete_slurm_scheduler(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<SlurmSchedulerModel> {
    Json(example_slurm_scheduler(Some(id)))
}

#[utoipa::path(
    get,
    path = "/slurm_schedulers/{id}",
    operation_id = "get_slurm_scheduler",
    params(("id" = i64, Path, description = "Slurm compute node configuration ID")),
    responses((status = 200, description = "Successful response", body = SlurmSchedulerModel))
)]
pub async fn get_slurm_scheduler(Path(id): Path<i64>) -> Json<SlurmSchedulerModel> {
    Json(example_slurm_scheduler(Some(id)))
}

#[utoipa::path(
    put,
    path = "/slurm_schedulers/{id}",
    operation_id = "update_slurm_scheduler",
    params(("id" = i64, Path, description = "Slurm compute node configuration ID")),
    request_body = SlurmSchedulerModel,
    responses((status = 200, description = "Successful response", body = SlurmSchedulerModel))
)]
pub async fn update_slurm_scheduler(
    Path(id): Path<i64>,
    Json(mut body): Json<SlurmSchedulerModel>,
) -> Json<SlurmSchedulerModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_slurm_scheduler(id: Option<i64>) -> SlurmSchedulerModel {
    SlurmSchedulerModel {
        id,
        workflow_id: 6,
        name: Some("name".to_string()),
        account: "account".to_string(),
        gres: Some("gres".to_string()),
        mem: Some("mem".to_string()),
        nodes: 1,
        ntasks_per_node: Some(5),
        partition: Some("partition".to_string()),
        qos: Some("normal".to_string()),
        tmp: Some("tmp".to_string()),
        walltime: "walltime".to_string(),
        extra: Some("extra".to_string()),
    }
}
