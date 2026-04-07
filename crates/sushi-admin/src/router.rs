use crate::routes::{dashboard, plugins, users, config, logs};
use axum::{
    extract::Request,
    middleware::Next,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::json;
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::PluginManager;

pub async fn build_admin_router(ctx: &SushiContext) -> Router<PluginManager> {
    let mut router: Router<PluginManager> = Router::new()
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
        router = router.route(
            &page_path,
            get(move |axum::extract::State(pm): axum::extract::State<PluginManager>| async move {
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

    router = router.with_state(ctx.plugins.clone());
    router.layer(axum::middleware::from_fn(admin_auth_middleware))
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
