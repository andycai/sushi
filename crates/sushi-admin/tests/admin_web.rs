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
use sushi_core::auth::authorizer::{CompiledPolicySnapshot, HttpBinding};
use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::plugin::manager::PageResolvedAssets;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::storage::Storage;
use sushi_core::web::template_service::TemplateService;
use tower::ServiceExt;

const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");
const RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/003_rbac.sql");
const MENU_MIGRATION_SQL: &str = include_str!("../../../migrations/004_menu.sql");
const MENUS_RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/005_menus_rbac.sql");
const UNIFIED_POLICY_V2_MIGRATION_SQL: &str =
    include_str!("../../../migrations/006_unified_policy_v2.sql");
const LEGACY_MENU_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS menu_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    icon TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    parent_id INTEGER,
    route TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (parent_id) REFERENCES menu_items(id) ON DELETE SET NULL
);

INSERT OR IGNORE INTO menu_items (id, label, icon, position, parent_id, route) VALUES
(1, 'Dashboard', 'layout-dashboard', 10, NULL, '/admin/'),
(2, 'Users', 'users', 20, NULL, '/admin/users'),
(3, 'Roles', 'shield', 30, NULL, '/admin/roles'),
(4, 'Permissions', 'key', 40, NULL, '/admin/permissions'),
(5, 'Plugins', 'package', 50, NULL, '/admin/plugins'),
(6, 'Config', 'settings', 60, NULL, '/admin/config'),
(7, 'Logs', 'file-text', 70, NULL, '/admin/logs');

INSERT INTO menu_items (label, icon, position, parent_id, route) VALUES
('KV Store', 'database', 51, 5, '/admin/kv'),
('KV Store', 'database', 51, 5, '/admin/kv'),
('KV Store', 'database', 51, 5, '/admin/kv');
"#;

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
    if !dir.exists() {
        return;
    }

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

    let plugins_root = workspace_root().join("plugins");
    if plugins_root.exists() {
        for entry in plugins_root
            .read_dir()
            .expect("failed to read plugins directory")
        {
            let entry = entry.expect("failed to read plugin entry");
            let plugin_root = entry.path();
            if !plugin_root.is_dir() {
                continue;
            }
            let plugin_templates = plugin_root.join("web").join("templates");
            if plugin_templates.exists() {
                collect_files_with_extension(&plugin_templates, "html", &mut paths);
            }
            let plugin_static = plugin_root.join("web").join("static");
            if plugin_static.exists() {
                collect_files_with_extension(&plugin_static, "js", &mut paths);
            }
        }
    }

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

fn directory_has_files(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }
    for entry in dir.read_dir().expect("failed to read directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.is_dir() {
            if directory_has_files(&path) {
                return true;
            }
        } else {
            return true;
        }
    }
    false
}

fn admin_http_bindings() -> Vec<HttpBinding> {
    vec![
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/".to_string(),
            policy_key: "admin.dashboard.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/logs".to_string(),
            policy_key: "admin.logs.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/api/logs".to_string(),
            policy_key: "admin.logs.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/config".to_string(),
            policy_key: "admin.config.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/api/config".to_string(),
            policy_key: "admin.config.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/plugins".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/plugins/{plugin}".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/plugins/table".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/api/plugins".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/api/plugins/{plugin}/pages".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/api/workspace/assets".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/menus".to_string(),
            policy_key: "admin.menus.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/menus/table".to_string(),
            policy_key: "admin.menus.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/api/menu".to_string(),
            policy_key: "admin.menus.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/users".to_string(),
            policy_key: "admin.users.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/users/table".to_string(),
            policy_key: "admin.users.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/roles".to_string(),
            policy_key: "admin.roles.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/roles/table".to_string(),
            policy_key: "admin.roles.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/permissions".to_string(),
            policy_key: "admin.permissions.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/permissions/table".to_string(),
            policy_key: "admin.permissions.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/roles/{id}/permissions/form".to_string(),
            policy_key: "admin.roles.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/users/create".to_string(),
            policy_key: "admin.users.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "DELETE".to_string(),
            path_pattern: "/admin/partials/users/{id}".to_string(),
            policy_key: "admin.users.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/roles/create".to_string(),
            policy_key: "admin.roles.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/roles/{id}/update".to_string(),
            policy_key: "admin.roles.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/roles/{id}/permissions".to_string(),
            policy_key: "admin.roles.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "DELETE".to_string(),
            path_pattern: "/admin/partials/roles/{id}".to_string(),
            policy_key: "admin.roles.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/permissions/create".to_string(),
            policy_key: "admin.permissions.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/permissions/{id}/update".to_string(),
            policy_key: "admin.permissions.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "DELETE".to_string(),
            path_pattern: "/admin/partials/permissions/{id}".to_string(),
            policy_key: "admin.permissions.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/menus/create".to_string(),
            policy_key: "admin.menus.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/partials/menus/{id}/update".to_string(),
            policy_key: "admin.menus.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "DELETE".to_string(),
            path_pattern: "/admin/partials/menus/{id}".to_string(),
            policy_key: "admin.menus.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "POST".to_string(),
            path_pattern: "/admin/api/menu".to_string(),
            policy_key: "admin.menus.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "PUT".to_string(),
            path_pattern: "/admin/api/menu/{id}".to_string(),
            policy_key: "admin.menus.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "DELETE".to_string(),
            path_pattern: "/admin/api/menu/{id}".to_string(),
            policy_key: "admin.menus.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/dashboard".to_string(),
            policy_key: "admin.dashboard.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/users".to_string(),
            policy_key: "admin.users.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/roles".to_string(),
            policy_key: "admin.roles.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/permissions".to_string(),
            policy_key: "admin.permissions.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/plugins".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/plugins/{plugin}".to_string(),
            policy_key: "admin.plugins.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/kv".to_string(),
            policy_key: "admin.kv.manage".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/config".to_string(),
            policy_key: "admin.config.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/logs".to_string(),
            policy_key: "admin.logs.view".to_string(),
        },
        HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/workspace/menus".to_string(),
            policy_key: "admin.menus.view".to_string(),
        },
    ]
}

async fn refresh_admin_authorizer(ctx: &SushiContext) {
    let grants_rows = ctx
        .db
        .query(
            r#"
            SELECT r.slug AS role_slug, pk.key AS policy_key
            FROM roles r
            JOIN role_policy_keys rpk ON rpk.role_id = r.id
            JOIN policy_keys pk ON pk.id = rpk.policy_key_id
            UNION
            SELECT r.slug AS role_slug, 'admin.' || p.slug AS policy_key
            FROM roles r
            JOIN role_permissions rp ON rp.role_id = r.id
            JOIN permissions p ON p.id = rp.permission_id
            "#,
            vec![],
        )
        .await
        .expect("failed to load role permission grants");

    let role_grants: Vec<(String, String)> = grants_rows
        .into_iter()
        .filter_map(|row| {
            let role = row.get("role_slug").and_then(Value::as_str)?;
            let policy_key = row.get("policy_key").and_then(Value::as_str)?;
            Some((role.to_string(), policy_key.to_string()))
        })
        .collect();

    let snapshot = CompiledPolicySnapshot::new(admin_http_bindings(), vec![], role_grants);
    ctx.authorizer.replace_snapshot(snapshot).await;
}

async fn build_app_with_context(static_url_prefix: Option<&str>) -> (axum::Router, SushiContext) {
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
    storage
        .run_migrations(RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 003_rbac");
    storage
        .run_migrations(MENU_MIGRATION_SQL)
        .await
        .expect("failed to run migration 004_menu");
    storage
        .run_migrations(MENUS_RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 005_menus_rbac");
    storage
        .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
        .await
        .expect("failed to run migration 006_unified_policy_v2");
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    (build_admin_router(&ctx).await, ctx)
}

async fn build_app(static_url_prefix: Option<&str>) -> axum::Router {
    let (app, _ctx) = build_app_with_context(static_url_prefix).await;
    app
}

async fn build_app_with_plugin_static(plugin_name: &str, plugin_static_dir: &Path) -> axum::Router {
    let templates_dir = templates_root();
    let static_dir = static_root();

    let mut config = SushiConfig::default();
    config.web.templates_dir = templates_dir.to_string_lossy().to_string();
    config.web.static_dir = static_dir.to_string_lossy().to_string();

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
    storage
        .run_migrations(RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 003_rbac");
    storage
        .run_migrations(MENU_MIGRATION_SQL)
        .await
        .expect("failed to run migration 004_menu");
    storage
        .run_migrations(MENUS_RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 005_menus_rbac");
    storage
        .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
        .await
        .expect("failed to run migration 006_unified_policy_v2");
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    ctx.plugins
        .register_plugin_static_root(plugin_name, plugin_static_dir.to_path_buf())
        .await;

    build_admin_router(&ctx).await
}

async fn build_app_with_plugin_page_assets(
    page_path: &str,
    assets: PageResolvedAssets,
) -> axum::Router {
    let templates_dir = templates_root();
    let static_dir = static_root();

    let mut config = SushiConfig::default();
    config.web.templates_dir = templates_dir.to_string_lossy().to_string();
    config.web.static_dir = static_dir.to_string_lossy().to_string();

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
    storage
        .run_migrations(RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 003_rbac");
    storage
        .run_migrations(MENU_MIGRATION_SQL)
        .await
        .expect("failed to run migration 004_menu");
    storage
        .run_migrations(MENUS_RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 005_menus_rbac");
    storage
        .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
        .await
        .expect("failed to run migration 006_unified_policy_v2");
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    ctx.plugins
        .register_admin_handler_with_assets(
            page_path,
            "kv-store",
            "KV Store",
            "missing-handler",
            assets,
        )
        .await;
    build_admin_router(&ctx).await
}

async fn build_app_with_legacy_menu_table() -> axum::Router {
    let templates_dir = templates_root();
    let static_dir = static_root();

    let mut config = SushiConfig::default();
    config.web.templates_dir = templates_dir.to_string_lossy().to_string();
    config.web.static_dir = static_dir.to_string_lossy().to_string();

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
    storage
        .run_migrations(RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 003_rbac");
    storage
        .run_migrations(LEGACY_MENU_SCHEMA_SQL)
        .await
        .expect("failed to run legacy menu schema migration");
    storage
        .run_migrations(MENUS_RBAC_MIGRATION_SQL)
        .await
        .expect("failed to run migration 005_menus_rbac");
    storage
        .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
        .await
        .expect("failed to run migration 006_unified_policy_v2");
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    build_admin_router(&ctx).await
}

#[test]
fn base_template_has_no_plugin_specific_module_mappings() {
    let base = templates_root().join("base.html");
    let html = fs::read_to_string(&base).expect("failed to read base template");

    assert!(!html.contains("/plugins/kv-store/kv.js"));
    assert!(!html.contains("kv:"));
}

#[test]
fn base_template_uses_dynamic_active_section_script_loading() {
    let base = templates_root().join("base.html");
    let html = fs::read_to_string(&base).expect("failed to read base template");

    assert!(html.contains("admin/js/{{ active_section }}.js"));
    assert!(!html.contains("active_section == \"dashboard\""));
}

#[test]
fn legacy_global_plugin_asset_dirs_have_no_files() {
    let root = workspace_root();
    let legacy_template_dir = root.join("web/templates/plugins");
    let legacy_static_dir = root.join("web/static/plugins");

    assert!(
        !directory_has_files(&legacy_template_dir),
        "legacy template plugin directory must be empty"
    );
    assert!(
        !directory_has_files(&legacy_static_dir),
        "legacy static plugin directory must be empty"
    );
}

#[tokio::test]
async fn workspace_assets_api_returns_plugin_assets_for_page_path() {
    let app = build_app_with_plugin_page_assets(
        "/admin/kv",
        PageResolvedAssets {
            js: vec!["/static/plugins/official/kv-store/kv.js".to_string()],
            css: vec!["/static/plugins/official/kv-store/kv.css".to_string()],
        },
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/workspace/assets?path=/admin/kv")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", admin_bearer_token()),
                )
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("failed to read body");
    let payload: Value = serde_json::from_slice(&body).expect("invalid json payload");
    assert_eq!(
        payload
            .get("js")
            .and_then(|value| value.as_array())
            .map(|arr| arr.len()),
        Some(1)
    );
    assert_eq!(
        payload
            .get("css")
            .and_then(|value| value.as_array())
            .map(|arr| arr.len()),
        Some(1)
    );
}

#[tokio::test]
async fn viewer_cannot_fetch_workspace_assets_for_users_path_without_admin_users_read() {
    let (app, ctx) = build_app_with_context(None).await;
    let admin = admin_bearer_token();

    let create_role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/roles/create")
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "slug=plugins_viewer&name=Plugins+Viewer&description=Can+view+plugins+only",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(create_role_response.status(), StatusCode::OK);

    let roles_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/roles/table")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let roles_table_body = to_bytes(roles_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let roles_table_html = String::from_utf8_lossy(&roles_table_body);
    let role_id = extract_dataset_id_by_slug(
        &roles_table_html,
        "data-role-slug",
        "plugins_viewer",
        "data-role-id",
    )
    .expect("role id should be discoverable");

    let permissions_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let permissions_table_body = to_bytes(permissions_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let permissions_table_html = String::from_utf8_lossy(&permissions_table_body);
    let plugins_view_permission_id = extract_dataset_id_by_slug(
        &permissions_table_html,
        "data-permission-slug",
        "plugins.view",
        "data-permission-id",
    )
    .expect("plugins.view permission id should be discoverable");

    let assign_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/partials/roles/{role_id}/permissions"))
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "permission_ids={plugins_view_permission_id}"
                )))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(assign_response.status(), StatusCode::OK);
    refresh_admin_authorizer(&ctx).await;

    let plugins_viewer = bearer_token_for_role("plugins_viewer");
    let allowed_target_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/workspace/assets?path=/admin/plugins")
                .header("authorization", format!("Bearer {plugins_viewer}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(allowed_target_response.status(), StatusCode::OK);

    let denied_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/workspace/assets?path=/admin/users")
                .header("authorization", format!("Bearer {plugins_viewer}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(denied_response.status(), StatusCode::FORBIDDEN);
}

fn admin_bearer_token() -> String {
    bearer_token_for_role("admin")
}

fn bearer_token_for_role(role: &str) -> String {
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    jwt.create_access_token(1, "admin", role)
        .expect("failed to create token")
}

fn extract_attr_value(source: &str, attr: &str) -> Option<String> {
    let marker = format!(r#"{attr}=""#);
    let start = source.find(&marker)? + marker.len();
    let rest = &source[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_dataset_id_by_slug(
    html: &str,
    slug_attr: &str,
    slug_value: &str,
    id_attr: &str,
) -> Option<i64> {
    let slug_marker = format!(r#"{slug_attr}="{slug_value}""#);
    let slug_pos = html.find(&slug_marker)?;
    let row_start = html[..slug_pos].rfind("<tr")?;
    let row_end = html[slug_pos..]
        .find('>')
        .map(|idx| slug_pos + idx)
        .unwrap_or(html.len());
    let row = &html[row_start..row_end];
    extract_attr_value(row, id_attr)?.parse::<i64>().ok()
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
async fn plugin_static_assets_are_served_from_plugin_directories() {
    let plugin_static_dir = std::env::temp_dir().join(format!(
        "sushi-admin-plugin-static-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&plugin_static_dir).expect("failed to create plugin static tempdir");
    fs::write(
        plugin_static_dir.join("kv.js"),
        "window.__pluginStaticLoaded = true;",
    )
    .expect("failed to write plugin static asset");

    let app = build_app_with_plugin_static("official/kv-store", &plugin_static_dir).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/static/plugins/official/kv-store/kv.js")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("failed to read body");
    let content = String::from_utf8(body.to_vec()).expect("static body must be utf-8");
    assert!(content.contains("__pluginStaticLoaded"));

    fs::remove_dir_all(&plugin_static_dir).expect("failed to clean plugin static tempdir");
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
async fn workspace_route_requires_auth_without_token() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/users")
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
async fn plugin_pages_api_rejects_unknown_plugin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/plugins/does-not-exist/pages")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn plugin_pages_api_returns_workspace_payload_for_discovered_plugin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let plugins_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/plugins")
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
    let plugins_payload: Value =
        serde_json::from_slice(&plugins_body).expect("invalid json payload for plugins");
    let Some(first_plugin_name) = plugins_payload
        .as_array()
        .and_then(|plugins| plugins.first())
        .and_then(|plugin| plugin.get("name"))
        .and_then(Value::as_str)
    else {
        return;
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/plugins/{first_plugin_name}/pages"))
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

    assert_eq!(
        payload
            .get("plugin")
            .and_then(|plugin| plugin.get("name"))
            .and_then(Value::as_str),
        Some(first_plugin_name)
    );
    assert!(
        payload.get("pages").and_then(Value::as_array).is_some(),
        "expected pages array payload, got {payload}"
    );
}

#[tokio::test]
async fn menu_api_returns_menu_items() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
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

    let menu = payload
        .get("menu")
        .and_then(Value::as_array)
        .expect("menu array missing");
    assert!(!menu.is_empty(), "menu should have items");

    // 验证 Dashboard 存在
    let dashboard = menu
        .iter()
        .find(|m| m.get("label").and_then(Value::as_str) == Some("Dashboard"));
    assert!(dashboard.is_some(), "Dashboard menu item should exist");

    let menus = menu
        .iter()
        .find(|m| m.get("route").and_then(Value::as_str) == Some("/admin/menus"));
    assert!(menus.is_some(), "Menus management entry should exist");

    let system = menu
        .iter()
        .find(|m| m.get("route").and_then(Value::as_str) == Some("/admin/system"))
        .expect("System menu item should exist");
    let system_id = system
        .get("id")
        .and_then(Value::as_i64)
        .expect("System menu id should exist");

    for route in [
        "/admin/users",
        "/admin/roles",
        "/admin/permissions",
        "/admin/config",
        "/admin/menus",
        "/admin/logs",
    ] {
        let item = menu
            .iter()
            .find(|m| m.get("route").and_then(Value::as_str) == Some(route))
            .unwrap_or_else(|| panic!("menu item for route {route} should exist"));
        let parent_id = item.get("parent_id").and_then(Value::as_i64);
        assert_eq!(
            parent_id,
            Some(system_id),
            "route {route} should be grouped under System menu"
        );
    }
}

#[tokio::test]
async fn menu_api_handles_legacy_menu_table_without_is_hidden_column() {
    let app = build_app_with_legacy_menu_table().await;
    let token = admin_bearer_token();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
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
    let menu = payload
        .get("menu")
        .and_then(Value::as_array)
        .expect("menu array missing");
    assert!(!menu.is_empty(), "menu should have items for legacy schema");
    let menus_count = menu
        .iter()
        .filter(|item| item.get("route").and_then(Value::as_str) == Some("/admin/menus"))
        .count();
    assert_eq!(menus_count, 1, "menus management entry should be unique");
    let kv_count = menu
        .iter()
        .filter(|item| item.get("route").and_then(Value::as_str) == Some("/admin/kv"))
        .count();
    assert_eq!(
        kv_count, 1,
        "legacy duplicate kv menu entries should be deduplicated"
    );
    let system = menu
        .iter()
        .find(|m| m.get("route").and_then(Value::as_str) == Some("/admin/system"))
        .expect("System menu item should be backfilled");
    let system_id = system
        .get("id")
        .and_then(Value::as_i64)
        .expect("System menu id should exist");
    for route in [
        "/admin/users",
        "/admin/roles",
        "/admin/permissions",
        "/admin/config",
        "/admin/menus",
        "/admin/logs",
    ] {
        let item = menu
            .iter()
            .find(|m| m.get("route").and_then(Value::as_str) == Some(route))
            .unwrap_or_else(|| panic!("menu item for route {route} should exist"));
        let parent_id = item.get("parent_id").and_then(Value::as_i64);
        assert_eq!(
            parent_id,
            Some(system_id),
            "legacy route {route} should be re-parented under System menu"
        );
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/admin/api/menu/1")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"is_hidden":true}"#))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn menu_api_crud_operations() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/menu")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"label":"Test Menu","icon":"settings","route":"/admin/test"}"#,
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::CREATED);

    // List and verify it exists
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
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
    let menu = payload
        .get("menu")
        .and_then(Value::as_array)
        .expect("menu array missing");
    let test_menu = menu
        .iter()
        .find(|m| m.get("label").and_then(Value::as_str) == Some("Test Menu"));
    assert!(test_menu.is_some(), "Test Menu should exist after create");
    let menu_id = test_menu
        .unwrap()
        .get("id")
        .and_then(Value::as_i64)
        .expect("menu id missing");

    // Update
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/admin/api/menu/{menu_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"label":"Updated Menu","is_hidden":true}"#))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    // Delete
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/menu/{menu_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn config_api_returns_sanitized_config_payload() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/config")
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

    assert!(payload.get("server").is_some(), "payload: {payload}");
    assert!(payload.get("database").is_some(), "payload: {payload}");
    assert!(payload.get("jwt").is_some(), "payload: {payload}");
    assert!(payload.get("plugins").is_some(), "payload: {payload}");
    assert!(
        payload.pointer("/jwt/secret").is_none(),
        "config api must not expose jwt secret: {payload}"
    );
}

#[tokio::test]
async fn logs_api_returns_logs_array_payload() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/logs")
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
    let logs = payload.get("logs").and_then(Value::as_array);
    assert!(
        logs.is_some(),
        "expected payload.logs array, got: {payload}"
    );
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
async fn standard_login_submit_returns_login_template_error_for_invalid_credentials() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin-login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
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
    assert!(html.contains("id=\"login-form\""), "html: {html}");
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
    if plugins_html.contains("Workspace") {
        assert!(
            plugins_html.contains("data-workspace-path"),
            "plugins workspace links should expose workspace path dataset: {plugins_html}"
        );
        assert!(
            plugins_html.contains("@click.prevent=\"openWorkspace("),
            "plugins workspace links should open via workspace tabs when available: {plugins_html}"
        );
    }
}

#[tokio::test]
async fn roles_and_permissions_pages_load_for_authenticated_admin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let roles_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/roles")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(roles_response.status(), StatusCode::OK);

    let permissions_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/permissions")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(permissions_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn roles_and_permissions_partials_return_html_for_authenticated_admin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let roles_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/roles/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(roles_response.status(), StatusCode::OK);
    let roles_body = to_bytes(roles_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let roles_html = String::from_utf8_lossy(&roles_body);
    assert!(
        roles_html.contains("No roles found") || roles_html.contains("<tr"),
        "roles_html: {roles_html}"
    );

    let permissions_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(permissions_response.status(), StatusCode::OK);
    let permissions_body = to_bytes(permissions_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let permissions_html = String::from_utf8_lossy(&permissions_body);
    assert!(
        permissions_html.contains("No permissions found") || permissions_html.contains("<tr"),
        "permissions_html: {permissions_html}"
    );
}

#[tokio::test]
async fn menus_page_and_partials_return_html_for_authenticated_admin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let page_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/menus")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(page_response.status(), StatusCode::OK);

    let partial_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/partials/menus/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(partial_response.status(), StatusCode::OK);
    let partial_body = to_bytes(partial_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let partial_html = String::from_utf8_lossy(&partial_body);
    assert!(
        partial_html.contains("No menu items found") || partial_html.contains("<tr"),
        "partial_html: {partial_html}"
    );
}

#[tokio::test]
async fn editor_can_access_users_page_with_assigned_permission() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("editor");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn viewer_cannot_access_users_page_without_permission() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn workspace_users_module_loads_for_authenticated_admin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn workspace_unknown_module_returns_not_found() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/unknown-module")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workspace_nested_module_uses_catch_all_and_returns_workspace_not_found() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/plugins/fake-plugin")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let payload = String::from_utf8_lossy(&body);
    assert!(
        payload.contains("workspace module not found"),
        "payload: {payload}"
    );
}

#[tokio::test]
async fn workspace_plugin_nested_path_without_registered_page_returns_not_found() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/plugins/kv-store/details")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn plugin_workspace_page_rejects_unknown_plugin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/plugins/does-not-exist")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn plugin_workspace_page_includes_quick_navigation_sections() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let plugins_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/plugins")
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
    let plugins_payload: Value =
        serde_json::from_slice(&plugins_body).expect("invalid json payload for plugins");
    let Some(first_plugin_name) = plugins_payload
        .as_array()
        .and_then(|plugins| plugins.first())
        .and_then(|plugin| plugin.get("name"))
        .and_then(Value::as_str)
    else {
        return;
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/admin/plugins/{first_plugin_name}"))
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
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Pinned pages"), "html: {html}");
    assert!(html.contains("Recently visited"), "html: {html}");
}

#[test]
fn plugin_workspace_template_includes_quick_navigation_blocks() {
    let template_path = templates_root()
        .join("admin")
        .join("fragments")
        .join("plugin_workspace_content.html");
    let html =
        fs::read_to_string(&template_path).expect("failed to read plugin workspace template");

    assert!(
        html.contains("data-plugin-workspace-nav"),
        "template missing workspace nav block marker: {}",
        template_path.display()
    );
    assert!(
        html.contains("Pinned pages"),
        "template missing pinned navigation heading: {}",
        template_path.display()
    );
    assert!(
        html.contains("Recently visited"),
        "template missing recent navigation heading: {}",
        template_path.display()
    );
}

#[test]
fn plugins_rows_template_opens_workspace_in_tab_when_available() {
    let template_path = templates_root()
        .join("admin")
        .join("partials")
        .join("plugins_rows.html");
    let html = fs::read_to_string(&template_path).expect("failed to read plugins rows template");

    assert!(
        html.contains("data-workspace-path"),
        "plugins rows should include workspace path dataset: {}",
        template_path.display()
    );
    assert!(
        html.contains("@click.prevent=\"openWorkspace("),
        "plugins rows should use openWorkspace handler for tab navigation: {}",
        template_path.display()
    );
}

#[tokio::test]
async fn viewer_cannot_access_users_workspace_without_permission() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn viewer_cannot_access_plugin_workspace_without_plugins_view_permission() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/plugins/fake-plugin")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn viewer_cannot_access_plugin_pages_api_without_plugins_view_permission() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/plugins/does-not-exist/pages")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_can_crud_permissions_via_partials() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/permissions/create")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "slug=audit.events.view&name=View+Audit+Events&module=audit&description=Read+audit+events",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(create_response.status(), StatusCode::OK);

    let table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(table_response.status(), StatusCode::OK);
    let table_body = to_bytes(table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let table_html = String::from_utf8_lossy(&table_body);
    assert!(
        table_html.contains("audit.events.view"),
        "permissions html: {table_html}"
    );
    let permission_id = extract_dataset_id_by_slug(
        &table_html,
        "data-permission-slug",
        "audit.events.view",
        "data-permission-id",
    )
    .expect("permission id should be discoverable");

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/partials/permissions/{permission_id}/update"
                ))
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=View+Audit+Trail&module=audit&description=Read+audit+trail+events",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(update_response.status(), StatusCode::OK);

    let updated_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let updated_table_body = to_bytes(updated_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let updated_table_html = String::from_utf8_lossy(&updated_table_body);
    assert!(
        updated_table_html.contains("View Audit Trail"),
        "updated permissions html: {updated_table_html}"
    );

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/partials/permissions/{permission_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(delete_response.status(), StatusCode::OK);

    let final_table_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let final_table_body = to_bytes(final_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let final_table_html = String::from_utf8_lossy(&final_table_body);
    assert!(
        !final_table_html.contains("audit.events.view"),
        "final permissions html: {final_table_html}"
    );
}

#[tokio::test]
async fn admin_can_crud_roles_and_assign_permissions() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let create_role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/roles/create")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "slug=auditor&name=Auditor&description=Read+audit+operations",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(create_role_response.status(), StatusCode::OK);

    let roles_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/roles/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(roles_table_response.status(), StatusCode::OK);
    let roles_table_body = to_bytes(roles_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let roles_table_html = String::from_utf8_lossy(&roles_table_body);
    let role_id = extract_dataset_id_by_slug(
        &roles_table_html,
        "data-role-slug",
        "auditor",
        "data-role-id",
    )
    .expect("role id should be discoverable");

    let permissions_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let permissions_table_body = to_bytes(permissions_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let permissions_table_html = String::from_utf8_lossy(&permissions_table_body);
    let logs_view_permission_id = extract_dataset_id_by_slug(
        &permissions_table_html,
        "data-permission-slug",
        "logs.view",
        "data-permission-id",
    )
    .expect("logs.view permission id should be discoverable");

    let assign_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/partials/roles/{role_id}/permissions"))
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "permission_ids={logs_view_permission_id}"
                )))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(assign_response.status(), StatusCode::OK);

    let permission_form_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/partials/roles/{role_id}/permissions/form"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(permission_form_response.status(), StatusCode::OK);
    let permission_form_body = to_bytes(permission_form_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let permission_form_html = String::from_utf8_lossy(&permission_form_body);
    assert!(
        permission_form_html.contains(&format!(r#"value="{logs_view_permission_id}""#)),
        "permissions form html: {permission_form_html}"
    );
    assert!(
        permission_form_html.contains("checked"),
        "permissions form should include checked assignment: {permission_form_html}"
    );

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/partials/roles/{role_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(delete_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn custom_role_tokens_follow_permission_matrix() {
    let (app, ctx) = build_app_with_context(None).await;
    let admin = admin_bearer_token();

    let create_role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/roles/create")
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "slug=auditor&name=Auditor&description=Read+only+auditor",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(create_role_response.status(), StatusCode::OK);

    let roles_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/roles/table")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let roles_table_body = to_bytes(roles_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let roles_table_html = String::from_utf8_lossy(&roles_table_body);
    let role_id = extract_dataset_id_by_slug(
        &roles_table_html,
        "data-role-slug",
        "auditor",
        "data-role-id",
    )
    .expect("role id should be discoverable");

    let permissions_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let permissions_table_body = to_bytes(permissions_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let permissions_table_html = String::from_utf8_lossy(&permissions_table_body);
    let users_view_permission_id = extract_dataset_id_by_slug(
        &permissions_table_html,
        "data-permission-slug",
        "users.view",
        "data-permission-id",
    )
    .expect("users.view permission id should be discoverable");

    let assign_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/partials/roles/{role_id}/permissions"))
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "permission_ids={users_view_permission_id}"
                )))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(assign_response.status(), StatusCode::OK);
    refresh_admin_authorizer(&ctx).await;

    let auditor = bearer_token_for_role("auditor");
    let can_view_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/users")
                .header("authorization", format!("Bearer {auditor}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(can_view_users.status(), StatusCode::OK);

    let cannot_create_users = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/users/create")
                .header("authorization", format!("Bearer {auditor}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "username=audited_user&email=audited@example.com&password=password123&role=viewer",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(cannot_create_users.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn custom_role_menus_permissions_are_enforced() {
    let (app, ctx) = build_app_with_context(None).await;
    let admin = admin_bearer_token();

    let create_role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/roles/create")
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "slug=menu_operator&name=Menu+Operator&description=Menu+catalog+operator",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(create_role_response.status(), StatusCode::OK);

    let roles_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/roles/table")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let roles_table_body = to_bytes(roles_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let roles_table_html = String::from_utf8_lossy(&roles_table_body);
    let role_id = extract_dataset_id_by_slug(
        &roles_table_html,
        "data-role-slug",
        "menu_operator",
        "data-role-id",
    )
    .expect("role id should be discoverable");

    let permissions_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/partials/permissions/table")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    let permissions_table_body = to_bytes(permissions_table_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let permissions_table_html = String::from_utf8_lossy(&permissions_table_body);
    let menus_view_permission_id = extract_dataset_id_by_slug(
        &permissions_table_html,
        "data-permission-slug",
        "menus.view",
        "data-permission-id",
    )
    .expect("menus.view permission id should be discoverable");
    let menus_manage_permission_id = extract_dataset_id_by_slug(
        &permissions_table_html,
        "data-permission-slug",
        "menus.manage",
        "data-permission-id",
    )
    .expect("menus.manage permission id should be discoverable");

    let assign_view_only_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/partials/roles/{role_id}/permissions"))
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "permission_ids={menus_view_permission_id}"
                )))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(assign_view_only_response.status(), StatusCode::OK);
    refresh_admin_authorizer(&ctx).await;

    let menu_operator = bearer_token_for_role("menu_operator");

    let can_view_menus_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/menus")
                .header("authorization", format!("Bearer {menu_operator}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(can_view_menus_page.status(), StatusCode::OK);

    let can_view_menu_api = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
                .header("authorization", format!("Bearer {menu_operator}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(can_view_menu_api.status(), StatusCode::OK);

    let cannot_create_menu_without_manage = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/menus/create")
                .header("authorization", format!("Bearer {menu_operator}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "label=Ops+Menu&icon=settings&route=%2Fadmin%2Fops&position=81",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(
        cannot_create_menu_without_manage.status(),
        StatusCode::FORBIDDEN
    );

    let assign_view_and_manage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/partials/roles/{role_id}/permissions"))
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "permission_ids={menus_manage_permission_id}"
                )))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(assign_view_and_manage_response.status(), StatusCode::OK);
    refresh_admin_authorizer(&ctx).await;

    let can_create_menu_with_manage = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/menus/create")
                .header("authorization", format!("Bearer {menu_operator}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "label=Ops+Menu&icon=settings&route=%2Fadmin%2Fops&position=81",
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(can_create_menu_with_manage.status(), StatusCode::OK);
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
    let path = workspace_root()
        .join("plugins")
        .join("official")
        .join("kv-store")
        .join("web")
        .join("templates")
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
