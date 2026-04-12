use crate::routes::{config, dashboard, login, logs, plugins, users};
use axum::{
    extract::Request,
    extract::State,
    middleware::Next,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use std::collections::HashSet;
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::context::SushiContext;
use tower_http::services::ServeDir;

/// Admin auth middleware state
#[derive(Clone)]
pub struct AdminAuthState {
    pub jwt: Arc<JwtService>,
    pub static_url_prefix: String,
}

pub async fn build_admin_router(ctx: &SushiContext) -> Router {
    let (static_dir, static_url_prefix) = {
        let cfg = ctx.config.get().await;
        (
            cfg.web.static_dir.clone(),
            cfg.web.static_url_prefix.clone(),
        )
    };
    let plugin_pages = ctx.plugins.list_admin_pages().await;

    let static_url_prefix = crate::render::normalize_static_url_prefix(&static_url_prefix);

    let static_router: Router<SushiContext> = Router::new()
        .nest_service(&static_url_prefix, ServeDir::new(static_dir))
        .with_state(());

    let mut router: Router<SushiContext> = Router::new()
        .merge(static_router)
        .route(
            "/admin-login",
            get(login::login_page).post(login::login_submit),
        )
        .route(
            "/admin",
            get(axum::response::Redirect::temporary("/admin/")),
        )
        .route("/admin/", get(dashboard::dashboard_page))
        .route("/admin/plugins", get(plugins::plugins_page))
        .route("/admin/users", get(users::users_page))
        .route("/admin/config", get(config::config_page))
        .route("/admin/api/config", get(config::config_api))
        .route("/admin/logs", get(logs::logs_page))
        .route("/admin/api/logs", get(logs::logs_api))
        .route(
            "/admin/partials/users/table",
            get(users::users_table_partial),
        )
        .route(
            "/admin/partials/users/create",
            post(users::users_create_partial),
        )
        .route(
            "/admin/partials/users/{id}",
            delete(users::users_delete_partial),
        )
        .route(
            "/admin/partials/plugins/table",
            get(plugins::plugins_table_partial),
        )
        .route("/admin/api/plugins", get(list_plugins_api));

    let reserved_paths: HashSet<&str> = HashSet::from([
        "/admin-login",
        "/admin",
        "/admin/",
        "/admin/plugins",
        "/admin/users",
        "/admin/config",
        "/admin/api/config",
        "/admin/logs",
        "/admin/api/logs",
        "/admin/partials/users/table",
        "/admin/partials/users/create",
        "/admin/partials/users/{id}",
        "/admin/partials/plugins/table",
        "/admin/api/plugins",
    ]);

    // Add dynamic admin pages from Lua plugins
    for page_path in plugin_pages {
        if reserved_paths.contains(page_path.as_str()) {
            tracing::warn!("skip plugin admin page due to route collision: {page_path}");
            continue;
        }

        let path = page_path.clone();
        let pm = ctx.plugins.clone();
        router = router.route(
            &page_path,
            get(move || async move {
                match pm.call_admin_handler(&path).await {
                    Some(Ok(html)) => axum::response::Html(html).into_response(),
                    Some(Err(e)) => {
                        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
                    }
                    None => (axum::http::StatusCode::NOT_FOUND, "not found").into_response(),
                }
            }),
        );
    }

    let auth_state = AdminAuthState {
        jwt: Arc::clone(&ctx.jwt),
        static_url_prefix,
    };

    router
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            admin_auth_middleware,
        ))
        .with_state(ctx.clone())
}

async fn list_plugins_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let plugins = ctx.plugins.list_plugins().await;
    axum::Json(plugins)
}

async fn admin_auth_middleware(
    axum::extract::State(state): axum::extract::State<AdminAuthState>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let path = req.uri().path();

    // Redirect /admin (no trailing slash) to /admin/ (with trailing slash)
    if path == "/admin" {
        return axum::response::Redirect::temporary("/admin/").into_response();
    }

    // Allow /admin-login (top-level) without auth — handled by login route
    if path == "/admin-login" {
        return next.run(req).await;
    }

    // Allow static assets without auth
    if matches_static_prefix(path, &state.static_url_prefix) {
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
                return (axum::http::StatusCode::FORBIDDEN, "Admin access required")
                    .into_response();
            }
            next.run(req).await
        }
        Err(_) => axum::response::Redirect::temporary("/admin-login").into_response(),
    }
}

fn matches_static_prefix(path: &str, prefix: &str) -> bool {
    if path == prefix {
        return true;
    }

    match path.strip_prefix(prefix) {
        Some(rest) => rest.starts_with('/'),
        None => false,
    }
}
