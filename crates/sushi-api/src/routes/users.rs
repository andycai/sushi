use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use sushi_core::auth::model::UserRole;
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::storage::sqlite::SqliteStorage;

#[derive(Clone)]
pub struct UsersRouteState {
    pub storage: Arc<SqliteStorage>,
}

pub fn users_routes(state: UsersRouteState) -> Router {
    Router::new()
        .route("/", get(list_users).post(create_user))
        .route("/{id}", delete(delete_user))
        .with_state(state)
}

async fn list_users(
    State(state): State<UsersRouteState>,
) -> impl IntoResponse {
    let repo = UserRepository::new(&state.storage);
    match repo.list_users().await {
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
            (StatusCode::OK, Json(json!(response))).into_response()
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

async fn create_user(
    State(state): State<UsersRouteState>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    let repo = UserRepository::new(&state.storage);

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
    let repo = UserRepository::new(&state.storage);
    match repo.delete_user(id).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(json!(null))).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    }
}
