use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::OpenApiAppState;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PingResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VersionResponse {
    pub version: String,
    pub api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_hash: Option<String>,
}

#[utoipa::path(
    get,
    path = "/ping",
    operation_id = "ping",
    responses(
        (status = 200, description = "Successful response", body = PingResponse)
    )
)]
pub async fn ping() -> Json<PingResponse> {
    Json(PingResponse {
        status: "ok".to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/version",
    operation_id = "get_version",
    responses(
        (status = 200, description = "Successful response", body = VersionResponse)
    )
)]
pub async fn version(State(state): State<OpenApiAppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        version: state.version,
        api_version: state.api_version,
        git_hash: (!state.access_control_enabled).then_some(state.git_hash),
    })
}
