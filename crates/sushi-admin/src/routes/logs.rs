use axum::extract::State;
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpResponse, HttpRouteSpec, MenuContributionSpec, StagedRegistrar,
};

pub async fn logs_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(logs_page_response(&ctx).await)
}

pub async fn logs_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(logs_api_response(&ctx).await)
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("host-admin.logs", "Logs", 70)
            .with_icon(Some("file-text".to_string()))
            .with_parent(Some("host-admin.system".to_string()))
            .with_route(Some("/admin/logs".to_string()))
            .with_policy(Some("admin.logs.view".to_string())),
    );
    let page_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new("/admin/logs", "Logs", "host-admin", "rust::logs-page")
            .with_policy(Some("admin.logs.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = page_ctx.clone();
                async move { Ok(logs_page_response(&ctx).await) }
            })),
    );

    staged.register_http(
        HttpRouteSpec::new("GET", "/admin/api/logs", "host-admin", "rust::logs-api")
            .with_policy(Some("admin.logs.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = ctx.clone();
                async move { Ok(logs_api_response(&ctx).await) }
            })),
    );
}

async fn logs_page_response(ctx: &SushiContext) -> HttpResponse {
    crate::render::render_template_http_response(ctx, "admin/logs.html", serde_json::json!({}))
        .await
}

async fn logs_api_response(ctx: &SushiContext) -> HttpResponse {
    let logs = ctx.logs.list(1000).await;
    HttpResponse::new(
        200,
        serde_json::to_vec(&serde_json::json!({ "logs": logs }))
            .expect("logs JSON serialization cannot fail"),
    )
    .with_header("content-type", "application/json")
}
