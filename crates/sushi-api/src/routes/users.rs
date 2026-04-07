use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use sushi_core::auth::model::UserRole;
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::storage::Storage;

#[derive(Clone)]
pub struct UsersRouteState {
    pub storage: Arc<dyn Storage>,
}

#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

pub fn users_routes(state: UsersRouteState) -> Router {
    Router::new()
        .route("/", get(list_users).post(create_user))
        .route("/{id}", delete(delete_user))
        .with_state(state)
}

async fn list_users(
    State(state): State<UsersRouteState>,
    Query(pagination): Query<PaginationParams>,
) -> impl IntoResponse {
    let repo = UserRepository::new(Arc::clone(&state.storage));
    
    // Validate pagination parameters
    let limit = pagination.limit.min(100).max(1);  // Cap at 100
    let offset = pagination.offset;
    
    match repo.list_users_paginated(limit, offset).await {
        Ok(users) => {
            let response: Vec<Value> = users
                .into_iter()
                .map(|u| {
                    json!({
                        "id": u.id,
                        "username": u.username,
                        "email": u.email,
                        "role": u.role.to_string(),
                        "created_at": u.created_at.to_rfc3339(),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({
                "users": response,
                "limit": limit,
                "offset": offset,
            }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: Option<String>,
}

impl CreateUserRequest {
    /// Validate user input fields
    fn validate(&self) -> Result<(), String> {
        // Validate username (3-32 chars, alphanumeric and underscore only)
        if self.username.len() < 3 || self.username.len() > 32 {
            return Err("Username must be between 3 and 32 characters".to_string());
        }
        if !self.username.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err("Username can only contain letters, numbers, and underscores".to_string());
        }
        
        // Validate email (basic format check)
        if self.email.is_empty() {
            return Err("Email is required".to_string());
        }
        if !self.email.contains('@') || !self.email.contains('.') {
            return Err("Invalid email format".to_string());
        }
        if self.email.len() > 255 {
            return Err("Email must be less than 255 characters".to_string());
        }
        
        // Validate password (min 8 chars)
        if self.password.len() < 8 {
            return Err("Password must be at least 8 characters".to_string());
        }
        if self.password.len() > 128 {
            return Err("Password must be less than 128 characters".to_string());
        }
        
        Ok(())
    }
}

async fn create_user(
    State(state): State<UsersRouteState>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    // Validate input
    if let Err(e) = req.validate() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response();
    }
    
    let repo = UserRepository::new(Arc::clone(&state.storage));

    let role = match req.role.as_deref() {
        Some("admin") => UserRole::Admin,
        Some("editor") => UserRole::Editor,
        _ => UserRole::Viewer,
    };

    let password_hash = match password::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response();
        }
    };

    match repo.create_user(&req.username, &req.email, &password_hash, role).await {
        Ok(user) => (
            StatusCode::CREATED,
            Json(json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "role": user.role.to_string(),
            })),
        ).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn delete_user(
    State(state): State<UsersRouteState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let repo = UserRepository::new(Arc::clone(&state.storage));
    match repo.delete_user(id).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(json!(null))).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    }
}
