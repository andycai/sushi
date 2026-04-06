use crate::routes::auth;
use crate::routes::kv;
use crate::routes::users;
use axum::Router;
use sushi_core::auth::middleware::require_auth;
use sushi_core::context::SushiContext;

pub fn build_api_router(ctx: &SushiContext) -> Router {
    let auth_route_state = auth::AuthRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
        jwt: std::sync::Arc::clone(&ctx.jwt),
    };
    let users_route_state = users::UsersRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
    };
    let kv_route_state = kv::KvRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
    };

    Router::new()
        .nest("/api/auth", auth::auth_routes(auth_route_state))
        .nest("/api/users", users::users_routes(users_route_state))
        .nest("/api/kv", kv::kv_routes(kv_route_state))
}

pub fn build_app(ctx: &SushiContext) -> Router {
    let auth_state = ctx.auth_state();

    let auth_route_state = auth::AuthRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
        jwt: std::sync::Arc::clone(&ctx.jwt),
    };
    let users_route_state = users::UsersRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
    };
    let kv_route_state = kv::KvRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
    };

    Router::new()
        .nest("/api/auth", auth::auth_routes(auth_route_state))
        .nest("/api/users", users::users_routes(users_route_state))
        .nest("/api/kv", kv::kv_routes(kv_route_state))
        .layer(axum::middleware::from_fn_with_state(auth_state, require_auth))
}
