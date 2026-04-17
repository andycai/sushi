use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use sushi_core::context::SushiContext;

pub async fn config_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/config.html").await
}

pub async fn config_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let cfg = ctx.config.get().await.clone();
    Json(serde_json::json!({
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
}
