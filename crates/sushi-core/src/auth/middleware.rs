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
    let path = req.uri().path();

    // Public auth endpoints stay accessible without token.
    if matches!(path, "/api/auth/login" | "/api/auth/refresh") {
        return next.run(req).await;
    }

    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => match extract_token_from_cookie(req.headers().get("cookie").and_then(|v| v.to_str().ok())) {
            Some(token) => token,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    "{\"error\":\"Missing authorization credentials\"}",
                )
                    .into_response();
            }
        },
    };

    match state.jwt_service.verify_token(token) {
        Ok(claims) => {
            // Validate token type - only access tokens are allowed for API access
            if claims.token_type != "access" {
                return (StatusCode::UNAUTHORIZED, "{\"error\":\"Invalid token type. Use access token for API access.\"}").into_response();
            }
            
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

fn extract_token_from_cookie(cookie_header: Option<&str>) -> Option<&str> {
    let cookie = cookie_header?;
    cookie
        .split(';')
        .find_map(|part| part.trim().strip_prefix("sushi_token="))
}
