use axum::http::StatusCode;
use axum::response::Response;
use sushi_core::context::SushiContext;
use sushi_core::runtime::HttpResponse;

pub async fn render_template(ctx: &SushiContext, name: &str) -> Response {
    render_template_with_context(ctx, name, serde_json::json!({})).await
}

pub async fn render_template_with_context(
    ctx: &SushiContext,
    name: &str,
    context: serde_json::Value,
) -> Response {
    sushi_api::router::plugin_http_response(render_template_http_response(ctx, name, context).await)
}

pub async fn render_template_http_response(
    ctx: &SushiContext,
    name: &str,
    context: serde_json::Value,
) -> HttpResponse {
    let static_url_prefix = {
        let cfg = ctx.config.get().await;
        normalize_static_url_prefix(&cfg.web.static_url_prefix)
    };

    let template_context = merge_static_prefix(context, &static_url_prefix);

    match ctx.templates.render(name, template_context) {
        Ok(html) => HttpResponse::new(StatusCode::OK.as_u16(), html)
            .with_header("content-type", "text/html; charset=utf-8"),
        Err(err) => {
            tracing::error!("template render error for {name}: {err}");
            HttpResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                "template render error",
            )
            .with_header("content-type", "text/plain; charset=utf-8")
        }
    }
}

fn merge_static_prefix(
    mut context: serde_json::Value,
    static_url_prefix: &str,
) -> serde_json::Value {
    match context {
        serde_json::Value::Object(ref mut map) => {
            map.insert(
                "static_url_prefix".to_string(),
                serde_json::Value::String(static_url_prefix.to_string()),
            );
            context
        }
        _ => serde_json::json!({
            "static_url_prefix": static_url_prefix,
            "data": context,
        }),
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
