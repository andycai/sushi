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
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::lua::vm::create_sandboxed_vm;
use sushi_core::plugin::manager::PageResolvedAssets;
use sushi_core::plugin::Plugin;
use sushi_core::runtime::{
    MenuContributionSpec, PluginInstanceId, ResolvedRuntimeEntry, RuntimePluginSource,
    StaticRootSpec,
};
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
const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
const PLUGIN_GOVERNANCE_MIGRATION_SQL: &str =
    include_str!("../../../migrations/008_plugin_governance_v1.sql");
const PLUGIN_GOVERNANCE_MIGRATION_NAME: &str = "008_plugin_governance_v1";
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
            method: "PATCH".to_string(),
            path_pattern: "/admin/api/plugins/{plugin}/state".to_string(),
            policy_key: "admin.plugins.manage".to_string(),
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

async fn run_plugin_governance_migration_if_needed(storage: &SqliteStorage) {
    let rows = storage
        .query(
            "SELECT 1 AS found FROM _sushi_migrations WHERE name = ?1 LIMIT 1",
            vec![Value::String(PLUGIN_GOVERNANCE_MIGRATION_NAME.to_string())],
        )
        .await
        .expect("failed to query migration 008_plugin_governance_v1 state");
    if rows.is_empty() {
        storage
            .run_migrations(PLUGIN_GOVERNANCE_MIGRATION_SQL)
            .await
            .expect("failed to run migration 008_plugin_governance_v1");
    }
}

#[tokio::test]
async fn plugin_governance_migration_helper_skips_when_already_applied() {
    let storage = SqliteStorage::new_in_memory()
        .await
        .expect("failed to init sqlite");
    storage
        .run_migrations(MIGRATION_SQL)
        .await
        .expect("failed to run migration 001_init");
    storage
        .execute(
            "INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (8, '008_plugin_governance_v1')",
            vec![],
        )
        .await
        .expect("failed to seed migration 008 marker");

    run_plugin_governance_migration_if_needed(&storage).await;

    let columns = storage
        .query("PRAGMA table_info(plugin_state)", vec![])
        .await
        .expect("failed to query plugin_state columns");
    let has_plugin_id = columns
        .iter()
        .any(|column| column.get("name").and_then(Value::as_str) == Some("plugin_id"));

    assert!(
        !has_plugin_id,
        "helper should skip applying migration SQL when marker is already present"
    );
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
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .expect("failed to run migration 007_cms");
    run_plugin_governance_migration_if_needed(&storage).await;
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    sushi_admin::builtin::activate_admin_shell(&ctx, &admin_shell_runtime_entry())
        .await
        .expect("Admin Shell builtin activation succeeds");
    sushi_admin::builtin::activate_host_admin(&ctx, &host_admin_runtime_entry())
        .await
        .expect("Host Admin builtin activation succeeds");
    sushi_admin::builtin::activate_governance(&ctx, &governance_runtime_entry())
        .await
        .expect("Governance builtin activation succeeds");
    sushi_admin::builtin::activate_rbac_admin(&ctx, &rbac_admin_runtime_entry())
        .await
        .expect("RBAC Admin builtin activation succeeds");
    sushi_admin::builtin::activate_menu_admin(&ctx, &menu_admin_runtime_entry())
        .await
        .expect("Menu Admin builtin activation succeeds");
    (build_admin_router(&ctx).await, ctx)
}

async fn build_app(static_url_prefix: Option<&str>) -> axum::Router {
    let (app, _ctx) = build_app_with_context(static_url_prefix).await;
    app
}

fn host_admin_runtime_entry() -> ResolvedRuntimeEntry {
    ResolvedRuntimeEntry {
        id: PluginInstanceId::new("host.admin").expect("host Admin entry ID is valid"),
        source: RuntimePluginSource::Builtin {
            key: "host-admin".to_string(),
            reference: "builtin:host-admin".to_string(),
        },
        enabled: true,
        required: true,
        config: serde_json::json!({}),
        grants: serde_json::json!({}),
        origin: "test".to_string(),
    }
}

fn admin_shell_runtime_entry() -> ResolvedRuntimeEntry {
    ResolvedRuntimeEntry {
        id: PluginInstanceId::new("admin.shell").expect("Admin Shell entry ID is valid"),
        source: RuntimePluginSource::Builtin {
            key: "admin-shell".to_string(),
            reference: "builtin:admin-shell".to_string(),
        },
        enabled: true,
        required: true,
        config: serde_json::json!({}),
        grants: serde_json::json!({}),
        origin: "test".to_string(),
    }
}

fn rbac_admin_runtime_entry() -> ResolvedRuntimeEntry {
    ResolvedRuntimeEntry {
        id: PluginInstanceId::new("rbac.admin").expect("RBAC Admin entry ID is valid"),
        source: RuntimePluginSource::Builtin {
            key: "rbac-admin".to_string(),
            reference: "builtin:rbac-admin".to_string(),
        },
        enabled: true,
        required: true,
        config: serde_json::json!({}),
        grants: serde_json::json!({}),
        origin: "test".to_string(),
    }
}

fn governance_runtime_entry() -> ResolvedRuntimeEntry {
    ResolvedRuntimeEntry {
        id: PluginInstanceId::new("governance.admin").expect("Governance entry ID is valid"),
        source: RuntimePluginSource::Builtin {
            key: "governance".to_string(),
            reference: "builtin:governance".to_string(),
        },
        enabled: true,
        required: true,
        config: serde_json::json!({}),
        grants: serde_json::json!({}),
        origin: "test".to_string(),
    }
}

fn menu_admin_runtime_entry() -> ResolvedRuntimeEntry {
    ResolvedRuntimeEntry {
        id: PluginInstanceId::new("menu.admin").expect("Menu Admin entry ID is valid"),
        source: RuntimePluginSource::Builtin {
            key: "menu-admin".to_string(),
            reference: "builtin:menu-admin".to_string(),
        },
        enabled: true,
        required: true,
        config: serde_json::json!({}),
        grants: serde_json::json!({}),
        origin: "test".to_string(),
    }
}

async fn build_app_with_host_admin() -> (axum::Router, SushiContext) {
    let (_, ctx) = build_app_with_context(None).await;
    sushi_admin::builtin::activate_host_admin(&ctx, &host_admin_runtime_entry())
        .await
        .expect("host Admin builtin activation succeeds");
    (build_admin_router(&ctx).await, ctx)
}

async fn build_app_with_rbac_admin() -> (axum::Router, SushiContext) {
    let (_, ctx) = build_app_with_context(None).await;
    sushi_admin::builtin::activate_host_admin(&ctx, &host_admin_runtime_entry())
        .await
        .expect("host Admin builtin activation succeeds");
    (build_admin_router(&ctx).await, ctx)
}

async fn build_app_with_cms_plugin_loaded(static_url_prefix: Option<&str>) -> axum::Router {
    let templates_dir = templates_root();
    let static_dir = static_root();
    let plugins_dir = workspace_root().join("plugins");

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
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .expect("failed to run migration 007_cms");
    run_plugin_governance_migration_if_needed(&storage).await;
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);

    let mut plugins = LuaPlugin::scan_dir(&plugins_dir)
        .await
        .expect("failed to scan plugins")
        .into_iter()
        .filter(|plugin| plugin.name() == "cms")
        .collect::<Vec<_>>();
    assert_eq!(plugins.len(), 1, "expected exactly one cms plugin");

    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");
    let ctx = SushiContext::new(config, storage, jwt, templates);
    sushi_admin::builtin::activate_admin_shell(&ctx, &admin_shell_runtime_entry())
        .await
        .expect("Admin Shell builtin activation succeeds");

    for plugin in plugins.drain(..) {
        let plugin_name = plugin.name().to_string();
        ctx.plugins
            .register_plugin_manifest_with_permissions(
                plugin.manifest(),
                plugin.effective_permissions(),
            )
            .await;
        plugin
            .init(&ctx)
            .await
            .unwrap_or_else(|err| panic!("failed to init cms plugin: {err}"));
        if let Some(lua) = plugin.into_vm() {
            ctx.plugins.register_vm(&plugin_name, lua).await;
        }
    }

    refresh_admin_authorizer(&ctx).await;
    build_admin_router(&ctx).await
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
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .expect("failed to run migration 007_cms");
    run_plugin_governance_migration_if_needed(&storage).await;
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
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .expect("failed to run migration 007_cms");
    run_plugin_governance_migration_if_needed(&storage).await;
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    sushi_admin::builtin::activate_admin_shell(&ctx, &admin_shell_runtime_entry())
        .await
        .expect("Admin Shell builtin activation succeeds");
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

async fn build_app_with_plugin_admin_page(page_path: &str) -> axum::Router {
    let (app, _ctx) = build_app_with_plugin_admin_page_and_context(page_path).await;
    app
}

async fn build_app_with_plugin_admin_page_and_context(
    page_path: &str,
) -> (axum::Router, SushiContext) {
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
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .expect("failed to run migration 007_cms");
    run_plugin_governance_migration_if_needed(&storage).await;
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    sushi_admin::builtin::activate_admin_shell(&ctx, &admin_shell_runtime_entry())
        .await
        .expect("Admin Shell builtin activation succeeds");

    let lua = create_sandboxed_vm().expect("failed to create sandboxed vm");
    let sushi = lua.create_table().expect("failed to create sushi table");
    let handlers = lua.create_table().expect("failed to create handlers table");
    sushi
        .set("__handlers", handlers)
        .expect("failed to set handlers table");
    lua.globals()
        .set("sushi", sushi)
        .expect("failed to set sushi global");
    lua.load(
        r#"
        sushi.__handlers["handler::cms_page"] = function(_args)
            return "<section>CMS workspace</section>"
        end
        "#,
    )
    .exec()
    .expect("failed to register cms admin handler");

    ctx.plugins.register_vm("cms", lua).await;
    ctx.plugins
        .register_admin_handler_with_assets(
            page_path,
            "cms",
            "CMS",
            "handler::cms_page",
            PageResolvedAssets::default(),
        )
        .await;

    (build_admin_router(&ctx).await, ctx)
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
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .expect("failed to run migration 007_cms");
    run_plugin_governance_migration_if_needed(&storage).await;
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates = TemplateService::new(&templates_dir).expect("failed to init template service");

    let ctx = SushiContext::new(config, storage, jwt, templates);
    refresh_admin_authorizer(&ctx).await;
    sushi_admin::builtin::activate_host_admin(&ctx, &host_admin_runtime_entry())
        .await
        .expect("Host Admin builtin activation succeeds");
    sushi_admin::builtin::activate_rbac_admin(&ctx, &rbac_admin_runtime_entry())
        .await
        .expect("RBAC Admin builtin activation succeeds");
    sushi_admin::builtin::activate_menu_admin(&ctx, &menu_admin_runtime_entry())
        .await
        .expect("Menu Admin builtin activation succeeds");
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
async fn admin_cms_workspace_page_renders() {
    let app = build_app_with_plugin_admin_page("/admin/cms").await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/cms")
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
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("CMS workspace"));
}

#[tokio::test]
async fn admin_router_discovers_and_removes_pages_after_router_build() {
    let (app, ctx) = build_app_with_context(None).await;
    let lua = create_sandboxed_vm().expect("failed to create sandboxed vm");
    let sushi = lua.create_table().expect("failed to create sushi table");
    let handlers = lua.create_table().expect("failed to create handlers table");
    sushi
        .set("__handlers", handlers.clone())
        .expect("failed to set handlers table");
    lua.globals()
        .set("sushi", sushi)
        .expect("failed to set sushi global");
    let handler = lua
        .create_async_function(|_, ()| async { Ok("<section>Dynamic admin</section>".to_string()) })
        .expect("failed to create handler");
    handlers
        .set("handler::dynamic_admin", handler)
        .expect("failed to register handler");
    ctx.plugins.register_vm("dynamic-admin", lua).await;
    ctx.plugins
        .register_admin_handler_with_assets(
            "/admin/dynamic",
            "dynamic-admin",
            "Dynamic Admin",
            "handler::dynamic_admin",
            PageResolvedAssets::default(),
        )
        .await;

    let request = || {
        Request::builder()
            .uri("/admin/dynamic")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", admin_bearer_token()),
            )
            .body(Body::empty())
            .expect("failed to build request")
    };
    let response = app
        .clone()
        .oneshot(request())
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    ctx.plugins
        .remove_owner_capabilities(&PluginInstanceId::legacy("dynamic-admin"))
        .await;
    let response = app.oneshot(request()).await.expect("request failed");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_router_discovers_and_removes_http_routes_after_router_build() {
    let (app, ctx) = build_app_with_context(None).await;
    let lua = create_sandboxed_vm().expect("failed to create sandboxed vm");
    let sushi = lua.create_table().expect("failed to create sushi table");
    let handlers = lua.create_table().expect("failed to create handlers table");
    sushi
        .set("__handlers", handlers)
        .expect("failed to set handlers table");
    lua.globals()
        .set("sushi", sushi)
        .expect("failed to set sushi global");
    lua.load(
        r#"
        sushi.__handlers["handler::dynamic_partial"] = function(args)
            local body = args[2] or ""
            return string.format(
                '{"__sushi_web_json":true,"status":202,"body":{"dispatch_path":"%s","body_size":%d}}',
                args.dispatch_path or "",
                string.len(body)
            )
        end
        "#,
    )
    .exec()
    .expect("failed to register dynamic partial handler");
    ctx.plugins.register_vm("dynamic-partial", lua).await;
    ctx.plugins
        .register_api_handler(
            "POST",
            "/admin/partials/dynamic",
            "dynamic-partial",
            "handler::dynamic_partial",
        )
        .await;

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/dynamic?mode=full")
                .body(Body::from(vec![0xff, 0x00, b'a']))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(unauthenticated.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        unauthenticated.headers().get(header::LOCATION),
        Some(&header::HeaderValue::from_static("/admin-login"))
    );

    let request = || {
        Request::builder()
            .method("POST")
            .uri("/admin/partials/dynamic?mode=full")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", admin_bearer_token()),
            )
            .body(Body::from(vec![0xff, 0x00, b'a']))
            .expect("failed to build request")
    };
    let response = app
        .clone()
        .oneshot(request())
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static("application/json"))
    );
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("failed to read response body");
    let payload: Value = serde_json::from_slice(&body).expect("invalid response json");
    assert_eq!(
        payload.get("dispatch_path").and_then(Value::as_str),
        Some("/admin/partials/dynamic?mode=full")
    );
    assert_eq!(payload.get("body_size").and_then(Value::as_u64), Some(3));

    ctx.plugins
        .remove_owner_capabilities(&PluginInstanceId::legacy("dynamic-partial"))
        .await;
    let response = app.oneshot(request()).await.expect("request failed");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn host_admin_routes_take_precedence_over_dynamic_http_routes() {
    let (app, ctx) = build_app_with_host_admin().await;
    let lua = create_sandboxed_vm().expect("failed to create sandboxed vm");
    let sushi = lua.create_table().expect("failed to create sushi table");
    let handlers = lua.create_table().expect("failed to create handlers table");
    sushi
        .set("__handlers", handlers.clone())
        .expect("failed to set handlers table");
    lua.globals()
        .set("sushi", sushi)
        .expect("failed to set sushi global");
    handlers
        .set(
            "handler::shadow",
            lua.create_async_function(|_, ()| async { Ok("plugin-shadow".to_string()) })
                .expect("failed to create shadow handler"),
        )
        .expect("failed to register shadow handler");
    ctx.plugins.register_vm("dynamic-shadow", lua).await;
    ctx.plugins
        .register_api_handler(
            "GET",
            "/admin/api/plugins",
            "dynamic-shadow",
            "handler::shadow",
        )
        .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/plugins")
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
    let body = to_bytes(response.into_body(), 16 * 1024)
        .await
        .expect("failed to read response body");
    assert_ne!(body.as_ref(), b"plugin-shadow");
    serde_json::from_slice::<Value>(&body).expect("host route should return json");
}

#[tokio::test]
async fn admin_cms_workspace_page_includes_plugin_assets() {
    let app = build_app_with_cms_plugin_loaded(None).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/cms")
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
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("/static/plugins/official/cms/cms.js"));
    assert!(html.contains("/static/plugins/official/cms/cms.css"));
}

#[tokio::test]
async fn plugin_admin_page_returns_forbidden_when_plugin_disabled() {
    let (app, ctx) = build_app_with_plugin_admin_page_and_context("/admin/cms").await;
    ctx.plugins
        .set_plugin_enabled("cms", false, Some("admin"), Some("disabled by test"))
        .await
        .expect("failed to disable cms plugin");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/cms")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", admin_bearer_token()),
                )
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[test]
fn admin_cms_category_delete_returns_flash_on_conflict() {
    let source = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/lua/interfaces/admin.lua"),
    )
    .expect("failed to read cms admin interface");

    assert!(source.contains("conflict_has_posts"));
    assert!(source.contains("Category still has posts and cannot be deleted"));
    assert!(source.contains("plugins/official/cms/fragments/flash.html"));
}

#[test]
fn admin_cms_template_uses_top_nav_and_panel_mounts() {
    let source = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/cms.html"),
    )
    .expect("failed to read cms template");

    assert!(source.contains("data-cms-top-nav"));
    assert!(source.contains("data-cms-panel=\"overview\""));
    assert!(source.contains("data-cms-panel=\"library\""));
    assert!(source.contains("data-cms-panel=\"editor\""));
    assert!(source.contains("id=\"cms-toast-stack\""));
}

#[test]
fn cms_js_defines_shortcuts_and_command_palette_hooks() {
    let source =
        std::fs::read_to_string(workspace_root().join("plugins/official/cms/web/static/cms.js"))
            .expect("failed to read cms.js");

    assert!(source.contains("Cmd/Ctrl+K"));
    assert!(source.contains("switchPanel"));
    assert!(source.contains("openCommandPalette"));
    assert!(source.contains("handleGlobalShortcut"));
    assert!(source.contains("const typingTarget = isTypingTarget(event.target);"));
    assert!(source.contains("if (!typingTarget && this.handleGotoSequence(event))"));
    assert!(source.contains("handleFeedbackResponse"));
    assert!(source.contains("showToast"));
    assert!(source.contains("previousSlug === '' || previousSlug !== nextSlug"));
    assert!(source.contains("initMarkdownEditors"));
    assert!(source.contains("data-cms-md-action"));
}

#[test]
fn cms_template_wires_overview_library_editor_endpoints() {
    let source = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/cms.html"),
    )
    .expect("failed to read cms template");

    assert!(source.contains("/admin/partials/cms/overview"));
    assert!(source.contains("/admin/partials/cms/library/posts"));
    assert!(source.contains("/admin/partials/cms/editor/posts/new"));
    assert!(source.contains("/admin/partials/cms/editor/save"));
    assert!(source.contains("/admin/partials/cms/status/transition"));
    assert!(source.contains("/admin/partials/cms/commands"));
}

#[test]
fn cms_editor_and_row_templates_expose_preview_and_markdown_helpers() {
    let editor = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/fragments/editor_panel.html"),
    )
    .expect("failed to read cms editor panel");
    let pages_rows = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/fragments/page_rows.html"),
    )
    .expect("failed to read page rows template");
    let posts_rows = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/fragments/post_rows.html"),
    )
    .expect("failed to read post rows template");

    assert!(editor.contains("data-cms-markdown-helper"));
    assert!(editor.contains("data-cms-md-action=\"preview\""));
    assert!(editor.contains("href=\"/app/pages/{{ item.slug }}\""));
    assert!(editor.contains("href=\"/admin/preview/cms/pages/{{ item.slug }}\""));
    assert!(editor.contains("href=\"/app/posts/{{ item.slug }}\""));
    assert!(editor.contains("href=\"/admin/preview/cms/posts/{{ item.slug }}\""));
    assert!(pages_rows.contains("href=\"/app/pages/{{ item.slug }}\""));
    assert!(pages_rows.contains("href=\"/admin/preview/cms/pages/{{ item.slug }}\""));
    assert!(pages_rows.contains("Preview"));
    assert!(posts_rows.contains("href=\"/app/posts/{{ item.slug }}\""));
    assert!(posts_rows.contains("href=\"/admin/preview/cms/posts/{{ item.slug }}\""));
    assert!(posts_rows.contains("Preview"));
}

#[tokio::test]
async fn role_permission_updates_refresh_authorizer_for_workspace_asset_checks() {
    let (app, _ctx) = build_app_with_context(None).await;
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

async fn register_test_plugin(ctx: &SushiContext, plugin_name: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sushi-admin-toggle-plugin-{}-{unique}",
        std::process::id()
    ));
    let plugin_dir = root.join("third_party").join(plugin_name);
    fs::create_dir_all(&plugin_dir).expect("failed to create toggle plugin directory");
    fs::write(
        plugin_dir.join("plugin.toml"),
        format!(
            r#"
[plugin]
name = "{plugin_name}"
version = "1.0.0"
kind = "third_party"
entry = "init.lua"

[permissions]
routes = true
"#
        ),
    )
    .expect("failed to write toggle plugin manifest");
    fs::write(
        plugin_dir.join("init.lua"),
        format!(
            r#"
sushi.api.route("GET", "/api/{plugin_name}", function()
    return "active"
end)
"#
        ),
    )
    .expect("failed to write toggle plugin source");

    let plugin = LuaPlugin::scan_dir(&root)
        .await
        .expect("failed to scan toggle plugin")
        .remove(0);
    ctx.plugins
        .register_plugin_manifest_with_permissions_and_identity(
            plugin.manifest(),
            plugin.effective_permissions(),
            plugin.path_id(),
            plugin.kind(),
        )
        .await;
    ctx.runtime_host.register_lua_source(&plugin, false).await;
    ctx.runtime_host
        .activate(ctx, plugin_name)
        .await
        .expect("failed to activate toggle plugin");
    root
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
                .uri("/static/js/alpine.min.js")
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
async fn plugin_static_assets_follow_runtime_owner_lifecycle() {
    let plugin_static_dir = std::env::temp_dir().join(format!(
        "sushi-admin-plugin-static-dynamic-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&plugin_static_dir).expect("failed to create plugin static tempdir");
    fs::write(
        plugin_static_dir.join("dynamic.js"),
        "window.__dynamicPluginStaticLoaded = true;",
    )
    .expect("failed to write plugin static asset");

    let (app, ctx) = build_app_with_context(None).await;
    let asset_uri = "/static/plugins/official/dynamic/dynamic.js";
    let request = || {
        Request::builder()
            .uri(asset_uri)
            .body(Body::empty())
            .expect("failed to build request")
    };

    let response = app
        .clone()
        .oneshot(request())
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let owner = PluginInstanceId::new("dynamic.default").unwrap();
    let registry = ctx.plugins.capability_registry();
    let mut staged = registry.stage(owner.clone());
    staged.register_static_root(StaticRootSpec::new(
        "official/dynamic",
        plugin_static_dir.clone(),
    ));
    registry.commit(staged).await.unwrap();

    let response = app
        .clone()
        .oneshot(request())
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    registry.remove_owner(&owner).await;
    let response = app.oneshot(request()).await.expect("request failed");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    fs::remove_dir_all(plugin_static_dir).expect("failed to clean plugin static tempdir");
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
async fn root_path_redirects_to_admin_root() {
    let app = build_app(None).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
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
    assert_eq!(location, Some("/admin/"));
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
                .uri("/assets/js/alpine.min.js")
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
    let (app, _) = build_app_with_host_admin().await;
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
    let (app, _) = build_app_with_host_admin().await;
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
    let (app, _) = build_app_with_host_admin().await;
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
async fn plugin_inspection_builtin_matches_static_read_only_responses() {
    let (app, ctx) = build_app_with_host_admin().await;
    let static_router = axum::Router::new()
        .route(
            "/admin/plugins",
            axum::routing::get(sushi_admin::routes::plugins::plugins_page),
        )
        .route(
            "/admin/plugins/{plugin}",
            axum::routing::get(sushi_admin::routes::plugins::plugin_workspace_page),
        )
        .route(
            "/admin/partials/plugins/table",
            axum::routing::get(sushi_admin::routes::plugins::plugins_table_partial),
        )
        .route(
            "/admin/api/plugins",
            axum::routing::get(sushi_admin::routes::plugins::plugins_api),
        )
        .route(
            "/admin/api/plugins/{plugin}/pages",
            axum::routing::get(sushi_admin::routes::plugins::plugin_pages_api),
        )
        .with_state(ctx);
    let token = admin_bearer_token();

    for path in [
        "/admin/plugins",
        "/admin/plugins/host-admin",
        "/admin/partials/plugins/table",
        "/admin/api/plugins",
        "/admin/api/plugins/host-admin/pages",
        "/admin/plugins/does-not-exist",
        "/admin/api/plugins/does-not-exist/pages",
    ] {
        let static_response = static_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("failed to build static request"),
            )
            .await
            .expect("static request failed");
        let builtin_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build builtin request"),
            )
            .await
            .expect("builtin request failed");

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read static body");
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read builtin body");
        assert_eq!(builtin_body, static_body, "path: {path}");
    }
}

#[tokio::test]
async fn rbac_admin_registers_owner_scoped_read_capabilities() {
    let (_, ctx) = build_app_with_rbac_admin().await;
    let snapshot = ctx.plugins.capability_snapshot().await;

    for path in ["/admin/users", "/admin/roles", "/admin/permissions"] {
        let registration = snapshot
            .admin_page(path)
            .unwrap_or_else(|| panic!("Admin page {path} should be registered"));
        assert_eq!(registration.owner.as_str(), "rbac.admin");
    }

    for (method, path, policy) in [
        ("GET", "/admin/partials/users/table", "admin.users.view"),
        ("POST", "/admin/partials/users/create", "admin.users.manage"),
        ("DELETE", "/admin/partials/users/{id}", "admin.users.manage"),
        ("GET", "/admin/partials/roles/table", "admin.roles.view"),
        ("POST", "/admin/partials/roles/create", "admin.roles.manage"),
        (
            "POST",
            "/admin/partials/roles/{id}/update",
            "admin.roles.manage",
        ),
        (
            "GET",
            "/admin/partials/roles/{id}/permissions/form",
            "admin.roles.view",
        ),
        (
            "POST",
            "/admin/partials/roles/{id}/permissions",
            "admin.roles.manage",
        ),
        ("DELETE", "/admin/partials/roles/{id}", "admin.roles.manage"),
        (
            "GET",
            "/admin/partials/permissions/table",
            "admin.permissions.view",
        ),
        (
            "POST",
            "/admin/partials/permissions/create",
            "admin.permissions.manage",
        ),
        (
            "POST",
            "/admin/partials/permissions/{id}/update",
            "admin.permissions.manage",
        ),
        (
            "DELETE",
            "/admin/partials/permissions/{id}",
            "admin.permissions.manage",
        ),
    ] {
        let registration = snapshot
            .match_http_on(sushi_core::runtime::HttpSurface::Admin, method, path)
            .unwrap_or_else(|| panic!("Admin route {path} should be registered"));
        assert_eq!(registration.owner.as_str(), "rbac.admin");
        assert_eq!(registration.value.policy_key.as_deref(), Some(policy));
    }

    for id in [
        "rbac-admin.users",
        "rbac-admin.roles",
        "rbac-admin.permissions",
    ] {
        let registration = snapshot
            .menu_contributions()
            .iter()
            .find(|registration| registration.value.id == id)
            .unwrap_or_else(|| panic!("menu contribution {id} should be registered"));
        assert_eq!(registration.owner.as_str(), "rbac.admin");
        assert_eq!(
            registration.value.parent_id.as_deref(),
            Some("host-admin.system")
        );
    }
}

#[tokio::test]
async fn rbac_admin_builtin_matches_static_read_only_responses() {
    let (app, ctx) = build_app_with_rbac_admin().await;
    let static_router = axum::Router::new()
        .route(
            "/admin/users",
            axum::routing::get(sushi_admin::routes::users::users_page),
        )
        .route(
            "/admin/roles",
            axum::routing::get(sushi_admin::routes::roles::roles_page),
        )
        .route(
            "/admin/permissions",
            axum::routing::get(sushi_admin::routes::permissions::permissions_page),
        )
        .route(
            "/admin/partials/users/table",
            axum::routing::get(sushi_admin::routes::users::users_table_partial),
        )
        .route(
            "/admin/partials/roles/table",
            axum::routing::get(sushi_admin::routes::roles::roles_table_partial),
        )
        .route(
            "/admin/partials/permissions/table",
            axum::routing::get(sushi_admin::routes::permissions::permissions_table_partial),
        )
        .with_state(ctx);
    let token = admin_bearer_token();

    for path in [
        "/admin/users",
        "/admin/roles",
        "/admin/permissions",
        "/admin/partials/users/table",
        "/admin/partials/roles/table",
        "/admin/partials/permissions/table",
    ] {
        let static_response = static_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("failed to build static request"),
            )
            .await
            .expect("static request failed");
        let builtin_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build builtin request"),
            )
            .await
            .expect("builtin request failed");

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read static body");
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read builtin body");
        assert_eq!(builtin_body, static_body, "path: {path}");
    }
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

    let system_plugins_entry = menu.iter().find(|item| {
        item.get("route").and_then(Value::as_str) == Some("/admin/plugins")
            && item.get("parent_id").and_then(Value::as_i64) == Some(system_id)
    });
    assert!(
        system_plugins_entry.is_some(),
        "Plugins management entry should exist under System menu"
    );
}

#[tokio::test]
async fn menu_api_projects_runtime_contribution_hierarchy() {
    let (app, ctx) = build_app_with_context(None).await;
    let mut staged = ctx
        .plugins
        .stage_builtin_activation(PluginInstanceId::new("notes.default").unwrap());
    staged.register_menu(
        MenuContributionSpec::new("notes.root", "Notes", 80)
            .with_icon(Some("notebook".to_string())),
    );
    staged.register_menu(
        MenuContributionSpec::new("notes.items", "Note Items", 81)
            .with_parent(Some("notes.root".to_string()))
            .with_route(Some("/admin/notes".to_string())),
    );
    ctx.plugins
        .prepare_owner_activation(staged)
        .await
        .unwrap()
        .publish()
        .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
                .header("authorization", format!("Bearer {}", admin_bearer_token()))
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
    let parent = menu
        .iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some("Notes"))
        .expect("runtime parent contribution should be projected");
    let parent_id = parent
        .get("id")
        .and_then(Value::as_i64)
        .expect("runtime parent id should exist");
    let child = menu
        .iter()
        .find(|item| item.get("route").and_then(Value::as_str) == Some("/admin/notes"))
        .expect("runtime child contribution should be projected");
    assert_eq!(
        child.get("parent_id").and_then(Value::as_i64),
        Some(parent_id)
    );
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

    let legacy_system_plugins_entry = menu.iter().find(|item| {
        item.get("route").and_then(Value::as_str) == Some("/admin/plugins")
            && item.get("parent_id").and_then(Value::as_i64) == Some(system_id)
    });
    assert!(
        legacy_system_plugins_entry.is_some(),
        "legacy schema should backfill a Plugins management entry under System menu"
    );

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
    let (app, _) = build_app_with_host_admin().await;
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
async fn config_builtin_matches_static_page_and_api_responses() {
    let (app, ctx) = build_app_with_host_admin().await;
    let static_router = axum::Router::new()
        .route(
            "/admin/config",
            axum::routing::get(sushi_admin::routes::config::config_page),
        )
        .route(
            "/admin/api/config",
            axum::routing::get(sushi_admin::routes::config::config_api),
        )
        .with_state(ctx);
    let token = admin_bearer_token();

    for path in ["/admin/config", "/admin/api/config"] {
        let static_response = static_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("failed to build static request"),
            )
            .await
            .expect("static request failed");
        let builtin_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build builtin request"),
            )
            .await
            .expect("builtin request failed");

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read static body");
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read builtin body");
        assert_eq!(builtin_body, static_body, "path: {path}");
    }
}

#[tokio::test]
async fn logs_api_returns_logs_array_payload() {
    let (app, _) = build_app_with_host_admin().await;
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
async fn logs_builtin_matches_static_page_and_api_responses() {
    let (app, ctx) = build_app_with_host_admin().await;
    ctx.logs.info("shadow-log-entry").await;
    let static_router = axum::Router::new()
        .route(
            "/admin/logs",
            axum::routing::get(sushi_admin::routes::logs::logs_page),
        )
        .route(
            "/admin/api/logs",
            axum::routing::get(sushi_admin::routes::logs::logs_api),
        )
        .with_state(ctx);
    let token = admin_bearer_token();

    for path in ["/admin/logs", "/admin/api/logs"] {
        let static_response = static_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("failed to build static request"),
            )
            .await
            .expect("static request failed");
        let builtin_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("failed to build builtin request"),
            )
            .await
            .expect("builtin request failed");

        assert_eq!(builtin_response.status(), static_response.status());
        assert_eq!(
            builtin_response.headers().get(header::CONTENT_TYPE),
            static_response.headers().get(header::CONTENT_TYPE)
        );
        let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read static body");
        let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
            .await
            .expect("failed to read builtin body");
        assert_eq!(builtin_body, static_body, "path: {path}");
    }
}

#[tokio::test]
async fn host_admin_registers_owner_scoped_menu_contributions() {
    let (_, ctx) = build_app_with_host_admin().await;
    let snapshot = ctx.plugins.capability_snapshot().await;
    let contributions = snapshot
        .menu_contributions()
        .iter()
        .filter(|registration| registration.owner.as_str() == "host.admin")
        .collect::<Vec<_>>();

    assert_eq!(contributions.len(), 4);

    let system = contributions
        .iter()
        .find(|registration| registration.value.id == "host-admin.system")
        .expect("system menu contribution is registered");
    assert_eq!(system.value.parent_id, None);
    assert_eq!(system.value.route, None);

    for (id, route, policy) in [
        ("host-admin.plugins", "/admin/plugins", "admin.plugins.view"),
        ("host-admin.config", "/admin/config", "admin.config.view"),
        ("host-admin.logs", "/admin/logs", "admin.logs.view"),
    ] {
        let contribution = contributions
            .iter()
            .find(|registration| registration.value.id == id)
            .unwrap_or_else(|| panic!("menu contribution is registered: {id}"));
        assert_eq!(contribution.value.route.as_deref(), Some(route));
        assert_eq!(contribution.value.policy_key.as_deref(), Some(policy));
        assert_eq!(
            contribution.value.parent_id.as_deref(),
            Some("host-admin.system")
        );
    }
}

#[tokio::test]
async fn admin_shell_registers_dashboard_capabilities() {
    let (_, ctx) = build_app_with_context(None).await;
    let snapshot = ctx.plugins.capability_snapshot().await;

    let page = snapshot
        .admin_page("/admin/")
        .expect("Admin Shell dashboard page should be registered");
    assert_eq!(page.owner.as_str(), "admin.shell");
    assert_eq!(page.value.plugin_name, "admin-shell");
    assert_eq!(
        page.value.policy_key.as_deref(),
        Some("admin.dashboard.view")
    );

    let contribution = snapshot
        .menu_contributions()
        .iter()
        .find(|registration| registration.value.id == "host-admin.dashboard")
        .expect("Dashboard menu contribution should be registered");
    assert_eq!(contribution.owner.as_str(), "admin.shell");
    assert_eq!(contribution.value.route.as_deref(), Some("/admin/"));
    assert_eq!(contribution.value.parent_id, None);

    let workspace = snapshot
        .match_http_on(
            sushi_core::runtime::HttpSurface::Admin,
            "GET",
            "/admin/workspace/{*module}",
        )
        .expect("Admin Shell workspace route should be registered");
    assert_eq!(workspace.owner.as_str(), "admin.shell");
    assert_eq!(workspace.value.plugin_name, "admin-shell");

    let assets = snapshot
        .match_http_on(
            sushi_core::runtime::HttpSurface::Admin,
            "GET",
            "/admin/api/workspace/assets",
        )
        .expect("Admin Shell workspace assets route should be registered");
    assert_eq!(assets.owner.as_str(), "admin.shell");
    assert_eq!(assets.value.plugin_name, "admin-shell");
    assert_eq!(
        assets.value.policy_key.as_deref(),
        Some("admin.plugins.view")
    );

    for method in ["GET", "POST"] {
        let login = snapshot
            .match_http_on(
                sushi_core::runtime::HttpSurface::Api,
                method,
                "/admin-login",
            )
            .expect("Admin Shell login route should be registered");
        assert_eq!(login.owner.as_str(), "admin.shell");
        assert!(login.value.is_public);
    }
}

#[tokio::test]
async fn required_admin_shell_builtin_rejects_runtime_toggle() {
    let (_, ctx) = build_app_with_context(None).await;

    let error = ctx
        .set_plugin_enabled("admin-shell", false, Some("test"), Some("required guard"))
        .await
        .expect_err("required Admin Shell builtin must reject ordinary runtime toggles");

    assert_eq!(
        error,
        "required_plugin_toggle_forbidden: plugin 'admin-shell' must be changed through profile and restart"
    );
    assert!(ctx
        .plugins
        .capability_snapshot()
        .await
        .admin_page("/admin/")
        .is_some());
}

#[tokio::test]
async fn required_host_admin_builtin_rejects_runtime_toggle() {
    let (_, ctx) = build_app_with_host_admin().await;

    let error = ctx
        .set_plugin_enabled("host-admin", false, Some("test"), Some("required guard"))
        .await
        .expect_err("required builtin must reject ordinary runtime toggles");

    assert_eq!(
        error,
        "required_plugin_toggle_forbidden: plugin 'host-admin' must be changed through profile and restart"
    );
    let snapshot = ctx.plugins.capability_snapshot().await;
    assert!(snapshot.admin_page("/admin/logs").is_some());
    assert!(snapshot
        .match_http_on(
            sushi_core::runtime::HttpSurface::Admin,
            "GET",
            "/admin/api/logs"
        )
        .is_some());
}

#[tokio::test]
async fn governance_builtin_owns_plugin_state_capability() {
    let (_, ctx) = build_app_with_context(None).await;
    let snapshot = ctx.plugins.capability_snapshot().await;
    let registration = snapshot
        .match_http_on(
            sushi_core::runtime::HttpSurface::Admin,
            "PATCH",
            "/admin/api/plugins/{plugin}/state",
        )
        .expect("governance plugin state route should be registered");

    assert_eq!(registration.owner.as_str(), "governance.admin");
    assert_eq!(registration.value.plugin_name, "governance");
    assert_eq!(
        registration.value.policy_key.as_deref(),
        Some("admin.plugins.manage")
    );
}

#[tokio::test]
async fn required_governance_builtin_rejects_runtime_toggle() {
    let (_, ctx) = build_app_with_context(None).await;

    let error = ctx
        .set_plugin_enabled("governance", false, Some("test"), Some("required guard"))
        .await
        .expect_err("required governance builtin must reject ordinary runtime toggles");

    assert_eq!(
        error,
        "required_plugin_toggle_forbidden: plugin 'governance' must be changed through profile and restart"
    );
    assert!(ctx
        .plugins
        .capability_snapshot()
        .await
        .match_http_on(
            sushi_core::runtime::HttpSurface::Admin,
            "PATCH",
            "/admin/api/plugins/{plugin}/state",
        )
        .is_some());
}

#[tokio::test]
async fn required_rbac_admin_builtin_rejects_runtime_toggle() {
    let (_, ctx) = build_app_with_rbac_admin().await;

    let error = ctx
        .set_plugin_enabled("rbac-admin", false, Some("test"), Some("required guard"))
        .await
        .expect_err("required RBAC builtin must reject ordinary runtime toggles");

    assert_eq!(
        error,
        "required_plugin_toggle_forbidden: plugin 'rbac-admin' must be changed through profile and restart"
    );
    let snapshot = ctx.plugins.capability_snapshot().await;
    assert!(snapshot.admin_page("/admin/users").is_some());
    assert!(snapshot
        .match_http_on(
            sushi_core::runtime::HttpSurface::Admin,
            "POST",
            "/admin/partials/roles/create"
        )
        .is_some());
}

#[tokio::test]
async fn menu_admin_registers_owner_scoped_capabilities() {
    let (_, ctx) = build_app_with_context(None).await;
    let snapshot = ctx.plugins.capability_snapshot().await;

    for (method, path, policy) in [
        ("GET", "/admin/api/menu", "admin.menus.view"),
        ("POST", "/admin/api/menu", "admin.menus.manage"),
        ("PUT", "/admin/api/menu/{id}", "admin.menus.manage"),
        ("DELETE", "/admin/api/menu/{id}", "admin.menus.manage"),
        ("GET", "/admin/partials/menus/table", "admin.menus.view"),
        ("POST", "/admin/partials/menus/create", "admin.menus.manage"),
        (
            "POST",
            "/admin/partials/menus/{id}/update",
            "admin.menus.manage",
        ),
        ("DELETE", "/admin/partials/menus/{id}", "admin.menus.manage"),
    ] {
        let registration = snapshot
            .match_http_on(sushi_core::runtime::HttpSurface::Admin, method, path)
            .unwrap_or_else(|| panic!("menu route {method} {path} should be registered"));
        assert_eq!(registration.owner.as_str(), "menu.admin");
        assert_eq!(registration.value.policy_key.as_deref(), Some(policy));
    }

    let page = snapshot
        .admin_page("/admin/menus")
        .expect("menu admin page should be registered");
    assert_eq!(page.owner.as_str(), "menu.admin");

    let contribution = snapshot
        .menu_contributions()
        .iter()
        .find(|registration| registration.value.id == "menu-admin.menus")
        .expect("menu admin contribution should be registered");
    assert_eq!(contribution.owner.as_str(), "menu.admin");
}

#[tokio::test]
async fn required_menu_admin_builtin_rejects_runtime_toggle() {
    let (_, ctx) = build_app_with_context(None).await;

    let error = ctx
        .set_plugin_enabled("menu-admin", false, Some("test"), Some("required guard"))
        .await
        .expect_err("required menu builtin must reject ordinary runtime toggles");

    assert_eq!(
        error,
        "required_plugin_toggle_forbidden: plugin 'menu-admin' must be changed through profile and restart"
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

    assert_eq!(response.status(), StatusCode::OK);
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
    assert!(html.contains("data-admin-login-shell"), "html: {html}");
    assert!(html.contains("data-enterprise-trust-panel"), "html: {html}");
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
    let (app, _) = build_app_with_host_admin().await;
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
async fn dashboard_builtin_matches_static_page_response() {
    let (app, ctx) = build_app_with_host_admin().await;
    let static_router = axum::Router::new()
        .route(
            "/admin/",
            axum::routing::get(sushi_admin::routes::dashboard::dashboard_page),
        )
        .with_state(ctx);
    let token = admin_bearer_token();

    let static_response = static_router
        .oneshot(
            Request::builder()
                .uri("/admin/")
                .body(Body::empty())
                .expect("failed to build static request"),
        )
        .await
        .expect("static request failed");
    let builtin_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build builtin request"),
        )
        .await
        .expect("builtin request failed");

    assert_eq!(builtin_response.status(), static_response.status());
    assert_eq!(
        builtin_response.headers().get(header::CONTENT_TYPE),
        static_response.headers().get(header::CONTENT_TYPE)
    );
    let static_body = to_bytes(static_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read static body");
    let builtin_body = to_bytes(builtin_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read builtin body");
    assert_eq!(builtin_body, static_body);
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
async fn workspace_cms_module_loads_for_authenticated_admin() {
    let app = build_app_with_cms_plugin_loaded(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/cms")
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
    assert!(
        html.contains("data-admin-workspace-module=\"cms\""),
        "html: {html}"
    );
    assert!(html.contains("data-cms-top-nav"), "html: {html}");
    assert!(
        !html.contains("id=\"admin-workspace\""),
        "workspace partial must not include full admin shell: {html}"
    );
    assert!(
        !html.contains("<!DOCTYPE html>"),
        "workspace partial must not include full page document: {html}"
    );
}

#[tokio::test]
async fn workspace_plugin_module_returns_forbidden_when_plugin_disabled() {
    let (app, ctx) = build_app_with_plugin_admin_page_and_context("/admin/cms").await;
    ctx.plugins
        .set_plugin_enabled("cms", false, Some("admin"), Some("disabled by test"))
        .await
        .expect("failed to disable cms plugin");
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/cms")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
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
    let (app, _) = build_app_with_host_admin().await;
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
    let (app, _) = build_app_with_host_admin().await;
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

#[test]
fn login_template_includes_enterprise_shell_markers() {
    let template_path = templates_root().join("admin").join("login.html");
    let html = fs::read_to_string(&template_path).expect("failed to read login template");

    assert!(
        html.contains("data-admin-login-shell"),
        "login template missing admin login shell marker: {}",
        template_path.display()
    );
    assert!(
        html.contains("data-enterprise-trust-panel"),
        "login template missing enterprise trust panel marker: {}",
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
    let (app, _) = build_app_with_host_admin().await;
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
async fn admin_can_toggle_plugin_enabled_state() {
    let (app, ctx) = build_app_with_context(None).await;
    let plugin_root = register_test_plugin(&ctx, "toggle-target").await;
    let token = admin_bearer_token();

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/toggle-target/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"enabled":false,"reason":"maintenance window"}"#,
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(disable_response.status(), StatusCode::OK);
    let disable_body = to_bytes(disable_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let disable_payload: Value =
        serde_json::from_slice(&disable_body).expect("invalid disable payload");
    assert_eq!(
        disable_payload.get("enabled").and_then(Value::as_bool),
        Some(false)
    );

    let disabled_state = ctx
        .plugins
        .list_plugins()
        .await
        .into_iter()
        .find(|plugin| plugin.name == "toggle-target")
        .map(|plugin| plugin.enabled);
    assert_eq!(disabled_state, Some(false));
    assert!(!ctx.plugins.has_vm("toggle-target").await);
    assert!(ctx
        .plugins
        .call_api_handler("GET", "/api/toggle-target", None)
        .await
        .is_none());

    let enable_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/toggle-target/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"enabled":true,"reason":"maintenance complete"}"#,
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(enable_response.status(), StatusCode::OK);
    let enable_body = to_bytes(enable_response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let enable_payload: Value =
        serde_json::from_slice(&enable_body).expect("invalid enable payload");
    assert_eq!(
        enable_payload.get("enabled").and_then(Value::as_bool),
        Some(true)
    );

    let enabled_state = ctx
        .plugins
        .list_plugins()
        .await
        .into_iter()
        .find(|plugin| plugin.name == "toggle-target")
        .map(|plugin| plugin.enabled);
    assert_eq!(enabled_state, Some(true));
    assert!(ctx.plugins.has_vm("toggle-target").await);
    assert_eq!(
        ctx.plugins
            .call_api_handler("GET", "/api/toggle-target", None)
            .await
            .unwrap()
            .unwrap(),
        "active"
    );
    fs::remove_dir_all(plugin_root).expect("failed to clean toggle plugin root");
}

#[tokio::test]
async fn admin_toggle_plugin_state_returns_not_found_for_unknown_plugin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/does-not-exist/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"enabled":false,"reason":"missing plugin"}"#))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let payload: Value = serde_json::from_slice(&body).expect("invalid json payload");
    assert_eq!(
        payload.get("error").and_then(Value::as_str),
        Some("plugin not found")
    );
}

#[tokio::test]
async fn admin_toggle_plugin_state_returns_internal_error_when_state_write_fails() {
    let (app, ctx) = build_app_with_context(None).await;
    let plugin_root = register_test_plugin(&ctx, "toggle-target").await;
    let token = admin_bearer_token();

    ctx.db
        .execute("DROP TABLE plugin_state_events", vec![])
        .await
        .expect("failed to break plugin_state_events table");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/toggle-target/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"enabled":false,"reason":"force write failure"}"#,
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let payload: Value = serde_json::from_slice(&body).expect("invalid json payload");
    assert_eq!(
        payload.get("error").and_then(Value::as_str),
        Some("failed to update plugin state")
    );
    let state = ctx
        .plugins
        .list_plugins()
        .await
        .into_iter()
        .find(|plugin| plugin.name == "toggle-target")
        .unwrap();
    assert!(state.enabled, "failed audit write must roll back intent");
    assert!(state.loaded, "failed audit write must keep runtime active");
    fs::remove_dir_all(plugin_root).expect("failed to clean toggle plugin root");
}

#[tokio::test]
async fn viewer_cannot_toggle_plugin_enabled_state() {
    let (app, ctx) = build_app_with_context(None).await;
    let plugin_root = register_test_plugin(&ctx, "toggle-target").await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/toggle-target/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"enabled":false,"reason":"unauthorized change"}"#,
                ))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let current_state = ctx
        .plugins
        .list_plugins()
        .await
        .into_iter()
        .find(|plugin| plugin.name == "toggle-target")
        .map(|plugin| plugin.enabled);
    assert_eq!(current_state, Some(true));
    fs::remove_dir_all(plugin_root).expect("failed to clean toggle plugin root");
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
