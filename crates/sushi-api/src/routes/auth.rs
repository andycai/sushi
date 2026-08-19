use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::middleware::AuthUser;
use sushi_core::auth::model::{LoginRequest, TokenResponse};
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::runtime::{HttpHandler, HttpRequest, HttpResponse, HttpRouteSpec, StagedRegistrar};
use sushi_core::storage::Storage;

#[derive(Clone)]
pub struct AuthRouteState {
    pub storage: Arc<dyn Storage>,
    pub jwt: Arc<JwtService>,
}

pub fn auth_routes(state: AuthRouteState) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/me", get(me))
        .with_state(state)
}

async fn login(
    State(state): State<AuthRouteState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    crate::router::plugin_http_response(login_response(&state.storage, &state.jwt, req).await)
}

async fn login_response(
    storage: &Arc<dyn Storage>,
    jwt: &Arc<JwtService>,
    req: LoginRequest,
) -> HttpResponse {
    let repo = UserRepository::new(Arc::clone(storage));
    match repo.find_by_username(&req.username).await {
        Ok(Some(user)) => {
            if password::verify_password(&req.password, &user.password_hash).unwrap_or(false) {
                match jwt
                    .create_access_token(user.id, &user.username, &user.role.to_string())
                    .and_then(|at| {
                        jwt.create_refresh_token(user.id, &user.username, &user.role.to_string())
                            .map(|rt| TokenResponse {
                                access_token: at,
                                refresh_token: rt,
                                token_type: "Bearer".to_string(),
                            })
                    }) {
                    Ok(tokens) => json_http_response(StatusCode::OK, json!(tokens)),
                    Err(e) => {
                        json_http_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": e }))
                    }
                }
            } else {
                json_http_response(
                    StatusCode::UNAUTHORIZED,
                    json!({ "error": "Invalid credentials" }),
                )
            }
        }
        Ok(None) => json_http_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Invalid credentials" }),
        ),
        Err(e) => json_http_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh(
    State(state): State<AuthRouteState>,
    Json(req): Json<RefreshRequest>,
) -> impl IntoResponse {
    crate::router::plugin_http_response(refresh_response(&state.jwt, req).await)
}

async fn refresh_response(jwt: &Arc<JwtService>, req: RefreshRequest) -> HttpResponse {
    match jwt.verify_token(&req.refresh_token) {
        Ok(claims) => {
            // Validate token type immediately after verification
            if claims.token_type != "refresh" {
                return json_http_response(
                    StatusCode::UNAUTHORIZED,
                    json!({ "error": "Invalid token type. Expected refresh token." }),
                );
            }

            match jwt.create_access_token(
                claims.sub.parse().unwrap_or(0),
                &claims.username,
                &claims.role,
            ) {
                Ok(access_token) => json_http_response(
                    StatusCode::OK,
                    json!({
                        "access_token": access_token,
                        "token_type": "Bearer"
                    }),
                ),
                Err(e) => {
                    json_http_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": e }))
                }
            }
        }
        Err(e) => json_http_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": format!("Invalid refresh token: {}", e) }),
        ),
    }
}

async fn me(
    axum::extract::Extension(user): axum::extract::Extension<AuthUser>,
) -> impl IntoResponse {
    Json(json!({
        "id": user.0.id,
        "username": user.0.username,
        "role": user.0.role.to_string(),
    }))
}

pub fn register_builtin_routes(
    staged: &mut StagedRegistrar,
    plugin_name: &'static str,
    storage: Arc<dyn Storage>,
    jwt: Arc<JwtService>,
) {
    let login_storage = Arc::clone(&storage);
    let login_jwt = Arc::clone(&jwt);
    staged.register_http(
        HttpRouteSpec::new("POST", "/api/auth/login", plugin_name, "rust::auth-login")
            .with_public(true)
            .with_rust_handler(HttpHandler::new(move |request| {
                let storage = Arc::clone(&login_storage);
                let jwt = Arc::clone(&login_jwt);
                async move {
                    let body = request.body.unwrap_or_default();
                    let payload = match serde_json::from_slice::<LoginRequest>(&body) {
                        Ok(payload) => payload,
                        Err(error) => {
                            return Ok(json_http_response(
                                StatusCode::BAD_REQUEST,
                                json!({ "error": format!("invalid login request body: {error}") }),
                            ))
                        }
                    };
                    Ok(login_response(&storage, &jwt, payload).await)
                }
            })),
    );

    let refresh_jwt = Arc::clone(&jwt);
    staged.register_http(
        HttpRouteSpec::new(
            "POST",
            "/api/auth/refresh",
            plugin_name,
            "rust::auth-refresh",
        )
        .with_public(true)
        .with_rust_handler(HttpHandler::new(move |request| {
            let jwt = Arc::clone(&refresh_jwt);
            async move {
                let body = request.body.unwrap_or_default();
                let payload = match serde_json::from_slice::<RefreshRequest>(&body) {
                    Ok(payload) => payload,
                    Err(error) => {
                        return Ok(json_http_response(
                            StatusCode::BAD_REQUEST,
                            json!({ "error": format!("invalid refresh request body: {error}") }),
                        ))
                    }
                };
                Ok(refresh_response(&jwt, payload).await)
            }
        })),
    );

    let me_jwt = Arc::clone(&jwt);
    staged.register_http(
        HttpRouteSpec::new("GET", "/api/auth/me", plugin_name, "rust::auth-me")
            .with_policy(Some("api.auth.me".to_string()))
            .with_rust_handler(HttpHandler::new(move |request| {
                let jwt = Arc::clone(&me_jwt);
                async move { Ok(me_response(&jwt, &request)) }
            })),
    );
}

fn me_response(jwt: &Arc<JwtService>, request: &HttpRequest) -> HttpResponse {
    let Some(token) = request_header(request, "authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| request_header(request, "cookie").and_then(cookie_token))
    else {
        return json_http_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Missing authorization credentials" }),
        );
    };
    match jwt.verify_token(token) {
        Ok(claims) if claims.token_type == "access" => json_http_response(
            StatusCode::OK,
            json!({
                "id": claims.sub.parse::<i64>().unwrap_or(0),
                "username": claims.username,
                "role": claims.role,
            }),
        ),
        Ok(_) => json_http_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Invalid token type. Use access token for API access." }),
        ),
        Err(_) => json_http_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Invalid token" }),
        ),
    }
}

fn request_header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| std::str::from_utf8(value).ok())
}

fn cookie_token(cookie: &str) -> Option<&str> {
    cookie
        .split(';')
        .find_map(|part| part.trim().strip_prefix("sushi_token="))
}

fn json_http_response(status: StatusCode, payload: serde_json::Value) -> HttpResponse {
    HttpResponse::new(
        status.as_u16(),
        serde_json::to_vec(&payload).expect("JSON value serialization cannot fail"),
    )
    .with_header("content-type", "application/json")
}
