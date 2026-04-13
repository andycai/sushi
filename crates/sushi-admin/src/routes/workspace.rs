use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use sushi_core::{auth::rbac::RbacRepository, context::SushiContext, storage::Storage};

fn module_template(module: &str) -> Option<&'static str> {
    match module {
        "dashboard" => Some("admin/workspace/dashboard.html"),
        "users" => Some("admin/workspace/users.html"),
        "roles" => Some("admin/workspace/roles.html"),
        "permissions" => Some("admin/workspace/permissions.html"),
        "plugins" => Some("admin/workspace/plugins.html"),
        "kv" => Some("admin/workspace/kv.html"),
        "config" => Some("admin/workspace/config.html"),
        "logs" => Some("admin/workspace/logs.html"),
        "menus" => Some("admin/workspace/menus.html"),
        _ => None,
    }
}

pub fn permission_for_module(module: &str) -> Option<&'static str> {
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
        _ => None,
    }
}

pub async fn workspace_partial(
    Path(module): Path<String>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    let Some(template) = module_template(module.as_str()) else {
        return (StatusCode::NOT_FOUND, "workspace module not found").into_response();
    };

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
    response
}
