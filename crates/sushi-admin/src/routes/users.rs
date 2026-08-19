use axum::extract::State;
use axum::extract::{Form, Path};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;
use sushi_core::auth::model::UserRole;
use sushi_core::auth::password;
use sushi_core::auth::rbac::RbacRepository;
use sushi_core::auth::repository::UserRepository;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpResponse, HttpRouteSpec, MenuContributionSpec, StagedRegistrar,
};
use sushi_core::storage::Storage;

pub async fn users_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(users_page_response(&ctx).await)
}

pub async fn users_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(users_table_response(&ctx).await)
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("rbac-admin.users", "Users", 20)
            .with_icon(Some("users".to_string()))
            .with_parent(Some("host-admin.system".to_string()))
            .with_route(Some("/admin/users".to_string()))
            .with_policy(Some("admin.users.view".to_string())),
    );
    let page_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new("/admin/users", "Users", "rbac-admin", "rust::users-page")
            .with_policy(Some("admin.users.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = page_ctx.clone();
                async move { Ok(users_page_response(&ctx).await) }
            })),
    );
    let table_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "GET",
            "/admin/partials/users/table",
            "rbac-admin",
            "rust::users-table",
        )
        .with_policy(Some("admin.users.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = table_ctx.clone();
            async move { Ok(users_table_response(&ctx).await) }
        })),
    );
    let create_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/admin/partials/users/create",
            "rbac-admin",
            "rust::users-create",
        )
        .with_policy(Some("admin.users.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = create_ctx.clone();
            async move {
                let form = match super::transport::decode_form(&request) {
                    Ok(form) => form,
                    Err(response) => return Ok(response),
                };
                Ok(users_create_response(&ctx, form).await)
            }
        })),
    );
    staged.register_http(
        HttpRouteSpec::new(
            "DELETE",
            "/admin/partials/users/{id}",
            "rbac-admin",
            "rust::users-delete",
        )
        .with_policy(Some("admin.users.manage".to_string()))
        .with_rust_handler(HttpHandler::new(move |request| {
            let ctx = ctx.clone();
            async move {
                let id =
                    match super::transport::path_i64(&request.path, "/admin/partials/users/", "") {
                        Ok(id) => id,
                        Err(response) => return Ok(response),
                    };
                Ok(users_delete_response(&ctx, id).await)
            }
        })),
    );
}

async fn users_page_response(ctx: &SushiContext) -> HttpResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let roles = repo.list_roles().await.unwrap_or_default();

    crate::render::render_template_http_response(
        ctx,
        "admin/users.html",
        serde_json::json!({
            "roles": roles,
        }),
    )
    .await
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
    sushi_api::router::plugin_http_response(users_create_response(&ctx, form).await)
}

async fn users_create_response(ctx: &SushiContext, form: CreateUserForm) -> HttpResponse {
    if let Err(message) = validate_create_user_form(&form) {
        return super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &message)
            .await;
    }

    let role_slug = form
        .role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("viewer")
        .to_ascii_lowercase();

    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let role_repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);

    match role_repo.find_role_by_slug(&role_slug).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return super::transport::flash_response(
                ctx,
                StatusCode::BAD_REQUEST,
                "error",
                "Selected role does not exist",
            )
            .await;
        }
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

    let role = UserRole::from_slug(&role_slug);
    let password_hash = match password::hash_password(&form.password) {
        Ok(hash) => hash,
        Err(err) => {
            return super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &err)
                .await
        }
    };

    match repo
        .create_user(&form.username, &form.email, &password_hash, role)
        .await
    {
        Ok(_) => {
            super::transport::flash_response_with_trigger(
                ctx,
                StatusCode::OK,
                "success",
                "User created.",
                r#"{"users:refresh":true,"users:close-modal":true}"#,
            )
            .await
        }
        Err(err) => {
            super::transport::flash_response(ctx, StatusCode::BAD_REQUEST, "error", &err).await
        }
    }
}

pub async fn users_delete_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(users_delete_response(&ctx, id).await)
}

async fn users_delete_response(ctx: &SushiContext, id: i64) -> HttpResponse {
    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo.delete_user(id).await {
        Ok(_) => {
            super::transport::flash_response_with_trigger(
                ctx,
                StatusCode::OK,
                "success",
                "User deleted.",
                "users:refresh",
            )
            .await
        }
        Err(err) => {
            super::transport::flash_response(ctx, StatusCode::NOT_FOUND, "error", &err).await
        }
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

async fn users_table_response(ctx: &SushiContext) -> HttpResponse {
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
        "admin/partials/users_rows.html",
        serde_json::json!({
            "users": users,
        }),
    )
    .await
}
