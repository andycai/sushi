# Official File Browser Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an official public web-only File Browser plugin at `/app/files` with multi-root, per-root capability controls, text edit support, upload, rename/delete/create actions, and download support.

**Architecture:** Keep all safety-critical filesystem logic in Rust runtime (`sushi-core` + `sushi-api`) and keep UI/workflow orchestration in `plugins/official/file-browser` Lua modules. Extend plugin route registration to support explicit `public` routes, and add `sushi.fs` as a constrained runtime API backed by validated plugin `plugin.toml` file-browser config.

**Tech Stack:** Rust (Axum/Tokio/mlua/serde), Lua plugin modules, HTMX + Alpine.js templates, SQLite-backed policy/auth snapshot already present in Sushi.

---

## Scope Check

The approved spec is a single cohesive subsystem (official file-browser plugin) plus narrowly scoped runtime support (`public` plugin routes + constrained `sushi.fs`). This should stay in one implementation plan.

## File Structure Map

- Create: `crates/sushi-core/src/fs/mod.rs` — constrained filesystem service (path validation, hidden/symlink policy, capability checks, list/read/write/create/rename/delete/upload/download token prep).
- Modify: `crates/sushi-core/src/lib.rs` — export new `fs` module.
- Modify: `crates/sushi-core/src/plugin/mod.rs` — parse/hold `[file_browser]` manifest config and per-root capabilities.
- Modify: `crates/sushi-core/src/plugin/manager.rs` — add plugin API route metadata for `public` + binary/query-aware dispatch support.
- Modify: `crates/sushi-core/src/lua/bindings.rs` — parse `sushi.api.route(..., { public = true })`, inject `sushi.fs`.
- Modify: `crates/sushi-core/src/lua/loader.rs` — validate file_browser config, wire fs service into Lua injection.
- Modify: `crates/sushi-api/src/router.rs` — implement plugin-route auth gate (public route bypass + existing policy checks), binary body support, query forwarding, download streaming endpoint.
- Modify: `crates/sushi-cli/src/commands/serve.rs` — stop layering generic `require_auth` over plugin router; rely on router-level plugin auth gate.
- Create: `crates/sushi-core/tests/file_browser_fs.rs` — runtime filesystem safety/capability tests.
- Create: `crates/sushi-core/tests/file_browser_plugin_behavior.rs` — plugin behavior integration tests against `PluginManager` + Lua runtime.
- Create: `docs/wiki/lua-api/sushi.fs.md` — runtime Lua API contract doc.
- Modify: `docs/wiki/lua-api/README.md` — add `sushi.fs` namespace.
- Create: `plugins/official/file-browser/plugin.toml`
- Create: `plugins/official/file-browser/init.lua`
- Create: `plugins/official/file-browser/lua/bootstrap/register.lua`
- Create: `plugins/official/file-browser/lua/interfaces/web.lua`
- Create: `plugins/official/file-browser/lua/domain/browser.lua`
- Create: `plugins/official/file-browser/lua/utils/form.lua`
- Create: `plugins/official/file-browser/lua/utils/path.lua`
- Create: `plugins/official/file-browser/web/templates/file_browser.html`
- Create: `plugins/official/file-browser/web/templates/fragments/list.html`
- Create: `plugins/official/file-browser/web/templates/fragments/editor.html`
- Create: `plugins/official/file-browser/web/templates/fragments/flash.html`
- Create: `plugins/official/file-browser/web/static/file_browser.js`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs` tests — official file-browser layout/contract assertions.
- Modify/Test: `crates/sushi-api/src/router.rs` tests — public-route bypass, private-route deny, query forwarding, binary upload body path.

---

### Task 1: Add `file_browser` Manifest Schema and Validation

**Files:**
- Modify: `crates/sushi-core/src/plugin/mod.rs`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Write failing manifest parse tests**

```rust
#[test]
fn parse_file_browser_config_from_manifest() {
    let manifest: PluginManifest = toml::from_str(
        r#"
[plugin]
name = "file-browser"
version = "0.1.0"
kind = "official"
entry = "init.lua"

[file_browser]
route_prefix = "/app/files"
hide_dotfiles = true
deny_symlink = true
text_extensions = ["txt", "md"]

[[file_browser.roots]]
id = "docs"
title = "Documents"
path = "/srv/docs"

[file_browser.roots.capabilities]
can_list = true
can_view_text = true
can_edit_text = true
can_create_text = true
can_create_dir = true
can_rename = true
can_delete = true
can_upload = true
can_download = true
"#,
    )
    .unwrap();

    let fb = manifest.file_browser.expect("file_browser missing");
    assert_eq!(fb.route_prefix, "/app/files");
    assert_eq!(fb.roots.len(), 1);
    assert_eq!(fb.roots[0].id, "docs");
    assert!(fb.roots[0].capabilities.can_upload);
}
```

- [ ] **Step 2: Run test to confirm failure**

Run: `cargo test -p sushi-core parse_file_browser_config_from_manifest -q`  
Expected: FAIL (`no field file_browser` on `PluginManifest`).

- [ ] **Step 3: Implement typed manifest structs**

```rust
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct FileBrowserConfig {
    #[serde(default = "default_file_browser_route_prefix")]
    pub route_prefix: String,
    #[serde(default = "default_true")]
    pub hide_dotfiles: bool,
    #[serde(default = "default_true")]
    pub deny_symlink: bool,
    #[serde(default)]
    pub text_extensions: Vec<String>,
    #[serde(default)]
    pub roots: Vec<FileBrowserRoot>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct FileBrowserRoot {
    pub id: String,
    pub title: String,
    pub path: String,
    pub capabilities: FileBrowserCapabilities,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct FileBrowserCapabilities {
    #[serde(default)]
    pub can_list: bool,
    #[serde(default)]
    pub can_view_text: bool,
    #[serde(default)]
    pub can_edit_text: bool,
    #[serde(default)]
    pub can_create_text: bool,
    #[serde(default)]
    pub can_create_dir: bool,
    #[serde(default)]
    pub can_rename: bool,
    #[serde(default)]
    pub can_delete: bool,
    #[serde(default)]
    pub can_upload: bool,
    #[serde(default)]
    pub can_download: bool,
}
```

- [ ] **Step 4: Add loader-time config validation**

```rust
fn validate_file_browser_config(cfg: &FileBrowserConfig) -> Result<(), PluginError> {
    if cfg.route_prefix.trim().is_empty() || !cfg.route_prefix.starts_with('/') {
        return Err(PluginError::InitFailed("file_browser.route_prefix must start with '/'".to_string()));
    }

    let mut ids = std::collections::HashSet::new();
    let mut canonical_roots = Vec::new();
    for root in &cfg.roots {
        if !root.id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
            return Err(PluginError::InitFailed(format!("invalid file_browser root id: {}", root.id)));
        }
        if !ids.insert(root.id.clone()) {
            return Err(PluginError::InitFailed(format!("duplicate file_browser root id: {}", root.id)));
        }
        let root_path = Path::new(&root.path);
        if !root_path.is_absolute() || !root_path.is_dir() {
            return Err(PluginError::InitFailed(format!("file_browser root must be existing absolute directory: {}", root.path)));
        }
        canonical_roots.push(std::fs::canonicalize(root_path).map_err(|e| PluginError::InitFailed(format!("canonicalize {} failed: {e}", root.path)))?);
    }
    for i in 0..canonical_roots.len() {
        for j in (i + 1)..canonical_roots.len() {
            if canonical_roots[i].starts_with(&canonical_roots[j]) || canonical_roots[j].starts_with(&canonical_roots[i]) {
                return Err(PluginError::InitFailed("file_browser roots must not overlap".to_string()));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run targeted tests**

Run: `cargo test -p sushi-core parse_file_browser_config_from_manifest -q`  
Expected: PASS

Run: `cargo test -p sushi-core test_scan_dir_finds_plugins -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/sushi-core/src/plugin/mod.rs crates/sushi-core/src/lua/loader.rs
git commit -m "feat(plugin): parse and validate file_browser manifest config"
```

### Task 2: Add Public Plugin Route Metadata + Binary/Query Dispatch

**Files:**
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Modify/Test: `crates/sushi-api/src/router.rs`

- [ ] **Step 1: Write failing tests for route `public` metadata and query forwarding**

```rust
#[tokio::test]
async fn api_route_public_flag_is_stored() {
    let manager = PluginManager::new();
    manager
        .register_api_handler_with_policy_and_public(
            "GET",
            "/app/files",
            "file-browser",
            "h_list",
            None,
            true,
        )
        .await;
    assert!(manager.is_api_route_public("GET", "/app/files").await);
}

#[tokio::test]
async fn plugin_api_dispatch_forwards_query_string_to_lua_handler() {
    // handler returns first arg (path) for assertion
    // expect "/app/files/list/docs?path=%2F"
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p sushi-core api_route_public_flag_is_stored -q`  
Expected: FAIL (`register_api_handler_with_policy_and_public` missing).

Run: `cargo test -p sushi-api plugin_api_dispatch_forwards_query_string_to_lua_handler -q`  
Expected: FAIL (query not forwarded).

- [ ] **Step 3: Extend Lua route options parsing**

```rust
fn parse_route_options(
    opts: Option<mlua::Table>,
) -> Result<(Option<String>, bool), mlua::Error> {
    let Some(table) = opts else { return Ok((None, false)); };
    let policy = parse_optional_policy(&table, "sushi.api.route")?;
    let public = match table.get::<mlua::Value>("public")? {
        mlua::Value::Nil => false,
        mlua::Value::Boolean(flag) => flag,
        _ => return Err(mlua::Error::RuntimeError("sushi.api.route opts.public must be boolean".to_string())),
    };
    if public && policy.is_some() {
        return Err(mlua::Error::RuntimeError("sushi.api.route cannot set both public and policy".to_string()));
    }
    Ok((policy, public))
}
```

- [ ] **Step 4: Add plugin manager route metadata and binary-aware dispatch API**

```rust
#[derive(Debug, Clone)]
struct ApiHandlerBinding {
    plugin_name: String,
    handler_key: String,
    policy_key: Option<String>,
    public: bool,
}

pub async fn is_api_route_public(&self, method: &str, path: &str) -> bool {
    let map = self.api_handlers.read().await;
    match_api_handler_binding(&map, method, path)
        .map(|binding| binding.public)
        .unwrap_or(false)
}

pub async fn call_api_handler_with_dispatch_path(
    &self,
    method: &str,
    path: &str,
    body: Option<Vec<u8>>,
    dispatch_path: &str,
) -> Option<Result<String, String>> {
    // first arg: dispatch_path (with query)
    // second arg: Lua binary string from body bytes if present
}
```

- [ ] **Step 5: Update router dispatch to pass query + bytes**

```rust
let path = req.uri().path().to_string();
let dispatch_path = match req.uri().query() {
    Some(q) if !q.is_empty() => format!("{path}?{q}"),
    _ => path.clone(),
};

let body = if method == "GET" {
    None
} else {
    Some(axum::body::to_bytes(req.into_body(), state.body_size_limit).await?.to_vec())
};

match state
    .plugins
    .call_api_handler_with_dispatch_path(&method, &path, body, &dispatch_path)
    .await
{
    // existing response handling
}
```

- [ ] **Step 6: Run focused tests**

Run: `cargo test -p sushi-core api_route_public_flag_is_stored -q`  
Expected: PASS

Run: `cargo test -p sushi-api plugin_api_dispatch_forwards_query_string_to_lua_handler -q`  
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/sushi-core/src/lua/bindings.rs crates/sushi-core/src/plugin/manager.rs crates/sushi-api/src/router.rs
git commit -m "feat(runtime): add public plugin route metadata and query-aware dispatch"
```

### Task 3: Implement Plugin Route Auth Gate (Public Bypass + Policy Protection)

**Files:**
- Modify: `crates/sushi-api/src/router.rs`
- Modify: `crates/sushi-cli/src/commands/serve.rs`

- [ ] **Step 1: Add failing auth tests for public/private plugin routes**

```rust
#[tokio::test]
async fn public_plugin_route_is_accessible_without_token() {
    // register GET /app/files as public=true
    // request without Authorization
    // expect 200
}

#[tokio::test]
async fn non_public_plugin_route_requires_auth() {
    // register GET /api/secure-file without public flag and with policy
    // request without Authorization
    // expect 401
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p sushi-api public_plugin_route_is_accessible_without_token -q`  
Expected: FAIL (401 from existing middleware).

- [ ] **Step 3: Move plugin auth decision into plugin dispatch path**

```rust
async fn ensure_plugin_route_access(
    state: &PluginApiState,
    req: &axum::extract::Request,
    method: &str,
    path: &str,
) -> Result<(), axum::response::Response> {
    if state.plugins.is_api_route_public(method, path).await {
        return Ok(());
    }

    let token = extract_bearer_or_cookie_token(req.headers()).ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, "{\"error\":\"Missing authorization credentials\"}").into_response()
    })?;

    let claims = state.jwt.verify_token(token).map_err(|_| {
        (StatusCode::UNAUTHORIZED, "{\"error\":\"Invalid token\"}").into_response()
    })?;

    let role = claims.role.clone();
    if state.authorizer.check_http(&role, "api", method, path).await.is_err() {
        return Err((StatusCode::FORBIDDEN, "{\"error\":\"Insufficient permissions for this API route\"}").into_response());
    }

    if path.starts_with("/admin/partials/") && role != "admin" {
        return Err((StatusCode::FORBIDDEN, "{\"error\":\"Admin role required for admin partial routes\"}").into_response());
    }

    Ok(())
}
```

- [ ] **Step 4: Remove `require_auth` wrapping from plugin router in serve command**

```rust
let plugin_api_router = sushi_api::router::build_plugin_api_routes(&ctx)
    .await
    .with_state(plugin_api_state);
```

- [ ] **Step 5: Run auth regression tests**

Run: `cargo test -p sushi-api public_plugin_route_is_accessible_without_token -q`  
Expected: PASS

Run: `cargo test -p sushi-api non_public_plugin_route_requires_auth -q`  
Expected: PASS

Run: `cargo test -p sushi-api test_build_app_requires_auth_for_users_route -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/sushi-api/src/router.rs crates/sushi-cli/src/commands/serve.rs
git commit -m "feat(api): support public plugin routes with inline plugin auth gate"
```

### Task 4: Implement `sushi.fs` Runtime Service and Lua Binding

**Files:**
- Create: `crates/sushi-core/src/fs/mod.rs`
- Modify: `crates/sushi-core/src/lib.rs`
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Create/Test: `crates/sushi-core/tests/file_browser_fs.rs`

- [ ] **Step 1: Add failing filesystem safety tests**

```rust
#[tokio::test]
async fn list_rejects_parent_directory_escape() {
    let svc = test_fs_service();
    let err = svc.list("docs", "../etc").await.unwrap_err();
    assert!(err.contains("invalid_path"));
}

#[tokio::test]
async fn read_text_rejects_non_whitelisted_extension() {
    let svc = test_fs_service();
    let err = svc.read_text("docs", "archive.bin").await.unwrap_err();
    assert!(err.contains("not_text_file"));
}

#[tokio::test]
async fn symlink_is_denied_when_policy_enabled() {
    let svc = test_fs_service();
    let err = svc.list("docs", "linked").await.unwrap_err();
    assert!(err.contains("forbidden_symlink"));
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p sushi-core --test file_browser_fs -q`  
Expected: FAIL (`module fs not found`).

- [ ] **Step 3: Implement constrained fs service**

```rust
pub struct FileBrowserFsService {
    route_prefix: String,
    hide_dotfiles: bool,
    deny_symlink: bool,
    text_extensions: std::collections::HashSet<String>,
    roots: std::collections::HashMap<String, RootConfig>,
}

impl FileBrowserFsService {
    pub async fn list(&self, root_id: &str, rel_path: &str) -> Result<Vec<FsEntry>, FsError> { /* ... */ }
    pub async fn read_text(&self, root_id: &str, rel_path: &str) -> Result<String, FsError> { /* ... */ }
    pub async fn write_text(&self, root_id: &str, rel_path: &str, content: &[u8]) -> Result<(), FsError> { /* ... */ }
    pub async fn create_text(&self, root_id: &str, rel_path: &str, content: &[u8]) -> Result<(), FsError> { /* ... */ }
    pub async fn mkdir(&self, root_id: &str, rel_path: &str) -> Result<(), FsError> { /* ... */ }
    pub async fn rename(&self, root_id: &str, from: &str, to: &str) -> Result<(), FsError> { /* ... */ }
    pub async fn delete(&self, root_id: &str, rel_path: &str) -> Result<(), FsError> { /* ... */ }
    pub async fn write_upload(&self, root_id: &str, rel_path: &str, bytes: &[u8]) -> Result<(), FsError> { /* ... */ }
    pub async fn prepare_download(&self, root_id: &str, rel_path: &str) -> Result<DownloadTicket, FsError> { /* ... */ }
}
```

- [ ] **Step 4: Inject `sushi.fs` API to Lua**

```rust
let fs_table = lua.create_table()?;
fs_table.set("list", lua.create_async_function(move |lua, (root_id, rel_path): (String, String)| async move {
    let entries = fs_service.list(&root_id, &rel_path).await.map_err(lua_fs_err)?;
    lua.to_value(&entries)
})?)?;
fs_table.set("read_text", lua.create_async_function(/* ... */)?)?;
fs_table.set("write_text", lua.create_async_function(/* ... */)?)?;
fs_table.set("write_upload", lua.create_async_function(/* ... */)?)?;
fs_table.set("prepare_download", lua.create_async_function(/* ... */)?)?;
sushi.set("fs", fs_table)?;
```

- [ ] **Step 5: Wire loader to build fs service from validated manifest**

```rust
let file_browser_service = self
    .manifest
    .file_browser
    .as_ref()
    .map(FileBrowserFsService::from_manifest)
    .transpose()?;

inject_sushi_api(
    lua,
    ctx,
    &self.effective_permissions,
    file_browser_service.as_ref(),
)
.await?;
```

- [ ] **Step 6: Run fs tests and Lua bindings tests**

Run: `cargo test -p sushi-core --test file_browser_fs -q`  
Expected: PASS

Run: `cargo test -p sushi-core --lib lua::bindings::tests::test_sushi_api_route_registration -q`  
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/sushi-core/src/fs/mod.rs crates/sushi-core/src/lib.rs crates/sushi-core/src/lua/bindings.rs crates/sushi-core/src/lua/loader.rs crates/sushi-core/tests/file_browser_fs.rs
git commit -m "feat(core): add constrained sushi.fs runtime API"
```

### Task 5: Build Official `file-browser` Plugin (Public Web UI Only)

**Files:**
- Create: `plugins/official/file-browser/plugin.toml`
- Create: `plugins/official/file-browser/init.lua`
- Create: `plugins/official/file-browser/lua/bootstrap/register.lua`
- Create: `plugins/official/file-browser/lua/interfaces/web.lua`
- Create: `plugins/official/file-browser/lua/domain/browser.lua`
- Create: `plugins/official/file-browser/lua/utils/form.lua`
- Create: `plugins/official/file-browser/lua/utils/path.lua`
- Create: `plugins/official/file-browser/web/templates/file_browser.html`
- Create: `plugins/official/file-browser/web/templates/fragments/list.html`
- Create: `plugins/official/file-browser/web/templates/fragments/editor.html`
- Create: `plugins/official/file-browser/web/templates/fragments/flash.html`
- Create: `plugins/official/file-browser/web/static/file_browser.js`

- [ ] **Step 1: Add failing loader contract tests for plugin presence/layout**

```rust
#[test]
fn file_browser_plugin_is_split_into_module_files() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    assert!(repo_root.join("plugins/official/file-browser/plugin.toml").is_file());
    assert!(repo_root.join("plugins/official/file-browser/init.lua").is_file());
    assert!(repo_root.join("plugins/official/file-browser/lua/bootstrap/register.lua").is_file());
    assert!(repo_root.join("plugins/official/file-browser/web/templates/file_browser.html").is_file());
    assert!(repo_root.join("plugins/official/file-browser/web/static/file_browser.js").is_file());
}
```

- [ ] **Step 2: Run test to confirm failure**

Run: `cargo test -p sushi-core file_browser_plugin_is_split_into_module_files -q`  
Expected: FAIL (missing files).

- [ ] **Step 3: Create plugin manifest + composition-root init**

```lua
-- plugins/official/file-browser/init.lua
local web = require("interfaces.web")
local register = require("bootstrap.register")

function sushi.init()
    local app = web.new()
    register.register(app)
    sushi.log.info("file-browser plugin: registered public web routes")
end
```

- [ ] **Step 4: Register only `/app/files/**` routes as `public = true`**

```lua
function M.register(app)
    sushi.api.route("GET", "/app/files", app.page, { public = true })
    sushi.api.route("GET", "/app/files/list/*", app.list_partial, { public = true })
    sushi.api.route("GET", "/app/files/open/*", app.open_partial, { public = true })
    sushi.api.route("POST", "/app/files/save/*", app.save_text, { public = true })
    sushi.api.route("POST", "/app/files/create-text", app.create_text, { public = true })
    sushi.api.route("POST", "/app/files/create-dir", app.create_dir, { public = true })
    sushi.api.route("POST", "/app/files/rename/*", app.rename_entry, { public = true })
    sushi.api.route("POST", "/app/files/delete/*", app.delete_entry, { public = true })
    sushi.api.route("POST", "/app/files/upload/*", app.upload_file, { public = true })
    sushi.api.route("GET", "/app/files/download/*", app.download_file, { public = true })
end
```

- [ ] **Step 5: Implement web handlers backed by `sushi.fs`**

```lua
function web.list_partial(args)
    local path = args[1] or "/app/files/list"
    local root_id, rel_path = path_utils.parse_root_and_rel(path, "/app/files/list/")
    local entries = sushi.fs.list(root_id, rel_path)
    return sushi.web.render("plugins/official/file-browser/fragments/list.html", {
        root_id = root_id,
        rel_path = rel_path,
        entries = entries,
    })
end

function web.upload_file(args)
    local path = args[1] or ""
    local body = args[2] or ""
    local root_id, rel_path = path_utils.parse_root_and_rel(path, "/app/files/upload/")
    sushi.fs.write_upload(root_id, rel_path, body)
    return sushi.web.render("plugins/official/file-browser/fragments/flash.html", {
        tone = "success",
        message = "Upload completed",
    })
end
```

- [ ] **Step 6: Add frontend HTMX/Alpine wiring**

```javascript
window.fileBrowserPage = function fileBrowserPage() {
  return {
    rootId: "",
    relPath: "",
    openPath(path) {
      const target = document.querySelector("#fb-main");
      htmx.ajax("GET", `/app/files/open/${path}`, { target, swap: "innerHTML" });
    },
    refreshList() {
      const target = document.querySelector("#fb-list");
      htmx.ajax("GET", `/app/files/list/${this.rootId}/${this.relPath}`, { target, swap: "innerHTML" });
    },
  };
};
```

- [ ] **Step 7: Run plugin loader contract tests**

Run: `cargo test -p sushi-core file_browser_plugin_is_split_into_module_files -q`  
Expected: PASS

Run: `cargo test -p sushi-core kv_store_plugin_bootstrap_registration_contract_is_stable -q`  
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add plugins/official/file-browser crates/sushi-core/src/lua/loader.rs
git commit -m "feat(plugin): add official public file-browser plugin"
```

### Task 6: Download Streaming + End-to-End Verification + Docs

**Files:**
- Modify: `crates/sushi-api/src/router.rs`
- Create/Test: `crates/sushi-core/tests/file_browser_plugin_behavior.rs`
- Create: `docs/wiki/lua-api/sushi.fs.md`
- Modify: `docs/wiki/lua-api/README.md`

- [ ] **Step 1: Add failing test for download endpoint behavior**

```rust
#[tokio::test]
async fn file_browser_download_returns_attachment_headers() {
    // register plugin route /app/files/download/*
    // call route and assert:
    // - 200 OK
    // - Content-Disposition contains filename
    // - body bytes match fixture
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p sushi-api file_browser_download_returns_attachment_headers -q`  
Expected: FAIL (current dispatcher always returns JSON/plain string).

- [ ] **Step 3: Add download response envelope handling in plugin dispatcher**

```rust
#[derive(serde::Deserialize)]
struct DownloadEnvelope {
    __sushi_file_download: bool,
    filename: String,
    mime: String,
    body_base64: String,
}

fn parse_download_envelope(body: &str) -> Option<(String, String, Vec<u8>)> {
    let parsed: DownloadEnvelope = serde_json::from_str(body).ok()?;
    if !parsed.__sushi_file_download { return None; }
    let bytes = base64::engine::general_purpose::STANDARD.decode(parsed.body_base64).ok()?;
    Some((parsed.filename, parsed.mime, bytes))
}
```

- [ ] **Step 4: Add end-to-end behavior tests for file-browser plugin**

```rust
#[tokio::test]
async fn file_browser_public_routes_work_without_token() {
    // setup temp root dir + plugin config
    // GET /app/files => 200
    // POST /app/files/create-dir => 200
    // POST /app/files/create-text => 200
    // POST /app/files/save/... => 200
    // GET /app/files/download/... => 200
}
```

- [ ] **Step 5: Document `sushi.fs` API**

```markdown
# sushi.fs

Constrained filesystem API for plugin-local configured roots.

## Methods
- `list(root_id, rel_path)`
- `read_text(root_id, rel_path)`
- `write_text(root_id, rel_path, content)`
- `create_text(root_id, rel_path, initial_content?)`
- `mkdir(root_id, rel_path)`
- `rename(root_id, from_rel_path, to_rel_path)`
- `delete(root_id, rel_path)`
- `write_upload(root_id, rel_path, bytes)`
- `prepare_download(root_id, rel_path)`
```

- [ ] **Step 6: Run full verification gate**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

Run: `cargo test --workspace -q`  
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/sushi-api/src/router.rs crates/sushi-core/tests/file_browser_plugin_behavior.rs docs/wiki/lua-api/sushi.fs.md docs/wiki/lua-api/README.md
git commit -m "feat(file-browser): add download response handling and e2e verification"
```

---

## Spec Self-Review Checklist

- Spec coverage:
  - Public web-only `/app/files/**` surface: Task 5
  - Multi-root + per-root capability config from `plugin.toml`: Task 1 + Task 4 + Task 5
  - `public` plugin route mechanism: Task 2 + Task 3
  - Text edit/create/dir-create/rename/delete/upload/download: Task 4 + Task 5 + Task 6
  - Hidden-path + symlink deny + path sandboxing: Task 4
  - Anonymous MVP with future auth path intact: Task 3 + Task 6 docs
- Placeholder scan: no `TODO`/`TBD`/“similar to”.
- Type consistency:
  - `file_browser` manifest struct names are consistent across Tasks 1 and 4.
  - `public` route metadata naming is consistent across Tasks 2 and 3.
  - `sushi.fs` method names are consistent across Tasks 4, 5, and docs in Task 6.

