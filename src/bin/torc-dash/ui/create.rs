//! Not yet ported.

use std::sync::Arc;

use axum::Router;

use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
}
