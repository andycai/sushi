use crate::routes::{
    config, dashboard, login, logs, menu, permissions, plugins, roles, users, workspace,
};
use axum::{
    extract::Request,
    extract::State,
    middleware::Next,
    response::IntoResponse,
    routing::{delete, get, get_service, patch, post},
    Router,
};
use std::collections::HashSet;
use std::sync::Arc;
use sushi_core::auth::authorizer::Authorizer;
use sushi_core::auth::jwt::JwtService;
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::PageResolvedAssets;
use tower_http::services::{ServeDir, ServeFile};

/// Admin auth middleware state
#[derive(Clone)]
pub struct AdminAuthState {
    pub jwt: Arc<JwtService>,
    pub static_url_prefix: String,
    pub authorizer: Arc<Authorizer>,
}

#[derive(Clone, Debug)]
pub struct AdminAuthContext {
    pub role: String,
    pub is_admin: bool,
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
    let plugin_static_roots = ctx.plugins.list_plugin_static_roots().await;

    let static_url_prefix = crate::render::normalize_static_url_prefix(&static_url_prefix);

    let mut static_router = Router::new();
    for (plugin_name, plugin_static_root) in plugin_static_roots {
        if !is_valid_plugin_mount_id(&plugin_name) {
            tracing::warn!("skip invalid plugin static mount name: {plugin_name}");
            continue;
        }
        if !plugin_static_root.is_dir() {
            tracing::warn!(
                "skip missing plugin static root for {}: {}",
                plugin_name,
                plugin_static_root.display()
            );
            continue;
        }
        let mount_path = format!("{static_url_prefix}/plugins/{plugin_name}");
        static_router = static_router.nest_service(&mount_path, ServeDir::new(plugin_static_root));
    }
    let static_router: Router<SushiContext> = static_router
        .nest_service(&static_url_prefix, ServeDir::new(&static_dir))
        .with_state(());

    // Favicon routes - serve at root level for browser compatibility
    // These must be added before auth middleware is applied
    let favicon_router = Router::new()
        .route(
            "/favicon.ico",
            get_service(ServeFile::new(format!("{static_dir}/favicon.svg"))),
        )
        .route(
            "/favicon.svg",
            get_service(ServeFile::new(format!("{static_dir}/favicon.svg"))),
        );

    let mut router: Router<SushiContext> = Router::new()
        .merge(static_router)
        .merge(favicon_router)
        .route(
            "/admin-login",
            get(login::login_page).post(login::login_submit),
        )
        .route(
            "/admin",
            get(axum::response::Redirect::temporary("/admin/")),
        )
        .route("/admin/", get(dashboard::dashboard_page))
        .route(
            "/admin/workspace/{*module}",
            get(workspace::workspace_partial),
        )
        .route(
            "/admin/api/workspace/assets",
            get(workspace::workspace_assets_api),
        )
        .route("/admin/plugins", get(plugins::plugins_page))
        .route(
            "/admin/plugins/{plugin}",
            get(plugins::plugin_workspace_page),
        )
        .route("/admin/users", get(users::users_page))
        .route("/admin/roles", get(roles::roles_page))
        .route("/admin/permissions", get(permissions::permissions_page))
        .route("/admin/config", get(config::config_page))
        .route("/admin/api/config", get(config::config_api))
        .route("/admin/logs", get(logs::logs_page))
        .route("/admin/api/logs", get(logs::logs_api))
        .route("/admin/menus", get(menu::menus_page))
        .merge(menu::routes())
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
            "/admin/partials/roles/table",
            get(roles::roles_table_partial),
        )
        .route(
            "/admin/partials/roles/create",
            post(roles::roles_create_partial),
        )
        .route(
            "/admin/partials/roles/{id}/update",
            post(roles::roles_update_partial),
        )
        .route(
            "/admin/partials/roles/{id}/permissions/form",
            get(roles::role_permissions_form_partial),
        )
        .route(
            "/admin/partials/roles/{id}/permissions",
            post(roles::role_permissions_update_partial),
        )
        .route(
            "/admin/partials/roles/{id}",
            delete(roles::roles_delete_partial),
        )
        .route(
            "/admin/partials/permissions/table",
            get(permissions::permissions_table_partial),
        )
        .route(
            "/admin/partials/permissions/create",
            post(permissions::permissions_create_partial),
        )
        .route(
            "/admin/partials/permissions/{id}/update",
            post(permissions::permissions_update_partial),
        )
        .route(
            "/admin/partials/permissions/{id}",
            delete(permissions::permissions_delete_partial),
        )
        .route(
            "/admin/partials/plugins/table",
            get(plugins::plugins_table_partial),
        )
        .route("/admin/api/plugins", get(list_plugins_api))
        .route(
            "/admin/api/plugins/{plugin}/pages",
            get(plugins::plugin_pages_api),
        )
        .route(
            "/admin/api/plugins/{plugin}/state",
            patch(plugins::plugin_state_api),
        );

    let reserved_paths: HashSet<&str> = HashSet::from([
        "/admin-login",
        "/admin",
        "/admin/",
        "/admin/workspace/{*module}",
        "/admin/api/workspace/assets",
        "/admin/plugins",
        "/admin/plugins/{plugin}",
        "/admin/system",
        "/admin/users",
        "/admin/roles",
        "/admin/permissions",
        "/admin/config",
        "/admin/api/config",
        "/admin/logs",
        "/admin/api/logs",
        "/admin/menus",
        "/admin/api/menu",
        "/admin/partials/users/table",
        "/admin/partials/users/create",
        "/admin/partials/users/{id}",
        "/admin/partials/roles/table",
        "/admin/partials/roles/create",
        "/admin/partials/roles/{id}/update",
        "/admin/partials/roles/{id}/permissions/form",
        "/admin/partials/roles/{id}/permissions",
        "/admin/partials/roles/{id}",
        "/admin/partials/permissions/table",
        "/admin/partials/permissions/create",
        "/admin/partials/permissions/{id}/update",
        "/admin/partials/permissions/{id}",
        "/admin/partials/plugins/table",
        "/admin/api/plugins/{plugin}/pages",
        "/admin/api/plugins/{plugin}/state",
        "/admin/partials/menus/table",
        "/admin/partials/menus/create",
        "/admin/partials/menus/{id}/update",
        "/admin/partials/menus/{id}",
        "/admin/api/plugins",
    ]);

    // Add dynamic admin pages from Lua plugins
    for page_path in plugin_pages {
        if reserved_paths.contains(page_path.as_str())
            || page_path.starts_with("/admin/workspace/")
            || is_plugin_workspace_root_path(page_path.as_str())
        {
            tracing::warn!("skip plugin admin page due to route collision: {page_path}");
            continue;
        }

        let path = page_path.clone();
        let pm = ctx.plugins.clone();
        let logs = ctx.logs.clone();
        router = router.route(
            &page_path,
            get(move || async move {
                match pm.call_admin_handler(&path).await {
                    Some(Ok(html)) => {
                        let assets = pm.admin_page_assets(&path).await.unwrap_or_default();
                        let html_with_assets = append_assets_to_html_response(&html, &assets);
                        axum::response::Html(html_with_assets).into_response()
                    }
                    Some(Err(e)) => {
                        if is_plugin_disabled_error(&e) {
                            return (
                                axum::http::StatusCode::FORBIDDEN,
                                plugin_disabled_message(&e),
                            )
                                .into_response();
                        }
                        let message = format!("plugin runtime error on admin page {path}: {e}");
                        tracing::error!("{message}");
                        logs.error(&message).await;
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
        authorizer: Arc::clone(&ctx.authorizer),
    };

    router
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            admin_auth_middleware,
        ))
        .with_state(ctx.clone())
}

fn is_valid_plugin_mount_id(plugin_mount_id: &str) -> bool {
    if plugin_mount_id.is_empty()
        || plugin_mount_id.starts_with('/')
        || plugin_mount_id.ends_with('/')
        || plugin_mount_id.contains("..")
    {
        return false;
    }

    let mut has_segment = false;
    for segment in plugin_mount_id.split('/') {
        if segment.is_empty() {
            return false;
        }
        has_segment = true;
        if !segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return false;
        }
    }

    has_segment
}

fn is_plugin_disabled_error(err: &str) -> bool {
    err.starts_with("plugin_disabled:")
}

fn plugin_disabled_message(err: &str) -> String {
    err.strip_prefix("plugin_disabled:")
        .map(str::trim)
        .filter(|msg| !msg.is_empty())
        .unwrap_or("plugin is disabled")
        .to_string()
}

async fn list_plugins_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let plugins = ctx.plugins.list_plugins().await;
    axum::Json(plugins)
}

async fn admin_auth_middleware(
    axum::extract::State(state): axum::extract::State<AdminAuthState>,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();

    // Redirect /admin (no trailing slash) to /admin/ (with trailing slash)
    if path == "/admin" {
        return axum::response::Redirect::temporary("/admin/").into_response();
    }

    // Allow /admin-login (top-level) without auth — handled by login route
    if path == "/admin-login" {
        return next.run(req).await;
    }

    // Allow favicon/favicon.ico without auth
    if path == "/favicon.ico" || path == "/favicon.svg" || path == "/favicon.png" {
        return next.run(req).await;
    }

    // Allow static assets without auth
    if matches_static_prefix(&path, &state.static_url_prefix) {
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

            let auth_context = AdminAuthContext {
                role: claims.role.clone(),
                is_admin: claims.role == "admin",
            };
            req.extensions_mut().insert(auth_context.clone());

            if auth_context.is_admin {
                return next.run(req).await;
            }

            match state
                .authorizer
                .check_http(&auth_context.role, "admin", &method, &path)
                .await
            {
                Ok(()) => next.run(req).await,
                Err(_) => (
                    axum::http::StatusCode::FORBIDDEN,
                    "Insufficient privileges for admin access",
                )
                    .into_response(),
            }
        }
        Err(_) => axum::response::Redirect::temporary("/admin-login").into_response(),
    }
}

fn is_plugin_workspace_root_path(path: &str) -> bool {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    segments.len() == 3 && segments[0] == "admin" && segments[1] == "plugins"
}

fn append_assets_to_html_response(html: &str, assets: &PageResolvedAssets) -> String {
    if assets.js.is_empty() && assets.css.is_empty() {
        return html.to_string();
    }

    let mut tags = String::new();
    for css in &assets.css {
        tags.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"{}\" data-admin-asset-css=\"{}\">",
            html_escape_attr(css),
            html_escape_attr(css)
        ));
    }
    for js in &assets.js {
        tags.push_str(&format!(
            "<script src=\"{}\" data-admin-asset-js=\"{}\" data-admin-asset-loaded=\"true\"></script>",
            html_escape_attr(js),
            html_escape_attr(js)
        ));
    }

    if html.contains("</body>") {
        return html.replacen("</body>", &format!("{tags}</body>"), 1);
    }

    format!("{html}{tags}")
}

fn html_escape_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
