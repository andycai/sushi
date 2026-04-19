use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use sushi_core::{auth::rbac::RbacRepository, context::SushiContext, storage::Storage};

use crate::router::AdminAuthContext;

fn module_template(module: &str) -> Option<&'static str> {
    match module {
        "dashboard" => Some("admin/fragments/dashboard_content.html"),
        "users" => Some("admin/fragments/users_content.html"),
        "roles" => Some("admin/fragments/roles_content.html"),
        "permissions" => Some("admin/fragments/permissions_content.html"),
        "plugins" => Some("admin/fragments/plugins_content.html"),
        "kv" => Some("plugins/official/kv-store/fragments/kv_content.html"),
        "config" => Some("admin/fragments/config_content.html"),
        "logs" => Some("admin/fragments/logs_content.html"),
        "menus" => Some("admin/fragments/menus_content.html"),
        _ => None,
    }
}

fn module_to_admin_path(module: &str) -> Option<String> {
    let module = module.trim_matches('/');
    if module.is_empty() {
        return None;
    }
    if module == "dashboard" {
        return Some("/admin/".to_string());
    }
    Some(format!("/admin/{module}"))
}

fn extract_workspace_module_fragment(html: &str) -> Option<String> {
    let lowered = html.to_ascii_lowercase();
    let marker_idx = lowered.find("data-admin-workspace-module")?;
    let section_start = lowered[..marker_idx].rfind("<section")?;

    let mut cursor = section_start;
    let mut depth = 0usize;

    loop {
        let next_open = lowered[cursor..].find("<section").map(|idx| cursor + idx);
        let next_close = lowered[cursor..].find("</section").map(|idx| cursor + idx);

        let (is_open, token_idx) = match (next_open, next_close) {
            (Some(open_idx), Some(close_idx)) => {
                if open_idx < close_idx {
                    (true, open_idx)
                } else {
                    (false, close_idx)
                }
            }
            (Some(open_idx), None) => (true, open_idx),
            (None, Some(close_idx)) => (false, close_idx),
            (None, None) => return None,
        };

        let tag_end = lowered[token_idx..]
            .find('>')
            .map(|idx| token_idx + idx + 1)?;

        if is_open {
            depth += 1;
            cursor = tag_end;
            continue;
        }

        depth = depth.checked_sub(1)?;
        cursor = tag_end;
        if depth == 0 {
            return Some(html[section_start..tag_end].to_string());
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WorkspaceAssetsResponse {
    pub js: Vec<String>,
    pub css: Vec<String>,
}

pub async fn workspace_assets_api(
    Query(query): Query<HashMap<String, String>>,
    State(ctx): State<SushiContext>,
    Extension(auth): Extension<AdminAuthContext>,
) -> impl IntoResponse {
    let Some(path) = query
        .get("path")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "missing path query parameter",
            })),
        )
            .into_response();
    };

    if !auth.is_admin
        && ctx
            .authorizer
            .check_http(&auth.role, "admin", "GET", path)
            .await
            .is_err()
    {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "forbidden",
            })),
        )
            .into_response();
    }

    let assets = ctx
        .plugins
        .admin_page_assets(path)
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        axum::Json(WorkspaceAssetsResponse {
            js: assets.js,
            css: assets.css,
        }),
    )
        .into_response()
}

pub async fn workspace_partial(
    Path(module): Path<String>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    let module = module.trim_matches('/').to_string();

    if let Some(template) = module_template(module.as_str()) {
        if module == "users" {
            let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
            let roles = repo.list_roles().await.unwrap_or_default();
            return crate::render::render_template_with_context(
                &ctx,
                template,
                serde_json::json!({
                    "roles": roles,
                }),
            )
            .await;
        }

        let response: Response = crate::render::render_template(&ctx, template).await;
        return response;
    };

    let segments = module
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() == 2 && segments[0] == "plugins" {
        return crate::routes::plugins::render_plugin_workspace_partial(&ctx, segments[1]).await;
    }

    let Some(path) = module_to_admin_path(&module) else {
        return (StatusCode::NOT_FOUND, "workspace module not found").into_response();
    };

    match ctx.plugins.call_admin_handler(&path).await {
        Some(Ok(html)) => {
            if let Some(fragment) = extract_workspace_module_fragment(&html) {
                return Html(fragment).into_response();
            }
            Html(html).into_response()
        }
        Some(Err(err)) => {
            if is_plugin_disabled_error(&err) {
                return (StatusCode::FORBIDDEN, plugin_disabled_message(&err)).into_response();
            }
            let message = format!("plugin runtime error on workspace page {path}: {err}");
            tracing::error!("{message}");
            ctx.logs.error(&message).await;
            (StatusCode::INTERNAL_SERVER_ERROR, err).into_response()
        }
        None => (StatusCode::NOT_FOUND, "workspace module not found").into_response(),
    }
}

fn is_plugin_disabled_error(err: &str) -> bool {
    err.starts_with("plugin_disabled:")
}

fn plugin_disabled_message(err: &str) -> String {
    err.strip_prefix("plugin_disabled:")
        .map(str::trim)
        .filter(|msg| !msg.is_empty())
        .unwrap_or("plugin is disabled")
        .to_string()
}
