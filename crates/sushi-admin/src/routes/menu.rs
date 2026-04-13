use axum::{
    extract::{Form, Path, State},
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;

#[derive(Debug, Serialize, Clone)]
pub struct MenuItem {
    pub id: i64,
    pub label: String,
    pub icon: Option<String>,
    pub position: i64,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: bool,
}

#[derive(Debug, Serialize)]
struct MenuTableRow {
    id: i64,
    label: String,
    icon: Option<String>,
    position: i64,
    parent_id: Option<i64>,
    parent_label: Option<String>,
    route: Option<String>,
    is_hidden: bool,
    is_system: bool,
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

#[derive(Debug, Deserialize)]
pub struct UpsertMenuForm {
    pub label: String,
    pub icon: Option<String>,
    pub position: Option<String>,
    pub parent_id: Option<String>,
    pub route: Option<String>,
    pub is_hidden: Option<String>,
}

#[derive(Debug)]
struct ParsedMenuForm {
    label: String,
    icon: Option<String>,
    position: i64,
    parent_id: Option<i64>,
    route: Option<String>,
    is_hidden: bool,
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

    // Keep one entry when historical data inserted duplicates.
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

    ctx.db
        .execute(
            "DELETE FROM menu_items
             WHERE parent_id IS NULL
               AND route = '/admin/menus'
               AND id <> (
                 SELECT MIN(id) FROM menu_items
                 WHERE parent_id IS NULL AND route = '/admin/menus'
               )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn is_system_route(route: &str) -> bool {
    matches!(
        route,
        "/admin/"
            | "/admin/users"
            | "/admin/roles"
            | "/admin/permissions"
            | "/admin/plugins"
            | "/admin/config"
            | "/admin/logs"
            | "/admin/menus"
            | "/admin/kv"
    )
}

fn normalize_optional(input: Option<&str>) -> Option<String> {
    input.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_checkbox(input: Option<&str>) -> bool {
    matches!(input.map(|v| v.trim().to_ascii_lowercase()), Some(v) if v == "1" || v == "true" || v == "on" || v == "yes")
}

fn parse_position(input: Option<&str>) -> Result<i64, String> {
    let raw = input.unwrap_or("0").trim();
    if raw.is_empty() {
        return Ok(0);
    }

    let parsed: i64 = raw
        .parse()
        .map_err(|_| "Position must be a valid integer".to_string())?;
    if !(0..=9999).contains(&parsed) {
        return Err("Position must be between 0 and 9999".to_string());
    }

    Ok(parsed)
}

fn parse_parent_id(input: Option<&str>) -> Result<Option<i64>, String> {
    let Some(raw) = normalize_optional(input) else {
        return Ok(None);
    };

    let parsed: i64 = raw
        .parse()
        .map_err(|_| "Parent menu id must be a valid integer".to_string())?;
    if parsed <= 0 {
        return Err("Parent menu id must be a positive integer".to_string());
    }

    Ok(Some(parsed))
}

fn parse_menu_form(
    form: &UpsertMenuForm,
    existing_id: Option<i64>,
) -> Result<ParsedMenuForm, String> {
    let label = form.label.trim();
    if label.is_empty() {
        return Err("Menu label is required".to_string());
    }
    if label.len() > 80 {
        return Err("Menu label must be 80 characters or fewer".to_string());
    }

    let icon = normalize_optional(form.icon.as_deref());
    if let Some(icon_name) = &icon {
        if icon_name.len() > 48 {
            return Err("Icon name must be 48 characters or fewer".to_string());
        }
    }

    let route = normalize_optional(form.route.as_deref());
    if let Some(path) = &route {
        if path.len() > 180 {
            return Err("Route must be 180 characters or fewer".to_string());
        }
        if !path.starts_with('/') {
            return Err("Route must start with '/'".to_string());
        }
    }

    let position = parse_position(form.position.as_deref())?;
    let parent_id = parse_parent_id(form.parent_id.as_deref())?;

    if let Some(id) = existing_id {
        if parent_id == Some(id) {
            return Err("A menu item cannot be its own parent".to_string());
        }
    }

    Ok(ParsedMenuForm {
        label: label.to_string(),
        icon,
        position,
        parent_id,
        route,
        is_hidden: parse_checkbox(form.is_hidden.as_deref()),
    })
}

async fn list_menu_items(ctx: &SushiContext) -> Result<Vec<MenuItem>, String> {
    ensure_menu_schema(ctx).await?;

    let rows = ctx
        .db
        .query(
            "SELECT id, label, icon, position, parent_id, route, is_hidden
             FROM menu_items
             ORDER BY
               CASE WHEN parent_id IS NULL THEN position ELSE (
                 SELECT pm.position FROM menu_items pm WHERE pm.id = menu_items.parent_id
               ) END ASC,
               CASE WHEN parent_id IS NULL THEN 0 ELSE 1 END ASC,
               position ASC,
               id ASC",
            vec![],
        )
        .await
        .map_err(|e| format!("failed to query menu items: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|row| MenuItem {
            id: row.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
            label: row
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            icon: row
                .get("icon")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            position: row.get("position").and_then(|v| v.as_i64()).unwrap_or(0),
            parent_id: row.get("parent_id").and_then(|v| v.as_i64()),
            route: row
                .get("route")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            is_hidden: row.get("is_hidden").and_then(|v| v.as_i64()).unwrap_or(0) != 0,
        })
        .collect())
}

pub async fn menu_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    match list_menu_items(&ctx).await {
        Ok(menu) => Json(MenuResponse { menu }).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err })),
        )
            .into_response(),
    }
}

pub async fn menus_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/menus.html").await
}

pub async fn menus_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    render_menu_rows(&ctx).await
}

pub async fn menus_create_partial(
    State(ctx): State<SushiContext>,
    Form(form): Form<UpsertMenuForm>,
) -> impl IntoResponse {
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return flash_response(
            &ctx,
            StatusCode::INTERNAL_SERVER_ERROR,
            "error",
            &format!("failed to ensure menu schema: {err}"),
        )
        .await;
    }

    let parsed = match parse_menu_form(&form, None) {
        Ok(parsed) => parsed,
        Err(err) => return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    };

    let result = ctx
        .db
        .execute(
            "INSERT INTO menu_items (label, icon, position, parent_id, route, is_hidden)
             VALUES (?, ?, ?, ?, ?, ?)",
            vec![
                parsed.label.into(),
                parsed.icon.into(),
                parsed.position.into(),
                parsed.parent_id.into(),
                parsed.route.into(),
                (if parsed.is_hidden { 1 } else { 0 }).into(),
            ],
        )
        .await;

    match result {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Menu item created.",
                r#"{"menus:refresh":true,"menus:close-editor":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err.to_string()).await,
    }
}

pub async fn menus_update_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
    Form(form): Form<UpsertMenuForm>,
) -> impl IntoResponse {
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return flash_response(
            &ctx,
            StatusCode::INTERNAL_SERVER_ERROR,
            "error",
            &format!("failed to ensure menu schema: {err}"),
        )
        .await;
    }

    let parsed = match parse_menu_form(&form, Some(id)) {
        Ok(parsed) => parsed,
        Err(err) => return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    };

    let result = ctx
        .db
        .execute(
            "UPDATE menu_items
             SET label = ?, icon = ?, position = ?, parent_id = ?, route = ?, is_hidden = ?
             WHERE id = ?",
            vec![
                parsed.label.into(),
                parsed.icon.into(),
                parsed.position.into(),
                parsed.parent_id.into(),
                parsed.route.into(),
                (if parsed.is_hidden { 1 } else { 0 }).into(),
                id.into(),
            ],
        )
        .await;

    match result {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Menu item updated.",
                r#"{"menus:refresh":true,"menus:close-editor":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err.to_string()).await,
    }
}

pub async fn menus_delete_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Err(err) = ensure_menu_schema(&ctx).await {
        return flash_response(
            &ctx,
            StatusCode::INTERNAL_SERVER_ERROR,
            "error",
            &format!("failed to ensure menu schema: {err}"),
        )
        .await;
    }

    let rows = match ctx
        .db
        .query(
            "SELECT route FROM menu_items WHERE id = ? LIMIT 1",
            vec![id.into()],
        )
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            return flash_response(
                &ctx,
                StatusCode::BAD_REQUEST,
                "error",
                &format!("failed to read menu item: {err}"),
            )
            .await;
        }
    };

    if rows.is_empty() {
        return flash_response(
            &ctx,
            StatusCode::BAD_REQUEST,
            "error",
            "Menu item not found",
        )
        .await;
    }

    let route = rows[0]
        .get("route")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if is_system_route(route) {
        return flash_response(
            &ctx,
            StatusCode::BAD_REQUEST,
            "error",
            "System menu items cannot be deleted.",
        )
        .await;
    }

    match ctx
        .db
        .execute("DELETE FROM menu_items WHERE id = ?", vec![id.into()])
        .await
    {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Menu item deleted.",
                "menus:refresh",
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err.to_string()).await,
    }
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

    let result = ctx
        .db
        .execute(
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
        )
        .await;

    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "success": true })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
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
            StatusCode::BAD_REQUEST,
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
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "success": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
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

    let result = ctx
        .db
        .execute("DELETE FROM menu_items WHERE id = ?", vec![id.into()])
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "success": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

async fn render_menu_rows(ctx: &SushiContext) -> Response {
    let items = match list_menu_items(ctx).await {
        Ok(items) => items,
        Err(err) => {
            return flash_response(ctx, StatusCode::INTERNAL_SERVER_ERROR, "error", &err).await;
        }
    };

    let mut label_by_id = std::collections::HashMap::new();
    for item in &items {
        label_by_id.insert(item.id, item.label.clone());
    }

    let rows: Vec<MenuTableRow> = items
        .iter()
        .map(|item| {
            let parent_label = item
                .parent_id
                .and_then(|parent_id| label_by_id.get(&parent_id).cloned());
            let route = item.route.clone();
            let is_system = route.as_deref().map(is_system_route).unwrap_or(false);

            MenuTableRow {
                id: item.id,
                label: item.label.clone(),
                icon: item.icon.clone(),
                position: item.position,
                parent_id: item.parent_id,
                parent_label,
                route,
                is_hidden: item.is_hidden,
                is_system,
            }
        })
        .collect();

    crate::render::render_template_with_context(
        ctx,
        "admin/partials/menus_rows.html",
        serde_json::json!({
            "menus": rows,
        }),
    )
    .await
}

async fn flash_response(
    ctx: &SushiContext,
    status: StatusCode,
    level: &str,
    message: &str,
) -> Response {
    let mut response = crate::render::render_template_with_context(
        ctx,
        "admin/partials/flash.html",
        serde_json::json!({
            "level": level,
            "message": message,
        }),
    )
    .await;
    *response.status_mut() = status;
    response
}

async fn flash_response_with_trigger(
    ctx: &SushiContext,
    status: StatusCode,
    level: &str,
    message: &str,
    trigger: &str,
) -> Response {
    let mut response = flash_response(ctx, status, level, message).await;
    if let Ok(value) = HeaderValue::from_str(trigger) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("hx-trigger"), value);
    }
    response
}

pub fn routes() -> Router<SushiContext> {
    Router::new()
        .route("/admin/api/menu", get(menu_api))
        .route("/admin/api/menu", post(create_menu_item))
        .route("/admin/api/menu/{id}", put(update_menu_item))
        .route("/admin/api/menu/{id}", delete(delete_menu_item))
        .route("/admin/partials/menus/table", get(menus_table_partial))
        .route("/admin/partials/menus/create", post(menus_create_partial))
        .route(
            "/admin/partials/menus/{id}/update",
            post(menus_update_partial),
        )
        .route("/admin/partials/menus/{id}", delete(menus_delete_partial))
}
