use axum::{
    body::Body,
    extract::Path,
    extract::Request,
    extract::State,
    http::{header, StatusCode},
    middleware::Next,
    response::IntoResponse,
    routing::{any, get, get_service},
    Router,
};
use std::sync::Arc;
use sushi_core::auth::authorizer::Authorizer;
use sushi_core::auth::jwt::JwtService;
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::PageResolvedAssets;
use sushi_core::runtime::{HttpRequest, HttpSurface};
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

pub async fn build_static_router(ctx: &SushiContext) -> Router {
    let (static_dir, static_url_prefix) = {
        let cfg = ctx.config.get().await;
        (
            cfg.web.static_dir.clone(),
            cfg.web.static_url_prefix.clone(),
        )
    };
    let static_url_prefix = crate::render::normalize_static_url_prefix(&static_url_prefix);

    let static_router: Router<SushiContext> = Router::new()
        .route(
            &format!("{static_url_prefix}/plugins/{{*path}}"),
            get(plugin_static_asset),
        )
        .nest_service(&static_url_prefix, ServeDir::new(&static_dir))
        .with_state(ctx.clone());

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

    static_router.merge(favicon_router).with_state(ctx.clone())
}

pub async fn build_admin_router(ctx: &SushiContext) -> Router {
    let static_url_prefix = {
        let cfg = ctx.config.get().await;
        crate::render::normalize_static_url_prefix(&cfg.web.static_url_prefix)
    };

    let mut router: Router<SushiContext> = Router::new()
        .route("/", get(axum::response::Redirect::temporary("/admin/")))
        .route(
            "/index.html",
            get(axum::response::Redirect::temporary("/admin/")),
        )
        .route("/admin-login", any(admin_login_dispatch))
        .route(
            "/admin",
            get(axum::response::Redirect::temporary("/admin/")),
        );

    // Dynamic plugin pages use a stable catch-all beneath /admin. Static Host routes
    // remain more specific and keep precedence without requiring Router rebuilds.
    router = router.route("/admin/{*plugin_page}", any(plugin_admin_fallback));
    router = router.route("/admin/", any(plugin_admin_fallback));
    router = router.method_not_allowed_fallback(plugin_admin_method_fallback);

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

async fn admin_login_dispatch(
    State(ctx): State<SushiContext>,
    req: Request,
) -> axum::response::Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let dispatch_path = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| path.clone());
    let snapshot = ctx.plugins.capability_snapshot().await;
    let Some(registration) = snapshot.match_http_on(HttpSurface::Api, &method, &path) else {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    };
    let registration = registration.value.clone();
    if !registration.is_public {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let headers = req
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
        .collect();
    let body = if method == "GET" {
        None
    } else {
        let body_size_limit = {
            let config = ctx.config.get().await;
            config.server.body_size_limit
        };
        match axum::body::to_bytes(req.into_body(), body_size_limit).await {
            Ok(body) => Some(body.to_vec()),
            Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        }
    };
    let request = HttpRequest::new(&method, &path, &dispatch_path, body).with_headers(headers);
    match ctx
        .plugins
        .dispatch_http_request_registration(&registration, request)
        .await
    {
        Ok(response) => sushi_api::router::plugin_http_response(response),
        Err(error) => sushi_api::router::plugin_http_error_response(error),
    }
}

async fn plugin_admin_method_fallback(
    State(ctx): State<SushiContext>,
    req: Request,
) -> axum::response::Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let snapshot = ctx.plugins.capability_snapshot().await;
    if snapshot
        .match_http_on(HttpSurface::Admin, &method, &path)
        .is_none()
    {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    plugin_admin_fallback(State(ctx), req).await.into_response()
}

async fn plugin_admin_fallback(State(ctx): State<SushiContext>, req: Request) -> impl IntoResponse {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let dispatch_path = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| path.clone());
    let snapshot = ctx.plugins.capability_snapshot().await;
    if method == "GET" {
        if let Some(registration) = snapshot.admin_page(&path) {
            let registration = registration.value.clone();
            let assets = PageResolvedAssets {
                js: registration.js.clone(),
                css: registration.css.clone(),
            };
            let headers = req
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
                .collect();
            let request =
                HttpRequest::new(&method, &path, &dispatch_path, None).with_headers(headers);

            return match ctx
                .plugins
                .dispatch_admin_request_registration(&registration, request)
                .await
            {
                Ok(mut response) => {
                    if let Ok(html) = String::from_utf8(response.body.clone()) {
                        response.body = append_assets_to_html_response(&html, &assets).into_bytes();
                    }
                    sushi_api::router::plugin_http_response(response)
                }
                Err(error) if is_plugin_disabled_error(&error) => {
                    let message = plugin_disabled_message(&error);
                    let warn_message = format!("plugin disabled on admin page {path}: {message}");
                    tracing::warn!("{warn_message}");
                    ctx.logs.warn(&warn_message).await;
                    (StatusCode::FORBIDDEN, message).into_response()
                }
                Err(error) => {
                    let message = format!("plugin runtime error on admin page {path}: {error}");
                    tracing::error!("{message}");
                    ctx.logs.error(&message).await;
                    (StatusCode::INTERNAL_SERVER_ERROR, error).into_response()
                }
            };
        }
    }

    let Some(registration) = snapshot.match_http_on(HttpSurface::Admin, &method, &path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let registration = registration.value.clone();
    let headers = req
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
        .collect();
    let body = if method == "GET" {
        None
    } else {
        let body_size_limit = {
            let config = ctx.config.get().await;
            config.server.body_size_limit
        };
        match axum::body::to_bytes(req.into_body(), body_size_limit).await {
            Ok(body) => Some(body.to_vec()),
            Err(_) => {
                let limit_kb = body_size_limit / 1024;
                return (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    [(header::CONTENT_TYPE, "text/plain")],
                    format!("request body too large (limit: {limit_kb}KB)"),
                )
                    .into_response();
            }
        }
    };

    let request = HttpRequest::new(&method, &path, &dispatch_path, body).with_headers(headers);
    match ctx
        .plugins
        .dispatch_http_request_registration(&registration, request)
        .await
    {
        Ok(response) => sushi_api::router::plugin_http_response(response),
        Err(error) if is_plugin_disabled_error(&error) => {
            let message = plugin_disabled_message(&error);
            let warn_message =
                format!("plugin disabled on admin HTTP route {method} {path}: {message}");
            tracing::warn!("{warn_message}");
            ctx.logs.warn(&warn_message).await;
            sushi_api::router::plugin_http_error_response(error)
        }
        Err(error) => {
            let message = format!("plugin runtime error on {method} {path}: {error}");
            tracing::error!("{message}");
            ctx.logs.error(&message).await;
            sushi_api::router::plugin_http_error_response(error)
        }
    }
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

async fn plugin_static_asset(
    State(ctx): State<SushiContext>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let mut segments = path.splitn(3, '/');
    let tier = segments.next().unwrap_or_default();
    let plugin_name = segments.next().unwrap_or_default();
    let asset_path = segments.next().unwrap_or_default();
    let plugin_id = format!("{tier}/{plugin_name}");
    if !is_valid_plugin_mount_id(&plugin_id) || asset_path.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let snapshot = ctx.plugins.capability_snapshot().await;
    let root = match snapshot
        .static_roots()
        .iter()
        .find(|registration| registration.value.plugin_id.as_str() == plugin_id)
        .map(|registration| registration.value.root.clone())
    {
        Some(root) => root,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let mut file_path = root.clone();
    for component in std::path::Path::new(asset_path).components() {
        match component {
            std::path::Component::Normal(segment) => file_path.push(segment),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return StatusCode::NOT_FOUND.into_response(),
        }
    }

    let canonical_root = match tokio::fs::canonicalize(&root).await {
        Ok(root) => root,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response()
        }
        Err(error) => {
            tracing::warn!(path = %root.display(), error = %error, "failed to resolve plugin static root");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let canonical_file = match tokio::fs::canonicalize(&file_path).await {
        Ok(path) if path.starts_with(&canonical_root) => path,
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response()
        }
        Err(error) => {
            tracing::warn!(path = %file_path.display(), error = %error, "failed to resolve plugin static asset");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    match tokio::fs::metadata(&canonical_file).await {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response()
        }
        Err(error) => {
            tracing::warn!(path = %canonical_file.display(), error = %error, "failed to inspect plugin static asset");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    let body = match tokio::fs::read(&canonical_file).await {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response()
        }
        Err(error) => {
            tracing::warn!(path = %canonical_file.display(), error = %error, "failed to read plugin static asset");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let content_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();
    ([(header::CONTENT_TYPE, content_type)], Body::from(body)).into_response()
}

async fn admin_auth_middleware(
    axum::extract::State(state): axum::extract::State<AdminAuthState>,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();

    // Redirect top-level and /admin root paths to canonical admin entry.
    if path == "/" || path == "/index.html" || path == "/admin" {
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

    if path != "/admin" && !path.starts_with("/admin/") {
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
