use axum::extract::State;
use axum::extract::Form;
use axum::http::{header, header::HeaderName, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use serde::Deserialize;
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;
use std::sync::Arc;

pub async fn login_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/login.html").await
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
        Ok(None) => return login_error_response("Invalid credentials", is_htmx),
        Err(_) => return login_error_response("Login service is unavailable", is_htmx),
    };

    let verified = password::verify_password(&form.password, &user.password_hash).unwrap_or(false);
    if !verified {
        return login_error_response("Invalid credentials", is_htmx);
    }

    let access_token = match ctx
        .jwt
        .create_access_token(user.id, &user.username, &user.role.to_string())
    {
        Ok(token) => token,
        Err(_) => return login_error_response("Failed to create session token", is_htmx),
    };

    let cookie = format!(
        "sushi_token={}; Path=/; Max-Age=86400; SameSite=Lax; HttpOnly",
        access_token
    );

    if is_htmx {
        let mut response = StatusCode::NO_CONTENT.into_response();
        if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, cookie_value);
        }
        response.headers_mut().insert(
            HeaderName::from_static("hx-redirect"),
            HeaderValue::from_static("/admin/"),
        );
        return response;
    }

    let mut response = Redirect::temporary("/admin/").into_response();
    if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, cookie_value);
    }
    response
}
fn login_error_response(message: &str, is_htmx: bool) -> axum::response::Response {
    if is_htmx {
        let html = format!(
            r#"<div class="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">{message}</div>"#
        );
        return (StatusCode::UNAUTHORIZED, axum::response::Html(html)).into_response();
    }

    (
        StatusCode::UNAUTHORIZED,
        axum::response::Html(format!("<h1>Login failed</h1><p>{message}</p>")),
    )
        .into_response()
}
