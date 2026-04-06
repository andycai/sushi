use crate::routes::{dashboard, plugins, users, config, logs, kv};
use axum::{
    extract::Request,
    middleware::Next,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::json;

pub fn build_admin_router() -> Router {
    Router::new()
        .route("/admin", get(axum::response::Redirect::temporary("/admin/")))
        .route("/admin/", get(dashboard::dashboard_page))
        .route("/admin/plugins", get(plugins::plugins_page))
        .route("/admin/users", get(users::users_page))
        .route("/admin/config", get(config::config_page))
        .route("/admin/logs", get(logs::logs_page))
        .route("/admin/kv", get(kv::kv_page))
        .route("/admin/api/plugins", get(list_plugins_api))
        .layer(axum::middleware::from_fn(admin_auth_middleware))
}

async fn list_plugins_api() -> impl IntoResponse {
    axum::Json(json!([]))
}

async fn admin_auth_middleware(req: Request, next: Next) -> impl IntoResponse {
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

    if token.is_none() {
        return axum::response::Redirect::temporary("/admin-login").into_response();
    }

    next.run(req).await
}
