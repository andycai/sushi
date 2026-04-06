use crate::routes::auth;
use axum::Router;
use sushi_core::auth::middleware::require_auth;
use sushi_core::context::SushiContext;

pub fn build_api_router(ctx: &SushiContext) -> Router {
    let auth_route_state = auth::AuthRouteState {
        storage: std::sync::Arc::clone(&ctx.db),
        jwt: std::sync::Arc::clone(&ctx.jwt),
    };

    Router::new().nest("/auth", auth::auth_routes(auth_route_state))
}

pub fn build_app(ctx: &SushiContext) -> Router {
    let auth_state = ctx.auth_state();

    Router::new()
        .nest("/api", build_api_router(ctx))
        .layer(axum::middleware::from_fn_with_state(auth_state, require_auth))
}
