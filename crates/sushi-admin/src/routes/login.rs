use axum::extract::Form;
use axum::extract::State;
use axum::http::{header, header::HeaderName, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::context::SushiContext;
use sushi_core::runtime::{HttpHandler, HttpRequest, HttpResponse, HttpRouteSpec, StagedRegistrar};
use sushi_core::storage::Storage;

pub async fn login_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/login.html").await
}

pub fn register_builtin_capabilities(staged: &mut StagedRegistrar, ctx: SushiContext) {
    let submit_ctx = ctx.clone();
    staged.register_http(
        HttpRouteSpec::new("GET", "/admin-login", "admin-shell", "rust::login-page")
            .with_public(true)
            .with_rust_handler(HttpHandler::new(move |_| {
                let ctx = ctx.clone();
                async move {
                    Ok(crate::render::render_template_http_response(
                        &ctx,
                        "admin/login.html",
                        serde_json::json!({}),
                    )
                    .await)
                }
            })),
    );
    staged.register_http(
        HttpRouteSpec::new("POST", "/admin-login", "admin-shell", "rust::login-submit")
            .with_public(true)
            .with_rust_handler(HttpHandler::new(move |request| {
                let ctx = submit_ctx.clone();
                async move { login_submit_http_response(&ctx, request).await }
            })),
    );
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_submit(
    State(ctx): State<SushiContext>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let is_htmx = headers
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let user = match repo.find_by_username(&form.username).await {
        Ok(Some(user)) => user,
        Ok(None) => return login_error_response(&ctx, "Invalid credentials", is_htmx).await,
        Err(_) => return login_error_response(&ctx, "Login service is unavailable", is_htmx).await,
    };

    let verified = password::verify_password(&form.password, &user.password_hash).unwrap_or(false);
    if !verified {
        return login_error_response(&ctx, "Invalid credentials", is_htmx).await;
    }

    let access_token =
        match ctx
            .jwt
            .create_access_token(user.id, &user.username, &user.role.to_string())
        {
            Ok(token) => token,
            Err(_) => {
                return login_error_response(&ctx, "Failed to create session token", is_htmx).await
            }
        };

    let cookie = format!(
        "sushi_token={}; Path=/; Max-Age=86400; SameSite=Lax; HttpOnly",
        access_token
    );

    if is_htmx {
        let mut response = StatusCode::NO_CONTENT.into_response();
        if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
            response
                .headers_mut()
                .append(header::SET_COOKIE, cookie_value);
        }
        response.headers_mut().insert(
            HeaderName::from_static("hx-redirect"),
            HeaderValue::from_static("/admin/"),
        );
        return response;
    }

    let mut response = Redirect::temporary("/admin/").into_response();
    if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie_value);
    }
    response
}

async fn login_submit_http_response(
    ctx: &SushiContext,
    request: HttpRequest,
) -> Result<HttpResponse, String> {
    let form = match crate::routes::transport::decode_form::<LoginForm>(&request) {
        Ok(form) => form,
        Err(response) => return Ok(response),
    };
    let is_htmx = request.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("hx-request")
            && std::str::from_utf8(value)
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
    });
    let repo = UserRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let user = match repo.find_by_username(&form.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(login_error_http_response(ctx, "Invalid credentials", is_htmx).await)
        }
        Err(_) => {
            return Ok(
                login_error_http_response(ctx, "Login service is unavailable", is_htmx).await,
            )
        }
    };
    let verified = password::verify_password(&form.password, &user.password_hash).unwrap_or(false);
    if !verified {
        return Ok(login_error_http_response(ctx, "Invalid credentials", is_htmx).await);
    }
    let access_token =
        match ctx
            .jwt
            .create_access_token(user.id, &user.username, &user.role.to_string())
        {
            Ok(token) => token,
            Err(_) => {
                return Ok(login_error_http_response(
                    ctx,
                    "Failed to create session token",
                    is_htmx,
                )
                .await)
            }
        };
    let cookie = format!(
        "sushi_token={}; Path=/; Max-Age=86400; SameSite=Lax; HttpOnly",
        access_token
    );
    if is_htmx {
        return Ok(
            HttpResponse::new(StatusCode::NO_CONTENT.as_u16(), Vec::new())
                .with_header("set-cookie", cookie)
                .with_header("hx-redirect", "/admin/"),
        );
    }
    Ok(
        HttpResponse::new(StatusCode::TEMPORARY_REDIRECT.as_u16(), Vec::new())
            .with_header("location", "/admin/")
            .with_header("set-cookie", cookie),
    )
}

async fn login_error_http_response(
    ctx: &SushiContext,
    message: &str,
    is_htmx: bool,
) -> HttpResponse {
    if is_htmx {
        let body = render_login_flash_html(ctx, message).unwrap_or_else(|| message.to_string());
        return HttpResponse::new(StatusCode::OK.as_u16(), body)
            .with_header("content-type", "text/html; charset=utf-8");
    }
    let mut response = crate::render::render_template_http_response(
        ctx,
        "admin/login.html",
        json!({ "error_message": message }),
    )
    .await;
    response.status = StatusCode::UNAUTHORIZED.as_u16();
    response
}

fn render_login_flash_html(ctx: &SushiContext, message: &str) -> Option<String> {
    match ctx.templates.render(
        "admin/partials/flash.html",
        json!({
            "level": "error",
            "message": message,
        }),
    ) {
        Ok(html) => Some(html),
        Err(err) => {
            tracing::error!("template render error for admin/partials/flash.html: {err}");
            None
        }
    }
}

async fn login_error_response(ctx: &SushiContext, message: &str, is_htmx: bool) -> Response {
    if is_htmx {
        let body = render_login_flash_html(ctx, message).unwrap_or_else(|| message.to_string());
        return axum::response::Html(body).into_response();
    }

    let mut response = crate::render::render_template_with_context(
        ctx,
        "admin/login.html",
        json!({
            "error_message": message,
        }),
    )
    .await;
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response
}
