use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;
use sushi_core::auth::rbac::RbacRepository;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpResponse, HttpRouteSpec, MenuContributionSpec, StagedRegistrar,
};
use sushi_core::storage::Storage;

pub async fn permissions_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(permissions_page_response(&ctx).await)
}

pub async fn permissions_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(permissions_table_response(&ctx).await)
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("rbac-admin.permissions", "Permissions", 40)
            .with_icon(Some("key".to_string()))
            .with_parent(Some("host-admin.system".to_string()))
            .with_route(Some("/admin/permissions".to_string()))
            .with_policy(Some("admin.permissions.view".to_string())),
    );
    let page_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new(
            "/admin/permissions",
            "Permissions",
            "rbac-admin",
            "rust::permissions-page",
        )
        .with_policy(Some("admin.permissions.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = page_ctx.clone();
            async move { Ok(permissions_page_response(&ctx).await) }
        })),
    );
    let table_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/partials/permissions/table",
            "rbac-admin",
            "rust::permissions-table",
        )
        .with_policy(Some("admin.permissions.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = table_ctx.clone();
            async move { Ok(permissions_table_response(&ctx).await) }
        })),
    );
    let create_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/admin/partials/permissions/create",
            "rbac-admin",
            "rust::permissions-create",
        )
        .with_policy(Some("admin.permissions.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = create_ctx.clone();
            async move {
                let form = match super::transport::decode_form(&request) {
                    Ok(form) => form,
                    Err(response) => return Ok(response),
                };
                Ok(permissions_create_response(&ctx, form).await)
            }
        })),
    );
    let update_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/admin/partials/permissions/{id}/update",
            "rbac-admin",
            "rust::permissions-update",
        )
        .with_policy(Some("admin.permissions.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = update_ctx.clone();
            async move {
                let id = match super::transport::path_i64(
                    &request.path,
                    "/admin/partials/permissions/",
                    "/update",
                ) {
                    Ok(id) => id,
                    Err(response) => return Ok(response),
                };
                let form = match super::transport::decode_form(&request) {
                    Ok(form) => form,
                    Err(response) => return Ok(response),
                };
                Ok(permissions_update_response(&ctx, id, form).await)
            }
        })),
    );
    staged.register_http(
        HttpRouteSpec::new(
            "DELETE",
            "/admin/partials/permissions/{id}",
            "rbac-admin",
            "rust::permissions-delete",
        )
        .with_policy(Some("admin.permissions.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = ctx.clone();
            async move {
                let id = match super::transport::path_i64(
                    &request.path,
                    "/admin/partials/permissions/",
                    "",
                ) {
                    Ok(id) => id,
                    Err(response) => return Ok(response),
                };
                Ok(permissions_delete_response(&ctx, id).await)
            }
        })),
    );
}

async fn permissions_page_response(ctx: &SushiContext) -> HttpResponse {
    crate::render::render_template_http_response(
        ctx,
        "admin/permissions.html",
        serde_json::json!({}),
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct CreatePermissionForm {
    pub slug: String,
    pub name: String,
    pub module: String,
    pub description: Option<String>,
}

pub async fn permissions_create_partial(
    State(ctx): State<SushiContext>,
    Form(form): Form<CreatePermissionForm>,
) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(permissions_create_response(&ctx, form).await)
}

async fn permissions_create_response(
    ctx: &SushiContext,
    form: CreatePermissionForm,
) -> HttpResponse {
    if let Err(message) = validate_create_permission_form(&form) {
        return super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &message)
            .await;
    }

    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo
        .create_permission(
            &form.slug.trim().to_ascii_lowercase(),
            form.name.trim(),
            &form.module.trim().to_ascii_lowercase(),
            form.description.as_deref().unwrap_or_default().trim(),
        )
        .await
    {
        Ok(_) => {
            super::transport::flash_response_with_trigger(
                ctx,
                StatusCode::OK,
                "success",
                "Permission created.",
                r#"{"permissions:refresh":true,"permissions:close-editor":true}"#,
            )
            .await
        }
        Err(err) => {
            super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &err).await
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdatePermissionForm {
    pub name: String,
    pub module: String,
    pub description: Option<String>,
}

pub async fn permissions_update_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
    Form(form): Form<UpdatePermissionForm>,
) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(permissions_update_response(&ctx, id, form).await)
}

async fn permissions_update_response(
    ctx: &SushiContext,
    id: i64,
    form: UpdatePermissionForm,
) -> HttpResponse {
    if let Err(message) = validate_update_permission_form(&form) {
        return super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &message)
            .await;
    }

    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo
        .update_permission(
            id,
            form.name.trim(),
            &form.module.trim().to_ascii_lowercase(),
            form.description.as_deref().unwrap_or_default().trim(),
        )
        .await
    {
        Ok(_) => {
            super::transport::flash_response_with_trigger(
                ctx,
                StatusCode::OK,
                "success",
                "Permission updated.",
                r#"{"permissions:refresh":true,"permissions:close-editor":true}"#,
            )
            .await
        }
        Err(err) => {
            super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &err).await
        }
    }
}

pub async fn permissions_delete_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(permissions_delete_response(&ctx, id).await)
}

async fn permissions_delete_response(ctx: &SushiContext, id: i64) -> HttpResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo.delete_permission(id).await {
        Ok(_) => {
            super::transport::flash_response_with_trigger(
                ctx,
                StatusCode::OK,
                "success",
                "Permission deleted.",
                "permissions:refresh",
            )
            .await
        }
        Err(err) => {
            super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &err).await
        }
    }
}

fn validate_create_permission_form(form: &CreatePermissionForm) -> Result<(), String> {
    validate_permission_slug(&form.slug)?;
    validate_permission_name(&form.name)?;
    validate_permission_module(&form.module)?;
    validate_permission_description(form.description.as_deref())?;
    Ok(())
}

fn validate_update_permission_form(form: &UpdatePermissionForm) -> Result<(), String> {
    validate_permission_name(&form.name)?;
    validate_permission_module(&form.module)?;
    validate_permission_description(form.description.as_deref())?;
    Ok(())
}

fn validate_permission_slug(input: &str) -> Result<(), String> {
    let slug = input.trim();
    if slug.len() < 3 || slug.len() > 80 {
        return Err("Permission key must be between 3 and 80 characters".to_string());
    }
    if !slug.chars().all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '_' || ch == '-'
    }) {
        return Err(
            "Permission key can only include lowercase letters, numbers, dots, dashes, and underscores".to_string(),
        );
    }
    Ok(())
}

fn validate_permission_name(input: &str) -> Result<(), String> {
    let name = input.trim();
    if name.is_empty() {
        return Err("Permission name is required".to_string());
    }
    if name.len() > 80 {
        return Err("Permission name must be 80 characters or fewer".to_string());
    }
    Ok(())
}

fn validate_permission_module(input: &str) -> Result<(), String> {
    let module = input.trim();
    if module.is_empty() {
        return Err("Module is required".to_string());
    }
    if module.len() > 40 {
        return Err("Module must be 40 characters or fewer".to_string());
    }
    if !module
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(
            "Module can only include lowercase letters, numbers, dashes, and underscores"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_permission_description(input: Option<&str>) -> Result<(), String> {
    if let Some(description) = input {
        if description.trim().len() > 280 {
            return Err("Description must be 280 characters or fewer".to_string());
        }
    }
    Ok(())
}

async fn permissions_table_response(ctx: &SushiContext) -> HttpResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let permissions = match repo.list_permissions().await {
        Ok(permissions) => permissions,
        Err(err) => {
            return super::transport::flash_response(
                ctx,
                StatusCode::INTERNAL_SERVER_ERROR,
                "error",
                &err,
            )
            .await;
        }
    };

    crate::render::render_template_http_response(
        ctx,
        "admin/partials/permissions_rows.html",
        serde_json::json!({
            "permissions": permissions,
        }),
    )
    .await
}
