use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::{PluginAdminPageInfo, PluginInfo};
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpRequest, HttpResponse, HttpRouteSpec, MenuContributionSpec,
    StagedRegistrar,
};

#[derive(Debug, Serialize)]
struct PluginWorkspaceResponse {
    plugin: PluginInfo,
    pages: Vec<PluginAdminPageInfo>,
}

#[derive(Debug, Deserialize)]
pub struct PluginStateMutationRequest {
    pub enabled: bool,
    pub reason: Option<String>,
}

pub async fn plugins_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(plugins_page_response(&ctx).await)
}

pub async fn plugins_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(plugins_table_response(&ctx).await)
}

pub async fn plugins_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(plugins_list_response(&ctx).await)
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("host-admin.plugins", "Plugins", 50)
            .with_icon(Some("package".to_string()))
            .with_parent(Some("host-admin.system".to_string()))
            .with_route(Some("/admin/plugins".to_string()))
            .with_policy(Some("admin.plugins.view".to_string())),
    );
    let page_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new(
            "/admin/plugins",
            "Plugins",
            "host-admin",
            "rust::plugins-page",
        )
        .with_policy(Some("admin.plugins.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = page_ctx.clone();
            async move { Ok(plugins_page_response(&ctx).await) }
        })),
    );

    let workspace_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new(
            "/admin/plugins/{plugin}",
            "Plugin Workspace",
            "host-admin",
            "rust::plugin-workspace",
        )
        .with_policy(Some("admin.plugins.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = workspace_ctx.clone();
            async move { Ok(plugin_workspace_response(&ctx, plugin_path(&request.path)).await) }
        })),
    );

    let partial_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/partials/plugins/table",
            "host-admin",
            "rust::plugins-table",
        )
        .with_policy(Some("admin.plugins.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = partial_ctx.clone();
            async move { Ok(plugins_table_response(&ctx).await) }
        })),
    );

    let list_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/api/plugins",
            "host-admin",
            "rust::plugins-list",
        )
        .with_policy(Some("admin.plugins.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = list_ctx.clone();
            async move { Ok(plugins_list_response(&ctx).await) }
        })),
    );

    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/api/plugins/{plugin}/pages",
            "host-admin",
            "rust::plugin-pages",
        )
        .with_policy(Some("admin.plugins.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = ctx.clone();
            async move { Ok(plugin_pages_response(&ctx, plugin_pages_path(&request.path)).await) }
        })),
    );
}

pub fn register_governance_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_http(
        HttpRouteSpec::new(
            "PATCH",
            "/admin/api/plugins/{plugin}/state",
            "governance",
            "rust::plugin-state",
        )
        .with_policy(Some("admin.plugins.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = ctx.clone();
            async move { Ok(plugin_state_http_response(&ctx, request).await) }
        })),
    );
}

async fn plugins_page_response(ctx: &SushiContext) -> HttpResponse {
    crate::render::render_template_http_response(ctx, "admin/plugins.html", serde_json::json!({}))
        .await
}

async fn plugins_table_response(ctx: &SushiContext) -> HttpResponse {
    let plugins = ctx.plugins.list_plugins().await;
    crate::render::render_template_http_response(
        &ctx,
        "admin/partials/plugins_rows.html",
        serde_json::json!({
            "plugins": plugins,
        }),
    )
    .await
}

async fn find_plugin(ctx: &SushiContext, plugin_name: &str) -> Option<PluginInfo> {
    ctx.plugins
        .list_plugins()
        .await
        .into_iter()
        .find(|plugin| plugin.name == plugin_name)
}

async fn plugin_workspace_context(
    ctx: &SushiContext,
    plugin_name: &str,
) -> Option<serde_json::Value> {
    let plugin = find_plugin(ctx, plugin_name).await?;
    let pages = ctx.plugins.list_admin_pages_for_plugin(plugin_name).await;

    Some(serde_json::json!({
        "plugin": plugin,
        "pages": pages,
    }))
}

pub async fn plugin_workspace_page(
    Path(plugin): Path<String>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(plugin_workspace_response(&ctx, Some(&plugin)).await)
}

pub async fn plugin_pages_api(
    Path(plugin): Path<String>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(plugin_pages_response(&ctx, Some(&plugin)).await)
}

async fn plugin_workspace_response(ctx: &SushiContext, plugin_name: Option<&str>) -> HttpResponse {
    let Some(plugin_name) = plugin_name else {
        return text_response(StatusCode::NOT_FOUND, "plugin workspace not found");
    };
    let Some(context) = plugin_workspace_context(ctx, plugin_name).await else {
        return text_response(StatusCode::NOT_FOUND, "plugin workspace not found");
    };

    crate::render::render_template_http_response(ctx, "admin/plugin_workspace.html", context).await
}

async fn plugin_pages_response(ctx: &SushiContext, plugin_name: Option<&str>) -> HttpResponse {
    let Some(plugin_name) = plugin_name else {
        return json_response(
            StatusCode::NOT_FOUND,
            serde_json::json!({ "error": "plugin not found" }),
        );
    };
    let Some(plugin_info) = find_plugin(ctx, plugin_name).await else {
        return json_response(
            StatusCode::NOT_FOUND,
            serde_json::json!({ "error": "plugin not found" }),
        );
    };

    let payload = PluginWorkspaceResponse {
        plugin: plugin_info,
        pages: ctx.plugins.list_admin_pages_for_plugin(plugin_name).await,
    };

    json_response(
        StatusCode::OK,
        serde_json::to_value(payload).expect("plugin workspace serialization cannot fail"),
    )
}

async fn plugins_list_response(ctx: &SushiContext) -> HttpResponse {
    let plugins = ctx.plugins.list_plugins().await;
    json_response(
        StatusCode::OK,
        serde_json::to_value(plugins).expect("plugin list serialization cannot fail"),
    )
}

fn plugin_path(path: &str) -> Option<&str> {
    path.strip_prefix("/admin/plugins/")
        .filter(|value| !value.contains('/'))
}

fn plugin_pages_path(path: &str) -> Option<&str> {
    path.strip_prefix("/admin/api/plugins/")
        .and_then(|value| value.strip_suffix("/pages"))
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

fn json_response(status: StatusCode, payload: serde_json::Value) -> HttpResponse {
    HttpResponse::new(
        status.as_u16(),
        serde_json::to_vec(&payload).expect("JSON value serialization cannot fail"),
    )
    .with_header("content-type", "application/json")
}

fn text_response(status: StatusCode, body: &str) -> HttpResponse {
    HttpResponse::new(status.as_u16(), body)
        .with_header("content-type", "text/plain; charset=utf-8")
}

async fn plugin_state_http_response(ctx: &SushiContext, request: HttpRequest) -> HttpResponse {
    let Some(plugin) = plugin_state_path(&request.path) else {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid plugin path"}),
        );
    };
    let payload = match serde_json::from_slice::<PluginStateMutationRequest>(
        request.body.as_deref().unwrap_or_default(),
    ) {
        Ok(payload) => payload,
        Err(_) => {
            return json_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                serde_json::json!({"error": "invalid request body"}),
            )
        }
    };
    let role = match request_role(ctx, &request) {
        Ok(role) => role,
        Err(error) => {
            tracing::warn!("plugin state request authentication failed: {error}");
            return json_response(
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "unauthorized"}),
            );
        }
    };
    let reason = payload
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match ctx
        .set_plugin_enabled(plugin, payload.enabled, Some(&role), reason)
        .await
    {
        Ok(plugin_info) => json_response(
            StatusCode::OK,
            serde_json::to_value(plugin_info).expect("PluginInfo serialization cannot fail"),
        ),
        Err(err) if err.starts_with("plugin not found:") => json_response(
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "plugin not found"}),
        ),
        Err(err) if err.starts_with("required_plugin_toggle_forbidden:") => json_response(
            StatusCode::CONFLICT,
            serde_json::json!({"error": "required_plugin_toggle_forbidden"}),
        ),
        Err(err) => {
            tracing::warn!(
                "failed to update plugin state: plugin={} enabled={} error={}",
                plugin,
                payload.enabled,
                err
            );
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({"error": "failed to update plugin state"}),
            )
        }
    }
}

fn plugin_state_path(path: &str) -> Option<&str> {
    path.strip_prefix("/admin/api/plugins/")
        .and_then(|value| value.strip_suffix("/state"))
        .filter(|value| !value.is_empty() && !value.contains('/'))
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

pub async fn render_plugin_workspace_partial(ctx: &SushiContext, plugin_name: &str) -> Response {
    let Some(context) = plugin_workspace_context(ctx, plugin_name).await else {
        return (StatusCode::NOT_FOUND, "workspace module not found").into_response();
    };

    crate::render::render_template_with_context(
        &ctx,
        "admin/fragments/plugin_workspace_content.html",
        context,
    )
    .await
}
