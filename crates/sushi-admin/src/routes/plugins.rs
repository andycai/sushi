use axum::extract::State;
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;

pub async fn plugins_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/plugins.html").await
}

pub async fn plugins_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let plugins = ctx.plugins.list_plugins().await;
    crate::render::render_template_with_context(
        &ctx,
        "admin/partials/plugins_rows.html",
        serde_json::json!({
            "plugins": plugins,
        }),
    )
    .await
}
