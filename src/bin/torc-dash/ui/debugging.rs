//! Not yet ported.

use std::sync::Arc;

use axum::Router;
use maud::{Markup, html};

use super::Ctx;
use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
}

pub async fn view(_st: &Arc<AppState>, _cx: &Ctx) -> Markup {
    html! { div.page { div.empty { "Not ported yet." } } }
}
