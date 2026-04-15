use crate::routes::{
    config, dashboard, login, logs, menu, permissions, plugins, roles, users, workspace,
};
use axum::{
    extract::Request,
    extract::State,
    middleware::Next,
    response::IntoResponse,
    routing::{delete, get, get_service, post},
    Router,
};
use std::collections::HashSet;
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::rbac::RbacRepository;
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::PageResolvedAssets;
use sushi_core::storage::Storage;
use tower_http::services::{ServeDir, ServeFile};

/// Admin auth middleware state
#[derive(Clone)]
pub struct AdminAuthState {
    pub jwt: Arc<JwtService>,
    pub static_url_prefix: String,
    pub storage: Arc<dyn Storage>,
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
        if plugin_name.trim().is_empty() || plugin_name.contains('/') {
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
        storage: ctx.db.clone() as Arc<dyn Storage>,
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

            if claims.role == "admin" {
                return next.run(req).await;
            }

            let required_permission = required_admin_permission(&method, path);
            let required_permission = match required_permission {
                Some(permission) => permission,
                None => {
                    return (
                        axum::http::StatusCode::FORBIDDEN,
                        "Insufficient privileges for admin access",
                    )
                        .into_response();
                }
            };

            let repo = RbacRepository::new(Arc::clone(&state.storage));
            match repo
                .role_has_permission(&claims.role, required_permission)
                .await
            {
                Ok(true) => next.run(req).await,
                Ok(false) => (
                    axum::http::StatusCode::FORBIDDEN,
                    "Insufficient privileges for admin access",
                )
                    .into_response(),
                Err(err) => (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Authorization check failed: {err}"),
                )
                    .into_response(),
            }
        }
        Err(_) => axum::response::Redirect::temporary("/admin-login").into_response(),
    }
}

fn required_admin_permission(method: &str, path: &str) -> Option<&'static str> {
    // Routes with identical access semantics are grouped for maintainability.
    if method == "GET" {
        if let Some(module) = path.strip_prefix("/admin/workspace/") {
            return workspace::permission_for_module(module);
        }
    }

    let read_map: &[(&str, &str)] = &[
        ("GET", "/admin/"),
        ("GET", "/admin/logs"),
        ("GET", "/admin/api/logs"),
        ("GET", "/admin/config"),
        ("GET", "/admin/api/config"),
        ("GET", "/admin/plugins"),
        ("GET", "/admin/plugins/{plugin}"),
        ("GET", "/admin/partials/plugins/table"),
        ("GET", "/admin/api/plugins"),
        ("GET", "/admin/api/plugins/{plugin}/pages"),
        ("GET", "/admin/api/workspace/assets"),
        ("GET", "/admin/menus"),
        ("GET", "/admin/partials/menus/table"),
        ("GET", "/admin/api/menu"),
        ("GET", "/admin/users"),
        ("GET", "/admin/partials/users/table"),
        ("GET", "/admin/roles"),
        ("GET", "/admin/partials/roles/table"),
        ("GET", "/admin/permissions"),
        ("GET", "/admin/partials/permissions/table"),
        ("GET", "/admin/partials/roles/{id}/permissions/form"),
    ];

    let write_map: &[(&str, &str)] = &[
        ("POST", "/admin/partials/users/create"),
        ("DELETE", "/admin/partials/users/{id}"),
        ("POST", "/admin/partials/roles/create"),
        ("POST", "/admin/partials/roles/{id}/update"),
        ("POST", "/admin/partials/roles/{id}/permissions"),
        ("DELETE", "/admin/partials/roles/{id}"),
        ("POST", "/admin/partials/permissions/create"),
        ("POST", "/admin/partials/permissions/{id}/update"),
        ("DELETE", "/admin/partials/permissions/{id}"),
        ("POST", "/admin/partials/menus/create"),
        ("POST", "/admin/partials/menus/{id}/update"),
        ("DELETE", "/admin/partials/menus/{id}"),
        ("POST", "/admin/api/menu"),
        ("PUT", "/admin/api/menu/{id}"),
        ("DELETE", "/admin/api/menu/{id}"),
    ];

    if path == "/admin/kv" || path.starts_with("/admin/partials/kv/") {
        return Some("kv.manage");
    }

    if read_map
        .iter()
        .any(|(m, p)| *m == method && admin_path_matches(path, p))
    {
        return Some(match path {
            "/admin/" => "dashboard.view",
            "/admin/logs" | "/admin/api/logs" => "logs.view",
            "/admin/config" | "/admin/api/config" => "config.view",
            "/admin/plugins" | "/admin/partials/plugins/table" | "/admin/api/plugins" => {
                "plugins.view"
            }
            _ if admin_path_matches(path, "/admin/plugins/{plugin}") => "plugins.view",
            _ if admin_path_matches(path, "/admin/api/plugins/{plugin}/pages") => "plugins.view",
            "/admin/api/workspace/assets" => "plugins.view",
            "/admin/menus" | "/admin/partials/menus/table" | "/admin/api/menu" => "menus.view",
            "/admin/users" | "/admin/partials/users/table" => "users.view",
            "/admin/roles" | "/admin/partials/roles/table" => "roles.view",
            "/admin/permissions" | "/admin/partials/permissions/table" => "permissions.view",
            _ => "roles.manage",
        });
    }

    if write_map
        .iter()
        .any(|(m, p)| *m == method && admin_path_matches(path, p))
    {
        return Some(match path {
            "/admin/partials/users/create" => "users.manage",
            _ if path.starts_with("/admin/partials/users/") => "users.manage",
            "/admin/partials/roles/create" => "roles.manage",
            _ if path.starts_with("/admin/partials/roles/") => "roles.manage",
            "/admin/partials/permissions/create" => "permissions.manage",
            _ if path.starts_with("/admin/partials/permissions/") => "permissions.manage",
            "/admin/partials/menus/create" => "menus.manage",
            _ if path.starts_with("/admin/partials/menus/") => "menus.manage",
            "/admin/api/menu" => "menus.manage",
            _ if path.starts_with("/admin/api/menu/") => "menus.manage",
            _ => return None,
        });
    }

    None
}

fn admin_path_matches(path: &str, pattern: &str) -> bool {
    let path_parts: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let pattern_parts: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if path_parts.len() != pattern_parts.len() {
        return false;
    }

    path_parts
        .iter()
        .zip(pattern_parts.iter())
        .all(|(actual, expected)| {
            if expected.starts_with('{') && expected.ends_with('}') {
                return true;
            }
            actual == expected
        })
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
