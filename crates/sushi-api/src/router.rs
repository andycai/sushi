use axum::response::IntoResponse;
use axum::Router;
use std::sync::Arc;
use sushi_core::auth::middleware::AuthState;
use sushi_core::context::SushiContext;
use sushi_core::logs::LogService;
use sushi_core::plugin::manager::PluginManager;
use sushi_core::runtime::{HttpRequest, HttpResponse, HttpSurface};

#[derive(Clone)]
struct ApiRouterState {
    plugins: PluginManager,
    auth_state: AuthState,
    logs: Arc<LogService>,
    body_size_limit: usize,
}

/// Build the stable API router backed by the current capability snapshot.
pub async fn build_router(ctx: &SushiContext) -> Router {
    let body_size_limit = {
        let config = ctx.config.get().await;
        config.server.body_size_limit
    };

    api_router(ApiRouterState {
        plugins: ctx.plugins.clone(),
        auth_state: ctx.auth_state(),
        logs: Arc::clone(&ctx.logs),
        body_size_limit,
    })
}

fn api_router(state: ApiRouterState) -> Router {
    Router::new()
        .fallback(plugin_api_dispatch)
        .with_state(state)
}

/// Generic plugin API handler — reads method+path+body from the request
/// and dispatches to the appropriate Lua handler.
async fn plugin_api_dispatch(
    axum::extract::State(state): axum::extract::State<ApiRouterState>,
    req: axum::extract::Request,
) -> impl axum::response::IntoResponse {
    let method = req.method().to_string();
    let match_path = req.uri().path().to_string();
    let dispatch_path = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| match_path.clone());
    let snapshot = state.plugins.capability_snapshot().await;
    let Some(registration) = snapshot.match_http_on(HttpSurface::Api, &method, &match_path) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "not found".to_string(),
        )
            .into_response();
    };
    let registration = registration.value.clone();

    if !registration.is_public {
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
        if state
            .auth_state
            .authorizer
            .check_http(
                &role_slug,
                registration.surface.as_str(),
                &method,
                &match_path,
            )
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
    let headers = req
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
        .collect();
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

    let request =
        HttpRequest::new(&method, &match_path, &dispatch_path, body).with_headers(headers);
    match state
        .plugins
        .dispatch_http_request_registration(&registration, request)
        .await
    {
        Ok(response) => plugin_http_response(response),
        Err(e) => {
            if !is_plugin_disabled_error(&e) {
                let message = format!("plugin runtime error on {method} {match_path}: {e}");
                tracing::error!("{message}");
                state.logs.error(&message).await;
            }
            plugin_http_error_response(e)
        }
    }
}

/// Map the transport-neutral plugin response to Axum.
pub fn plugin_http_response(response: HttpResponse) -> axum::response::Response {
    let status = axum::http::StatusCode::from_u16(response.status)
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    let mut mapped = axum::response::Response::new(axum::body::Body::from(response.body));
    *mapped.status_mut() = status;
    for (name, value) in response.headers {
        let Ok(name) = axum::http::HeaderName::from_bytes(name.as_bytes()) else {
            tracing::warn!(header = %name, "ignored invalid plugin response header name");
            continue;
        };
        let Ok(value) = axum::http::HeaderValue::from_str(&value) else {
            tracing::warn!(header = %name, "ignored invalid plugin response header value");
            continue;
        };
        mapped.headers_mut().append(name, value);
    }
    mapped
}

/// Map a plugin dispatch error to the shared HTTP response contract.
pub fn plugin_http_error_response(error: String) -> axum::response::Response {
    if is_plugin_disabled_error(&error) {
        let body = serde_json::json!({
            "error": "plugin_disabled",
            "message": plugin_disabled_message(&error),
        })
        .to_string();
        (
            axum::http::StatusCode::FORBIDDEN,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response()
    } else {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            error,
        )
            .into_response()
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

fn extract_token_from_cookie(cookie_header: Option<&str>) -> Option<&str> {
    let cookie = cookie_header?;
    cookie
        .split(';')
        .find_map(|part| part.trim().strip_prefix("sushi_token="))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::auth;
    use axum::body::to_bytes;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{header, Request};
    use serde_json::Value;
    use sushi_core::auth::authorizer::{CompiledPolicySnapshot, HttpBinding};
    use sushi_core::auth::jwt::JwtService;
    use sushi_core::auth::middleware::{require_auth, AuthState};
    use sushi_core::config::{ConfigStore, SushiConfig};
    use sushi_core::context::SushiContext;
    use sushi_core::lua::vm::create_sandboxed_vm;
    use sushi_core::runtime::{
        HttpRouteSpec, PluginInstanceId, ResolvedRuntimeEntry, RuntimePluginSource,
    };
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

    async fn register_test_api_handler(
        manager: &PluginManager,
        method: &str,
        path: &str,
        plugin_name: &str,
        handler_key: &str,
        policy_key: Option<&str>,
        is_public: bool,
    ) {
        let mut staged = manager.stage_owner_activation(PluginInstanceId::legacy(plugin_name));
        staged.register_http(
            HttpRouteSpec::new(method, path, plugin_name, handler_key)
                .with_policy(policy_key.map(ToOwned::to_owned))
                .with_public(is_public),
        );
        manager
            .capability_registry()
            .commit(staged)
            .await
            .expect("test API registration should commit");
    }

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

    fn identity_runtime_entry() -> ResolvedRuntimeEntry {
        ResolvedRuntimeEntry {
            id: PluginInstanceId::new("identity.core").expect("identity entry ID is valid"),
            source: RuntimePluginSource::Builtin {
                key: "identity".to_string(),
                reference: "builtin:identity".to_string(),
            },
            enabled: true,
            required: true,
            config: serde_json::json!({}),
            grants: serde_json::json!({}),
            origin: "test".to_string(),
        }
    }

    fn api_core_runtime_entry() -> ResolvedRuntimeEntry {
        ResolvedRuntimeEntry {
            id: PluginInstanceId::new("api.core").expect("API core entry ID is valid"),
            source: RuntimePluginSource::Builtin {
                key: "api-core".to_string(),
                reference: "builtin:api-core".to_string(),
            },
            enabled: true,
            required: true,
            config: serde_json::json!({}),
            grants: serde_json::json!({}),
            origin: "test".to_string(),
        }
    }

    async fn activate_api_builtins(ctx: &SushiContext) {
        crate::builtin::activate_identity(ctx, &identity_runtime_entry())
            .await
            .expect("identity builtin activation succeeds");
        crate::builtin::activate_api_core(ctx, &api_core_runtime_entry())
            .await
            .expect("API core builtin activation succeeds");
    }

    async fn test_api_router(ctx: &SushiContext) -> Router {
        build_test_plugin_api_routes(ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::clone(&ctx.logs),
                body_size_limit: 1024,
            })
    }

    type PluginApiState = ApiRouterState;

    async fn build_test_plugin_api_routes(_ctx: &SushiContext) -> Router<ApiRouterState> {
        Router::new().fallback(plugin_api_dispatch)
    }

    fn static_users_router(ctx: &SushiContext) -> Router {
        let state = crate::routes::users::UsersRouteState {
            storage: ctx.db.clone() as Arc<dyn Storage>,
        };
        Router::new()
            .nest("/api/users", crate::routes::users::users_routes(state))
            .layer(axum::middleware::from_fn_with_state(
                ctx.auth_state(),
                require_auth,
            ))
    }

    fn static_auth_router(ctx: &SushiContext) -> Router {
        let state = auth::AuthRouteState {
            storage: ctx.db.clone() as Arc<dyn Storage>,
            jwt: Arc::clone(&ctx.jwt),
        };
        Router::new()
            .nest("/api/auth", auth::auth_routes(state))
            .layer(axum::middleware::from_fn_with_state(
                ctx.auth_state(),
                require_auth,
            ))
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
        register_test_api_handler(
            &manager,
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

    #[tokio::test]
    async fn plugin_api_dispatch_ignores_admin_surface_routes() {
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
        register_test_api_handler(
            &manager,
            "GET",
            "/admin/partials/kv/table",
            "plugin",
            handler_key,
            Some("admin.kv.manage"),
            false,
        )
        .await;

        let auth_state = auth_state_with_snapshot(vec![], vec![]);
        let token = auth_state
            .jwt_service
            .create_access_token(2, "editor_user", "editor")
            .expect("failed to create editor access token");

        let state = PluginApiState {
            plugins: manager,
            auth_state,
            logs: Arc::new(LogService::new()),
            body_size_limit: 1024,
        };

        let req = Request::builder()
            .method("GET")
            .uri("/admin/partials/kv/table")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "not found");
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
        register_test_api_handler(
            &manager,
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
        register_test_api_handler(
            &manager,
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
        register_test_api_handler(
            &manager,
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
        register_test_api_handler(
            &manager,
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
        register_test_api_handler(
            &manager,
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
    async fn test_plugin_api_dispatch_removes_route_when_plugin_disabled() {
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

        let ctx = test_context().await;
        ctx.plugins.register_vm("plugin", lua).await;
        register_test_api_handler(
            &ctx.plugins,
            "GET",
            "/api/test",
            "plugin",
            handler_key,
            None,
            true,
        )
        .await;
        ctx.set_plugin_enabled("plugin", false, Some("admin"), Some("test"))
            .await
            .unwrap();

        let state = PluginApiState {
            plugins: ctx.plugins.clone(),
            auth_state: ctx.auth_state(),
            logs: Arc::clone(&ctx.logs),
            body_size_limit: 1024,
        };

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let response = plugin_api_dispatch(State(state), req).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(bytes.as_ref(), b"not found");
    }

    #[tokio::test]
    async fn unified_router_dispatches_missing_auth_route_as_not_found() {
        let ctx = test_context().await;
        let app = build_router(&ctx).await;

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

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unified_router_dispatches_missing_users_route_as_not_found() {
        let ctx = test_context().await;
        let app = build_router(&ctx).await;
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/users")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn production_api_router_dispatches_users_through_builtin() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let app = build_router(&ctx).await;
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/users?limit=1")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).expect("invalid users payload");
        assert_eq!(payload.get("limit").and_then(Value::as_u64), Some(1));
    }

    #[tokio::test]
    async fn production_api_router_dispatches_auth_through_builtin() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let app = build_router(&ctx).await;

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

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).expect("invalid login payload");
        assert_eq!(
            payload.get("error").and_then(Value::as_str),
            Some("Invalid credentials")
        );
    }

    #[tokio::test]
    async fn users_builtin_accepts_cookie_token() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let app = test_api_router(&ctx).await;
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
    async fn users_builtin_matches_static_get_response() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");
        let request = || {
            Request::builder()
                .method("GET")
                .uri("/api/users?limit=17&offset=2")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap()
        };

        let static_response = static_users_router(&ctx).oneshot(request()).await.unwrap();
        let builtin_response = test_api_router(&ctx)
            .await
            .oneshot(request())
            .await
            .unwrap();

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(builtin_body, static_body);
    }

    #[tokio::test]
    async fn auth_builtin_login_is_public_and_matches_static_response() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"missing","password":"does-not-matter"}"#,
                ))
                .unwrap()
        };

        let static_response = static_auth_router(&ctx).oneshot(request()).await.unwrap();
        let builtin_response = test_api_router(&ctx)
            .await
            .oneshot(request())
            .await
            .unwrap();

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(builtin_body, static_body);
    }

    #[tokio::test]
    async fn auth_builtin_me_requires_access_token_and_returns_claims() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let app = test_api_router(&ctx).await;
        let missing_credentials = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            missing_credentials.status(),
            axum::http::StatusCode::UNAUTHORIZED
        );

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
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).expect("invalid me payload");
        assert_eq!(
            payload.get("username").and_then(Value::as_str),
            Some("viewer_user")
        );
        assert_eq!(payload.get("role").and_then(Value::as_str), Some("viewer"));
    }

    #[tokio::test]
    async fn auth_builtin_refresh_matches_static_error_response() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"refresh_token":"invalid.token.here"}"#))
                .unwrap()
        };

        let static_response = static_auth_router(&ctx).oneshot(request()).await.unwrap();
        let builtin_response = test_api_router(&ctx)
            .await
            .oneshot(request())
            .await
            .unwrap();

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(builtin_body, static_body);
    }

    #[tokio::test]
    async fn auth_builtin_maps_invalid_json_to_bad_request() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let app = test_api_router(&ctx).await;

        for path in ["/api/auth/login", "/api/auth/refresh"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from("{"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok()),
                Some("application/json")
            );
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let payload: Value = serde_json::from_slice(&body).expect("invalid error payload");
            assert!(payload.get("error").and_then(Value::as_str).is_some());
        }
    }

    #[tokio::test]
    async fn users_builtin_maps_invalid_inputs_to_bad_request() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;
        let token = ctx
            .jwt
            .create_access_token(1, "admin", "admin")
            .expect("failed to create test access token");
        let app = test_api_router(&ctx).await;
        let requests = [
            Request::builder()
                .method("GET")
                .uri("/api/users?limit=invalid")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{"))
                .unwrap(),
            Request::builder()
                .method("DELETE")
                .uri("/api/users/not-a-number")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        ];

        for request in requests {
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok()),
                Some("application/json")
            );
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let payload: Value = serde_json::from_slice(&body).expect("invalid error payload");
            assert!(payload.get("error").and_then(Value::as_str).is_some());
        }
    }

    #[tokio::test]
    async fn required_api_builtins_reject_runtime_toggle() {
        let ctx = test_context().await;
        activate_api_builtins(&ctx).await;

        let identity_error = ctx
            .set_plugin_enabled("identity", false, Some("test"), Some("required guard"))
            .await
            .expect_err("required builtin must reject ordinary runtime toggles");

        assert_eq!(
            identity_error,
            "required_plugin_toggle_forbidden: plugin 'identity' must be changed through profile and restart"
        );
        let api_core_error = ctx
            .set_plugin_enabled("api-core", false, Some("test"), Some("required guard"))
            .await
            .expect_err("required builtin must reject ordinary runtime toggles");
        assert_eq!(
            api_core_error,
            "required_plugin_toggle_forbidden: plugin 'api-core' must be changed through profile and restart"
        );
        let snapshot = ctx.plugins.capability_snapshot().await;
        let auth = snapshot
            .match_http_on(HttpSurface::Api, "POST", "/api/auth/login")
            .expect("rejected toggle must preserve identity capabilities");
        assert_eq!(auth.owner.as_str(), "identity.core");
        let users = snapshot
            .match_http_on(HttpSurface::Api, "GET", "/api/users")
            .expect("rejected toggle must preserve API core capabilities");
        assert_eq!(users.owner.as_str(), "api.core");
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
        register_test_api_handler(
            &ctx.plugins,
            "GET",
            "/api/plugin/public",
            "plugin",
            handler_key,
            None,
            true,
        )
        .await;

        let app = build_test_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
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
    async fn plugin_router_discovers_routes_registered_after_router_build() {
        let ctx = test_context().await;
        let app = build_test_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
            });

        let lua = create_sandboxed_vm().unwrap();
        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();
        let handler = lua
            .create_async_function(|_, ()| async { Ok("dynamic".to_string()) })
            .unwrap();
        handlers.set("h_dynamic", handler).unwrap();
        ctx.plugins.register_vm("plugin", lua).await;
        register_test_api_handler(
            &ctx.plugins,
            "GET",
            "/api/plugin/dynamic",
            "plugin",
            "h_dynamic",
            None,
            true,
        )
        .await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/plugin/dynamic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), b"dynamic");
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
        register_test_api_handler(
            &ctx.plugins,
            "GET",
            "/api/plugin/private",
            "plugin",
            handler_key,
            None,
            false,
        )
        .await;

        let app = build_test_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
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
        register_test_api_handler(
            &ctx.plugins,
            "GET",
            "/app/files/download/docs",
            "plugin",
            handler_key,
            None,
            true,
        )
        .await;

        let app = build_test_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
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
        register_test_api_handler(
            &ctx.plugins,
            "GET",
            "/app/files",
            "plugin",
            handler_key,
            None,
            true,
        )
        .await;

        let app = build_test_plugin_api_routes(&ctx)
            .await
            .with_state(PluginApiState {
                plugins: ctx.plugins.clone(),
                auth_state: ctx.auth_state(),
                logs: Arc::new(LogService::new()),
                body_size_limit: 1024,
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

        activate_api_builtins(&ctx).await;
        let app = test_api_router(&ctx).await;
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
        activate_api_builtins(&ctx).await;
        let app = test_api_router(&ctx).await;
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
