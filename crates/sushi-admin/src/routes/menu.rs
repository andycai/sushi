use axum::{extract::State, routing::get, Json, Router};
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MenuItem {
    pub id: i64,
    pub label: String,
    pub icon: Option<String>,
    pub position: i64,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuResponse {
    pub menu: Vec<MenuItem>,
}

pub async fn menu_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let rows = ctx.db
        .query(
            "SELECT id, label, icon, position, parent_id, route
             FROM menu_items
             ORDER BY position ASC, id ASC",
            vec![]
        )
        .await
        .unwrap_or_default();

    let menu: Vec<MenuItem> = rows.into_iter().map(|row| MenuItem {
        id: row.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        label: row.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        icon: row.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string()),
        position: row.get("position").and_then(|v| v.as_i64()).unwrap_or(0),
        parent_id: row.get("parent_id").and_then(|v| v.as_i64()),
        route: row.get("route").and_then(|v| v.as_str()).map(|s| s.to_string()),
    }).collect();

    Json(MenuResponse { menu })
}

pub fn routes() -> Router<SushiContext> {
    Router::new().route("/admin/api/menu", get(menu_api))
}
