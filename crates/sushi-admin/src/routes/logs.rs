use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use sushi_core::context::SushiContext;

pub async fn logs_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/logs.html").await
}

pub async fn logs_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let logs = ctx.logs.list(1000).await;
    Json(serde_json::json!({
        "logs": logs,
    }))
}
