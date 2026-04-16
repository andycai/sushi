use crate::routes::auth;
use crate::routes::users;
use axum::response::IntoResponse;
use axum::Router;
use serde_json::Value;
use std::sync::Arc;
use sushi_core::auth::middleware::require_auth;
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
    let path = req.uri().path().to_string();

    // Extract body for non-GET requests
    let body = if method == "GET" {
        None
    } else {
        match axum::body::to_bytes(req.into_body(), state.body_size_limit).await {
            Ok(b) => match String::from_utf8(b.to_vec()) {
                Ok(s) => Some(s),
                Err(_) => {
                    // TODO: add mechanism to handle binary file streams for Lua plugins
                    return (
                        axum::http::StatusCode::BAD_REQUEST,
                        [(axum::http::header::CONTENT_TYPE, "text/plain")],
                        "bad request: binary or non-utf8 bodies are not supported yet",
                    )
                        .into_response();
                }
            },
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

    match state.plugins.call_api_handler(&method, &path, body).await {
        Some(Ok(response_body)) => {
            if let Some((status, body)) = parse_status_envelope(&response_body) {
                (
                    status,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    body,
                )
                    .into_response()
            } else {
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    response_body,
                )
                    .into_response()
            }
        }
        Some(Err(e)) => {
            let message = format!("plugin runtime error on {method} {path}: {e}");
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
        ]
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

    async fn test_context() -> SushiContext {
        let config = ConfigStore::new(SushiConfig::default());
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage.run_migrations(MIGRATION_SQL).await.unwrap();
        storage.run_migrations(RBAC_MIGRATION_SQL).await.unwrap();
        storage
            .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
            .await
            .unwrap();
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
            .register_api_handler("GET", "/api/test", "plugin", handler_key)
            .await;

        let state = PluginApiState {
            plugins: manager,
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
            .register_api_handler("GET", "/api/test", "plugin", handler_key)
            .await;

        let state = PluginApiState {
            plugins: manager,
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
            .register_api_handler("GET", "/api/test", "plugin", handler_key)
            .await;

        let logs = Arc::new(LogService::new());
        let state = PluginApiState {
            plugins: manager,
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
