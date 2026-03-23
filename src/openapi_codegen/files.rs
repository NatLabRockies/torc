#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::models::{DeleteCountResponse, FileModel, ListFilesResponse};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct FilesListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub produced_by_job_id: Option<i64>,
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
    pub path: Option<String>,
    #[param(nullable = true)]
    pub is_output: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteFilesQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/files",
    operation_id = "create_file",
    request_body = FileModel,
    responses((status = 200, description = "Successful response", body = FileModel))
)]
pub async fn create_file(Json(mut body): Json<FileModel>) -> Json<FileModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/files",
    operation_id = "delete_files",
    params(DeleteFilesQuery),
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_files(Query(_query): Query<DeleteFilesQuery>) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/files",
    operation_id = "list_files",
    params(FilesListQuery),
    responses((status = 200, description = "Successful response", body = ListFilesResponse))
)]
pub async fn list_files(Query(query): Query<FilesListQuery>) -> Json<ListFilesResponse> {
    Json(ListFilesResponse {
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
    path = "/files/{id}",
    operation_id = "delete_file",
    params(("id" = i64, Path, description = "ID of the file record.")),
    responses((status = 200, description = "Successful response", body = FileModel))
)]
pub async fn delete_file(Path(id): Path<i64>) -> Json<FileModel> {
    Json(example_file(Some(id)))
}

#[utoipa::path(
    get,
    path = "/files/{id}",
    operation_id = "get_file",
    params(("id" = i64, Path, description = "ID of the files record")),
    responses((status = 200, description = "Successful response", body = FileModel))
)]
pub async fn get_file(Path(id): Path<i64>) -> Json<FileModel> {
    Json(example_file(Some(id)))
}

#[utoipa::path(
    put,
    path = "/files/{id}",
    operation_id = "update_file",
    params(("id" = i64, Path, description = "ID of the file.")),
    request_body = FileModel,
    responses((status = 200, description = "Successful response", body = FileModel))
)]
pub async fn update_file(Path(id): Path<i64>, Json(mut body): Json<FileModel>) -> Json<FileModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_file(id: Option<i64>) -> FileModel {
    FileModel {
        id,
        workflow_id: 6,
        name: "name".to_string(),
        path: "path".to_string(),
        st_mtime: Some(1.4658129805029452),
    }
}
