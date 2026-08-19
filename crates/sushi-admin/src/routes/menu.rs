use axum::{
    extract::{Form, Path, State},
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use sushi_core::context::SushiContext;
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpResponse, HttpRouteSpec, MenuContributionSpec, StagedRegistrar,
};
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

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar) {
    staged.register_menu(
        MenuContributionSpec::new("host-admin.system", "System", 60)
            .with_icon(Some("settings".to_string())),
    );
}

pub fn register_menu_admin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("menu-admin.menus", "Menus", 61)
            .with_icon(Some("settings".to_string()))
            .with_parent(Some("host-admin.system".to_string()))
            .with_route(Some("/admin/menus".to_string()))
            .with_policy(Some("admin.menus.view".to_string())),
    );

    let page_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new("/admin/menus", "Menus", "menu-admin", "rust::menus-page")
            .with_policy(Some("admin.menus.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = page_ctx.clone();
                async move {
                    let response = menus_page(State(ctx)).await;
                    Ok(super::transport::from_axum_response(response).await)
                }
            })),
    );

    let api_get_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new("GET", "/admin/api/menu", "menu-admin", "rust::menu-api")
            .with_policy(Some("admin.menus.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = api_get_ctx.clone();
                async move {
                    let response = menu_api(State(ctx)).await;
                    Ok(super::transport::from_axum_response(response).await)
                }
            })),
    );

    let api_create_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/admin/api/menu",
            "menu-admin",
            "rust::menu-api-create",
        )
        .with_policy(Some("admin.menus.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = api_create_ctx.clone();
            async move {
                let payload = match decode_json(&request) {
                    Ok(payload) => payload,
                    Err(response) => return Ok(response),
                };
                let response = create_menu_item(State(ctx), Json(payload)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );

    let api_update_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "PUT",
            "/admin/api/menu/{id}",
            "menu-admin",
            "rust::menu-api-update",
        )
        .with_policy(Some("admin.menus.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = api_update_ctx.clone();
            async move {
                let id = match super::transport::path_i64(&request.path, "/admin/api/menu/", "") {
                    Ok(id) => id,
                    Err(response) => return Ok(response),
                };
                let payload = match decode_json(&request) {
                    Ok(payload) => payload,
                    Err(response) => return Ok(response),
                };
                let response = update_menu_item(State(ctx), Path(id), Json(payload)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );

    let api_delete_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "DELETE",
            "/admin/api/menu/{id}",
            "menu-admin",
            "rust::menu-api-delete",
        )
        .with_policy(Some("admin.menus.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = api_delete_ctx.clone();
            async move {
                let id = match super::transport::path_i64(&request.path, "/admin/api/menu/", "") {
                    Ok(id) => id,
                    Err(response) => return Ok(response),
                };
                let response = delete_menu_item(State(ctx), Path(id)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );

    register_menu_partial_capabilities(staged, ctx);
}

fn decode_json<T: serde::de::DeserializeOwned>(
    request: &sushi_core::runtime::HttpRequest,
) -> Result<T, HttpResponse> {
    serde_json::from_slice(request.body.as_deref().unwrap_or_default()).map_err(|error| {
        HttpResponse::new(
            StatusCode::BAD_REQUEST.as_u16(),
            serde_json::to_vec(&serde_json::json!({
                "error": format!("invalid menu request body: {error}")
            }))
            .expect("menu error JSON serialization cannot fail"),
        )
        .with_header("content-type", "application/json")
    })
}

fn register_menu_partial_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    let table_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/partials/menus/table",
            "menu-admin",
            "rust::menus-table",
        )
        .with_policy(Some("admin.menus.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = table_ctx.clone();
            async move {
                let response = menus_table_partial(State(ctx)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );

    let create_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/admin/partials/menus/create",
            "menu-admin",
            "rust::menus-create",
        )
        .with_policy(Some("admin.menus.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = create_ctx.clone();
            async move {
                let form = match super::transport::decode_form(&request) {
                    Ok(form) => form,
                    Err(response) => return Ok(response),
                };
                let response = menus_create_partial(State(ctx), Form(form)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );

    let update_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/admin/partials/menus/{id}/update",
            "menu-admin",
            "rust::menus-update",
        )
        .with_policy(Some("admin.menus.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = update_ctx.clone();
            async move {
                let id = match super::transport::path_i64(
                    &request.path,
                    "/admin/partials/menus/",
                    "/update",
                ) {
                    Ok(id) => id,
                    Err(response) => return Ok(response),
                };
                let form = match super::transport::decode_form(&request) {
                    Ok(form) => form,
                    Err(response) => return Ok(response),
                };
                let response = menus_update_partial(State(ctx), Path(id), Form(form)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );

    staged.register_http(
        HttpRouteSpec::new(
            "DELETE",
            "/admin/partials/menus/{id}",
            "menu-admin",
            "rust::menus-delete",
        )
        .with_policy(Some("admin.menus.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = ctx.clone();
            async move {
                let id =
                    match super::transport::path_i64(&request.path, "/admin/partials/menus/", "") {
                        Ok(id) => id,
                        Err(response) => return Ok(response),
                    };
                let response = menus_delete_partial(State(ctx), Path(id)).await;
                Ok(super::transport::from_axum_response(response).await)
            }
        })),
    );
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

    ctx.db
        .execute(
            "CREATE TABLE IF NOT EXISTS runtime_menu_items (
                contribution_id TEXT PRIMARY KEY,
                owner_id TEXT NOT NULL,
                menu_item_id INTEGER NOT NULL UNIQUE,
                FOREIGN KEY (menu_item_id) REFERENCES menu_items(id) ON DELETE CASCADE
            )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;
    ctx.db
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_runtime_menu_items_owner_id
             ON runtime_menu_items(owner_id)",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    // Keep one entry when historical data inserted duplicates.
    ctx.db
        .execute(
            "DELETE FROM menu_items
             WHERE route = '/admin/kv'
               AND id <> (
                 SELECT MIN(id) FROM menu_items
                 WHERE route = '/admin/kv'
               )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    ctx.db
        .execute(
            "DELETE FROM menu_items
             WHERE route = '/admin/cms'
               AND id <> (
                 SELECT MIN(id) FROM menu_items
                 WHERE route = '/admin/cms'
               )",
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    ctx.db
        .execute(
            "DELETE FROM menu_items
             WHERE route = '/admin/menus'
               AND id <> (
                 SELECT MIN(id) FROM menu_items
                 WHERE route = '/admin/menus'
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
            | "/admin/system"
            | "/admin/config"
            | "/admin/logs"
            | "/admin/menus"
            | "/admin/kv"
            | "/admin/cms"
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
    project_runtime_menu_contributions(ctx).await?;

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

async fn project_runtime_menu_contributions(ctx: &SushiContext) -> Result<(), String> {
    let snapshot = ctx.plugins.capability_snapshot().await;
    let contributions = snapshot
        .menu_contributions()
        .iter()
        .map(|registration| (registration.value.clone(), registration.owner.to_string()))
        .collect::<Vec<_>>();
    let active_ids = contributions
        .iter()
        .map(|(contribution, _)| contribution.id.clone())
        .collect::<HashSet<_>>();
    let mut projected = HashMap::new();
    let mut pending = contributions;

    while !pending.is_empty() {
        let mut progress = false;
        let mut deferred = Vec::new();
        for (contribution, owner_id) in pending {
            let parent_id = match contribution.parent_id.as_deref() {
                None => None,
                Some(parent_id) => match projected.get(parent_id).copied() {
                    Some(id) => Some(id),
                    None if active_ids.contains(parent_id) => {
                        deferred.push((contribution, owner_id));
                        continue;
                    }
                    None => match find_legacy_parent_id(ctx, parent_id).await? {
                        Some(id) => Some(id),
                        None => {
                            return Err(format!(
                                "runtime menu contribution '{}' references unknown parent '{}'",
                                contribution.id, parent_id
                            ));
                        }
                    },
                },
            };

            let item_id =
                upsert_runtime_menu_item(ctx, &contribution, &owner_id, parent_id).await?;
            projected.insert(contribution.id, item_id);
            progress = true;
        }
        if !progress {
            return Err(
                "runtime menu contribution hierarchy contains an unresolved parent".to_string(),
            );
        }
        pending = deferred;
    }

    let mapped = ctx
        .db
        .query(
            "SELECT contribution_id, menu_item_id FROM runtime_menu_items",
            vec![],
        )
        .await
        .map_err(|e| format!("failed to query runtime menu mappings: {e}"))?;
    for row in mapped {
        let contribution_id = row
            .get("contribution_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if active_ids.contains(contribution_id) {
            continue;
        }
        let Some(menu_item_id) = row.get("menu_item_id").and_then(Value::as_i64) else {
            continue;
        };
        ctx.db
            .execute(
                "DELETE FROM runtime_menu_items WHERE contribution_id = ?",
                vec![contribution_id.to_string().into()],
            )
            .await
            .map_err(|e| format!("failed to remove stale runtime menu mapping: {e}"))?;
        ctx.db
            .execute(
                "DELETE FROM menu_items WHERE id = ?",
                vec![menu_item_id.into()],
            )
            .await
            .map_err(|e| format!("failed to remove stale runtime menu item: {e}"))?;
    }

    Ok(())
}

async fn find_legacy_parent_id(
    ctx: &SushiContext,
    contribution_id: &str,
) -> Result<Option<i64>, String> {
    if contribution_id == "host-admin.system" {
        let rows = ctx
            .db
            .query(
                "SELECT id FROM menu_items WHERE label = 'System' AND parent_id IS NULL ORDER BY id LIMIT 1",
                vec![],
            )
            .await
            .map_err(|e| format!("failed to find legacy System menu: {e}"))?;
        return Ok(rows
            .first()
            .and_then(|row| row.get("id").and_then(Value::as_i64)));
    }
    Ok(None)
}

async fn upsert_runtime_menu_item(
    ctx: &SushiContext,
    contribution: &sushi_core::runtime::MenuContributionSpec,
    owner_id: &str,
    parent_id: Option<i64>,
) -> Result<i64, String> {
    let effective_route = contribution
        .route
        .clone()
        .or_else(|| (contribution.id == "host-admin.system").then(|| "/admin/system".to_string()));
    let mapped = ctx
        .db
        .query(
            "SELECT menu_item_id FROM runtime_menu_items WHERE contribution_id = ? LIMIT 1",
            vec![contribution.id.clone().into()],
        )
        .await
        .map_err(|e| format!("failed to query runtime menu mapping: {e}"))?;
    if let Some(menu_item_id) = mapped
        .first()
        .and_then(|row| row.get("menu_item_id").and_then(Value::as_i64))
    {
        return Ok(menu_item_id);
    }

    let legacy_id = if let Some(route) = effective_route.as_deref() {
        let rows = ctx
            .db
            .query(
                "SELECT id FROM menu_items WHERE route = ? ORDER BY id LIMIT 1",
                vec![route.to_string().into()],
            )
            .await
            .map_err(|e| format!("failed to find legacy runtime menu item: {e}"))?;
        rows.first()
            .and_then(|row| row.get("id").and_then(Value::as_i64))
    } else {
        let rows = if let Some(parent_id) = parent_id {
            ctx.db
                .query(
                    "SELECT id FROM menu_items WHERE label = ? AND parent_id = ? ORDER BY id LIMIT 1",
                    vec![contribution.label.clone().into(), parent_id.into()],
                )
                .await
        } else {
            ctx.db
                .query(
                    "SELECT id FROM menu_items WHERE label = ? AND parent_id IS NULL ORDER BY id LIMIT 1",
                    vec![contribution.label.clone().into()],
                )
                .await
        }
        .map_err(|e| format!("failed to find legacy runtime menu item: {e}"))?;
        rows.first()
            .and_then(|row| row.get("id").and_then(Value::as_i64))
    };

    let menu_item_id = if let Some(menu_item_id) = legacy_id {
        ctx.db
            .execute(
                "UPDATE menu_items SET parent_id = ? WHERE id = ?",
                vec![parent_id.into(), menu_item_id.into()],
            )
            .await
            .map_err(|e| format!("failed to align legacy runtime menu parent: {e}"))?;
        menu_item_id
    } else {
        let rows = ctx
            .db
            .query(
                "INSERT INTO menu_items (label, icon, position, parent_id, route)
                 VALUES (?, ?, ?, ?, ?)
                 RETURNING id",
                vec![
                    contribution.label.clone().into(),
                    contribution.icon.clone().into(),
                    contribution.position.into(),
                    parent_id.into(),
                    effective_route.into(),
                ],
            )
            .await
            .map_err(|e| format!("failed to insert runtime menu item: {e}"))?;
        rows.first()
            .and_then(|row| row.get("id").and_then(Value::as_i64))
            .ok_or_else(|| "runtime menu item insert did not return an id".to_string())?
    };

    ctx.db
        .execute(
            "INSERT INTO runtime_menu_items (contribution_id, owner_id, menu_item_id)
             VALUES (?, ?, ?)",
            vec![
                contribution.id.clone().into(),
                owner_id.to_string().into(),
                menu_item_id.into(),
            ],
        )
        .await
        .map_err(|e| format!("failed to record runtime menu mapping: {e}"))?;
    Ok(menu_item_id)
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
