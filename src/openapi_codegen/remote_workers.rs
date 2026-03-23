#![allow(dead_code)]

use axum::{Json, extract::Path};

use crate::models::RemoteWorkerModel;

#[utoipa::path(
    post,
    path = "/workflows/{id}/remote_workers",
    operation_id = "create_remote_workers",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = Vec<String>,
    responses((status = 200, description = "Successful response", body = [RemoteWorkerModel]))
)]
pub async fn create_remote_workers(
    Path(id): Path<i64>,
    Json(workers): Json<Vec<String>>,
) -> Json<Vec<RemoteWorkerModel>> {
    Json(
        workers
            .into_iter()
            .map(|worker| RemoteWorkerModel {
                worker,
                workflow_id: id,
            })
            .collect(),
    )
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/remote_workers",
    operation_id = "list_remote_workers",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = [RemoteWorkerModel]))
)]
pub async fn list_remote_workers(Path(id): Path<i64>) -> Json<Vec<RemoteWorkerModel>> {
    Json(vec![RemoteWorkerModel {
        worker: "user@hostname:22".to_string(),
        workflow_id: id,
    }])
}

#[utoipa::path(
    delete,
    path = "/workflows/{id}/remote_workers/{worker}",
    operation_id = "delete_remote_worker",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("worker" = String, Path, description = "Worker address")
    ),
    responses((status = 200, description = "Successful response", body = RemoteWorkerModel))
)]
pub async fn delete_remote_worker(
    Path((id, worker)): Path<(i64, String)>,
) -> Json<RemoteWorkerModel> {
    Json(RemoteWorkerModel {
        worker,
        workflow_id: id,
    })
}
