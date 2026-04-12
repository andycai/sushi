use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use sushi_admin::router::build_admin_router;
use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::web::template_service::TemplateService;
use tower::ServiceExt;

const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root missing")
        .to_path_buf()
}

fn templates_root() -> PathBuf {
    workspace_root().join("web").join("templates")
}

fn static_root() -> PathBuf {
    workspace_root().join("web").join("static")
}

fn collect_admin_template_paths() -> Vec<PathBuf> {
    let templates_dir = templates_root();
    let mut paths = Vec::new();

    let admin_dir = templates_dir.join("admin");
    if admin_dir.exists() {
        collect_html_files(&admin_dir, &mut paths);
    }

    let base = templates_dir.join("base.html");
    if base.exists() {
        paths.push(base);
    }

    paths
}

fn collect_html_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in dir.read_dir().expect("failed to read template directory") {
        let entry = entry.expect("failed to read template entry");
        let path = entry.path();
        if path.is_dir() {
            collect_html_files(&path, paths);
        } else if matches!(path.extension().and_then(|ext| ext.to_str()), Some("html")) {
            paths.push(path);
        }
    }
}

fn collect_files_with_extension(dir: &Path, ext: &str, paths: &mut Vec<PathBuf>) {
    for entry in dir.read_dir().expect("failed to read directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, ext, paths);
        } else if matches!(path.extension().and_then(|value| value.to_str()), Some(value) if value == ext)
        {
            paths.push(path);
        }
    }
}

fn collect_template_and_ui_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_files_with_extension(&templates_root(), "html", &mut paths);
    collect_files_with_extension(&static_root().join("admin"), "js", &mut paths);
    collect_files_with_extension(&static_root().join("plugins"), "js", &mut paths);
    paths
}

const ASSET_ATTRIBUTES: [&str; 2] = ["src", "href"];
const EXTERNAL_URL_PREFIXES: [&str; 3] = ["http://", "https://", "//"];

fn extract_attribute_values<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let html_lower = html.to_ascii_lowercase();
    let lower_bytes = html_lower.as_bytes();
    let mut values = Vec::new();
    let mut offset = 0;

    while let Some(pos) = html_lower[offset..].find(attr) {
        let attr_start = offset + pos;

        if attr_start > 0 {
            let prev = lower_bytes[attr_start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'-' {
                offset = attr_start + attr.len();
                continue;
            }
        }

        if attr_start + attr.len() < lower_bytes.len() {
            let next = lower_bytes[attr_start + attr.len()];
            if next.is_ascii_alphanumeric() || next == b'-' {
                offset = attr_start + attr.len();
                continue;
            }
        }

        let mut idx = attr_start + attr.len();
        while idx < lower_bytes.len() && lower_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        if idx >= lower_bytes.len() || lower_bytes[idx] != b'=' {
            offset = attr_start + attr.len();
            continue;
        }

        idx += 1;
        while idx < lower_bytes.len() && lower_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        if idx >= lower_bytes.len() {
            break;
        }

        let delim = lower_bytes[idx];
        if delim != b'"' && delim != b'\'' {
            offset = attr_start + attr.len();
            continue;
        }

        let value_start = idx + 1;
        let mut value_end = value_start;
        while value_end < lower_bytes.len() && lower_bytes[value_end] != delim {
            value_end += 1;
        }

        if value_end >= lower_bytes.len() {
            break;
        }

        values.push(&html[value_start..value_end]);
        offset = value_end + 1;
    }

    values
}

fn is_external_asset(value: &str) -> bool {
    let trimmed = value.trim_start();
    EXTERNAL_URL_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

fn assert_no_external_assets_in_html(source: &str, html: &str) {
    for attr in ASSET_ATTRIBUTES {
        for value in extract_attribute_values(html, attr) {
            assert!(
                !is_external_asset(value),
                "{} references external {} value `{}`",
                source,
                attr,
                value.trim()
            );
        }
    }
}

async fn build_app(static_url_prefix: Option<&str>) -> axum::Router {
    let templates_dir = templates_root();
    let static_dir = static_root();

    let mut config = SushiConfig::default();
    config.web.templates_dir = templates_dir.to_string_lossy().to_string();
    config.web.static_dir = static_dir.to_string_lossy().to_string();
    if let Some(prefix) = static_url_prefix {
        config.web.static_url_prefix = prefix.to_string();
    }

    let config = ConfigStore::new(config);
    let storage = SqliteStorage::new_in_memory()
        .await
        .expect("failed to init sqlite");
    storage
        .run_migrations(MIGRATION_SQL)
        .await
        .expect("failed to run migration 001_init");
    storage
        .run_migrations(KV_MIGRATION_SQL)
        .await
        .expect("failed to run migration 002_kv_store");
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    build_admin_router(&ctx).await
}

fn admin_bearer_token() -> String {
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    jwt.create_access_token(1, "admin", "admin")
        .expect("failed to create token")
}

#[tokio::test]
async fn login_and_static_routes_work() {
    let app = build_app(None).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin-login")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/static/js/alpine-3.15.11.js")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_requires_auth_without_token() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok());
    assert_eq!(location, Some("/admin-login"));
}

#[tokio::test]
async fn custom_static_prefix_is_used_in_templates_and_routes() {
    let app = build_app(Some("/assets")).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin-login")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("assets/admin/js/login.js"), "html: {html}");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/js/alpine-3.15.11.js")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_prefix_is_rejected_for_static() {
    let app = build_app(Some("/admin")).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok());
    assert_eq!(location, Some("/admin-login"));
}

#[tokio::test]
async fn plugins_api_returns_list_payload() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/plugins")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let payload: Value = serde_json::from_slice(&body).expect("invalid json payload");
    assert!(payload.is_array(), "expected array payload, got: {payload}");
}

#[tokio::test]
async fn htmx_login_submit_returns_error_snippet_for_invalid_credentials() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin-login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header("hx-request", "true")
                .body(Body::from("username=missing&password=wrong"))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Invalid credentials"), "html: {html}");
}

#[tokio::test]
async fn users_partial_requires_auth() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/partials/users/table")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok());
    assert_eq!(location, Some("/admin-login"));
}

#[tokio::test]
async fn users_and_plugins_partials_return_html_for_authenticated_admin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let users_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/users/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(users_response.status(), StatusCode::OK);
    let users_body = to_bytes(users_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let users_html = String::from_utf8_lossy(&users_body);
    assert!(
        users_html.contains("No users found") || users_html.contains("<tr"),
        "users_html: {users_html}"
    );

    let plugins_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/partials/plugins/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(plugins_response.status(), StatusCode::OK);
    let plugins_body = to_bytes(plugins_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let plugins_html = String::from_utf8_lossy(&plugins_body);
    assert!(
        plugins_html.contains("No plugins found") || plugins_html.contains("<tr"),
        "plugins_html: {plugins_html}"
    );
}

#[tokio::test]
async fn templates_do_not_reference_external_cdn_links() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin-login")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let html = String::from_utf8_lossy(&body);

    assert_no_external_assets_in_html("/admin-login response", &html);
}

#[tokio::test]
async fn all_admin_templates_exclude_external_cdn_links() {
    let template_paths = collect_admin_template_paths();
    assert!(
        !template_paths.is_empty(),
        "expected at least one admin template"
    );

    for path in template_paths {
        let html = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("failed to read template file {}", path.display()));

        let source = format!("template {}", path.display());
        assert_no_external_assets_in_html(&source, &html);
    }
}

#[tokio::test]
async fn templates_and_ui_scripts_avoid_native_confirm_apis() {
    let paths = collect_template_and_ui_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one template or ui file"
    );

    for path in paths {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("failed to read source file {}", path.display()));
        assert!(
            !source.contains("hx-confirm="),
            "{} still uses hx-confirm",
            path.display()
        );
        assert!(
            !source.contains("alert("),
            "{} still uses alert()",
            path.display()
        );
        assert!(
            !source.contains("confirm("),
            "{} still uses confirm()",
            path.display()
        );
    }
}

#[tokio::test]
async fn kv_rows_template_uses_dataset_for_alpine_edit_action() {
    let path = templates_root()
        .join("plugins")
        .join("kv-store")
        .join("partials")
        .join("rows.html");
    let html = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("failed to read template file {}", path.display()));

    assert!(
        html.contains("@click=\"openEdit($el.dataset.key, $el.dataset.value)\""),
        "template should call openEdit with dataset values: {}",
        path.display()
    );
    assert!(
        !html.contains("@click=\"openEdit({{"),
        "template should not interpolate JSON directly in click handlers: {}",
        path.display()
    );
}
