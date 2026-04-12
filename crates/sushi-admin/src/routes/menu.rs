use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;

#[derive(Debug, Serialize)]
pub struct MenuItem {
    pub id: i64,
    pub label: String,
    pub icon: Option<String>,
    pub position: i64,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateMenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub position: Option<i64>,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMenuItem {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub position: Option<i64>,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct MenuResponse {
    pub menu: Vec<MenuItem>,
}

pub async fn menu_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let rows = ctx.db
        .query(
            "SELECT id, label, icon, position, parent_id, route, is_hidden
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
        is_hidden: row.get("is_hidden").and_then(|v| v.as_i64()).unwrap_or(0) != 0,
    }).collect();

    Json(MenuResponse { menu })
}

pub async fn create_menu_item(
    State(ctx): State<SushiContext>,
    Json(payload): Json<CreateMenuItem>,
) -> impl IntoResponse {
    let position = payload.position.unwrap_or(0);
    let is_hidden = payload.is_hidden.unwrap_or(false);

    let result = ctx.db.execute(
        "INSERT INTO menu_items (label, icon, position, parent_id, route, is_hidden)
         VALUES (?, ?, ?, ?, ?, ?)",
        vec![
            payload.label.into(),
            payload.icon.clone().into(),
            position.into(),
            payload.parent_id.into(),
            payload.route.clone().into(),
            (if is_hidden { 1 } else { 0 }).into(),
        ],
    ).await;

    match result {
        Ok(_) => (
            axum::http::StatusCode::CREATED,
            Json(serde_json::json!({ "success": true })),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn update_menu_item(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateMenuItem>,
) -> impl IntoResponse {
    // Build dynamic update query
    let mut set_clauses = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    if let Some(label) = &payload.label {
        set_clauses.push("label = ?");
        values.push(label.clone().into());
    }
    if let Some(icon) = &payload.icon {
        set_clauses.push("icon = ?");
        values.push(icon.clone().into());
    }
    if let Some(position) = payload.position {
        set_clauses.push("position = ?");
        values.push(position.into());
    }
    if let Some(parent_id) = payload.parent_id {
        set_clauses.push("parent_id = ?");
        values.push(parent_id.into());
    }
    if let Some(route) = &payload.route {
        set_clauses.push("route = ?");
        values.push(route.clone().into());
    }
    if let Some(is_hidden) = payload.is_hidden {
        set_clauses.push("is_hidden = ?");
        values.push((if is_hidden { 1 } else { 0 }).into());
    }

    if set_clauses.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "No fields to update" })),
        );
    }

    values.push(id.into());
    let query = format!(
        "UPDATE menu_items SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let result = ctx.db.execute(&query, values).await;

    match result {
        Ok(_) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({ "success": true })),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn delete_menu_item(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = ctx.db
        .execute("DELETE FROM menu_items WHERE id = ?", vec![id.into()])
        .await;

    match result {
        Ok(_) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({ "success": true })),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub fn routes() -> Router<SushiContext> {
    Router::new()
        .route("/admin/api/menu", get(menu_api))
        .route("/admin/api/menu", post(create_menu_item))
        .route("/admin/api/menu/:id", put(update_menu_item))
        .route("/admin/api/menu/:id", delete(delete_menu_item))
}
