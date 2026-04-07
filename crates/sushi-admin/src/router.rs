use crate::routes::{dashboard, plugins, users, config, logs};
use axum::{
    extract::Request,
    middleware::Next,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::json;
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::context::SushiContext;

/// Admin auth middleware state
#[derive(Clone)]
pub struct AdminAuthState {
    pub jwt: Arc<JwtService>,
}

pub async fn build_admin_router(ctx: &SushiContext) -> Router {
    let mut router: Router = Router::new()
        .route("/admin", get(axum::response::Redirect::temporary("/admin/")))
        .route("/admin/", get(dashboard::dashboard_page))
        .route("/admin/plugins", get(plugins::plugins_page))
        .route("/admin/users", get(users::users_page))
        .route("/admin/config", get(config::config_page))
        .route("/admin/logs", get(logs::logs_page))
        .route("/admin/api/plugins", get(list_plugins_api));

    // Add dynamic admin pages from Lua plugins
    let plugin_pages = ctx.plugins.list_admin_pages().await;
    for page_path in plugin_pages {
        let path = page_path.clone();
        let pm = ctx.plugins.clone();
        router = router.route(
            &page_path,
            get(move || async move {
                match pm.call_admin_handler(&path).await {
                    Some(Ok(html)) => axum::response::Html(html).into_response(),
                    Some(Err(e)) => (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        e,
                    ).into_response(),
                    None => (axum::http::StatusCode::NOT_FOUND, "not found").into_response(),
                }
            }),
        );
    }

    let auth_state = AdminAuthState {
        jwt: Arc::clone(&ctx.jwt),
    };
    
    router.layer(axum::middleware::from_fn_with_state(auth_state, admin_auth_middleware))
}

async fn list_plugins_api() -> impl IntoResponse {
    // Note: This endpoint is protected by the middleware, but doesn't have access to PluginManager
    // In a real implementation, you'd pass PluginManager through app state or extensions
    // For now, return empty data as this is a demo endpoint
    axum::Json(json!({
        "routes": [],
        "commands": [],
        "pages": [],
    }))
}

async fn admin_auth_middleware(
    axum::extract::State(state): axum::extract::State<AdminAuthState>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();

    // Redirect /admin (no trailing slash) to /admin/ (with trailing slash)
    if path == "/admin" {
        return axum::response::Redirect::temporary("/admin/").into_response();
    }

    // Allow /admin-login (top-level) without auth — handled by login_router
    if path == "/admin-login" {
        return next.run(req).await;
    }

    // Auth check on all /admin/* routes
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .and_then(|c| {
                    c.split(';')
                        .find(|s| s.trim().starts_with("sushi_token="))
                        .map(|s| s.trim().strip_prefix("sushi_token=").unwrap_or(""))
                })
        });

    let token = match token {
        Some(t) => t,
        None => return axum::response::Redirect::temporary("/admin-login").into_response(),
    };

    // Validate the JWT token
    match state.jwt.verify_token(token) {
        Ok(claims) => {
            // Only allow access tokens, not refresh tokens
            if claims.token_type != "access" {
                return axum::response::Redirect::temporary("/admin-login").into_response();
            }
            // Only allow admin role for admin panel access
            if claims.role != "admin" {
                return (
                    axum::http::StatusCode::FORBIDDEN,
                    "Admin access required"
                ).into_response();
            }
            next.run(req).await
        }
        Err(_) => axum::response::Redirect::temporary("/admin-login").into_response(),
    }
}
