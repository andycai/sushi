use axum::extract::State;
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;

pub async fn login_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/login.html").await
}
