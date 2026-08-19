use axum::extract::State;
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{
    AdminPageSpec, HttpHandler, HttpResponse, MenuContributionSpec, StagedRegistrar,
};

pub async fn dashboard_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    sushi_api::router::plugin_http_response(dashboard_page_response(&ctx).await)
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    staged.register_menu(
        MenuContributionSpec::new("host-admin.dashboard", "Dashboard", 10)
            .with_icon(Some("layout-dashboard".to_string()))
            .with_route(Some("/admin/".to_string()))
            .with_policy(Some("admin.dashboard.view".to_string())),
    );
    staged.register_admin(
        AdminPageSpec::new(
            "/admin/",
            "Dashboard",
            "admin-shell",
            "rust::dashboard-page",
        )
        .with_policy(Some("admin.dashboard.view".to_string()))
        .with_rust_handler(HttpHandler::new(move |_| {
            let ctx = ctx.clone();
            async move { Ok(dashboard_page_response(&ctx).await) }
        })),
    );
}

async fn dashboard_page_response(ctx: &SushiContext) -> HttpResponse {
    crate::render::render_template_http_response(ctx, "admin/dashboard.html", serde_json::json!({}))
        .await
}
