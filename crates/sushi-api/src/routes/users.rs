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
use sushi_core::auth::rbac::RbacRepository;
use sushi_core::auth::repository::UserRepository;
use sushi_core::runtime::{HttpHandler, HttpRequest, HttpResponse, HttpRouteSpec, StagedRegistrar};
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
    crate::router::plugin_http_response(list_users_response(&state.storage, pagination).await)
}

async fn list_users_response(
    storage: &Arc<dyn Storage>,
    pagination: PaginationParams,
) -> HttpResponse {
    let repo = UserRepository::new(Arc::clone(storage));

    // Validate pagination parameters
    let limit = pagination.limit.min(100).max(1); // Cap at 100
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
            json_http_response(
                StatusCode::OK,
                json!({
                    "users": response,
                    "limit": limit,
                    "offset": offset,
                }),
            )
        }
        Err(e) => json_http_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": e })),
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
        if !self
            .username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
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
    crate::router::plugin_http_response(create_user_response(&state.storage, req).await)
}

async fn create_user_response(storage: &Arc<dyn Storage>, req: CreateUserRequest) -> HttpResponse {
    // Validate input
    if let Err(e) = req.validate() {
        return json_http_response(StatusCode::BAD_REQUEST, json!({ "error": e }));
    }

    let repo = UserRepository::new(Arc::clone(storage));
    let role_repo = RbacRepository::new(Arc::clone(storage));

    let role_slug = req
        .role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("viewer")
        .to_ascii_lowercase();
    match role_repo.find_role_by_slug(&role_slug).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return json_http_response(
                StatusCode::BAD_REQUEST,
                json!({ "error": "Selected role does not exist" }),
            );
        }
        Err(err) => {
            return json_http_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": err }));
        }
    }
    let role = UserRole::from_slug(&role_slug);

    let password_hash = match password::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            return json_http_response(StatusCode::BAD_REQUEST, json!({ "error": e }));
        }
    };

    match repo
        .create_user(&req.username, &req.email, &password_hash, role)
        .await
    {
        Ok(user) => json_http_response(
            StatusCode::CREATED,
            json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "role": user.role.to_string(),
            }),
        ),
        Err(e) => json_http_response(StatusCode::BAD_REQUEST, json!({ "error": e })),
    }
}

async fn delete_user(
    State(state): State<UsersRouteState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    crate::router::plugin_http_response(delete_user_response(&state.storage, id).await)
}

async fn delete_user_response(storage: &Arc<dyn Storage>, id: i64) -> HttpResponse {
    let repo = UserRepository::new(Arc::clone(storage));
    match repo.delete_user(id).await {
        Ok(()) => json_http_response(StatusCode::NO_CONTENT, json!(null)),
        Err(e) => json_http_response(StatusCode::NOT_FOUND, json!({ "error": e })),
    }
}

pub fn register_builtin_routes(
    staged: &mut StagedRegistrar,
    plugin_name: &'static str,
    storage: Arc<dyn Storage>,
) {
    let list_storage = Arc::clone(&storage);
    staged.register_http(
        HttpRouteSpec::new("GET", "/api/users", plugin_name, "rust::users-list")
            .with_policy(Some("api.users.read".to_string()))
            .with_rust_handler(HttpHandler::new(move |request| {
                let storage = Arc::clone(&list_storage);
                async move {
                    let pagination = match parse_pagination(&request) {
                        Ok(pagination) => pagination,
                        Err(error) => return Ok(bad_request_response(error)),
                    };
                    Ok(list_users_response(&storage, pagination).await)
                }
            })),
    );

    let create_storage = Arc::clone(&storage);
    staged.register_http(
        HttpRouteSpec::new("POST", "/api/users", plugin_name, "rust::users-create")
            .with_policy(Some("api.users.manage".to_string()))
            .with_rust_handler(HttpHandler::new(move |request| {
                let storage = Arc::clone(&create_storage);
                async move {
                    let body = request.body.unwrap_or_default();
                    let payload = match serde_json::from_slice::<CreateUserRequest>(&body) {
                        Ok(payload) => payload,
                        Err(error) => {
                            return Ok(bad_request_response(format!(
                                "invalid users request body: {error}"
                            )))
                        }
                    };
                    Ok(create_user_response(&storage, payload).await)
                }
            })),
    );

    staged.register_http(
        HttpRouteSpec::new("DELETE", "/api/users/*", plugin_name, "rust::users-delete")
            .with_policy(Some("api.users.manage".to_string()))
            .with_rust_handler(HttpHandler::new(move |request| {
                let storage = Arc::clone(&storage);
                async move {
                    let id = match request
                        .path
                        .strip_prefix("/api/users/")
                        .and_then(|value| value.parse::<i64>().ok())
                    {
                        Some(id) => id,
                        None => {
                            return Ok(bad_request_response(format!(
                                "invalid user id in path: {}",
                                request.path
                            )))
                        }
                    };
                    Ok(delete_user_response(&storage, id).await)
                }
            })),
    );
}

fn parse_pagination(request: &HttpRequest) -> Result<PaginationParams, String> {
    let query = request
        .dispatch_path
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();
    let mut pagination = PaginationParams {
        limit: default_limit(),
        offset: 0,
    };
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "limit" => {
                pagination.limit = value
                    .parse()
                    .map_err(|error| format!("invalid limit query value: {error}"))?;
            }
            "offset" => {
                pagination.offset = value
                    .parse()
                    .map_err(|error| format!("invalid offset query value: {error}"))?;
            }
            _ => {}
        }
    }
    Ok(pagination)
}

fn json_http_response(status: StatusCode, payload: Value) -> HttpResponse {
    HttpResponse::new(
        status.as_u16(),
        serde_json::to_vec(&payload).expect("JSON value serialization cannot fail"),
    )
    .with_header("content-type", "application/json")
}

fn bad_request_response(error: impl Into<String>) -> HttpResponse {
    json_http_response(StatusCode::BAD_REQUEST, json!({ "error": error.into() }))
}
