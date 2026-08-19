use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use sushi_core::runtime::{HttpHandler, HttpRequest, HttpResponse, HttpRouteSpec, StagedRegistrar};
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

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    let assets_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/workspace/{*module}",
            "admin-shell",
            "rust::workspace-partial",
        )
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = ctx.clone();
            async move { workspace_http_response(&ctx, request).await }
        })),
    );
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/api/workspace/assets",
            "admin-shell",
            "rust::workspace-assets",
        )
        .with_policy(Some("admin.plugins.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = assets_ctx.clone();
            async move { workspace_assets_http_response(&ctx, request).await }
        })),
    );
}

pub async fn workspace_http_response(
    ctx: &SushiContext,
    request: HttpRequest,
) -> Result<HttpResponse, String> {
    let Some(module) = request.path.strip_prefix("/admin/workspace/") else {
        return Ok(HttpResponse::new(
            StatusCode::NOT_FOUND.as_u16(),
            "workspace module not found",
        ));
    };
    let response = workspace_partial(Path(module.to_string()), State(ctx.clone())).await;
    Ok(crate::routes::transport::from_axum_response(response).await)
}

async fn workspace_assets_http_response(
    ctx: &SushiContext,
    request: HttpRequest,
) -> Result<HttpResponse, String> {
    let query = request
        .dispatch_path
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();
    let query = serde_urlencoded::from_str::<HashMap<String, String>>(query)
        .map_err(|error| format!("invalid workspace assets query: {error}"))?;
    let Some(path) = query
        .get("path")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Ok(HttpResponse::new(
            StatusCode::BAD_REQUEST.as_u16(),
            serde_json::to_vec(&serde_json::json!({
                "error": "missing path query parameter",
            }))
            .expect("workspace assets error JSON serialization cannot fail"),
        )
        .with_header("content-type", "application/json"));
    };
    let role = request_role(ctx, &request)?;
    if role != "admin"
        && ctx
            .authorizer
            .check_http(&role, "admin", "GET", path)
            .await
            .is_err()
    {
        return Ok(HttpResponse::new(
            StatusCode::FORBIDDEN.as_u16(),
            serde_json::to_vec(&serde_json::json!({ "error": "forbidden" }))
                .expect("workspace assets forbidden JSON serialization cannot fail"),
        )
        .with_header("content-type", "application/json"));
    }
    let assets = ctx
        .plugins
        .admin_page_assets(path)
        .await
        .unwrap_or_default();
    let body = serde_json::to_vec(&WorkspaceAssetsResponse {
        js: assets.js,
        css: assets.css,
    })
    .map_err(|error| format!("failed to serialize workspace assets: {error}"))?;
    Ok(HttpResponse::new(StatusCode::OK.as_u16(), body)
        .with_header("content-type", "application/json"))
}

fn request_role(ctx: &SushiContext, request: &HttpRequest) -> Result<String, String> {
    let bearer = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .and_then(|(_, value)| std::str::from_utf8(value).ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let cookie = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .and_then(|(_, value)| std::str::from_utf8(value).ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                cookie
                    .trim()
                    .strip_prefix("sushi_token=")
                    .filter(|token| !token.is_empty())
            })
        });
    let token = bearer
        .or(cookie)
        .ok_or_else(|| "missing Admin authentication context".to_string())?;
    let claims = ctx.jwt.verify_token(token)?;
    if claims.token_type != "access" {
        return Err("invalid Admin authentication token type".to_string());
    }
    Ok(claims.role)
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
                let message = plugin_disabled_message(&err);
                let warn_message = format!("plugin disabled on workspace page {path}: {message}");
                tracing::warn!("{warn_message}");
                ctx.logs.warn(&warn_message).await;
                return (StatusCode::FORBIDDEN, message).into_response();
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
