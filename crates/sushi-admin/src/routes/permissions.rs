use axum::extract::{Form, Path, State};
use axum::http::{header::HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;
use sushi_core::auth::rbac::RbacRepository;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;

pub async fn permissions_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/permissions.html").await
}

pub async fn permissions_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    render_permissions_rows(&ctx).await
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
    if let Err(message) = validate_create_permission_form(&form) {
        return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &message).await;
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
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Permission created.",
                r#"{"permissions:refresh":true,"permissions:close-editor":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
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
    if let Err(message) = validate_update_permission_form(&form) {
        return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &message).await;
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
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Permission updated.",
                r#"{"permissions:refresh":true,"permissions:close-editor":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    }
}

pub async fn permissions_delete_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo.delete_permission(id).await {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Permission deleted.",
                "permissions:refresh",
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
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

async fn render_permissions_rows(ctx: &SushiContext) -> Response {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let permissions = match repo.list_permissions().await {
        Ok(permissions) => permissions,
        Err(err) => {
            return flash_response(ctx, StatusCode::INTERNAL_SERVER_ERROR, "error", &err).await;
        }
    };

    crate::render::render_template_with_context(
        ctx,
        "admin/partials/permissions_rows.html",
        serde_json::json!({
            "permissions": permissions,
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
