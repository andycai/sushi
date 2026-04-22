use crate::routes::auth;
use crate::routes::users;
use axum::response::IntoResponse;
use axum::Router;
use serde_json::Value;
use std::sync::Arc;
use sushi_core::auth::middleware::{require_auth, AuthState};
use sushi_core::context::SushiContext;
use sushi_core::logs::LogService;
use sushi_core::plugin::manager::PluginManager;

pub fn build_api_router(ctx: &SushiContext) -> Router {
    let auth_route_state = auth::AuthRouteState {
        storage: ctx.db.clone() as Arc<dyn sushi_core::storage::Storage>,
        jwt: std::sync::Arc::clone(&ctx.jwt),
    };
    let users_route_state = users::UsersRouteState {
        storage: ctx.db.clone() as Arc<dyn sushi_core::storage::Storage>,
    };

    Router::new()
        .nest("/api/auth", auth::auth_routes(auth_route_state))
        .nest("/api/users", users::users_routes(users_route_state))
}

pub fn build_app(ctx: &SushiContext) -> Router {
    let auth_state = ctx.auth_state();

    let auth_route_state = auth::AuthRouteState {
        storage: ctx.db.clone() as Arc<dyn sushi_core::storage::Storage>,
        jwt: std::sync::Arc::clone(&ctx.jwt),
    };
    let users_route_state = users::UsersRouteState {
        storage: ctx.db.clone() as Arc<dyn sushi_core::storage::Storage>,
    };

    Router::new()
        .nest("/api/auth", auth::auth_routes(auth_route_state))
        .nest("/api/users", users::users_routes(users_route_state))
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            require_auth,
        ))
}

/// Plugin API route handler state.
#[derive(Clone)]
pub struct PluginApiState {
    pub plugins: PluginManager,
    pub auth_state: AuthState,
    pub logs: Arc<LogService>,
    pub body_size_limit: usize,
    pub route_map: Vec<(String, String)>, // (method, path) pairs
}

/// Build a router that dispatches plugin API routes.
/// Each plugin route gets its own Axum route entry.
/// Lua wildcard paths ending with `/*` are converted to Axum `{*path}` catch-all.
pub async fn build_plugin_api_routes(ctx: &SushiContext) -> Router<PluginApiState> {
    let routes = ctx.plugins.list_api_routes().await;
    let mut router = Router::new();

    for (method, path) in routes {
        // Convert Lua wildcard /* to Axum catch-all /{*wild}
        let axum_path = if path.ends_with("/*") {
            format!("{}/{{*wild}}", &path[..path.len() - 2])
        } else if path.ends_with("*") {
            format!("{}/{{*wild}}", &path[..path.len() - 1])
        } else {
            path.clone()
        };

        router = match method.as_str() {
            "GET" => router.route(&axum_path, axum::routing::get(plugin_api_dispatch)),
            "POST" => router.route(&axum_path, axum::routing::post(plugin_api_dispatch)),
            "PUT" => router.route(&axum_path, axum::routing::put(plugin_api_dispatch)),
            "DELETE" => router.route(&axum_path, axum::routing::delete(plugin_api_dispatch)),
            "PATCH" => router.route(&axum_path, axum::routing::patch(plugin_api_dispatch)),
            _ => continue,
        };
    }

    router
}

/// Generic plugin API handler — reads method+path+body from the request
/// and dispatches to the appropriate Lua handler.
async fn plugin_api_dispatch(
    axum::extract::State(state): axum::extract::State<PluginApiState>,
    req: axum::extract::Request,
) -> impl axum::response::IntoResponse {
    let method = req.method().to_string();
    let match_path = req.uri().path().to_string();
    let dispatch_path = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| match_path.clone());

    if !state
        .plugins
        .is_api_route_public(&method, &match_path)
        .await
    {
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
                        axum::http::StatusCode::UNAUTHORIZED,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        "{\"error\":\"Missing authorization credentials\"}".to_string(),
                    )
                        .into_response();
                }
            },
        };

        let claims = match state.auth_state.jwt_service.verify_token(token) {
            Ok(claims) => claims,
            Err(_) => {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    "{\"error\":\"Invalid token\"}".to_string(),
                )
                    .into_response();
            }
        };

        if claims.token_type != "access" {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                "{\"error\":\"Invalid token type. Use access token for API access.\"}".to_string(),
            )
                .into_response();
        }

        let role_slug = claims.role.clone();
        let policy_surface = policy_surface_for_match_path(&match_path);

        if state
            .auth_state
            .authorizer
            .check_http(&role_slug, policy_surface, &method, &match_path)
            .await
            .is_err()
        {
            return (
                axum::http::StatusCode::FORBIDDEN,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                "{\"error\":\"Insufficient permissions for this API route\"}".to_string(),
            )
                .into_response();
        }
    }

    // Extract body for non-GET requests
    let body = if method == "GET" {
        None
    } else {
        match axum::body::to_bytes(req.into_body(), state.body_size_limit).await {
            Ok(b) => Some(b.to_vec()),
            Err(_) => {
                let limit_kb = state.body_size_limit / 1024;
                return (
                    axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                    [(axum::http::header::CONTENT_TYPE, "text/plain")],
                    format!("request body too large (limit: {}KB)", limit_kb),
                )
                    .into_response();
            }
        }
    };

    match state
        .plugins
        .dispatch_api_handler(&method, &match_path, &dispatch_path, body)
        .await
    {
        Some(Ok(response_body)) => {
            if let Some((file_name, mime, body)) = parse_download_envelope(&response_body) {
                let mut response = axum::response::Response::new(axum::body::Body::from(body));
                *response.status_mut() = axum::http::StatusCode::OK;

                let mime_header = axum::http::HeaderValue::from_str(&mime).unwrap_or_else(|_| {
                    axum::http::HeaderValue::from_static("application/octet-stream")
                });
                response
                    .headers_mut()
                    .insert(axum::http::header::CONTENT_TYPE, mime_header);

                let safe_name = sanitize_content_disposition_name(&file_name);
                let disposition = format!("attachment; filename=\"{safe_name}\"");
                let disposition_header = axum::http::HeaderValue::from_str(&disposition)
                    .unwrap_or_else(|_| axum::http::HeaderValue::from_static("attachment"));
                response
                    .headers_mut()
                    .insert(axum::http::header::CONTENT_DISPOSITION, disposition_header);
                response
            } else if let Some((status, body)) = parse_status_envelope(&response_body) {
                (
                    status,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    body,
                )
                    .into_response()
            } else {
                let content_type = infer_response_content_type(&response_body);
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, content_type)],
                    response_body,
                )
                    .into_response()
            }
        }
        Some(Err(e)) => {
            if is_plugin_disabled_error(&e) {
                let body = serde_json::json!({
                    "error": "plugin_disabled",
                    "message": plugin_disabled_message(&e),
                })
                .to_string();
                return (
                    axum::http::StatusCode::FORBIDDEN,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    body,
                )
                    .into_response();
            }
            let message = format!("plugin runtime error on {method} {match_path}: {e}");
            tracing::error!("{message}");
            state.logs.error(&message).await;
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                e,
            )
                .into_response()
        }
        None => (
            axum::http::StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "not found".to_string(),
        )
            .into_response(),
    }
}

fn policy_surface_for_match_path(path: &str) -> &'static str {
    if path == "/admin" || path.starts_with("/admin/") {
        "admin"
    } else {
        "api"
    }
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

#[derive(serde::Deserialize)]
struct DownloadEnvelope {
    #[serde(default)]
    __sushi_file_download: bool,
    file_name: String,
    mime: String,
    body_hex: String,
}

fn parse_download_envelope(body: &str) -> Option<(String, String, Vec<u8>)> {
    let parsed: DownloadEnvelope = serde_json::from_str(body).ok()?;
    if !parsed.__sushi_file_download {
        return None;
    }
    let decoded = decode_hex_bytes(&parsed.body_hex)?;
    Some((parsed.file_name, parsed.mime, decoded))
}

fn extract_token_from_cookie(cookie_header: Option<&str>) -> Option<&str> {
    let cookie = cookie_header?;
    cookie
        .split(';')
        .find_map(|part| part.trim().strip_prefix("sushi_token="))
}

fn decode_hex_bytes(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let chunk = std::str::from_utf8(&bytes[index..index + 2]).ok()?;
        let value = u8::from_str_radix(chunk, 16).ok()?;
        out.push(value);
        index += 2;
    }
    Some(out)
}

fn sanitize_content_disposition_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            '"' | '\\' | '\r' | '\n' => '_',
            _ => ch,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "download.bin".to_string()
    } else {
        sanitized
    }
}

fn infer_response_content_type(body: &str) -> &'static str {
    let trimmed = body.trim_start();
    if trimmed.starts_with('<') {
        "text/html; charset=utf-8"
    } else if serde_json::from_str::<Value>(body).is_ok() {
        "application/json"
    } else {
        "text/plain; charset=utf-8"
    }
}

fn parse_status_envelope(body: &str) -> Option<(axum::http::StatusCode, String)> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let obj = parsed.as_object()?;
    let sentinel = obj.get("__sushi_web_json")?.as_bool()?;
    if !sentinel {
        return None;
    }
    let status = obj.get("status")?.as_u64()?;
    let status_u16 = u16::try_from(status).ok()?;
    let status_code = axum::http::StatusCode::from_u16(status_u16).ok()?;
    let payload = obj.get("body")?;
    let encoded = serde_json::to_string(payload).ok()?;
    Some((status_code, encoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{header, Request};
    use serde_json::Value;
    use sushi_core::auth::authorizer::{CompiledPolicySnapshot, HttpBinding};
    use sushi_core::auth::jwt::JwtService;
    use sushi_core::auth::middleware::AuthState;
    use sushi_core::config::{ConfigStore, SushiConfig};
    use sushi_core::context::SushiContext;
    use sushi_core::lua::vm::create_sandboxed_vm;
    use sushi_core::storage::sqlite::SqliteStorage;
    use sushi_core::storage::Storage;
    use sushi_core::web::template_service::TemplateService;
    use tower::ServiceExt;

    const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
    const RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/003_rbac.sql");
    const UNIFIED_POLICY_V2_MIGRATION_SQL: &str =
        include_str!("../../../migrations/006_unified_policy_v2.sql");
    const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
    const PLUGIN_GOVERNANCE_MIGRATION_SQL: &str =
        include_str!("../../../migrations/008_plugin_governance_v1.sql");
    const PLUGIN_GOVERNANCE_MIGRATION_NAME: &str = "008_plugin_governance_v1";

    fn api_http_bindings() -> Vec<HttpBinding> {
        vec![
            HttpBinding {
                surface: "api".to_string(),
                method: "GET".to_string(),
                path_pattern: "/api/users".to_string(),
                policy_key: "api.users.read".to_string(),
            },
            HttpBinding {
                surface: "api".to_string(),
                method: "POST".to_string(),
                path_pattern: "/api/users".to_string(),
                policy_key: "api.users.manage".to_string(),
            },
            HttpBinding {
                surface: "api".to_string(),
                method: "DELETE".to_string(),
                path_pattern: "/api/users/{id}".to_string(),
                policy_key: "api.users.manage".to_string(),
            },
            HttpBinding {
                surface: "api".to_string(),
                method: "GET".to_string(),
                path_pattern: "/api/auth/me".to_string(),
                policy_key: "api.auth.me".to_string(),
            },
        ]
    }

    fn test_auth_state() -> AuthState {
        AuthState {
            jwt_service: Arc::new(JwtService::new(
                "test-secret-key-at-least-32-chars-long!",
                3600,
                604800,
            )),
            authorizer: Arc::new(sushi_core::auth::authorizer::Authorizer::new(
                CompiledPolicySnapshot::default(),
            )),
        }
    }

    fn auth_state_with_snapshot(
        http_bindings: Vec<HttpBinding>,
        role_grants: Vec<(&str, &str)>,
    ) -> AuthState {
        AuthState {
            jwt_service: Arc::new(JwtService::new(
                "test-secret-key-at-least-32-chars-long!",
                3600,
                604800,
            )),
            authorizer: Arc::new(sushi_core::auth::authorizer::Authorizer::new(
                CompiledPolicySnapshot::new(
                    http_bindings,
                    vec![],
                    role_grants
                        .into_iter()
                        .map(|(role, key)| (role.to_string(), key.to_string()))
                        .collect(),
                ),
            )),
        }
    }

    async fn refresh_api_authorizer(ctx: &SushiContext) {
        let grants_rows = ctx
            .db
            .query(
                r#"
                SELECT r.slug AS role_slug, pk.key AS policy_key
                FROM roles r
                JOIN role_policy_keys rpk ON rpk.role_id = r.id
                JOIN policy_keys pk ON pk.id = rpk.policy_key_id
                "#,
                vec![],
            )
            .await
            .expect("failed to load policy grants");

        let role_grants: Vec<(String, String)> = grants_rows
            .into_iter()
            .filter_map(|row| {
                let role = row.get("role_slug").and_then(Value::as_str)?;
                let policy_key = row.get("policy_key").and_then(Value::as_str)?;
                Some((role.to_string(), policy_key.to_string()))
            })
            .collect();

        let snapshot = CompiledPolicySnapshot::new(api_http_bindings(), vec![], role_grants);
        ctx.authorizer.replace_snapshot(snapshot).await;
    }

    async fn run_plugin_governance_migration_if_needed(storage: &SqliteStorage) {
        let rows = storage
            .query(
                "SELECT 1 AS found FROM _sushi_migrations WHERE name = ?1 LIMIT 1",
                vec![Value::String(PLUGIN_GOVERNANCE_MIGRATION_NAME.to_string())],
            )
            .await
            .expect("failed to query migration 008_plugin_governance_v1 state");
        if rows.is_empty() {
            storage
                .run_migrations(PLUGIN_GOVERNANCE_MIGRATION_SQL)
                .await
                .expect("failed to run migration 008_plugin_governance_v1");
        }
    }

    async fn test_context() -> SushiContext {
        let config = ConfigStore::new(SushiConfig::default());
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage.run_migrations(MIGRATION_SQL).await.unwrap();
        storage.run_migrations(RBAC_MIGRATION_SQL).await.unwrap();
        storage
            .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
            .await
            .unwrap();
        storage
            .run_migrations(CMS_MIGRATION_SQL)
            .await
            .expect("failed to run migration 007_cms");
        run_plugin_governance_migration_if_needed(&storage).await;
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);

        let templates_root =
            std::env::temp_dir().join(format!("sushi-api-router-test-{}", std::process::id()));
        std::fs::create_dir_all(&templates_root).unwrap();
        let templates = TemplateService::new(&templates_root).unwrap();

        let ctx = SushiContext::new(config, storage, jwt, templates);
        refresh_api_authorizer(&ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_applies_status_envelope() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_test";
        let handler = lua
            .create_async_function(|_, ()| async {
                Ok(r#"{"__sushi_web_json":true,"status":201,"body":{"ok":true}}"#.to_string())
            })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/test",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);

        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, r#"{"ok":true}"#);
    }

    #[test]
    fn plugin_policy_surface_uses_admin_for_admin_paths() {
        assert_eq!(
            policy_surface_for_match_path("/admin/partials/cms/overview"),
            "admin"
        );
        assert_eq!(policy_surface_for_match_path("/admin/cms"), "admin");
        assert_eq!(policy_surface_for_match_path("/api/cms/pages"), "api");
    }

    #[tokio::test]
    async fn plugin_api_dispatch_allows_editor_for_admin_partial_when_policy_granted() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_admin_partial";
        let handler = lua
            .create_async_function(|_, ()| async { Ok("<div>ok</div>".to_string()) })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/admin/partials/kv/table",
                "plugin",
                handler_key,
                Some("admin.kv.manage"),
                false,
            )
            .await;

        let auth_state = auth_state_with_snapshot(
            vec![HttpBinding {
                surface: "admin".to_string(),
                method: "GET".to_string(),
                path_pattern: "/admin/partials/kv/table".to_string(),
                policy_key: "admin.kv.manage".to_string(),
            }],
            vec![("editor", "admin.kv.manage")],
        );
        let token = auth_state
            .jwt_service
            .create_access_token(2, "editor_user", "editor")
            .expect("failed to create editor access token");

        let state = PluginApiState {
            plugins: manager,
            auth_state,
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = Request::builder()
            .method("GET")
            .uri("/admin/partials/kv/table")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "<div>ok</div>");
    }

    #[tokio::test]
    async fn plugin_api_dispatch_denies_admin_partial_without_admin_surface_grant() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_admin_partial_denied";
        let handler = lua
            .create_async_function(|_, ()| async { Ok("<div>denied</div>".to_string()) })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/admin/partials/kv/table",
                "plugin",
                handler_key,
                Some("admin.kv.manage"),
                false,
            )
            .await;

        let auth_state = auth_state_with_snapshot(
            vec![HttpBinding {
                surface: "admin".to_string(),
                method: "GET".to_string(),
                path_pattern: "/admin/partials/kv/table".to_string(),
                policy_key: "admin.kv.manage".to_string(),
            }],
            vec![],
        );
        let token = auth_state
            .jwt_service
            .create_access_token(3, "viewer_user", "viewer")
            .expect("failed to create viewer access token");

        let state = PluginApiState {
            plugins: manager,
            auth_state,
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = Request::builder()
            .method("GET")
            .uri("/admin/partials/kv/table")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("Insufficient permissions"), "body: {body}");
    }

    #[tokio::test]
    async fn plugin_governance_migration_helper_skips_when_already_applied() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage.run_migrations(MIGRATION_SQL).await.unwrap();
        storage
            .execute(
                "INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (8, '008_plugin_governance_v1')",
                vec![],
            )
            .await
            .unwrap();

        run_plugin_governance_migration_if_needed(&storage).await;

        let columns = storage
            .query("PRAGMA table_info(plugin_state)", vec![])
            .await
            .unwrap();
        let has_plugin_id = columns
            .iter()
            .any(|column| column.get("name").and_then(Value::as_str) == Some("plugin_id"));

        assert!(
            !has_plugin_id,
            "helper should skip applying migration SQL when marker is already present"
        );
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_forwards_path_query_to_lua_handler() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_query";
        lua.load(
            r#"
            sushi.__handlers["h_query"] = function(args)
                return args.dispatch_path or ""
            end
            "#,
        )
        .exec()
        .unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/test",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/test?foo=bar&baz=qux")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "/api/test?foo=bar&baz=qux");
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_keeps_non_envelope_body() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_test";
        let handler = lua
            .create_async_function(|_, ()| async {
                Ok(r#"{"status":201,"body":{"ok":true}}"#.to_string())
            })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/test",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, r#"{"status":201,"body":{"ok":true}}"#);
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_forwards_query_string_to_handler() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_query";
        lua.load(&format!(
            r#"
sushi.__handlers["{handler_key}"] = function(args)
    return args[1] .. "|" .. (args.dispatch_path or "")
end
"#
        ))
        .exec()
        .unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/app/files/list/docs",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = Request::builder()
            .method("GET")
            .uri("/app/files/list/docs?path=%2F")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "/app/files/list/docs|/app/files/list/docs?path=%2F");
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_accepts_binary_request_body() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_binary";
        lua.load(&format!(
            r#"
sushi.__handlers["{handler_key}"] = function(args)
    local body = args[2] or ""
    return tostring(string.len(body))
end
"#
        ))
        .exec()
        .unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "POST",
                "/api/upload",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .body(Body::from(vec![0xff, 0x00, b'a']))
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "3");
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_records_runtime_errors_in_log_service() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_test";
        let handler = lua
            .create_async_function(|lua, ()| async move {
                lua.load("error('boom from lua')").eval::<String>()
            })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/test",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let logs = Arc::new(LogService::new());
        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: logs.clone(),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let entries = logs.list(20).await;
        assert!(
            entries.iter().any(|entry| {
                entry.level == "ERROR"
                    && entry.message.contains("plugin runtime error")
                    && entry.message.contains("GET /api/test")
                    && entry.message.contains("boom from lua")
            }),
            "expected runtime error to be recorded in log service"
        );
    }

    #[tokio::test]
    async fn test_plugin_api_dispatch_returns_forbidden_when_plugin_disabled() {
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_test";
        let handler = lua
            .create_async_function(|_, ()| async { Ok(r#"{"ok":true}"#.to_string()) })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        let manager = PluginManager::new();
        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/test",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;
        manager
            .set_plugin_enabled("plugin", false, Some("admin"), Some("test"))
            .await
            .unwrap();

        let state = PluginApiState {
            plugins: manager,
            auth_state: test_auth_state(),
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
            route_map: Vec::new(),
        };

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&axum::http::HeaderValue::from_static("application/json"))
        );
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "error": "plugin_disabled",
                "message": "plugin 'plugin' is disabled",
            })
        );
    }

    #[tokio::test]
    async fn test_build_app_allows_login_without_token() {
        let ctx = test_context().await;
        let app = build_app(&ctx);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"missing","password":"does-not-matter"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Request reaches login handler; it should not be blocked by middleware.
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("Invalid credentials"), "{body}");
    }

    #[tokio::test]
    async fn test_build_app_allows_me_with_viewer_token() {
        let ctx = test_context().await;
        let app = build_app(&ctx);
        let token = ctx
            .jwt
            .create_access_token(2, "viewer_user", "viewer")
            .expect("failed to create test access token");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).expect("invalid me payload");
        assert_eq!(
            payload.get("username").and_then(Value::as_str),
            Some("viewer_user")
        );
        assert_eq!(payload.get("role").and_then(Value::as_str), Some("viewer"));
    }

    #[tokio::test]
    async fn test_build_app_requires_auth_for_users_route() {
        let ctx = test_context().await;
        let app = build_app(&ctx);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("Missing authorization credentials"), "{body}");
    }

    #[tokio::test]
    async fn test_build_app_accepts_cookie_token_for_users_route() {
        let ctx = test_context().await;
        let app = build_app(&ctx);
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/users")
                    .header(header::COOKIE, format!("sushi_token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_public_plugin_route_is_accessible_without_token() {
        let ctx = test_context().await;
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_public";
        let handler = lua
            .create_async_function(|_, ()| async { Ok(r#"{"ok":true}"#.to_string()) })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        ctx.plugins.register_vm("plugin", lua).await;
        ctx.plugins
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/plugin/public",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let app = build_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
                route_map: Vec::new(),
            });

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/plugin/public")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_non_public_plugin_route_requires_auth_without_token() {
        let ctx = test_context().await;
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_private";
        let handler = lua
            .create_async_function(|_, ()| async { Ok(r#"{"ok":true}"#.to_string()) })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        ctx.plugins.register_vm("plugin", lua).await;
        ctx.plugins
            .register_api_handler("GET", "/api/plugin/private", "plugin", handler_key)
            .await;

        let app = build_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
                route_map: Vec::new(),
            });

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/plugin/private")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn file_browser_download_returns_attachment_headers() {
        let ctx = test_context().await;
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_download";
        let handler = lua
            .create_async_function(|_, ()| async {
                Ok(
                    r#"{"__sushi_file_download":true,"file_name":"report.bin","mime":"application/octet-stream","body_hex":"000102ff"}"#
                        .to_string(),
                )
            })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        ctx.plugins.register_vm("plugin", lua).await;
        ctx.plugins
            .register_api_handler_with_policy_and_public(
                "GET",
                "/app/files/download/docs",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let app = build_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
                route_map: Vec::new(),
            });

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/app/files/download/docs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/octet-stream")
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok()),
            Some("attachment; filename=\"report.bin\"")
        );

        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), &[0x00, 0x01, 0x02, 0xff]);
    }

    #[tokio::test]
    async fn html_plugin_route_returns_text_html_content_type() {
        let ctx = test_context().await;
        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler_key = "h_html";
        let handler = lua
            .create_async_function(|_, ()| async {
                Ok("<!doctype html><html><body>ok</body></html>".to_string())
            })
            .unwrap();
        handlers.set(handler_key, handler).unwrap();

        ctx.plugins.register_vm("plugin", lua).await;
        ctx.plugins
            .register_api_handler_with_policy_and_public(
                "GET",
                "/app/files",
                "plugin",
                handler_key,
                None,
                true,
            )
            .await;

        let app = build_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
                route_map: Vec::new(),
            });

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/app/files")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
    }

    #[tokio::test]
    async fn test_create_user_accepts_custom_role_when_role_exists() {
        let ctx = test_context().await;
        ctx.db
            .execute(
                "INSERT INTO roles (slug, name, description) VALUES (?1, ?2, ?3)",
                vec![
                    serde_json::json!("auditor"),
                    serde_json::json!("Auditor"),
                    serde_json::json!("Read-only audit role"),
                ],
            )
            .await
            .expect("failed to insert custom role");

        let app = build_app(&ctx);
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/users")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"auditor_user","email":"auditor@example.com","password":"password123","role":"auditor"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::CREATED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).expect("invalid create-user payload");
        assert_eq!(
            payload.get("role").and_then(Value::as_str),
            Some("auditor"),
            "payload: {payload}"
        );
    }

    #[tokio::test]
    async fn test_create_user_rejects_unknown_role() {
        let ctx = test_context().await;
        let app = build_app(&ctx);
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/users")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"unknown_role_user","email":"unknown@example.com","password":"password123","role":"does_not_exist"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).expect("invalid error payload");
        assert_eq!(
            payload.get("error").and_then(Value::as_str),
            Some("Selected role does not exist"),
            "payload: {payload}"
        );
    }
}
