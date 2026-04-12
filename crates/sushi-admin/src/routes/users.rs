use axum::extract::State;
use axum::extract::{Form, Path};
use axum::http::{header::HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;
use sushi_core::auth::model::UserRole;
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;

pub async fn users_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/users.html").await
}

pub async fn users_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    render_users_rows(&ctx).await
}

#[derive(Debug, Deserialize)]
pub struct CreateUserForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: Option<String>,
}

pub async fn users_create_partial(
    State(ctx): State<SushiContext>,
    Form(form): Form<CreateUserForm>,
) -> impl IntoResponse {
    if let Err(message) = validate_create_user_form(&form) {
        return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &message).await;
    }

    let role = match form.role.as_deref() {
        Some("admin") => UserRole::Admin,
        Some("editor") => UserRole::Editor,
        _ => UserRole::Viewer,
    };

    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let password_hash = match password::hash_password(&form.password) {
        Ok(hash) => hash,
        Err(err) => return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    };

    match repo
        .create_user(&form.username, &form.email, &password_hash, role)
        .await
    {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "User created.",
                r#"{"users:refresh":true,"users:close-modal":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    }
}

pub async fn users_delete_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo.delete_user(id).await {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "User deleted.",
                "users:refresh",
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::NOT_FOUND, "error", &err).await,
    }
}

fn validate_create_user_form(form: &CreateUserForm) -> Result<(), String> {
    if form.username.len() < 3 || form.username.len() > 32 {
        return Err("Username must be between 3 and 32 characters".to_string());
    }
    if !form
        .username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("Username can only contain letters, numbers, and underscores".to_string());
    }
    if form.email.is_empty() {
        return Err("Email is required".to_string());
    }
    if !form.email.contains('@') || !form.email.contains('.') {
        return Err("Invalid email format".to_string());
    }
    if form.password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }
    Ok(())
}

async fn render_users_rows(ctx: &SushiContext) -> Response {
    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let users = match repo.list_users_paginated(100, 0).await {
        Ok(users) => users
            .into_iter()
            .map(|user| {
                serde_json::json!({
                    "id": user.id,
                    "username": user.username,
                    "email": user.email,
                    "role": user.role.to_string(),
                })
            })
            .collect::<Vec<_>>(),
        Err(err) => {
            return flash_response(ctx, StatusCode::INTERNAL_SERVER_ERROR, "error", &err).await;
        }
    };

    crate::render::render_template_with_context(
        ctx,
        "admin/partials/users_rows.html",
        serde_json::json!({
            "users": users,
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
