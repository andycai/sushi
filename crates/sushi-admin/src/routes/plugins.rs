use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::{PluginAdminPageInfo, PluginInfo};

use crate::router::AdminAuthContext;

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
    crate::render::render_template(&ctx, "admin/plugins.html").await
}

pub async fn plugins_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let plugins = ctx.plugins.list_plugins().await;
    crate::render::render_template_with_context(
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
    let Some(context) = plugin_workspace_context(&ctx, &plugin).await else {
        return (StatusCode::NOT_FOUND, "plugin workspace not found").into_response();
    };

    crate::render::render_template_with_context(&ctx, "admin/plugin_workspace.html", context).await
}

pub async fn plugin_pages_api(
    Path(plugin): Path<String>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    let Some(plugin_info) = find_plugin(&ctx, &plugin).await else {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "plugin not found"
            })),
        )
            .into_response();
    };

    let payload = PluginWorkspaceResponse {
        plugin: plugin_info,
        pages: ctx.plugins.list_admin_pages_for_plugin(&plugin).await,
    };

    (StatusCode::OK, axum::Json(payload)).into_response()
}

pub async fn plugin_state_api(
    Path(plugin): Path<String>,
    State(ctx): State<SushiContext>,
    auth: Option<Extension<AdminAuthContext>>,
    axum::Json(payload): axum::Json<PluginStateMutationRequest>,
) -> impl IntoResponse {
    let actor = auth.as_ref().map(|Extension(ctx)| ctx.role.as_str());
    let reason = payload
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match ctx
        .plugins
        .set_plugin_enabled(&plugin, payload.enabled, actor, reason)
        .await
    {
        Ok(plugin_info) => (StatusCode::OK, axum::Json(plugin_info)).into_response(),
        Err(err) if err.starts_with("plugin not found:") => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "plugin not found"
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": err
            })),
        )
            .into_response(),
    }
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
