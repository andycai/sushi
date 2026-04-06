use crate::routes::{dashboard, plugins, users, config, logs};
use axum::{routing::get, Router, Json};
use serde_json::json;

pub fn build_admin_router() -> Router {
    Router::new()
        .route("/", get(dashboard::dashboard_page))
        .route("/plugins", get(plugins::plugins_page))
        .route("/users", get(users::users_page))
        .route("/config", get(config::config_page))
        .route("/logs", get(logs::logs_page))
        .route("/api/plugins", get(list_plugins_api))
}

async fn list_plugins_api() -> Json<serde_json::Value> {
    Json(json!([]))
}
