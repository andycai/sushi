use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{HttpRequest, HttpResponse};

pub fn decode_form<T>(request: &HttpRequest) -> Result<T, HttpResponse>
where
    T: DeserializeOwned,
{
    serde_urlencoded::from_bytes(request.body.as_deref().unwrap_or_default()).map_err(|error| {
        HttpResponse::new(
            StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
            format!("Failed to deserialize form body: {error}"),
        )
        .with_header("content-type", "text/plain; charset=utf-8")
    })
}

pub fn path_i64(path: &str, prefix: &str, suffix: &str) -> Result<i64, HttpResponse> {
    let Some(value) = path
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .filter(|value| !value.is_empty() && !value.contains('/'))
    else {
        return Err(path_rejection());
    };

    value.parse::<i64>().map_err(|_| path_rejection())
}

pub async fn flash_response(
    ctx: &SushiContext,
    status: StatusCode,
    level: &str,
    message: &str,
) -> HttpResponse {
    let mut response = crate::render::render_template_http_response(
        ctx,
        "admin/partials/flash.html",
        serde_json::json!({
            "level": level,
            "message": message,
        }),
    )
    .await;
    response.status = status.as_u16();
    response
}

pub async fn flash_response_with_trigger(
    ctx: &SushiContext,
    status: StatusCode,
    level: &str,
    message: &str,
    trigger: &str,
) -> HttpResponse {
    flash_response(ctx, status, level, message)
        .await
        .with_header("hx-trigger", trigger)
}

pub async fn from_axum_response(response: impl IntoResponse) -> HttpResponse {
    let response: Response = response.into_response();
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect();
    match axum::body::to_bytes(response.into_body(), usize::MAX).await {
        Ok(body) => HttpResponse {
            status,
            headers,
            body: body.to_vec(),
        },
        Err(error) => HttpResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            format!("failed to collect handler response body: {error}"),
        )
        .with_header("content-type", "text/plain; charset=utf-8"),
    }
}

fn path_rejection() -> HttpResponse {
    HttpResponse::new(StatusCode::BAD_REQUEST.as_u16(), "Invalid URL parameter")
        .with_header("content-type", "text/plain; charset=utf-8")
}
