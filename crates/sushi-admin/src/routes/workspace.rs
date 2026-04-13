use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use std::sync::Arc;
use sushi_core::{auth::rbac::RbacRepository, context::SushiContext, storage::Storage};

fn module_template(module: &str) -> Option<&'static str> {
    match module {
        "dashboard" => Some("admin/fragments/dashboard_content.html"),
        "users" => Some("admin/fragments/users_content.html"),
        "roles" => Some("admin/fragments/roles_content.html"),
        "permissions" => Some("admin/fragments/permissions_content.html"),
        "plugins" => Some("admin/fragments/plugins_content.html"),
        "kv" => Some("plugins/kv-store/fragments/kv_content.html"),
        "config" => Some("admin/fragments/config_content.html"),
        "logs" => Some("admin/fragments/logs_content.html"),
        "menus" => Some("admin/fragments/menus_content.html"),
        _ => None,
    }
}

pub fn permission_for_module(module: &str) -> Option<&'static str> {
    let module = module.trim_matches('/');

    match module {
        "dashboard" => Some("dashboard.view"),
        "users" => Some("users.view"),
        "roles" => Some("roles.view"),
        "permissions" => Some("permissions.view"),
        "plugins" => Some("plugins.view"),
        "kv" => Some("kv.manage"),
        "config" => Some("config.view"),
        "logs" => Some("logs.view"),
        "menus" => Some("menus.view"),
        _ if module.starts_with("plugins/") => Some("plugins.view"),
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
        Some(Ok(html)) => Html(html).into_response(),
        Some(Err(err)) => {
            let message = format!("plugin runtime error on workspace page {path}: {err}");
            tracing::error!("{message}");
            ctx.logs.error(&message).await;
            (StatusCode::INTERNAL_SERVER_ERROR, err).into_response()
        }
        None => (StatusCode::NOT_FOUND, "workspace module not found").into_response(),
    }
}
