use crate::auth::jwt::JwtService;
use crate::auth::model::User;
use crate::auth::model::UserRole;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthUser(pub User);

#[derive(Clone)]
pub struct AuthState {
    pub jwt_service: Arc<JwtService>,
}

pub async fn require_auth(
    axum::extract::State(state): axum::extract::State<AuthState>,
    mut req: Request,
    next: Next,
) -> Response {
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return (StatusCode::UNAUTHORIZED, "{\"error\":\"Missing authorization header\"}").into_response(),
    };

    match state.jwt_service.verify_token(token) {
        Ok(claims) => {
            let role = match claims.role.as_str() {
                "admin" => UserRole::Admin,
                "editor" => UserRole::Editor,
                _ => UserRole::Viewer,
            };
            let user = User {
                id: claims.sub.parse().unwrap_or(0),
                username: claims.username.clone(),
                email: String::new(),
                password_hash: String::new(),
                role,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            req.extensions_mut().insert(AuthUser(user));
            next.run(req).await
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "{\"error\":\"Invalid token\"}").into_response(),
    }
}
