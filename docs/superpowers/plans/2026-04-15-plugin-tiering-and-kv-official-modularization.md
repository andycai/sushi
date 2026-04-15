# Plugin Tiering and KV Official Modularization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add strict official/third-party plugin tiering with runtime-enforced permission policy, then migrate `kv-store` to `plugins/official/kv-store` with real multi-file Lua modules.

**Architecture:** The loader switches from flat discovery to category discovery (`plugins/official/*`, `plugins/third_party/*`) and refuses legacy top-level plugin folders. Each plugin gets a runtime `plugin_path_id` (`official/<name>` or `third_party/<name>`) used for templates/static assets. Official plugins run with forced full permissions while third-party plugins keep manifest permissions. A plugin-local safe `require` loader enables multi-file Lua plugin architecture without reopening global Lua filesystem access.

**Tech Stack:** Rust (`tokio`, `mlua`, `serde`, `toml`, `axum`), Lua 5.4, MiniJinja template loader, cargo tests.

---

## File Structure Map

- Modify: `crates/sushi-core/src/plugin/mod.rs` (add `plugin.kind` contract + tier metadata)
- Modify: `crates/sushi-core/src/plugin/manager.rs` (register effective permissions for UI/runtime metadata)
- Create: `crates/sushi-core/src/lua/module_loader.rs` (safe plugin-local Lua `require`)
- Modify: `crates/sushi-core/src/lua/mod.rs` (export module loader)
- Modify: `crates/sushi-core/src/lua/loader.rs` (tiered discovery, legacy rejection, path-id plumbing, effective permissions)
- Modify: `crates/sushi-cli/src/app.rs` (bootstrap wiring for tiered plugins/template roots/static roots)
- Modify: `crates/sushi-core/src/web/template_service.rs` (support `plugins/<tier>/<name>/...` template keys)
- Modify: `crates/sushi-admin/src/router.rs` (allow category-aware static mount ids)
- Modify: `crates/sushi-admin/tests/admin_web.rs` (category-aware static path tests)
- Modify: `crates/sushi-core/tests/template_service.rs` (category-aware template resolution tests)
- Modify: `crates/sushi-core/src/lua/loader.rs` tests (tiering + legacy-fatal + kv path updates)
- Move/Modify: `plugins/kv-store/*` -> `plugins/official/kv-store/*` (official plugin migration)
- Move/Modify: `plugins/_example/*` -> `plugins/third_party/_example/*` (prevent legacy-layout fatal startup)
- Create: `plugins/official/kv-store/lua/**` (modular Lua files)
- Modify: `docs/engineering/plugin-authoring-standards.md` (tiered layout, `plugin.kind`, official vs third-party rules)

---

### Task 1: Add Plugin Kind Contract and Effective Permission Policy

**Files:**
- Modify: `crates/sushi-core/src/plugin/mod.rs`
- Test: `crates/sushi-core/src/plugin/mod.rs` (`#[cfg(test)]` block)

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_parse_plugin_manifest_with_kind_official() {
    let toml_str = r#"
[plugin]
name = "kv-store"
version = "0.1.0"
entry = "init.lua"
kind = "official"
"#;

    let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
    assert_eq!(manifest.plugin.kind, PluginKind::Official);
}

#[test]
fn test_parse_plugin_manifest_requires_kind() {
    let toml_str = r#"
[plugin]
name = "missing-kind"
version = "0.1.0"
entry = "init.lua"
"#;

    let result: Result<PluginManifest, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn test_official_effective_permissions_are_forced_full_access() {
    let raw = Permissions {
        routes: false,
        commands: false,
        admin: false,
        database: DatabasePermission::None,
    };

    let effective = effective_permissions(PluginKind::Official, &raw);
    assert!(effective.routes);
    assert!(effective.commands);
    assert!(effective.admin);
    assert_eq!(effective.database, DatabasePermission::Admin);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core test_parse_plugin_manifest_with_kind_official test_parse_plugin_manifest_requires_kind test_official_effective_permissions_are_forced_full_access -q`
Expected: FAIL (`PluginMeta` has no `kind`; no effective permission helper).

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    Official,
    ThirdParty,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_entry")]
    pub entry: String,
    pub kind: PluginKind,
}

pub fn effective_permissions(kind: PluginKind, raw: &Permissions) -> Permissions {
    match kind {
        PluginKind::Official => Permissions {
            routes: true,
            commands: true,
            admin: true,
            database: DatabasePermission::Admin,
        },
        PluginKind::ThirdParty => raw.clone(),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core test_parse_plugin_manifest_with_kind_official test_parse_plugin_manifest_requires_kind test_official_effective_permissions_are_forced_full_access -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin/mod.rs
git commit -m "feat(core): add plugin kind contract and effective permission policy"
```

---

### Task 2: Implement Tiered Discovery and Legacy Layout Fatal Validation

**Files:**
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/src/lua/loader.rs` (`#[cfg(test)]` block)

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn test_scan_dir_loads_tiered_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    create_tiered_plugin_dir(tmp.path(), "official", "kv-store", "official");
    create_tiered_plugin_dir(tmp.path(), "third_party", "demo", "third_party");

    let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
    assert_eq!(plugins.len(), 2);
    assert!(plugins.iter().any(|p| p.path_id() == "official/kv-store"));
    assert!(plugins.iter().any(|p| p.path_id() == "third_party/demo"));
}

#[tokio::test]
async fn test_scan_dir_rejects_legacy_flat_plugin_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    create_flat_plugin_dir(tmp.path(), "legacy-plugin");

    let err = LuaPlugin::scan_dir(tmp.path()).await.unwrap_err();
    assert!(err.to_string().contains("legacy plugin directory layout is not supported"));
}

#[tokio::test]
async fn test_scan_dir_rejects_kind_category_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    create_tiered_plugin_dir(tmp.path(), "official", "bad", "third_party");

    let err = LuaPlugin::scan_dir(tmp.path()).await.unwrap_err();
    assert!(err.to_string().contains("plugin.kind does not match directory category"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core test_scan_dir_loads_tiered_plugins test_scan_dir_rejects_legacy_flat_plugin_dirs test_scan_dir_rejects_kind_category_mismatch -q`
Expected: FAIL (current scan only handles `plugins/<name>` flat structure).

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct LuaPlugin {
    manifest: PluginManifest,
    effective_permissions: Permissions,
    lua: Option<mlua::Lua>,
    plugin_dir: PathBuf,
    category: PluginKind,
    path_id: String,
}

pub async fn scan_dir(dir: &Path) -> Result<Vec<Self>, PluginError> {
    let mut plugins = Vec::new();

    // hard-fail legacy folders under plugins/*
    enforce_no_legacy_plugin_dirs(dir).await?;

    for (category_dir, category_kind) in [("official", PluginKind::Official), ("third_party", PluginKind::ThirdParty)] {
        let root = dir.join(category_dir);
        if !root.is_dir() {
            continue;
        }

        let mut entries = tokio::fs::read_dir(&root).await.map_err(PluginError::IoError)?;
        while let Some(entry) = entries.next_entry().await.map_err(PluginError::IoError)? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest = read_manifest(&path).await?;
            if manifest.plugin.kind != category_kind {
                return Err(PluginError::ManifestError(format!(
                    "plugin.kind does not match directory category: {} in {}",
                    manifest.plugin.name,
                    path.display()
                )));
            }

            let lua = create_sandboxed_vm().map_err(|e| {
                PluginError::LuaError(format!("create VM for {}: {e}", manifest.plugin.name))
            })?;

            let path_id = format!("{category_dir}/{}", manifest.plugin.name);
            plugins.push(Self {
                effective_permissions: effective_permissions(manifest.plugin.kind, &manifest.permissions),
                manifest,
                lua: Some(lua),
                plugin_dir: path,
                category: category_kind,
                path_id,
            });
        }
    }

    Ok(plugins)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core test_scan_dir_loads_tiered_plugins test_scan_dir_rejects_legacy_flat_plugin_dirs test_scan_dir_rejects_kind_category_mismatch -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs
git commit -m "feat(core): enforce tiered plugin discovery and legacy layout rejection"
```

---

### Task 3: Add Safe Plugin-Local Lua Module Loader (`require`)

**Files:**
- Create: `crates/sushi-core/src/lua/module_loader.rs`
- Modify: `crates/sushi-core/src/lua/mod.rs`
- Modify: `crates/sushi-core/src/lua/loader.rs` (install loader before entry execution)
- Test: `crates/sushi-core/src/lua/module_loader.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn require_loads_plugin_local_module() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_root = tmp.path().join("official").join("kv-store");
    std::fs::create_dir_all(plugin_root.join("lua/domain")).unwrap();
    std::fs::write(plugin_root.join("lua/domain/store.lua"), "return { ping = function() return 'ok' end }").unwrap();

    let lua = create_sandboxed_vm().unwrap();
    install_plugin_require(&lua, &plugin_root).unwrap();

    let value: String = lua.load("local m = require('domain.store'); return m.ping() ").eval().unwrap();
    assert_eq!(value, "ok");
}

#[test]
fn require_rejects_parent_traversal() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_root = tmp.path().join("official").join("kv-store");
    std::fs::create_dir_all(plugin_root.join("lua")).unwrap();

    let lua = create_sandboxed_vm().unwrap();
    install_plugin_require(&lua, &plugin_root).unwrap();

    let err = lua.load("return require('../secrets')").exec().unwrap_err();
    assert!(err.to_string().contains("unsafe module path"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core require_loads_plugin_local_module require_rejects_parent_traversal -q`
Expected: FAIL (sandbox has no `require`).

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/lua/module_loader.rs
pub fn install_plugin_require(lua: &mlua::Lua, plugin_root: &Path) -> mlua::Result<()> {
    let modules_root = plugin_root.join("lua");
    let cache = lua.create_table()?;

    let require_fn = lua.create_function(move |lua, module: String| {
        validate_module_name(&module)?;

        if let Ok(cached) = cache.get::<mlua::Value>(module.clone()) {
            if !matches!(cached, mlua::Value::Nil) {
                return Ok(cached);
            }
        }

        let rel = format!("{}.lua", module.replace('.', "/"));
        let file = safe_module_join(&modules_root, &rel)
            .ok_or_else(|| mlua::Error::RuntimeError(format!("unsafe module path: {module}")))?;

        let source = std::fs::read_to_string(&file)
            .map_err(|e| mlua::Error::RuntimeError(format!("read module {} failed: {e}", file.display())))?;

        let loaded: mlua::Value = lua.load(&source).set_name(&module).eval()?;
        let normalized = if matches!(loaded, mlua::Value::Nil) {
            mlua::Value::Boolean(true)
        } else {
            loaded
        };
        cache.set(module, normalized.clone())?;
        Ok(normalized)
    })?;

    lua.globals().set("require", require_fn)?;
    Ok(())
}
```

```rust
// crates/sushi-core/src/lua/loader.rs (inside init)
inject_sushi_api(lua, ctx, &self.effective_permissions)
    .await
    .map_err(|e| PluginError::LuaError(format!("inject API: {e}")))?;
install_plugin_require(lua, &self.plugin_dir)
    .map_err(|e| PluginError::LuaError(format!("install require: {e}")))?;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core require_loads_plugin_local_module require_rejects_parent_traversal -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/module_loader.rs crates/sushi-core/src/lua/mod.rs crates/sushi-core/src/lua/loader.rs
git commit -m "feat(core): add safe plugin-local lua module loader"
```

---

### Task 4: Switch Template and Static Asset Resolution to Category-Aware Path IDs

**Files:**
- Modify: `crates/sushi-core/src/web/template_service.rs`
- Modify: `crates/sushi-core/src/lua/loader.rs` (asset URL generation uses `plugin_path_id`)
- Modify: `crates/sushi-admin/src/router.rs` (static mount id validation)
- Test: `crates/sushi-core/tests/template_service.rs`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn render_plugin_template_from_tiered_plugin_template_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("base.html"), "<html>{% block body %}{% endblock %}</html>").unwrap();

    let plugin_templates = tempfile::tempdir().unwrap();
    std::fs::write(
        plugin_templates.path().join("page.html"),
        "{% extends \"base.html\" %}{% block body %}Tiered {{ title }}{% endblock %}",
    ).unwrap();

    let svc = TemplateService::new_with_plugin_roots(
        root.path(),
        vec![("official/kv-store".to_string(), plugin_templates.path().to_path_buf())],
    ).unwrap();

    let html = svc
        .render("plugins/official/kv-store/page.html", serde_json::json!({"title": "Plugin"}))
        .unwrap();
    assert_eq!(html, "<html>Tiered Plugin</html>");
}

#[tokio::test]
async fn plugin_static_assets_support_tiered_mount_id() {
    let plugin_static_dir = tempfile::tempdir().unwrap();
    std::fs::write(plugin_static_dir.path().join("kv.js"), "window.__tiered = true;").unwrap();

    let app = build_app_with_plugin_static("official/kv-store", plugin_static_dir.path()).await;
    let response = app
        .oneshot(Request::builder().uri("/static/plugins/official/kv-store/kv.js").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sushi-core render_plugin_template_from_tiered_plugin_template_root -q && cargo test -p sushi-admin plugin_static_assets_support_tiered_mount_id -q`
Expected: FAIL (template parser expects one segment after `plugins/`; admin router rejects `/` in plugin static id).

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/web/template_service.rs
fn split_plugin_template_name(name: &str) -> Option<(String, &str)> {
    let rest = name.strip_prefix("plugins/")?;
    let mut parts = rest.splitn(3, '/');
    let tier = parts.next()?;
    let plugin_name = parts.next()?;
    let plugin_path = parts.next()?;
    if tier.is_empty() || plugin_name.is_empty() || plugin_path.is_empty() {
        return None;
    }
    Some((format!("{tier}/{plugin_name}"), plugin_path))
}
```

```rust
// crates/sushi-core/src/lua/loader.rs
target.push(format!(
    "{static_url_prefix}/plugins/{plugin_path_id}/{}",
    normalized_path
));
```

```rust
// crates/sushi-admin/src/router.rs
fn is_valid_plugin_mount_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains("..")
        && !id.starts_with('/')
        && id
            .split('/')
            .all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

for (plugin_mount_id, plugin_static_root) in plugin_static_roots {
    if !is_valid_plugin_mount_id(&plugin_mount_id) {
        tracing::warn!("skip invalid plugin static mount name: {plugin_mount_id}");
        continue;
    }
    let mount_path = format!("{static_url_prefix}/plugins/{plugin_mount_id}");
    static_router = static_router.nest_service(&mount_path, ServeDir::new(plugin_static_root));
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core render_plugin_template_from_tiered_plugin_template_root -q && cargo test -p sushi-admin plugin_static_assets_support_tiered_mount_id -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/web/template_service.rs crates/sushi-core/src/lua/loader.rs crates/sushi-admin/src/router.rs crates/sushi-core/tests/template_service.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "refactor(core/admin): support tiered plugin template and static path ids"
```

---

### Task 5: Wire Bootstrap to Register Tiered Roots and Effective Permissions

**Files:**
- Modify: `crates/sushi-cli/src/app.rs`
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Test: `crates/sushi-core/src/plugin/manager.rs` (`#[cfg(test)]` block)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn register_plugin_manifest_with_effective_permissions_uses_effective_values() {
    let manager = PluginManager::new();
    let manifest: PluginManifest = toml::from_str(r#"
[plugin]
name = "kv-store"
version = "0.1.0"
kind = "official"
entry = "init.lua"
"#).unwrap();

    let effective = Permissions {
        routes: true,
        commands: true,
        admin: true,
        database: DatabasePermission::Admin,
    };

    manager
        .register_plugin_manifest_with_permissions(&manifest, &effective)
        .await;

    let items = manager.list_plugins().await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].permissions.database, "admin");
    assert!(items[0].permissions.routes);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core register_plugin_manifest_with_effective_permissions_uses_effective_values -q`
Expected: FAIL (method missing).

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/plugin/manager.rs
pub async fn register_plugin_manifest_with_permissions(
    &self,
    manifest: &PluginManifest,
    effective: &Permissions,
) {
    let mut plugin_info = self.plugin_info.write().await;
    let loaded = plugin_info
        .get(&manifest.plugin.name)
        .map(|item| item.loaded)
        .unwrap_or(false);

    plugin_info.insert(
        manifest.plugin.name.clone(),
        PluginInfo {
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            description: manifest.plugin.description.clone(),
            loaded,
            permissions: PluginPermissionsView {
                routes: effective.routes,
                commands: effective.commands,
                admin: effective.admin,
                database: db_permission_name(&effective.database).to_string(),
            },
        },
    );
}
```

```rust
// crates/sushi-cli/src/app.rs
for plugin in &lua_plugins {
    let template_root = plugin.web_templates_dir();
    if template_root.is_dir() {
        plugin_template_roots.push((plugin.path_id().to_string(), template_root));
    }
    let static_root = plugin.web_static_dir();
    if static_root.is_dir() {
        plugin_static_roots.push((plugin.path_id().to_string(), static_root));
    }
}

for plugin in lua_plugins {
    let plugin_name = plugin.name().to_string();
    ctx.plugins
        .register_plugin_manifest_with_permissions(plugin.manifest(), plugin.effective_permissions())
        .await;
    // unchanged: init, register_vm, error handling
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core register_plugin_manifest_with_effective_permissions_uses_effective_values -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-cli/src/app.rs crates/sushi-core/src/plugin/manager.rs
git commit -m "refactor(core/cli): register tiered roots and effective plugin permissions"
```

---

### Task 6: Migrate `kv-store` to Official Tier and Split into Lua Modules

**Files:**
- Move: `plugins/kv-store` -> `plugins/official/kv-store`
- Create: `plugins/official/kv-store/lua/utils/json.lua`
- Create: `plugins/official/kv-store/lua/utils/form.lua`
- Create: `plugins/official/kv-store/lua/utils/html.lua`
- Create: `plugins/official/kv-store/lua/infra/db.lua`
- Create: `plugins/official/kv-store/lua/domain/store.lua`
- Create: `plugins/official/kv-store/lua/interfaces/api.lua`
- Create: `plugins/official/kv-store/lua/interfaces/admin.lua`
- Create: `plugins/official/kv-store/lua/interfaces/cli.lua`
- Create: `plugins/official/kv-store/lua/bootstrap/register.lua`
- Modify: `plugins/official/kv-store/init.lua`
- Modify: `plugins/official/kv-store/plugin.toml`
- Move: `plugins/_example` -> `plugins/third_party/_example`

- [ ] **Step 1: Write the failing regression tests for new tiered path contract**

```rust
#[test]
fn kv_store_plugin_is_official_and_uses_tiered_template_paths() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/official/kv-store/plugin.toml");
    let source = std::fs::read_to_string(plugin_path).unwrap();
    assert!(source.contains("kind = \"official\""));

    let init_path = repo_root.join("plugins/official/kv-store/init.lua");
    let init_source = std::fs::read_to_string(init_path).unwrap();
    assert!(init_source.contains("plugins/official/kv-store/kv.html"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core kv_store_plugin_is_official_and_uses_tiered_template_paths -q`
Expected: FAIL (file still under `plugins/kv-store`).

- [ ] **Step 3: Write minimal implementation (filesystem move + modular Lua files)**

```bash
mkdir -p plugins/official plugins/third_party
git mv plugins/kv-store plugins/official/kv-store
git mv plugins/_example plugins/third_party/_example
mkdir -p plugins/official/kv-store/lua/{utils,infra,domain,interfaces,bootstrap}
```

```toml
# plugins/official/kv-store/plugin.toml
[plugin]
name = "kv-store"
version = "0.2.0"
description = "KV store management — API, admin UI, and CLI"
entry = "init.lua"
kind = "official"

[permissions]
routes = true
commands = true
admin = true
database = "admin"

[admin.assets.bundles.workspace]
js = ["kv.js"]
css = []
```

```lua
-- plugins/official/kv-store/init.lua
local utils_json = require("utils.json")
local utils_form = require("utils.form")
local infra_db = require("infra.db")
local domain_store = require("domain.store")
local api = require("interfaces.api")
local admin = require("interfaces.admin")
local cli = require("interfaces.cli")
local register = require("bootstrap.register")

function sushi.init()
    local deps = {
        json = utils_json,
        form = utils_form,
        db = infra_db,
        store = domain_store,
        api = api,
        admin = admin,
        cli = cli,
    }
    register.register(deps)
    sushi.log.info("kv-store official plugin initialized")
end
```

```lua
-- plugins/official/kv-store/lua/bootstrap/register.lua
local M = {}

function M.register(deps)
    sushi.api.route("GET", "/api/kv", deps.api.dispatch)
    sushi.api.route("GET", "/api/kv/*", deps.api.dispatch)
    sushi.api.route("POST", "/api/kv", deps.api.dispatch)
    sushi.api.route("PUT", "/api/kv/*", deps.api.dispatch)
    sushi.api.route("DELETE", "/api/kv/*", deps.api.delete_dispatch)

    sushi.api.route("GET", "/admin/partials/kv/table", deps.admin.table_partial)
    sushi.api.route("POST", "/admin/partials/kv/upsert", deps.admin.upsert_partial)
    sushi.api.route("POST", "/admin/partials/kv/delete", deps.admin.delete_partial)

    sushi.web.page("/admin/kv", "plugins/official/kv-store/kv.html", {
        title = "KV Store",
        assets = { bundles = { "workspace" } },
    })

    sushi.cli.command("kv-list", "List all KV entries", deps.cli.kv_list)
    sushi.cli.command("kv-get", "Get a KV entry by key", deps.cli.kv_get)
    sushi.cli.command("kv-set", "Set a KV entry (key + value)", deps.cli.kv_set)
    sushi.cli.command("kv-del", "Delete a KV entry by key", deps.cli.kv_del)
end

return M
```

- [ ] **Step 4: Run targeted tests to verify it passes**

Run: `cargo test -p sushi-core kv_store_plugin_is_official_and_uses_tiered_template_paths -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/official/kv-store plugins/third_party/_example crates/sushi-core/src/lua/loader.rs
git commit -m "refactor(plugin): migrate kv-store to official tier and modular lua files"
```

---

### Task 7: Update Remaining Tests and Documentation for Tiered Paths and Rules

**Files:**
- Modify: `crates/sushi-core/src/lua/loader.rs` tests (old `plugins/kv-store` path assertions)
- Modify: `crates/sushi-admin/tests/admin_web.rs` (old `/static/plugins/kv-store/...` assertions)
- Modify: `crates/sushi-core/tests/template_service.rs` (tiered plugin key)
- Modify: `docs/engineering/plugin-authoring-standards.md`

- [ ] **Step 1: Write failing checks (grep-based and targeted tests)**

```bash
rg -n "plugins/kv-store|/static/plugins/kv-store|plugins/<plugin-name>/" crates/sushi-core/src/lua/loader.rs crates/sushi-admin/tests/admin_web.rs crates/sushi-core/tests/template_service.rs docs/engineering/plugin-authoring-standards.md
```

Expected: Matches found that still use flat plugin paths.

- [ ] **Step 2: Run tests to verify current failures before update**

Run: `cargo test -p sushi-core kv_store_plugin_no_longer_embeds_html -q && cargo test -p sushi-admin plugin_static_assets_are_served_from_plugin_directories -q`
Expected: FAIL due to moved plugin paths and new mount ids.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/lua/loader.rs tests (example updates)
let plugin_path = repo_root.join("plugins/official/kv-store/init.lua");
assert!(plugin_source.contains("plugins/official/kv-store/kv.html"));
assert!(plugin_source.contains("sushi.web.render(\"plugins/official/kv-store/partials/flash.html\""));
```

```rust
// crates/sushi-admin/tests/admin_web.rs
let app = build_app_with_plugin_static("official/kv-store", &plugin_static_dir).await;
.uri("/static/plugins/official/kv-store/kv.js")
```

```markdown
# docs/engineering/plugin-authoring-standards.md (key contract section)
Each plugin lives under one of:
- `plugins/official/<plugin-name>/`
- `plugins/third_party/<plugin-name>/`

`plugin.toml` must set:

[plugin]
kind = "official" # or "third_party"

Legacy `plugins/<plugin-name>/` is not supported and causes startup failure.
Official plugins run with enforced full permissions; third-party plugins use declared permissions.
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p sushi-core -q && cargo test -p sushi-admin -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs crates/sushi-admin/tests/admin_web.rs crates/sushi-core/tests/template_service.rs docs/engineering/plugin-authoring-standards.md
git commit -m "test/docs: align paths and contracts with tiered plugin architecture"
```

---

### Task 8: Full Workspace Verification and Final Integration Commit

**Files:**
- Modify: none (verification + optional tiny fixes if failures found)
- Test: workspace-wide

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace -q`
Expected: PASS.

- [ ] **Step 2: Run focused startup path verification**

Run: `cargo run -p sushi-cli -- plugin list`
Expected: command succeeds and lists `kv-store` as loaded plugin.

- [ ] **Step 3: Verify no legacy plugin directories remain**

Run: `find plugins -mindepth 1 -maxdepth 1 -type d | sort`
Expected: only `plugins/official` and `plugins/third_party` are present at top level.

- [ ] **Step 4: Validate clean diff semantics**

Run: `git status --short`
Expected: empty working tree.

- [ ] **Step 5: Commit (only if Step 4 reveals intended uncommitted final fixes)**

```bash
git add -A
git commit -m "chore(release): finalize plugin tiering and kv official modularization"
```

---

## Spec Coverage Self-Review

- **Directory + kind dual validation:** Covered in Task 1 + Task 2.
- **Legacy flat folder immediate abandonment + fatal startup:** Covered in Task 2 and Task 8 verification.
- **Official full permission enforcement:** Covered in Task 1 + Task 5.
- **Category-aware external paths:** Covered in Task 4 + Task 7.
- **Safe modular Lua loader:** Covered in Task 3.
- **KV migration to official + modular files:** Covered in Task 6.
- **Third-party path continuity with permission limits:** Covered by Task 2, Task 5, and `_example` move in Task 6.

No uncovered spec requirement remains.

## Placeholder / Consistency Self-Check

- No `TODO`/`TBD` placeholders in tasks.
- All tasks include explicit file targets, runnable commands, and expected outcomes.
- Names are consistent across tasks (`plugin.kind`, `PluginKind`, `path_id`, `official/kv-store`).
