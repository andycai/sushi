use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::lua::bindings::inject_plugin_api;
use sushi_core::lua::contract::{ContractSchemaVersion, LuaCapabilityContract};
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::lua::permission::engine::{CapabilityKind, PermissionDecisionEngine};
use sushi_core::lua::vm::create_sandboxed_vm;
use sushi_core::plugin::{DatabasePermission, Permissions, Plugin, PluginError};
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::web::template_service::TemplateService;

async fn make_test_context() -> (SushiContext, tempfile::TempDir) {
    let config = ConfigStore::new(SushiConfig::default());
    let db = SqliteStorage::new_in_memory().await.unwrap();
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let templates_dir = tempfile::tempdir().unwrap();
    let templates = TemplateService::new(templates_dir.path()).unwrap();
    let ctx = SushiContext::new(config, db, jwt, templates);
    (ctx, templates_dir)
}

async fn activate_plugin(plugin: &LuaPlugin, ctx: &SushiContext) -> Result<(), PluginError> {
    ctx.runtime_host.register_lua_source(plugin, false).await;
    ctx.runtime_host.activate(ctx, plugin.name()).await
}

async fn create_contract_test_plugin_with_manifest(
    source: &str,
    permissions_toml: &str,
    policy_scopes: &[&str],
) -> (
    LuaPlugin,
    SushiContext,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let plugin_root = tempfile::tempdir().unwrap();
    let plugin_dir = plugin_root.path().join("third_party").join("contract_case");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let policies_toml = if policy_scopes.is_empty() {
        String::new()
    } else {
        let scopes = policy_scopes
            .iter()
            .map(|scope| format!("\"{scope}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("\n[policies]\nscopes = [{scopes}]\n")
    };

    std::fs::write(
        plugin_dir.join("plugin.toml"),
        format!(
            r#"
schema_version = 1

[plugin]
name = "contract_case"
version = "0.1.0"
entry = "init.lua"

{permissions_toml}{policies_toml}
"#
        ),
    )
    .unwrap();
    std::fs::write(plugin_dir.join("init.lua"), source).unwrap();

    let mut plugins = LuaPlugin::scan_dir(plugin_root.path())
        .await
        .expect("scan test plugin");
    let plugin = plugins.remove(0);

    let templates_dir = tempfile::tempdir().unwrap();
    let templates = TemplateService::new(templates_dir.path()).unwrap();
    let config = ConfigStore::new(SushiConfig::default());
    let db = SqliteStorage::new_in_memory().await.unwrap();
    db.run_migrations(include_str!("../../../migrations/001_init.sql"))
        .await
        .unwrap();
    db.run_migrations(include_str!("../../../migrations/003_rbac.sql"))
        .await
        .unwrap();
    db.run_migrations(include_str!(
        "../../../migrations/006_unified_policy_v2.sql"
    ))
    .await
    .unwrap();
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let ctx = SushiContext::new(config, db, jwt, templates);

    (plugin, ctx, plugin_root, templates_dir)
}

async fn create_contract_test_plugin(
    source: &str,
) -> (
    LuaPlugin,
    SushiContext,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    create_contract_test_plugin_with_manifest(
        source,
        r#"[permissions]
routes = true
commands = true
admin = true
database = false
"#,
        &["admin.notes.*"],
    )
    .await
}

#[test]
fn contract_kernel_exports_v2_types() {
    let version = ContractSchemaVersion::V2;
    let contract = LuaCapabilityContract::default();
    assert_eq!(version.as_str(), "v2");
    assert!(contract.entries.is_empty());

    let tagged_api = serde_json::json!({
        "surface": "api",
        "method": "GET",
        "path": "/health"
    });
    let parsed: sushi_core::lua::contract::LuaCapabilityEntry =
        serde_json::from_value(tagged_api.clone()).expect("api variant should deserialize");
    assert_eq!(
        serde_json::to_value(&parsed).expect("api variant should serialize"),
        tagged_api
    );

    let roundtrip = LuaCapabilityContract {
        entries: vec![parsed],
    };
    let encoded = serde_json::to_string(&roundtrip).expect("contract should serialize");
    let decoded: LuaCapabilityContract =
        serde_json::from_str(&encoded).expect("contract should deserialize");
    assert_eq!(decoded, roundtrip);
}

#[test]
fn deny_by_default_hides_unauthorized_capabilities() {
    let engine = PermissionDecisionEngine::new(Permissions::default(), true);

    assert!(!engine.is_visible(CapabilityKind::ApiRoute));
    assert!(!engine.is_visible(CapabilityKind::AdminPage));
    assert!(!engine.is_visible(CapabilityKind::CliCommand));
    assert!(!engine.is_visible(CapabilityKind::WebRender));
    assert!(!engine.is_visible(CapabilityKind::DbRead));
    assert!(!engine.is_visible(CapabilityKind::DbWrite));
    assert!(engine.is_visible(CapabilityKind::Event));
    assert!(engine.is_visible(CapabilityKind::Fs));
}

#[test]
fn db_write_visibility_requires_write_or_admin() {
    let none_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::None,
            ..Permissions::default()
        },
        true,
    );
    assert!(!none_db.is_visible(CapabilityKind::DbWrite));

    let read_only_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::ReadOnly,
            ..Permissions::default()
        },
        true,
    );
    assert!(!read_only_db.is_visible(CapabilityKind::DbWrite));

    let write_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::Write,
            ..Permissions::default()
        },
        true,
    );
    assert!(write_db.is_visible(CapabilityKind::DbWrite));

    let admin_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::Admin,
            ..Permissions::default()
        },
        true,
    );
    assert!(admin_db.is_visible(CapabilityKind::DbWrite));
}

#[tokio::test]
async fn contract_registry_supports_web_db_event_fs_entries() {
    let source = r#"
function sushi.init()
  local noop = function() return "ok" end
  sushi.capability.register({ surface = "web", kind = "page", path = "/admin/notes", template = "plugins/official/kv-store/kv.html", title = "Notes", policy = "admin.notes.read", handler = noop })
  sushi.capability.register({ surface = "db", kind = "query", name = "notes_read" })
  sushi.capability.register({ surface = "event", kind = "emit", event = "notes.changed" })
  sushi.capability.register({ surface = "fs", kind = "read_text", root = "docs" })
end
"#;

    let (plugin, ctx, _plugin_root, _templates_dir) = create_contract_test_plugin(source).await;
    activate_plugin(&plugin, &ctx)
        .await
        .expect("plugin initializes");

    assert!(
        ctx.plugins
            .admin_page_policy("/admin/notes")
            .await
            .is_some(),
        "web contract entry should persist admin page policy metadata",
    );
}

#[tokio::test]
async fn contract_registry_api_requires_routes_permission() {
    let source = r#"
function sushi.init()
  local noop = function() return "ok" end
  sushi.capability.register({ surface = "api", method = "GET", path = "/api/notes", handler = noop })
end
"#;
    let (plugin, ctx, _plugin_root, _templates_dir) = create_contract_test_plugin_with_manifest(
        source,
        r#"[permissions]
routes = false
commands = false
admin = true
database = false
"#,
        &[],
    )
    .await;

    let err = activate_plugin(&plugin, &ctx)
        .await
        .expect_err("api contract entry should require routes permission");
    let message = err.to_string();
    assert!(message.contains("api entries"));
    assert!(message.contains("routes permission is disabled"));
}

#[tokio::test]
async fn contract_registry_web_page_requires_admin_permission() {
    let source = r#"
function sushi.init()
  local noop = function() return "ok" end
  sushi.capability.register({ surface = "web", kind = "page", path = "/admin/notes", title = "Notes", handler = noop })
end
"#;
    let (plugin, ctx, _plugin_root, _templates_dir) = create_contract_test_plugin_with_manifest(
        source,
        r#"[permissions]
routes = true
commands = false
admin = false
database = false
"#,
        &[],
    )
    .await;

    let err = activate_plugin(&plugin, &ctx)
        .await
        .expect_err("web page contract entry should require admin permission");
    let message = err.to_string();
    assert!(message.contains("web page, admin page, or menu entries"));
    assert!(message.contains("admin permission is disabled"));
}

#[tokio::test]
async fn unauthorized_api_namespace_is_not_injected() {
    let lua = create_sandboxed_vm().unwrap();
    let (ctx, _templates_dir) = make_test_context().await;

    let permissions = Permissions::default();
    let plugin_context = ctx.plugin_context(&permissions);
    inject_plugin_api(&lua, &plugin_context, &permissions)
        .await
        .expect("lua bindings should inject into sandbox vm");

    let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
    assert!(!sushi.contains_key("api").unwrap());
    assert!(sushi.contains_key("capability").unwrap());
}
