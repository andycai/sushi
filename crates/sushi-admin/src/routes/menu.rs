use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    response::IntoResponse,
    Json, Router,
};
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

async fn ensure_menu_schema(ctx: &SushiContext) -> Result<(), String> {
    ctx.db
        .execute(
            "CREATE TABLE IF NOT EXISTS menu_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                label TEXT NOT NULL,
                icon TEXT,
                position INTEGER NOT NULL DEFAULT 0,
                parent_id INTEGER,
                route TEXT,
                is_hidden INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (parent_id) REFERENCES menu_items(id) ON DELETE SET NULL
            )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    let columns = ctx
        .db
        .query("PRAGMA table_info(menu_items)", vec![])
        .await
        .map_err(|e| e.to_string())?;
    let has_is_hidden = columns.iter().any(|column| {
        column
            .get("name")
            .and_then(|value| value.as_str())
            .map(|name| name == "is_hidden")
            .unwrap_or(false)
    });

    if !has_is_hidden {
        ctx.db
            .execute(
                "ALTER TABLE menu_items ADD COLUMN is_hidden INTEGER NOT NULL DEFAULT 0",
                vec![],
            )
            .await
            .map_err(|e| e.to_string())?;
    }

    // Seed built-in top-level menu entries idempotently.
    ctx.db
        .execute(
            "INSERT OR IGNORE INTO menu_items (id, label, icon, position, parent_id, route)
             VALUES
               (1, 'Dashboard', 'layout-dashboard', 10, NULL, '/admin/'),
               (2, 'Users', 'users', 20, NULL, '/admin/users'),
               (3, 'Roles', 'shield', 30, NULL, '/admin/roles'),
               (4, 'Permissions', 'key', 40, NULL, '/admin/permissions'),
               (5, 'Plugins', 'package', 50, NULL, '/admin/plugins'),
               (6, 'Config', 'settings', 60, NULL, '/admin/config'),
               (7, 'Logs', 'file-text', 70, NULL, '/admin/logs')",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    // Seed the menu management entry for `/admin/menus`.
    ctx.db
        .execute(
            "INSERT INTO menu_items (label, icon, position, parent_id, route)
             SELECT 'Menus', 'settings', 61, NULL, '/admin/menus'
             WHERE NOT EXISTS (
               SELECT 1 FROM menu_items
               WHERE parent_id IS NULL AND route = '/admin/menus'
             )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    // Seed KV child menu only when no matching route exists.
    ctx.db
        .execute(
            "INSERT INTO menu_items (label, icon, position, parent_id, route)
             SELECT 'KV Store', 'database', 51, 5, '/admin/kv'
             WHERE NOT EXISTS (
               SELECT 1 FROM menu_items
               WHERE parent_id = 5 AND route = '/admin/kv'
             )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    // Older deployments may have inserted this child multiple times.
    // Keep the oldest row to preserve references and remove duplicates.
    ctx.db
        .execute(
            "DELETE FROM menu_items
             WHERE parent_id = 5
               AND route = '/admin/kv'
               AND id <> (
                 SELECT MIN(id) FROM menu_items
                 WHERE parent_id = 5 AND route = '/admin/kv'
               )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn menu_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("failed to ensure menu schema: {err}") })),
        )
            .into_response();
    }

    let rows = match ctx
        .db
        .query(
            "SELECT id, label, icon, position, parent_id, route, is_hidden
             FROM menu_items
             ORDER BY position ASC, id ASC",
            vec![],
        )
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("failed to query menu items: {err}") })),
            )
                .into_response();
        }
    };

    let menu: Vec<MenuItem> = rows.into_iter().map(|row| MenuItem {
        id: row.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        label: row.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        icon: row.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string()),
        position: row.get("position").and_then(|v| v.as_i64()).unwrap_or(0),
        parent_id: row.get("parent_id").and_then(|v| v.as_i64()),
        route: row.get("route").and_then(|v| v.as_str()).map(|s| s.to_string()),
        is_hidden: row.get("is_hidden").and_then(|v| v.as_i64()).unwrap_or(0) != 0,
    }).collect();

    Json(MenuResponse { menu }).into_response()
}

pub async fn menus_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/menus.html").await
}

pub async fn create_menu_item(
    State(ctx): State<SushiContext>,
    Json(payload): Json<CreateMenuItem>,
) -> impl IntoResponse {
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("failed to ensure menu schema: {err}") })),
        );
    }

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
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("failed to ensure menu schema: {err}") })),
        );
    }

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
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("failed to ensure menu schema: {err}") })),
        );
    }

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
        .route("/admin/api/menu/{id}", put(update_menu_item))
        .route("/admin/api/menu/{id}", delete(delete_menu_item))
}
