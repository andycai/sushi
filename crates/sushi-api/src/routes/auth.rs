use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::middleware::AuthUser;
use sushi_core::auth::model::{LoginRequest, TokenResponse};
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::storage::Storage;

#[derive(Clone)]
pub struct AuthRouteState {
    pub storage: Arc<dyn Storage>,
    pub jwt: Arc<JwtService>,
}

pub fn auth_routes(state: AuthRouteState) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/me", get(me))
        .with_state(state)
}

async fn login(
    State(state): State<AuthRouteState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let repo = UserRepository::new(Arc::clone(&state.storage));
    match repo.find_by_username(&req.username).await {
        Ok(Some(user)) => {
            if password::verify_password(&req.password, &user.password_hash).unwrap_or(false) {
                match state
                    .jwt
                    .create_access_token(user.id, &user.username, &user.role.to_string())
                    .and_then(|at| {
                        state
                            .jwt
                            .create_refresh_token(user.id, &user.username, &user.role.to_string())
                            .map(|rt| TokenResponse {
                                access_token: at,
                                refresh_token: rt,
                                token_type: "Bearer".to_string(),
                            })
                    }) {
                    Ok(tokens) => (StatusCode::OK, Json(json!(tokens))).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": e })),
                    )
                        .into_response(),
                }
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Invalid credentials" })),
                )
                    .into_response()
            }
        }
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid credentials" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh(
    State(state): State<AuthRouteState>,
    Json(req): Json<RefreshRequest>,
) -> impl IntoResponse {
    match state.jwt.verify_token(&req.refresh_token) {
        Ok(claims) => {
            // Validate token type immediately after verification
            if claims.token_type != "refresh" {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Invalid token type. Expected refresh token." })),
                )
                    .into_response();
            }

            match state.jwt.create_access_token(
                claims.sub.parse().unwrap_or(0),
                &claims.username,
                &claims.role,
            ) {
                Ok(access_token) => (
                    StatusCode::OK,
                    Json(json!({
                        "access_token": access_token,
                        "token_type": "Bearer"
                    })),
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e })),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": format!("Invalid refresh token: {}", e) })),
        )
            .into_response(),
    }
}

async fn me(
    axum::extract::Extension(user): axum::extract::Extension<AuthUser>,
) -> impl IntoResponse {
    Json(json!({
        "id": user.0.id,
        "username": user.0.username,
        "role": user.0.role.to_string(),
    }))
}
