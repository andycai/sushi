use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use sushi_core::context::SushiContext;

pub async fn render_template(ctx: &SushiContext, name: &str) -> Response {
    let static_url_prefix = {
        let cfg = ctx.config.get().await;
        normalize_static_url_prefix(&cfg.web.static_url_prefix)
    };

    match ctx.templates.render(
        name,
        serde_json::json!({
            "static_url_prefix": static_url_prefix,
        }),
    ) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!("template render error for {name}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "template render error").into_response()
        }
    }
}

pub fn normalize_static_url_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("static_url_prefix is empty; falling back to /static");
        return "/static".to_string();
    }

    let mut prefix = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };

    if prefix.len() > 1 {
        prefix = prefix.trim_end_matches('/').to_string();
    }

    if prefix == "/" {
        tracing::warn!("static_url_prefix '/' is not allowed; falling back to /static");
        return "/static".to_string();
    }

    if prefix == "/admin" || prefix.starts_with("/admin/") {
        tracing::warn!(
            "static_url_prefix '{prefix}' conflicts with /admin; falling back to /static"
        );
        return "/static".to_string();
    }

    prefix
}
