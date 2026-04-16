use crate::auth::authorizer::Authorizer;
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
    pub authorizer: Arc<Authorizer>,
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

    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => match extract_token_from_cookie(
            req.headers().get("cookie").and_then(|v| v.to_str().ok()),
        ) {
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
                return (
                    StatusCode::UNAUTHORIZED,
                    "{\"error\":\"Invalid token type. Use access token for API access.\"}",
                )
                    .into_response();
            }

            let role = UserRole::from_slug(&claims.role);
            let role_slug = role.as_str().to_string();
            let method = req.method().as_str().to_string();

            if state
                .authorizer
                .check_http(&role_slug, "api", &method, path)
                .await
                .is_err()
            {
                return (
                    StatusCode::FORBIDDEN,
                    "{\"error\":\"Insufficient permissions for this API route\"}",
                )
                    .into_response();
            }

            // Admin partial endpoints are privileged and require admin role.
            if path.starts_with("/admin/partials/") && !role.is_admin() {
                return (
                    StatusCode::FORBIDDEN,
                    "{\"error\":\"Admin role required for admin partial routes\"}",
                )
                    .into_response();
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::authorizer::{CompiledPolicySnapshot, HttpBinding};
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    fn auth_state(
        http_bindings: Vec<HttpBinding>,
        role_grants: Vec<(&str, &str)>,
    ) -> (AuthState, Arc<JwtService>) {
        let jwt = Arc::new(JwtService::new(
            "test-secret-key-at-least-32-chars-long!",
            3600,
            604800,
        ));
        let authorizer = Arc::new(Authorizer::new(CompiledPolicySnapshot::new(
            http_bindings,
            vec![],
            role_grants
                .into_iter()
                .map(|(role, policy_key)| (role.to_string(), policy_key.to_string()))
                .collect(),
        )));
        (
            AuthState {
                jwt_service: Arc::clone(&jwt),
                authorizer,
            },
            jwt,
        )
    }

    #[tokio::test]
    async fn non_admin_cannot_access_admin_partials() {
        let (state, jwt) = auth_state(
            vec![HttpBinding {
                surface: "api".to_string(),
                method: "GET".to_string(),
                path_pattern: "/admin/partials/kv/table".to_string(),
                policy_key: "api.admin.partials.read".to_string(),
            }],
            vec![("editor", "api.admin.partials.read")],
        );
        let app = Router::new()
            .route("/admin/partials/kv/table", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(state, require_auth))
            .with_state(());

        let token = jwt
            .create_access_token(1, "editor-user", "editor")
            .expect("failed to build token");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/partials/kv/table")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("request failed");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_can_access_admin_partials() {
        let (state, jwt) = auth_state(
            vec![HttpBinding {
                surface: "api".to_string(),
                method: "GET".to_string(),
                path_pattern: "/admin/partials/kv/table".to_string(),
                policy_key: "api.admin.partials.read".to_string(),
            }],
            vec![("admin", "api.admin.partials.read")],
        );
        let app = Router::new()
            .route("/admin/partials/kv/table", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(state, require_auth))
            .with_state(());

        let token = jwt
            .create_access_token(1, "admin-user", "admin")
            .expect("failed to build token");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/partials/kv/table")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("request failed");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn viewer_without_policy_grant_is_denied_api_route() {
        let (state, jwt) = auth_state(
            vec![HttpBinding {
                surface: "api".to_string(),
                method: "GET".to_string(),
                path_pattern: "/api/users".to_string(),
                policy_key: "api.users.read".to_string(),
            }],
            vec![("admin", "api.users.read")],
        );
        let app = Router::new()
            .route("/api/users", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(state, require_auth))
            .with_state(());

        let token = jwt
            .create_access_token(1, "viewer-user", "viewer")
            .expect("failed to build token");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/users")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("request failed");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
