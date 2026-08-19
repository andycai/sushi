use axum::extract::State;
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpResponse, HttpRouteSpec, MenuContributionSpec, StagedRegistrar,
};

pub async fn config_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(config_page_response(&ctx).await)
}

pub async fn config_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(config_api_response(&ctx).await)
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("host-admin.config", "Config", 60)
            .with_icon(Some("settings".to_string()))
            .with_parent(Some("host-admin.system".to_string()))
            .with_route(Some("/admin/config".to_string()))
            .with_policy(Some("admin.config.view".to_string())),
    );
    let page_ctx = ctx.clone();
    staged.register_admin(
        AdminPageSpec::new("/admin/config", "Config", "host-admin", "rust::config-page")
            .with_policy(Some("admin.config.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = page_ctx.clone();
                async move { Ok(config_page_response(&ctx).await) }
            })),
    );

    staged.register_http(
        HttpRouteSpec::new("GET", "/admin/api/config", "host-admin", "rust::config-api")
            .with_policy(Some("admin.config.view".to_string()))
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = ctx.clone();
                async move { Ok(config_api_response(&ctx).await) }
            })),
    );
}

async fn config_page_response(ctx: &SushiContext) -> HttpResponse {
    crate::render::render_template_http_response(ctx, "admin/config.html", serde_json::json!({}))
        .await
}

async fn config_api_response(ctx: &SushiContext) -> HttpResponse {
    let cfg = ctx.config.get().await.clone();
    HttpResponse::new(
        200,
        serde_json::to_vec(&serde_json::json!({
        "server": {
            "host": cfg.server.host,
            "port": cfg.server.port,
        },
        "database": {
            "path": cfg.database.path,
        },
        "jwt": {
            "access_ttl": cfg.jwt.access_ttl,
            "refresh_ttl": cfg.jwt.refresh_ttl,
        },
        "plugins": {
            "directory": cfg.plugins.directory,
        },
        "file_browser": {
            "root_dir": cfg.file_browser.root_dir,
        },
        }))
        .expect("config JSON serialization cannot fail"),
    )
    .with_header("content-type", "application/json")
}
