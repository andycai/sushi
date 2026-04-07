use crate::routes::auth;
use crate::routes::users;
use axum::response::IntoResponse;
use axum::Router;
use std::sync::Arc;
use sushi_core::auth::middleware::require_auth;
use sushi_core::context::SushiContext;
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
        .layer(axum::middleware::from_fn_with_state(auth_state, require_auth))
}

/// Plugin API route handler state.
#[derive(Clone)]
pub struct PluginApiState {
    pub plugins: PluginManager,
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
        Some(Ok(response_body)) => (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            response_body,
        )
            .into_response(),
        Some(Err(e)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            e,
        )
            .into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "not found".to_string(),
        )
            .into_response(),
    }
}
