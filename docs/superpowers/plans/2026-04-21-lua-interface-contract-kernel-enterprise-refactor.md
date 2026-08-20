# Lua Interface Contract Kernel Enterprise Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace function-by-function Lua exports with a capability contract kernel that fully covers `api/admin/cli/web/db/event/fs`, enforces deny-by-default visibility, and supports large-scale plugin development with minimal future Rust API churn.

**Architecture:** Introduce a `lua::contract + lua::permission + lua::registry + lua::injector + lua::adapters` pipeline, then cut loader/manager dispatch to registry-derived bindings. Keep runtime governance and policy checks centralized with deterministic reason codes and audit fields, and migrate official plugins (`kv-store`, `file-browser`, `cms`) to the new contract registration shape.

**Tech Stack:** Rust (Tokio/Axum/Clap/mlua/serde), Lua 5.4 plugins, SQLite policy/governance storage, existing Sushi plugin runtime and tests.

---

## File Structure Map

- Create: `crates/sushi-core/src/lua/contract/mod.rs` — contract root types and schema version.
- Create: `crates/sushi-core/src/lua/contract/schema/api.rs` — API contract schema.
- Create: `crates/sushi-core/src/lua/contract/schema/admin.rs` — admin contract schema.
- Create: `crates/sushi-core/src/lua/contract/schema/cli.rs` — CLI contract schema.
- Create: `crates/sushi-core/src/lua/contract/schema/web.rs` — web contract schema.
- Create: `crates/sushi-core/src/lua/contract/schema/db.rs` — db contract schema.
- Create: `crates/sushi-core/src/lua/contract/schema/event.rs` — event contract schema.
- Create: `crates/sushi-core/src/lua/contract/schema/fs.rs` — fs contract schema.
- Create: `crates/sushi-core/src/lua/permission/engine.rs` — single permission decision engine.
- Create: `crates/sushi-core/src/lua/registry/mod.rs` — normalized capability registry + snapshots.
- Create: `crates/sushi-core/src/lua/injector/mod.rs` — deny-by-default injection and contract registration entrypoint.
- Create: `crates/sushi-core/src/lua/adapters/api.rs` — API loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/adapters/admin.rs` — admin loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/adapters/cli.rs` — CLI loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/adapters/web.rs` — web loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/adapters/db.rs` — db loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/adapters/event.rs` — event loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/adapters/fs.rs` — fs loader/runtime bridge.
- Create: `crates/sushi-core/src/lua/errors.rs` — stable reason codes and structured contract errors.
- Modify: `crates/sushi-core/src/lua/mod.rs` — export new modules.
- Modify: `crates/sushi-core/src/lua/bindings.rs` — convert to thin assembler.
- Modify: `crates/sushi-core/src/lua/loader.rs` — consume contract registry snapshot (not `__pending_*`).
- Modify: `crates/sushi-core/src/plugin/manager.rs` — central dispatch reason-code semantics.
- Modify: `crates/sushi-api/src/router.rs` — API mapping for reason codes.
- Modify: `crates/sushi-admin/src/routes/workspace.rs` — admin mapping for reason codes.
- Modify: `crates/sushi-cli/src/commands/run.rs` — CLI mapping for reason codes.
- Modify: `plugins/official/kv-store/lua/bootstrap/register.lua` — migrate to `sushi.capability.register`.
- Modify: `plugins/official/file-browser/lua/bootstrap/register.lua` — migrate to `sushi.capability.register`.
- Modify: `plugins/official/cms/lua/bootstrap/register.lua` — migrate to `sushi.capability.register`.
- Create: `crates/sushi-core/tests/lua_contract_kernel.rs` — contract kernel + visibility tests.
- Create: `crates/sushi-core/tests/lua_contract_registry.rs` — normalization and conflict tests.
- Create: `docs/wiki/guides/lua-contract-migration.md` — third-party migration guide.
- Modify: `docs/wiki/lua-api/README.md` — contract-first Lua API overview.
- Modify: `docs/wiki/lua-api/sushi.api.md` — contract-first API route registration docs.
- Modify: `docs/wiki/lua-api/sushi.admin.md` — contract-first admin registration docs.
- Modify: `docs/wiki/lua-api/sushi.cli.md` — contract-first CLI registration docs.
- Modify: `docs/wiki/lua-api/sushi.web.md` — contract-first web registration docs.
- Modify: `docs/wiki/lua-api/sushi.db.md` — contract-first DB capability docs.
- Modify: `docs/wiki/lua-api/sushi.event.md` — contract-first event capability docs.
- Modify: `docs/wiki/lua-api/sushi.fs.md` — contract-first fs capability docs.
- Modify: `docs/engineering/plugin-authoring-standards.md` — deny-by-default + contract rules.

---

### Task 1: Scaffold Contract Kernel Modules

**Files:**
- Create: `crates/sushi-core/src/lua/contract/mod.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/api.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/admin.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/cli.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/web.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/db.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/event.rs`
- Create: `crates/sushi-core/src/lua/contract/schema/fs.rs`
- Modify: `crates/sushi-core/src/lua/mod.rs`
- Test: `crates/sushi-core/tests/lua_contract_kernel.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/sushi-core/tests/lua_contract_kernel.rs
use sushi_core::lua::contract::{ContractSchemaVersion, LuaCapabilityContract};

#[test]
fn contract_kernel_exports_v2_types() {
    let version = ContractSchemaVersion::V2;
    let contract = LuaCapabilityContract::default();
    assert_eq!(version.as_str(), "v2");
    assert!(contract.entries.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core contract_kernel_exports_v2_types -q`  
Expected: FAIL with unresolved `lua::contract` types.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/lua/contract/mod.rs
pub mod schema {
    pub mod admin;
    pub mod api;
    pub mod cli;
    pub mod db;
    pub mod event;
    pub mod fs;
    pub mod web;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractSchemaVersion {
    V2,
}

impl ContractSchemaVersion {
    pub fn as_str(self) -> &'static str {
        "v2"
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct LuaCapabilityContract {
    pub entries: Vec<LuaCapabilityEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "surface", rename_all = "snake_case")]
pub enum LuaCapabilityEntry {
    Api(schema::api::ApiRouteContract),
    Admin(schema::admin::AdminPageContract),
    Cli(schema::cli::CliCommandContract),
    Web(schema::web::WebContract),
    Db(schema::db::DbContract),
    Event(schema::event::EventContract),
    Fs(schema::fs::FsContract),
}
```

```rust
// crates/sushi-core/src/lua/mod.rs
pub mod bindings;
pub mod contract;
pub mod loader;
pub mod module_loader;
pub mod vm;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core contract_kernel_exports_v2_types -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/mod.rs crates/sushi-core/src/lua/contract crates/sushi-core/tests/lua_contract_kernel.rs
git commit -m "refactor(core): scaffold lua contract kernel modules"
```

### Task 2: Implement Permission Engine (P1 Deny-by-Default)

**Files:**
- Create: `crates/sushi-core/src/lua/permission/engine.rs`
- Modify: `crates/sushi-core/src/lua/mod.rs`
- Test: `crates/sushi-core/tests/lua_contract_kernel.rs`

- [ ] **Step 1: Write failing matrix tests**

```rust
// crates/sushi-core/tests/lua_contract_kernel.rs
use sushi_core::lua::permission::engine::{CapabilityKind, PermissionDecisionEngine};
use sushi_core::plugin::{DatabasePermission, Permissions};

#[test]
fn deny_by_default_hides_unauthorized_capabilities() {
    let engine = PermissionDecisionEngine::new(Permissions::default(), true);
    assert!(!engine.is_visible(CapabilityKind::ApiRoute));
    assert!(!engine.is_visible(CapabilityKind::CliCommand));
    assert!(!engine.is_visible(CapabilityKind::AdminPage));
}

#[test]
fn db_write_visibility_requires_write_or_admin() {
    let engine = PermissionDecisionEngine::new(
        Permissions {
            routes: false,
            commands: false,
            admin: false,
            database: DatabasePermission::Write,
        },
        true,
    );
    assert!(engine.is_visible(CapabilityKind::DbWrite));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sushi-core deny_by_default_hides_unauthorized_capabilities -q`  
Expected: FAIL with unresolved `lua::permission` module.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/lua/permission/engine.rs
use crate::plugin::{DatabasePermission, Permissions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    ApiRoute,
    AdminPage,
    CliCommand,
    WebRender,
    DbRead,
    DbWrite,
    Event,
    Fs,
}

#[derive(Debug, Clone)]
pub struct PermissionDecisionEngine {
    permissions: Permissions,
    plugin_enabled: bool,
}

impl PermissionDecisionEngine {
    pub fn new(permissions: Permissions, plugin_enabled: bool) -> Self {
        Self {
            permissions,
            plugin_enabled,
        }
    }

    pub fn is_visible(&self, capability: CapabilityKind) -> bool {
        if !self.plugin_enabled {
            return false;
        }
        match capability {
            CapabilityKind::ApiRoute => self.permissions.routes,
            CapabilityKind::AdminPage => self.permissions.admin,
            CapabilityKind::CliCommand => self.permissions.commands,
            CapabilityKind::WebRender => self.permissions.admin || self.permissions.routes,
            CapabilityKind::DbRead => !matches!(self.permissions.database, DatabasePermission::None),
            CapabilityKind::DbWrite => {
                matches!(
                    self.permissions.database,
                    DatabasePermission::Write | DatabasePermission::Admin
                )
            }
            CapabilityKind::Event | CapabilityKind::Fs => true,
        }
    }
}
```

```rust
// crates/sushi-core/src/lua/mod.rs
pub mod permission {
    pub mod engine;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core deny_by_default_hides_unauthorized_capabilities -q`  
Expected: PASS

Run: `cargo test -p sushi-core db_write_visibility_requires_write_or_admin -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/mod.rs crates/sushi-core/src/lua/permission/engine.rs crates/sushi-core/tests/lua_contract_kernel.rs
git commit -m "feat(core): add deny-by-default lua permission engine"
```

### Task 3: Implement Registry and Stable Contract Errors

**Files:**
- Create: `crates/sushi-core/src/lua/registry/mod.rs`
- Create: `crates/sushi-core/src/lua/errors.rs`
- Modify: `crates/sushi-core/src/lua/mod.rs`
- Test: `crates/sushi-core/tests/lua_contract_registry.rs`

- [ ] **Step 1: Write failing registry tests**

```rust
// crates/sushi-core/tests/lua_contract_registry.rs
use sushi_core::lua::contract::schema::api::ApiRouteContract;
use sushi_core::lua::errors::LuaContractErrorCode;
use sushi_core::lua::registry::CapabilityRegistry;

#[test]
fn registry_stores_api_metadata() {
    let mut registry = CapabilityRegistry::default();
    registry
        .register_api(ApiRouteContract {
            method: "GET".to_string(),
            path: "/api/notes".to_string(),
            handler_key: "h_1".to_string(),
            policy: Some("api.notes.read".to_string()),
            public: false,
        })
        .unwrap();

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.api_routes.len(), 1);
    assert_eq!(snapshot.api_routes[0].policy.as_deref(), Some("api.notes.read"));
}

#[test]
fn registry_rejects_public_policy_conflict() {
    let mut registry = CapabilityRegistry::default();
    let err = registry
        .register_api(ApiRouteContract {
            method: "GET".to_string(),
            path: "/api/open".to_string(),
            handler_key: "h_2".to_string(),
            policy: Some("api.open.read".to_string()),
            public: true,
        })
        .unwrap_err();
    assert_eq!(err.code(), LuaContractErrorCode::RegistrationDenied);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core registry_stores_api_metadata -q`  
Expected: FAIL with unresolved registry/errors types.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/lua/errors.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaContractErrorCode {
    CapabilityNotVisible,
    RegistrationDenied,
    PolicyScopeViolation,
    PluginDisabled,
    PluginNotLoaded,
}

#[derive(Debug, Clone)]
pub struct LuaContractError {
    code: LuaContractErrorCode,
    message: String,
}

impl LuaContractError {
    pub fn new(code: LuaContractErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn code(&self) -> LuaContractErrorCode {
        self.code
    }
}
```

```rust
// crates/sushi-core/src/lua/registry/mod.rs
use crate::lua::contract::schema::api::ApiRouteContract;
use crate::lua::errors::{LuaContractError, LuaContractErrorCode};

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    api_routes: Vec<ApiRouteContract>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilitySnapshot {
    pub api_routes: Vec<ApiRouteContract>,
}

impl CapabilityRegistry {
    pub fn register_api(&mut self, route: ApiRouteContract) -> Result<(), LuaContractError> {
        if route.public && route.policy.is_some() {
            return Err(LuaContractError::new(
                LuaContractErrorCode::RegistrationDenied,
                "public route cannot include policy",
            ));
        }
        self.api_routes.push(route);
        Ok(())
    }

    pub fn snapshot(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            api_routes: self.api_routes.clone(),
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core registry_stores_api_metadata -q`  
Expected: PASS

Run: `cargo test -p sushi-core registry_rejects_public_policy_conflict -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/mod.rs crates/sushi-core/src/lua/errors.rs crates/sushi-core/src/lua/registry/mod.rs crates/sushi-core/tests/lua_contract_registry.rs
git commit -m "feat(core): add lua contract registry and stable error codes"
```

### Task 4: Refactor Injector + Bindings Entry

**Files:**
- Create: `crates/sushi-core/src/lua/injector/mod.rs`
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Modify: `crates/sushi-core/src/lua/mod.rs`
- Test: `crates/sushi-core/tests/lua_contract_kernel.rs`

- [ ] **Step 1: Write failing injector test**

```rust
// crates/sushi-core/tests/lua_contract_kernel.rs
use sushi_core::lua::bindings::inject_sushi_api;
use sushi_core::lua::vm::create_sandboxed_vm;

#[tokio::test]
async fn unauthorized_api_namespace_is_not_injected() {
    async fn make_test_context() -> sushi_core::context::SushiContext {
        use sushi_core::auth::jwt::JwtService;
        use sushi_core::config::ConfigStore;
        use sushi_core::storage::sqlite::SqliteStorage;
        use sushi_core::web::template_service::TemplateService;

        let config = ConfigStore::new(sushi_core::config::SushiConfig::default());
        let db = SqliteStorage::new_in_memory().await.unwrap();
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        let templates_dir = tempfile::tempdir().unwrap();
        let templates = TemplateService::new(templates_dir.path()).unwrap();
        sushi_core::context::SushiContext::new(config, db, jwt, templates)
    }

    let lua = create_sandboxed_vm().unwrap();
    let ctx = make_test_context().await;

    inject_sushi_api(&lua, &ctx, &sushi_core::plugin::Permissions::default())
        .await
        .unwrap();

    let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
    assert!(!sushi.contains_key("api").unwrap());
    assert!(sushi.contains_key("capability").unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core unauthorized_api_namespace_is_not_injected -q`  
Expected: FAIL because `sushi.capability` is not yet injected.

- [ ] **Step 3: Implement injector delegation path**

```rust
// crates/sushi-core/src/lua/injector/mod.rs
use crate::lua::permission::engine::{CapabilityKind, PermissionDecisionEngine};
use crate::plugin::Permissions;
use mlua::Lua;

pub fn inject(lua: &Lua, permissions: Permissions, enabled: bool) -> Result<(), mlua::Error> {
    let sushi: mlua::Table = lua.globals().get("sushi")?;
    let engine = PermissionDecisionEngine::new(permissions, enabled);

    let capability = lua.create_table()?;
    capability.set(
        "register",
        lua.create_function(|lua, entry: mlua::Table| {
            let sushi: mlua::Table = lua.globals().get("sushi")?;
            let pending: mlua::Table = sushi.get("__contract_registry")?;
            let len = pending.raw_len();
            pending.set(len + 1, entry)?;
            Ok(())
        })?,
    )?;
    sushi.set("capability", capability)?;

    if engine.is_visible(CapabilityKind::ApiRoute) {
        sushi.set("api", lua.create_table()?)?;
    }

    Ok(())
}
```

```rust
// crates/sushi-core/src/lua/bindings.rs (assembly shape)
let sushi = lua.create_table()?;
sushi.set("__contract_registry", lua.create_table()?)?;
lua.globals().set("sushi", sushi)?;
crate::lua::injector::inject(lua, permissions.clone(), true)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core unauthorized_api_namespace_is_not_injected -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/mod.rs crates/sushi-core/src/lua/injector/mod.rs crates/sushi-core/src/lua/bindings.rs crates/sushi-core/tests/lua_contract_kernel.rs
git commit -m "refactor(core): inject lua capability entrypoint"
```

### Task 5: Loader Cutover to Contract Registry Snapshot

**Files:**
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Create: `crates/sushi-core/src/lua/adapters/api.rs`
- Create: `crates/sushi-core/src/lua/adapters/admin.rs`
- Create: `crates/sushi-core/src/lua/adapters/cli.rs`
- Test: `crates/sushi-core/src/lua/loader.rs` (existing test module)

- [ ] **Step 1: Write failing loader test**

```rust
// crates/sushi-core/src/lua/loader.rs (test module)
#[tokio::test]
async fn loader_reads_contract_registry_for_api_routes() {
    async fn create_contract_test_plugin(
        source: &str,
    ) -> (crate::lua::loader::LuaPlugin, crate::context::SushiContext) {
        use crate::auth::jwt::JwtService;
        use crate::config::ConfigStore;
        use crate::storage::sqlite::SqliteStorage;
        use crate::web::template_service::TemplateService;
        use std::fs;

        let plugin_root = tempfile::tempdir().unwrap().into_path();
        let plugin_dir = plugin_root.join("third_party").join("contract_case");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"[plugin]
name = "contract_case"
version = "0.1.0"
kind = "third_party"

[permissions]
routes = true

[policies]
scopes = ["api.notes.*"]
"#,
        )
        .unwrap();
        fs::write(plugin_dir.join("init.lua"), source).unwrap();

        let mut plugins = crate::lua::loader::LuaPlugin::scan_dir(plugin_root.as_path())
            .await
            .unwrap();
        let plugin = plugins.remove(0);

        let config = ConfigStore::new(crate::config::SushiConfig::default());
        let db = SqliteStorage::new_in_memory().await.unwrap();
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        let templates_dir = tempfile::tempdir().unwrap().into_path();
        let templates = TemplateService::new(templates_dir.as_path()).unwrap();
        let ctx = crate::context::SushiContext::new(config, db, jwt, templates);
        (plugin, ctx)
    }

    let source = r#"
function sushi.init()
  local h = function() return "ok" end
  sushi.capability.register({
    surface = "api",
    method = "GET",
    path = "/api/notes",
    handler = h,
    policy = "api.notes.read"
  })
end
"#;

    let (plugin, ctx) = create_contract_test_plugin(source).await;
    plugin.init(&ctx).await.expect("plugin initializes");

    assert_eq!(
        ctx.plugins.api_route_policy("GET", "/api/notes").await.as_deref(),
        Some("api.notes.read")
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core loader_reads_contract_registry_for_api_routes -q`  
Expected: FAIL because loader still relies on `__pending_routes`.

- [ ] **Step 3: Implement loader registry adapter path**

```rust
// crates/sushi-core/src/lua/loader.rs (core flow)
let sushi: mlua::Table = lua.globals().get("sushi")?;
let raw_registry: mlua::Table = sushi.get("__contract_registry")?;
let snapshot = crate::lua::adapters::api::snapshot_from_lua(lua, raw_registry)?;

for route in snapshot.api_routes {
    if route.public && route.policy.is_some() {
        return Err(PluginError::InitFailed(format!(
            "route {} {} cannot declare both policy and public=true",
            route.method, route.path
        )));
    }

    if let Some(policy_key) = route.policy.as_deref() {
        validate_policy_scope(plugin_name, &format!("route {} {}", route.method, route.path), policy_key, allowed_policy_scopes)?;
        policy_repo
            .upsert_plugin_http_binding("api", &route.method, &route.path, policy_key, plugin_name)
            .await
            .map_err(|err| PluginError::InitFailed(format!("failed to persist policy binding for route {} {}: {err}", route.method, route.path)))?;
    }

    ctx.plugins
        .register_api_handler_with_policy_and_public(
            &route.method,
            &route.path,
            plugin_name,
            &route.handler_key,
            route.policy.as_deref(),
            route.public,
        )
        .await;
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p sushi-core loader_reads_contract_registry_for_api_routes -q`  
Expected: PASS

Run: `cargo test -p sushi-core plugin_load_persists_policy_metadata_for_registrations -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs crates/sushi-core/src/lua/adapters/api.rs crates/sushi-core/src/lua/adapters/admin.rs crates/sushi-core/src/lua/adapters/cli.rs
git commit -m "refactor(core): load plugin contracts from registry snapshot"
```

### Task 6: Add Web/DB/Event/FS Adapters for Full Surface Coverage

**Files:**
- Create: `crates/sushi-core/src/lua/adapters/web.rs`
- Create: `crates/sushi-core/src/lua/adapters/db.rs`
- Create: `crates/sushi-core/src/lua/adapters/event.rs`
- Create: `crates/sushi-core/src/lua/adapters/fs.rs`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/tests/lua_contract_kernel.rs`

- [ ] **Step 1: Write failing full-surface assertions**

```rust
// crates/sushi-core/tests/lua_contract_kernel.rs
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
    let (plugin, ctx) = create_contract_test_plugin(source).await;
    plugin.init(&ctx).await.expect("plugin initializes");
    assert!(ctx.plugins.admin_page_policy("/admin/notes").await.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core contract_registry_supports_web_db_event_fs_entries -q`  
Expected: FAIL because only API/Admin/CLI adapters are implemented.

- [ ] **Step 3: Implement remaining adapter modules**

```rust
// crates/sushi-core/src/lua/adapters/web.rs
pub fn parse_web_entry(entry: &mlua::Table) -> Result<WebEntry, mlua::Error> {
    Ok(WebEntry {
        kind: entry.get::<String>("kind")?,
        path: entry.get::<Option<String>>("path")?,
        title: entry.get::<Option<String>>("title")?,
        template: entry.get::<Option<String>>("template")?,
        policy: entry.get::<Option<String>>("policy")?,
        handler_key: entry.get::<String>("handler_key")?,
    })
}
```

```rust
// crates/sushi-core/src/lua/adapters/db.rs
pub fn parse_db_entry(entry: &mlua::Table) -> Result<DbEntry, mlua::Error> {
    Ok(DbEntry {
        kind: entry.get::<String>("kind")?,
        name: entry.get::<String>("name")?,
    })
}
```

```rust
// crates/sushi-core/src/lua/loader.rs (integration point)
let snapshot = crate::lua::adapters::web::extend_snapshot_with_web(lua, raw_registry, snapshot)?;
let snapshot = crate::lua::adapters::db::extend_snapshot_with_db(lua, raw_registry, snapshot)?;
let snapshot = crate::lua::adapters::event::extend_snapshot_with_event(lua, raw_registry, snapshot)?;
let snapshot = crate::lua::adapters::fs::extend_snapshot_with_fs(lua, raw_registry, snapshot)?;
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p sushi-core contract_registry_supports_web_db_event_fs_entries -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test lua_contract_kernel -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/adapters/web.rs crates/sushi-core/src/lua/adapters/db.rs crates/sushi-core/src/lua/adapters/event.rs crates/sushi-core/src/lua/adapters/fs.rs crates/sushi-core/src/lua/loader.rs crates/sushi-core/tests/lua_contract_kernel.rs
git commit -m "feat(core): add web db event fs contract adapters"
```

### Task 7: Unified Dispatch Reason Codes (API/Admin/CLI)

**Files:**
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Modify: `crates/sushi-api/src/router.rs`
- Modify: `crates/sushi-admin/src/routes/workspace.rs`
- Modify: `crates/sushi-cli/src/commands/run.rs`
- Test: `crates/sushi-core/src/plugin/manager.rs` (test module)
- Test: `crates/sushi-api/src/router.rs` (test module)

- [ ] **Step 1: Write failing reason-code tests**

```rust
// crates/sushi-core/src/plugin/manager.rs (test module)
#[tokio::test]
async fn disabled_dispatch_has_plugin_disabled_reason() {
    let manager = PluginManager::new_with_storage(test_storage().await);
    manager.mark_plugin_loaded("notes", true).await;
    manager.set_plugin_enabled("notes", false).await.unwrap();

    let err = manager.call_cli_handler("notes-run", vec![]).await.unwrap_err();
    assert_eq!(err.reason_code(), "plugin_disabled");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core disabled_dispatch_has_plugin_disabled_reason -q`  
Expected: FAIL because dispatch errors do not expose stable reason codes.

- [ ] **Step 3: Implement reason-code transport and mappings**

```rust
// crates/sushi-core/src/plugin/manager.rs
#[derive(Debug, thiserror::Error)]
pub enum PluginDispatchError {
    #[error("plugin disabled")]
    PluginDisabled,
    #[error("plugin not loaded")]
    PluginNotLoaded,
    #[error("handler missing")]
    HandlerMissing,
    #[error("execution failed: {0}")]
    Execution(String),
}

impl PluginDispatchError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::PluginDisabled => "plugin_disabled",
            Self::PluginNotLoaded => "plugin_not_loaded",
            Self::HandlerMissing => "handler_missing",
            Self::Execution(_) => "plugin_execution_error",
        }
    }
}
```

```rust
// crates/sushi-api/src/router.rs
if let Err(err) = ctx.plugins.dispatch_api_handler(method, path, dispatch_path, body).await {
    if err.reason_code() == "plugin_disabled" {
        return (
            axum::http::StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({"error":"plugin_disabled"})),
        )
            .into_response();
    }
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p sushi-core disabled_dispatch_has_plugin_disabled_reason -q`  
Expected: PASS

Run: `cargo test -p sushi-api test_plugin_api_dispatch_returns_403_when_plugin_disabled -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin/manager.rs crates/sushi-api/src/router.rs crates/sushi-admin/src/routes/workspace.rs crates/sushi-cli/src/commands/run.rs
git commit -m "feat(runtime): unify plugin dispatch reason codes"
```

### Task 8: Migrate KV Store and File Browser Official Plugins

**Files:**
- Modify: `plugins/official/kv-store/lua/bootstrap/register.lua`
- Modify: `plugins/official/file-browser/lua/bootstrap/register.lua`
- Test: `crates/sushi-core/src/lua/loader.rs` (source assertions)
- Test: `crates/sushi-core/tests/file_browser_plugin_behavior.rs`

- [ ] **Step 1: Write failing source assertions**

```rust
// crates/sushi-core/src/lua/loader.rs (test module)
#[test]
fn kv_bootstrap_uses_contract_registration() {
    let source = std::fs::read_to_string("plugins/official/kv-store/lua/bootstrap/register.lua").unwrap();
    assert!(source.contains("sushi.capability.register"));
    assert!(!source.contains("sushi.api.route("));
}

#[test]
fn file_browser_bootstrap_uses_contract_registration() {
    let source = std::fs::read_to_string("plugins/official/file-browser/lua/bootstrap/register.lua").unwrap();
    assert!(source.contains("sushi.capability.register"));
    assert!(!source.contains("sushi.api.route("));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sushi-core kv_bootstrap_uses_contract_registration -q`  
Expected: FAIL

Run: `cargo test -p sushi-core file_browser_bootstrap_uses_contract_registration -q`  
Expected: FAIL

- [ ] **Step 3: Migrate Lua bootstrap registration code**

```lua
-- plugins/official/kv-store/lua/bootstrap/register.lua
local M = {}

local function reg(surface, payload)
  payload.surface = surface
  sushi.capability.register(payload)
end

function M.register(deps)
  reg("api", { method = "GET", path = "/api/kv", handler = deps.api.dispatch, policy = "api.kv.read" })
  reg("api", { method = "POST", path = "/api/kv", handler = deps.api.dispatch, policy = "api.kv.write" })
  reg("admin", { path = "/admin/kv", title = "KV Store", template = "plugins/official/kv-store/kv.html", policy = "admin.kv.read", assets = { bundles = { "workspace" } } })
  reg("cli", { name = "kv-list", description = "List all KV entries", handler = deps.cli.kv_list, policy = "cli.kv.list" })
end

return M
```

```lua
-- plugins/official/file-browser/lua/bootstrap/register.lua
local M = {}

local function reg(payload)
  payload.surface = "api"
  sushi.capability.register(payload)
end

function M.register(app)
  reg({ method = "GET", path = "/app/files", handler = app.page, public = true })
  reg({ method = "GET", path = "/app/files/list/*", handler = app.list_partial, public = true })
  reg({ method = "POST", path = "/app/files/upload/*", handler = app.upload_file, public = true })
end

return M
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p sushi-core kv_bootstrap_uses_contract_registration -q`  
Expected: PASS

Run: `cargo test -p sushi-core file_browser_bootstrap_uses_contract_registration -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test file_browser_plugin_behavior -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/kv-store/lua/bootstrap/register.lua plugins/official/file-browser/lua/bootstrap/register.lua crates/sushi-core/src/lua/loader.rs crates/sushi-core/tests/file_browser_plugin_behavior.rs
git commit -m "refactor(plugin): migrate kv and file-browser to contract registration"
```

### Task 9: Migrate CMS Official Plugin

**Files:**
- Modify: `plugins/official/cms/lua/bootstrap/register.lua`
- Test: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`

- [ ] **Step 1: Write failing CMS assertion test**

```rust
// crates/sushi-core/src/lua/loader.rs (test module)
#[test]
fn cms_bootstrap_uses_contract_registration() {
    let source = std::fs::read_to_string("plugins/official/cms/lua/bootstrap/register.lua").unwrap();
    assert!(source.contains("sushi.capability.register"));
    assert!(!source.contains("sushi.api.route("));
    assert!(!source.contains("sushi.cli.command("));
    assert!(!source.contains("sushi.web.page("));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core cms_bootstrap_uses_contract_registration -q`  
Expected: FAIL

- [ ] **Step 3: Convert CMS bootstrap to contract payloads**

```lua
-- plugins/official/cms/lua/bootstrap/register.lua
local M = {}

local function reg(surface, payload)
  payload.surface = surface
  sushi.capability.register(payload)
end

function M.register(deps)
  reg("api", { method = "GET", path = "/api/cms/pages", handler = deps.api.pages_list, policy = "api.cms.read" })
  reg("api", { method = "POST", path = "/api/cms/pages", handler = deps.api.pages_create, policy = "api.cms.write" })
  reg("api", { method = "GET", path = "/app/cms", handler = deps.api.public_home, public = true })
  reg("admin", { path = "/admin/cms", title = "CMS", template = "plugins/official/cms/cms.html", policy = "admin.cms.read", assets = { bundles = { "workspace" } } })
  reg("cli", { name = "cms", description = "CMS CRUD command", handler = deps.cli.cms_dispatch, policy = "cli.cms.execute" })
end

return M
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p sushi-core cms_bootstrap_uses_contract_registration -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/lua/bootstrap/register.lua crates/sushi-core/src/lua/loader.rs crates/sushi-core/tests/cms_plugin_behavior.rs
git commit -m "refactor(plugin): migrate cms to contract registration"
```

### Task 10: Update Docs + Migration Guide

**Files:**
- Create: `docs/wiki/guides/lua-contract-migration.md`
- Modify: `docs/wiki/lua-api/README.md`
- Modify: `docs/wiki/lua-api/sushi.api.md`
- Modify: `docs/wiki/lua-api/sushi.admin.md`
- Modify: `docs/wiki/lua-api/sushi.cli.md`
- Modify: `docs/wiki/lua-api/sushi.web.md`
- Modify: `docs/wiki/lua-api/sushi.db.md`
- Modify: `docs/wiki/lua-api/sushi.event.md`
- Modify: `docs/wiki/lua-api/sushi.fs.md`
- Modify: `docs/engineering/plugin-authoring-standards.md`

- [ ] **Step 1: Write failing doc check**

Run: `rg -n "sushi\.capability\.register" docs/wiki/lua-api`  
Expected: FAIL with no matches.

- [ ] **Step 2: Write migration guide + contract-first examples**

````markdown
<!-- docs/wiki/guides/lua-contract-migration.md -->
# Lua Contract Migration Guide (V2)

## Removed registration APIs
- `sushi.api.route(...)`
- `sushi.cli.command(...)`
- `sushi.admin.page(...)`
- `sushi.web.page(...)`

## Required replacement
```lua
sushi.capability.register({
  surface = "api",
  method = "GET",
  path = "/api/items",
  handler = handlers.items,
  policy = "api.items.read"
})
```

## Security model
Capabilities not allowed by manifest + runtime governance are not injected into Lua.
````

- [ ] **Step 3: Update authoring standards with strict contract language**

```markdown
<!-- docs/engineering/plugin-authoring-standards.md -->
### Contract-first registration (required)
- Plugins register capabilities only via `sushi.capability.register({...})`.
- Legacy direct registration APIs are removed in this major upgrade.
- Permission model is deny-by-default at injection time.
```

- [ ] **Step 4: Re-run doc checks**

Run: `rg -n "sushi\.capability\.register|deny-by-default|Legacy direct registration APIs are removed" docs/wiki docs/engineering/plugin-authoring-standards.md`  
Expected: PASS with matches in updated files.

- [ ] **Step 5: Commit**

```bash
git add docs/wiki/guides/lua-contract-migration.md docs/wiki/lua-api/README.md docs/wiki/lua-api/sushi.api.md docs/wiki/lua-api/sushi.admin.md docs/wiki/lua-api/sushi.cli.md docs/wiki/lua-api/sushi.web.md docs/wiki/lua-api/sushi.db.md docs/wiki/lua-api/sushi.event.md docs/wiki/lua-api/sushi.fs.md docs/engineering/plugin-authoring-standards.md
git commit -m "docs(lua): publish v2 contract-first api and migration guide"
```

### Task 11: Final Verification

- [ ] **Step 1: Run new contract test suites**

Run: `cargo test -p sushi-core --test lua_contract_kernel -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test lua_contract_registry -q`  
Expected: PASS

- [ ] **Step 2: Run plugin and admin/api regression tests**

Run: `cargo test -p sushi-core --test file_browser_plugin_behavior -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

- [ ] **Step 3: Run full workspace gate**

Run: `cargo test --workspace -q`  
Expected: PASS
