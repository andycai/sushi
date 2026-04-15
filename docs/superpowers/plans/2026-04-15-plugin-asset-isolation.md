# Plugin Asset Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a strict plugin-owned admin asset pipeline where plugin pages declare JS/CSS lists, backend resolves and validates them, and frontend loads them dynamically without plugin hardcoding in `base.html`.

**Architecture:** Extend plugin manifest + Lua page registration with declarative asset lists, resolve assets during plugin initialization, persist resolved per-page assets in `PluginManager`, expose a protected admin assets API, and update the admin loader/workspace flow to fetch assets by path before rendering interactive page content.

**Tech Stack:** Rust (`axum`, `mlua`, `serde`, `toml`), Alpine.js/HTMX, Sushi plugin runtime, cargo tests.

---

## File Structure Map

- Modify: `crates/sushi-core/src/plugin/mod.rs` (manifest schema for admin asset bundles)
- Modify: `crates/sushi-core/src/lua/bindings.rs` (parse `sushi.web.page` options with `assets` fields)
- Modify: `crates/sushi-core/src/lua/loader.rs` (resolve + validate assets at plugin init)
- Modify: `crates/sushi-core/src/plugin/manager.rs` (store/query per-page resolved assets)
- Modify: `crates/sushi-cli/src/app.rs` (no behavior change target; keep plugin root wiring aligned)
- Modify: `crates/sushi-admin/src/router.rs` (new assets API route + permission mapping)
- Modify: `crates/sushi-admin/src/routes/workspace.rs` (assets API handler)
- Modify: `web/static/admin/js/module-loader.js` (runtime asset fetching/loading)
- Modify: `web/static/admin/js/workspace.js` (preload assets before HTMX swap)
- Modify: `web/templates/base.html` (remove plugin-specific module mapping)
- Modify: `plugins/kv-store/plugin.toml` (declare admin bundles)
- Modify: `plugins/kv-store/init.lua` (attach page asset config)
- Modify: `crates/sushi-admin/tests/admin_web.rs` (API + template regressions)
- Modify: `crates/sushi-core/tests/template_service.rs` (template root safety already present; extend as needed)
- Modify: `docs/engineering/plugin-authoring-standards.md` (final contract examples)

---

### Task 1: Add Plugin Admin Asset Schema to Manifest

**Files:**
- Modify: `crates/sushi-core/src/plugin/mod.rs`
- Test: `crates/sushi-core/src/plugin/mod.rs` (existing `#[cfg(test)]` block)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_parse_plugin_manifest_admin_asset_bundles() {
    let toml_str = r#"
[plugin]
name = "asset_plugin"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["kv.js", "shared/table.js"]
css = ["kv.css"]
"#;

    let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
    let workspace = manifest
        .admin
        .as_ref()
        .and_then(|admin| admin.assets.as_ref())
        .and_then(|assets| assets.bundles.get("workspace"))
        .expect("workspace bundle missing");

    assert_eq!(workspace.js, vec!["kv.js", "shared/table.js"]);
    assert_eq!(workspace.css, vec!["kv.css"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core test_parse_plugin_manifest_admin_asset_bundles -q`
Expected: FAIL (missing `admin` schema on `PluginManifest`).

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginAdminConfig {
    #[serde(default)]
    pub assets: Option<PluginAdminAssetsConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginAdminAssetsConfig {
    #[serde(default)]
    pub bundles: std::collections::BTreeMap<String, PluginAssetBundle>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginAssetBundle {
    #[serde(default)]
    pub js: Vec<String>,
    #[serde(default)]
    pub css: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub admin: Option<PluginAdminConfig>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core test_parse_plugin_manifest_admin_asset_bundles -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin/mod.rs
git commit -m "feat(core): add plugin admin asset bundle schema"
```

---

### Task 2: Parse `assets` in `sushi.web.page` Options

**Files:**
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Test: `crates/sushi-core/src/lua/bindings.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn test_lua_web_page_accepts_assets_lists() {
    let lua = create_sandboxed_vm().unwrap();
    let ctx = test_context().await;
    let mut permissions = Permissions::default();
    permissions.admin = true;

    inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

    lua.load(r#"
      sushi.web.page("/admin/lua", "admin/page.html", {
        title = "Lua Page",
        assets = {
          bundles = {"workspace"},
          js = {"pages/a.js", "pages/b.js"},
          css = {"pages/a.css"}
        }
      })
    "#).exec().unwrap();

    let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
    let pending: mlua::Table = sushi.get("__pending_pages").unwrap();
    let entry: mlua::Table = pending.get(1).unwrap();
    let assets: mlua::Table = entry.get("assets").unwrap();
    let js: mlua::Table = assets.get("js").unwrap();
    assert_eq!(js.raw_len(), 2);
}

#[tokio::test]
async fn test_lua_web_page_rejects_invalid_asset_path() {
    let lua = create_sandboxed_vm().unwrap();
    let ctx = test_context().await;
    let mut permissions = Permissions::default();
    permissions.admin = true;

    inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

    let result: mlua::Result<()> = lua.load(r#"
      sushi.web.page("/admin/lua", "admin/page.html", {
        assets = { js = {"../escape.js"} }
      })
    "#).exec();

    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sushi-core test_lua_web_page_accepts_assets_lists test_lua_web_page_rejects_invalid_asset_path -q`
Expected: FAIL (no assets parsing/validation in binding).

- [ ] **Step 3: Write minimal implementation**

```rust
// inside sushi.web.page binding parsing
let assets = parse_page_assets(&table.get::<mlua::Value>("assets")?)?;
entry.set("assets", assets_to_lua_table(lua, &assets)?)?;

fn validate_asset_relative_path(path: &str) -> Result<(), mlua::Error> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains("..")
        || path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("//")
    {
        return Err(mlua::Error::RuntimeError(format!(
            "invalid asset path: {path}"
        )));
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core test_lua_web_page_accepts_assets_lists test_lua_web_page_rejects_invalid_asset_path -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/bindings.rs
git commit -m "feat(core): parse page asset lists in lua web.page"
```

---

### Task 3: Resolve Bundles + Page Assets at Plugin Init

**Files:**
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Test: `crates/sushi-core/src/lua/loader.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn page_assets_resolve_bundle_then_page_assets() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_dir = tmp.path().join("asset_plugin");
    std::fs::create_dir_all(plugin_dir.join("web/static/pages")).unwrap();
    std::fs::write(plugin_dir.join("web/static/kv.js"), "console.log('kv')").unwrap();
    std::fs::write(plugin_dir.join("web/static/pages/extra.js"), "console.log('extra')").unwrap();

    let manifest: PluginManifest = toml::from_str(
        r#"
[plugin]
name = "asset_plugin"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["kv.js"]
"#,
    )
    .unwrap();

    let resolved = resolve_page_assets_for_test(
        "asset_plugin",
        &manifest,
        &["workspace".to_string()],
        &["pages/extra.js".to_string()],
        &[],
        &plugin_dir.join("web/static"),
    )
    .unwrap();

    assert_eq!(
        resolved.js,
        vec![
            "/static/plugins/asset_plugin/kv.js".to_string(),
            "/static/plugins/asset_plugin/pages/extra.js".to_string()
        ]
    );
}

#[tokio::test]
async fn page_assets_fail_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_dir = tmp.path().join("asset_plugin");
    std::fs::create_dir_all(plugin_dir.join("web/static")).unwrap();

    let manifest: PluginManifest = toml::from_str(
        r#"
[plugin]
name = "asset_plugin"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["missing.js"]
"#,
    )
    .unwrap();

    let err = resolve_page_assets_for_test(
        "asset_plugin",
        &manifest,
        &["workspace".to_string()],
        &[],
        &[],
        &plugin_dir.join("web/static"),
    )
    .unwrap_err();

    assert!(err.to_string().contains("missing.js"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sushi-core page_assets_resolve_bundle_then_page_assets page_assets_fail_when_file_missing -q`
Expected: FAIL (asset resolution/storage not implemented).

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PageResolvedAssets {
    pub js: Vec<String>,
    pub css: Vec<String>,
}

// in loader when reading __pending_pages:
let resolved_assets = resolve_page_assets(
    plugin_name,
    &self.manifest,
    &entry,
    &self.web_static_dir(),
)?;
ctx.plugins
    .register_admin_handler_with_assets(&path, plugin_name, &title, &handler_key, resolved_assets)
    .await;
```

```rust
fn resolve_page_assets(
    plugin_name: &str,
    manifest: &PluginManifest,
    bundle_names: &[String],
    page_js: &[String],
    page_css: &[String],
    static_root: &Path,
) -> Result<PageResolvedAssets, PluginError> {
    let mut out = PageResolvedAssets::default();
    let mut seen_js = std::collections::HashSet::new();
    let mut seen_css = std::collections::HashSet::new();

    for bundle in bundle_names {
        let def = manifest
            .admin
            .as_ref()
            .and_then(|admin| admin.assets.as_ref())
            .and_then(|assets| assets.bundles.get(bundle))
            .ok_or_else(|| PluginError::InitFailed(format!("unknown asset bundle: {bundle}")))?;
        push_assets(plugin_name, static_root, &def.js, &mut out.js, &mut seen_js)?;
        push_assets(plugin_name, static_root, &def.css, &mut out.css, &mut seen_css)?;
    }

    push_assets(plugin_name, static_root, page_js, &mut out.js, &mut seen_js)?;
    push_assets(plugin_name, static_root, page_css, &mut out.css, &mut seen_css)?;
    Ok(out)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core page_assets_resolve_bundle_then_page_assets page_assets_fail_when_file_missing -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs crates/sushi-core/src/plugin/manager.rs
git commit -m "feat(core): resolve and persist plugin page assets"
```

---

### Task 4: Expose Protected Workspace Assets API

**Files:**
- Modify: `crates/sushi-admin/src/routes/workspace.rs`
- Modify: `crates/sushi-admin/src/router.rs`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn workspace_assets_api_returns_plugin_assets_for_page_path() {
    let app = build_app(None).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/workspace/assets?path=/admin/kv")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_bearer_token()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-admin workspace_assets_api_returns_plugin_assets_for_page_path -q`
Expected: FAIL (route missing).

- [ ] **Step 3: Write minimal implementation**

```rust
// router.rs
.route(
  "/admin/api/workspace/assets",
  get(workspace::workspace_assets_api),
)
```

```rust
// workspace.rs
#[derive(serde::Serialize)]
struct WorkspaceAssetsResponse { js: Vec<String>, css: Vec<String> }

pub async fn workspace_assets_api(
    Query(query): Query<std::collections::HashMap<String, String>>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    let Some(path) = query.get("path").map(|v| v.trim()).filter(|v| !v.is_empty()) else {
        return (StatusCode::BAD_REQUEST, "missing path").into_response();
    };

    let assets = ctx.plugins.admin_page_assets(path).await.unwrap_or_default();
    axum::Json(WorkspaceAssetsResponse { js: assets.js, css: assets.css }).into_response()
}
```

```rust
// permission mapping
("GET", "/admin/api/workspace/assets") => "plugins.view"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-admin workspace_assets_api_returns_plugin_assets_for_page_path -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/src/router.rs crates/sushi-admin/src/routes/workspace.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): add workspace assets API for plugin pages"
```

---

### Task 5: Frontend Dynamic Asset Loader (No Plugin Hardcoding in Base)

**Files:**
- Modify: `web/static/admin/js/module-loader.js`
- Modify: `web/static/admin/js/workspace.js`
- Modify: `web/templates/base.html`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn base_template_has_no_plugin_specific_module_mappings() {
    let base = templates_root().join("base.html");
    let html = fs::read_to_string(&base).unwrap();

    assert!(!html.contains("/plugins/kv-store/kv.js"));
    assert!(!html.contains("kv:"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-admin base_template_has_no_plugin_specific_module_mappings -q`
Expected: FAIL (base currently includes `kv` mapping/script).

- [ ] **Step 3: Write minimal implementation**

```javascript
// module-loader.js
async function loadAssetsForPath(path) {
  const resp = await fetch(`/admin/api/workspace/assets?path=${encodeURIComponent(path)}`);
  if (!resp.ok) return;
  const payload = await resp.json();
  await loadCssList(payload.css || []);
  await loadJsList(payload.js || []);
}

function loadForPath(path) {
  return Promise.resolve()
    .then(() => loadModule(moduleFromPath(path)))
    .then(() => loadAssetsForPath(path));
}
```

```javascript
// workspace.js (keep current preload hook)
Promise.resolve(moduleLoader.loadForPath(path))
  .catch(() => false)
  .finally(() => requestPane());
```

```html
{# remove entries like: kv: "{{ static_prefix }}/plugins/kv-store/kv.js" #}
{# remove branch: {% elif active_section == "kv" %}<script src="{{ static_prefix }}/plugins/kv-store/kv.js" data-admin-module="kv" data-admin-module-loaded="true"></script>{% endif %} #}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-admin base_template_has_no_plugin_specific_module_mappings -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/static/admin/js/module-loader.js web/static/admin/js/workspace.js web/templates/base.html crates/sushi-admin/tests/admin_web.rs
git commit -m "refactor(admin): load plugin assets dynamically via workspace API"
```

---

### Task 6: Migrate KV Plugin to Declarative Asset Lists

**Files:**
- Modify: `plugins/kv-store/plugin.toml`
- Modify: `plugins/kv-store/init.lua`
- Modify: `crates/sushi-core/src/lua/loader.rs` (path assertions for moved assets)
- Modify: `crates/sushi-admin/tests/admin_web.rs` (kv template path checks)

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn kv_store_plugin_declares_admin_asset_bundles() {
    let plugin_path = workspace_root().join("plugins/kv-store/plugin.toml");
    let source = fs::read_to_string(plugin_path).unwrap();
    assert!(source.contains("[admin.assets.bundles.workspace]"));
    assert!(source.contains("js = [\"kv.js\"]"));
}
```

```rust
#[test]
fn kv_store_registration_uses_page_assets_option() {
    let plugin_path = workspace_root().join("plugins/kv-store/init.lua");
    let source = fs::read_to_string(plugin_path).unwrap();
    assert!(source.contains("assets = {"));
    assert!(source.contains("bundles = {\"workspace\"}"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sushi-core kv_store_plugin_declares_admin_asset_bundles kv_store_registration_uses_page_assets_option -q`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```toml
# plugins/kv-store/plugin.toml
[admin.assets.bundles.workspace]
js = ["kv.js"]
css = []
```

```lua
-- plugins/kv-store/init.lua
sushi.web.page("/admin/kv", "plugins/kv-store/kv.html", {
  title = "KV Store",
  assets = {
    bundles = {"workspace"}
  }
})
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core kv_store_plugin_declares_admin_asset_bundles kv_store_registration_uses_page_assets_option -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/kv-store/plugin.toml plugins/kv-store/init.lua crates/sushi-core/src/lua/loader.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "refactor(plugin kv): declare page assets via bundle config"
```

---

### Task 7: Enforce No Legacy Global Plugin Asset Directories

**Files:**
- Modify: `crates/sushi-admin/tests/admin_web.rs`
- Modify: `docs/engineering/plugin-authoring-standards.md`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn legacy_global_plugin_asset_dirs_have_no_files() {
    let root = workspace_root();
    let legacy_template_dir = root.join("web/templates/plugins");
    let legacy_static_dir = root.join("web/static/plugins");

    let has_legacy_templates = legacy_template_dir.exists()
        && legacy_template_dir.read_dir().unwrap().next().is_some();
    let has_legacy_static = legacy_static_dir.exists()
        && legacy_static_dir.read_dir().unwrap().next().is_some();

    assert!(!has_legacy_templates, "legacy template plugin directory must be empty");
    assert!(!has_legacy_static, "legacy static plugin directory must be empty");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-admin legacy_global_plugin_asset_dirs_have_no_files -q`
Expected: FAIL while legacy files still exist.

- [ ] **Step 3: Write minimal implementation**

```markdown
# docs/engineering/plugin-authoring-standards.md
- Do not place plugin files under `web/templates/plugins/**` or `web/static/plugins/**`.
- Plugin resources must be under `plugins/<name>/web/**`.
```

```bash
# remove or keep-empty legacy global plugin directories
find web/templates/plugins -type f -delete
find web/static/plugins -type f -delete
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-admin legacy_global_plugin_asset_dirs_have_no_files -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/tests/admin_web.rs docs/engineering/plugin-authoring-standards.md web/templates/plugins web/static/plugins
git commit -m "test(admin): enforce plugin resource isolation from global web dirs"
```

---

### Task 8: Final Verification Gate and Integration Commit

**Files:**
- Modify: (none required unless verification finds regressions)

- [ ] **Step 1: Run focused plugin/core tests**

Run: `cargo test -p sushi-core --test template_service -q && cargo test -p sushi-admin --test admin_web -q`
Expected: all PASS.

- [ ] **Step 2: Run workspace validation**

Run: `cargo test --workspace -q`
Expected: all PASS.

- [ ] **Step 3: Manual smoke check commands**

```bash
cargo run -p sushi-cli -- serve --config config.toml
# Open /admin/kv and /admin/plugins/<plugin>
# Verify no "openEdit is not defined" in browser console
```

Expected: plugin pages interactive on first load and workspace tab switch.

- [ ] **Step 4: Commit any final fixups**

```bash
git add -A
git commit -m "chore(admin): finalize plugin asset isolation integration"
```

- [ ] **Step 5: Prepare review notes**

```markdown
- Removed plugin script hardcoding from base template.
- Added declarative plugin asset bundles and page-level asset references.
- Added strict path/file validation and isolation tests.
```

---

## Self-Review

### 1) Spec coverage check

- Plugin-only directory ownership: covered by Tasks 6 and 7.
- Page-level asset lists + bundle model: covered by Tasks 1, 2, and 6.
- Runtime resolution and strict validation: covered by Task 3.
- Workspace asset API: covered by Task 4.
- Dynamic loader + remove base plugin hardcoding: covered by Task 5.
- Verification and acceptance commands: covered by Task 8.

### 2) Placeholder scan

- No `TBD`/`TODO` placeholders.
- Each task includes concrete files, commands, and code snippets.

### 3) Type and naming consistency

- Uses consistent names: `bundles`, `js`, `css`, `admin_page_assets`, `/admin/api/workspace/assets`.
- Path convention consistent across plan: plugin assets resolved to `/static/plugins/<plugin>/<path>`.
