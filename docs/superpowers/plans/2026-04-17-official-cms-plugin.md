# Official CMS Plugin (MVP) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an official `cms` Lua plugin that provides Page/Post/Category CRUD with soft delete, `/app` public SSR pages, `/admin/cms` admin workspace, and CLI operations.

**Architecture:** Add a new DB migration for CMS tables, then implement `plugins/official/cms` with the same layered structure used by `kv-store` (`infra`, `domain`, `interfaces`, `bootstrap`). Keep routing/policy registration centralized in `bootstrap/register.lua`, and verify behavior with loader contracts plus API/admin integration tests.

**Tech Stack:** Rust (Axum/Tokio tests, SQLite migrations), Lua plugin runtime (`mlua`), Sushi plugin APIs (`sushi.api`, `sushi.web`, `sushi.cli`, `sushi.db`), Alpine.js/HTMX templates for admin.

---

## Scope Check

This spec is one subsystem (official CMS plugin) with one small supporting runtime change (pass query string to plugin handlers so `/app/posts?category=...` works). It should stay in one implementation plan.

## File Structure Map

- Create: `migrations/007_cms.sql` — CMS schema (`cms_pages`, `cms_posts`, `cms_categories`) and indexes.
- Modify: `crates/sushi-cli/src/app.rs` — run migration 007 at bootstrap.
- Modify: `crates/sushi-admin/tests/admin_web.rs` — run migration 007 in test app builders.
- Modify: `crates/sushi-api/src/router.rs` — pass path+query to Lua handler args while matching by path.
- Modify: `crates/sushi-core/src/plugin/manager.rs` — support dispatch path override for API handlers.
- Create: `plugins/official/cms/plugin.toml`
- Create: `plugins/official/cms/init.lua`
- Create: `plugins/official/cms/lua/bootstrap/register.lua`
- Create: `plugins/official/cms/lua/infra/db.lua`
- Create: `plugins/official/cms/lua/utils/slug.lua`
- Create: `plugins/official/cms/lua/utils/validate.lua`
- Create: `plugins/official/cms/lua/utils/markdown.lua`
- Create: `plugins/official/cms/lua/domain/page.lua`
- Create: `plugins/official/cms/lua/domain/post.lua`
- Create: `plugins/official/cms/lua/domain/category.lua`
- Create: `plugins/official/cms/lua/interfaces/api.lua`
- Create: `plugins/official/cms/lua/interfaces/admin.lua`
- Create: `plugins/official/cms/lua/interfaces/cli.lua`
- Create: `plugins/official/cms/web/templates/cms.html`
- Create: `plugins/official/cms/web/templates/fragments/page_rows.html`
- Create: `plugins/official/cms/web/templates/fragments/post_rows.html`
- Create: `plugins/official/cms/web/templates/fragments/category_rows.html`
- Create: `plugins/official/cms/web/templates/fragments/flash.html`
- Create: `plugins/official/cms/web/templates/public/page_detail.html`
- Create: `plugins/official/cms/web/templates/public/post_list.html`
- Create: `plugins/official/cms/web/templates/public/post_detail.html`
- Create: `plugins/official/cms/web/templates/public/category_detail.html`
- Create: `plugins/official/cms/web/static/cms.js`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs` — CMS plugin structure/registration contract tests.
- Create/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs` — API-level behavioral tests (published visibility, soft delete, category delete guard).
- Modify/Test: `crates/sushi-api/src/router.rs` tests — query-forwarding contract.
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs` — `/admin/cms` and partial flow tests.

---

### Task 1: Add CMS Migration and Bootstrap Wiring

**Files:**
- Create: `migrations/007_cms.sql`
- Modify: `crates/sushi-cli/src/app.rs`
- Modify: `crates/sushi-admin/tests/admin_web.rs`
- Modify: `crates/sushi-api/src/router.rs`

- [ ] **Step 1: Write failing migration wiring checks**

```rust
// crates/sushi-cli/src/app.rs
const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
```

```rust
// crates/sushi-admin/tests/admin_web.rs
const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
```

```rust
// crates/sushi-api/src/router.rs (test module)
const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
```

- [ ] **Step 2: Run focused test to verify missing migration file**

Run: `cargo test -p sushi-api router::tests::test_plugin_api_dispatch_applies_status_envelope -q`  
Expected: FAIL with include error for `migrations/007_cms.sql`.

- [ ] **Step 3: Create migration and execute it in bootstrap/test builders**

```sql
-- migrations/007_cms.sql
CREATE TABLE IF NOT EXISTS cms_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS cms_pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    markdown_body TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS cms_posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    excerpt TEXT,
    markdown_body TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published')),
    category_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT,
    FOREIGN KEY (category_id) REFERENCES cms_categories(id)
);

CREATE INDEX IF NOT EXISTS idx_cms_pages_status_deleted ON cms_pages(status, deleted_at);
CREATE INDEX IF NOT EXISTS idx_cms_posts_status_deleted ON cms_posts(status, deleted_at);
CREATE INDEX IF NOT EXISTS idx_cms_posts_category_deleted ON cms_posts(category_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_cms_categories_slug_deleted ON cms_categories(slug, deleted_at);

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (7, '007_cms');
```

```rust
// crates/sushi-cli/src/app.rs
const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
// ...
storage
    .run_migrations(CMS_MIGRATION_SQL)
    .await
    .context("failed to run cms migrations")?;
```

```rust
// crates/sushi-admin/tests/admin_web.rs and crates/sushi-api/src/router.rs tests
storage
    .run_migrations(CMS_MIGRATION_SQL)
    .await
    .expect("failed to run migration 007_cms");
```

- [ ] **Step 4: Run migration-related checks**

Run: `cargo test -p sushi-api router::tests::test_plugin_api_dispatch_applies_status_envelope -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web admin_requires_auth_without_token -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add migrations/007_cms.sql crates/sushi-cli/src/app.rs crates/sushi-api/src/router.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(cms): add cms schema migration and bootstrap wiring"
```

### Task 2: Pass Query String to Plugin API Handlers

**Files:**
- Modify: `crates/sushi-api/src/router.rs`
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Modify/Test: `crates/sushi-api/src/router.rs` (test module)

- [ ] **Step 1: Add failing router test for query forwarding**

```rust
#[tokio::test]
async fn test_plugin_api_dispatch_forwards_path_query_to_lua_handler() {
    let lua = create_sandboxed_vm().unwrap();
    let sushi = lua.create_table().unwrap();
    let handlers = lua.create_table().unwrap();
    sushi.set("__handlers", handlers.clone()).unwrap();
    lua.globals().set("sushi", sushi).unwrap();

    let handler = lua
        .create_async_function(|_, args: mlua::Variadic<String>| async move {
            Ok(args.first().cloned().unwrap_or_default())
        })
        .unwrap();
    handlers.set("h_query", handler).unwrap();

    let manager = PluginManager::new();
    manager.register_vm("cms", lua).await;
    manager
        .register_api_handler("GET", "/app/posts", "cms", "h_query")
        .await;

    let state = PluginApiState {
        plugins: manager,
        logs: Arc::new(LogService::new()),
        body_size_limit: 1024,
        route_map: vec![],
    };

    let req = Request::builder()
        .method("GET")
        .uri("/app/posts?category=tech")
        .body(Body::empty())
        .unwrap();
    let response = plugin_api_dispatch(State(state), req).await.into_response();
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        String::from_utf8(bytes.to_vec()).unwrap(),
        "/app/posts?category=tech"
    );
}
```

- [ ] **Step 2: Run new test to confirm current behavior fails**

Run: `cargo test -p sushi-api router::tests::test_plugin_api_dispatch_forwards_path_query_to_lua_handler -q`  
Expected: FAIL because handler receives `/app/posts` without query.

- [ ] **Step 3: Implement path-match vs dispatch-path split**

```rust
// crates/sushi-api/src/router.rs (plugin_api_dispatch)
let path = req.uri().path().to_string();
let dispatch_path = match req.uri().query() {
    Some(query) if !query.is_empty() => format!("{path}?{query}"),
    _ => path.clone(),
};

match state
    .plugins
    .call_api_handler_with_dispatch_path(&method, &path, body, &dispatch_path)
    .await
{
    // unchanged response handling
}
```

```rust
// crates/sushi-core/src/plugin/manager.rs
pub async fn call_api_handler_with_dispatch_path(
    &self,
    method: &str,
    path: &str,
    body: Option<String>,
    dispatch_path: &str,
) -> Option<Result<String, String>> {
    let map = self.api_handlers.read().await;
    let binding = match_api_handler_binding(&map, method, path)?;
    let plugin_name = binding.plugin_name;
    let handler_key = binding.handler_key;
    drop(map);

    let args = match body {
        Some(b) => vec![dispatch_path.to_string(), b],
        None => vec![dispatch_path.to_string()],
    };
    Some(
        self.call_handler_with_args(&plugin_name, &handler_key, &args)
            .await,
    )
}
```

- [ ] **Step 4: Add regression coverage for existing call path**

```rust
// keep old method for compatibility
pub async fn call_api_handler(
    &self,
    method: &str,
    path: &str,
    body: Option<String>,
) -> Option<Result<String, String>> {
    self.call_api_handler_with_dispatch_path(method, path, body, path)
        .await
}
```

- [ ] **Step 5: Run router/manager tests**

Run: `cargo test -p sushi-api router::tests::test_plugin_api_dispatch_forwards_path_query_to_lua_handler -q`  
Expected: PASS

Run: `cargo test -p sushi-core plugin::manager::tests::call_api_handler_matches_wildcards -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/sushi-api/src/router.rs crates/sushi-core/src/plugin/manager.rs
git commit -m "feat(api): forward query strings to plugin handler args"
```

### Task 3: Scaffold Official CMS Plugin and Registration Contracts

**Files:**
- Create: `plugins/official/cms/plugin.toml`
- Create: `plugins/official/cms/init.lua`
- Create: `plugins/official/cms/lua/bootstrap/register.lua`
- Create: `plugins/official/cms/lua/interfaces/api.lua`
- Create: `plugins/official/cms/lua/interfaces/admin.lua`
- Create: `plugins/official/cms/lua/interfaces/cli.lua`
- Create: `plugins/official/cms/lua/domain/page.lua`
- Create: `plugins/official/cms/lua/domain/post.lua`
- Create: `plugins/official/cms/lua/domain/category.lua`
- Create: `plugins/official/cms/lua/infra/db.lua`
- Create: `plugins/official/cms/lua/utils/slug.lua`
- Create: `plugins/official/cms/lua/utils/validate.lua`
- Create: `plugins/official/cms/lua/utils/markdown.lua`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing loader characterization tests**

```rust
#[test]
fn cms_plugin_files_exist_and_are_modular() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    assert!(root.join("plugins/official/cms/plugin.toml").is_file());
    assert!(root.join("plugins/official/cms/lua/interfaces/api.lua").is_file());
    assert!(root.join("plugins/official/cms/lua/interfaces/admin.lua").is_file());
    assert!(root.join("plugins/official/cms/lua/interfaces/cli.lua").is_file());
}

#[test]
fn cms_plugin_registration_contract_is_stable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let source = std::fs::read_to_string(
        root.join("plugins/official/cms/lua/bootstrap/register.lua"),
    )
    .unwrap();
    assert!(source.contains("sushi.web.page(\"/admin/cms\""));
    assert!(source.contains("sushi.api.route(\"GET\", \"/app/posts\""));
    assert!(source.contains("sushi.cli.command(\"cms\""));
}
```

- [ ] **Step 2: Run test to verify scaffolding is missing**

Run: `cargo test -p sushi-core cms_plugin_files_exist_and_are_modular -q`  
Expected: FAIL with missing `plugins/official/cms/...`.

- [ ] **Step 3: Create plugin manifest and composition root**

```toml
# plugins/official/cms/plugin.toml
[plugin]
name = "cms"
version = "0.1.0"
description = "Official CMS plugin with page/post/category modules"
entry = "init.lua"
kind = "official"

[permissions]
routes = true
commands = true
admin = true
database = "admin"

[policies]
scopes = [
  "api.cms.*",
  "admin.cms.*",
  "cli.cms.*"
]

[admin.assets.bundles.workspace]
js = ["cms.js"]
css = []
```

```lua
-- plugins/official/cms/init.lua
local db = require("infra.db")
local slug = require("utils.slug")
local validate = require("utils.validate")
local markdown = require("utils.markdown")

local page_domain = require("domain.page")
local post_domain = require("domain.post")
local category_domain = require("domain.category")

local api_factory = require("interfaces.api")
local admin_factory = require("interfaces.admin")
local cli_factory = require("interfaces.cli")
local bootstrap = require("bootstrap.register")

function sushi.init()
    local deps = { db = db, slug = slug, validate = validate, markdown = markdown }
    local page = page_domain.new(deps)
    local post = post_domain.new(deps)
    local category = category_domain.new(deps)

    local api = api_factory.new({ page = page, post = post, category = category, markdown = markdown })
    local admin = admin_factory.new({ page = page, post = post, category = category, markdown = markdown })
    local cli = cli_factory.new({ page = page, post = post, category = category })

    bootstrap.register({ api = api, admin = admin, cli = cli })
    sushi.log.info("cms plugin: registered API routes, admin page, and CLI commands")
end
```

- [ ] **Step 4: Implement route/page/command registration map**

```lua
-- plugins/official/cms/lua/bootstrap/register.lua
local M = {}

function M.register(deps)
    sushi.api.route("GET", "/api/cms/pages", deps.api.pages_list, { policy = "api.cms.pages.read" })
    sushi.api.route("POST", "/api/cms/pages", deps.api.pages_create, { policy = "api.cms.pages.write" })
    sushi.api.route("PUT", "/api/cms/pages/*", deps.api.pages_update, { policy = "api.cms.pages.write" })
    sushi.api.route("DELETE", "/api/cms/pages/*", deps.api.pages_delete, { policy = "api.cms.pages.delete" })

    sushi.api.route("GET", "/api/cms/posts", deps.api.posts_list, { policy = "api.cms.posts.read" })
    sushi.api.route("POST", "/api/cms/posts", deps.api.posts_create, { policy = "api.cms.posts.write" })
    sushi.api.route("PUT", "/api/cms/posts/*", deps.api.posts_update, { policy = "api.cms.posts.write" })
    sushi.api.route("DELETE", "/api/cms/posts/*", deps.api.posts_delete, { policy = "api.cms.posts.delete" })

    sushi.api.route("GET", "/api/cms/categories", deps.api.categories_list, { policy = "api.cms.categories.read" })
    sushi.api.route("POST", "/api/cms/categories", deps.api.categories_create, { policy = "api.cms.categories.write" })
    sushi.api.route("PUT", "/api/cms/categories/*", deps.api.categories_update, { policy = "api.cms.categories.write" })
    sushi.api.route("DELETE", "/api/cms/categories/*", deps.api.categories_delete, { policy = "api.cms.categories.delete" })

    sushi.api.route("GET", "/app/pages/*", deps.api.public_page_detail, { policy = "api.cms.public.read" })
    sushi.api.route("GET", "/app/posts", deps.api.public_post_list, { policy = "api.cms.public.read" })
    sushi.api.route("GET", "/app/posts/*", deps.api.public_post_detail, { policy = "api.cms.public.read" })
    sushi.api.route("GET", "/app/categories/*", deps.api.public_category_detail, { policy = "api.cms.public.read" })

    sushi.api.route("GET", "/admin/partials/cms/pages/table", deps.admin.pages_table_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/pages/upsert", deps.admin.pages_upsert_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/pages/delete", deps.admin.pages_delete_partial, { policy = "admin.cms.write" })
    sushi.api.route("GET", "/admin/partials/cms/posts/table", deps.admin.posts_table_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/posts/upsert", deps.admin.posts_upsert_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/posts/delete", deps.admin.posts_delete_partial, { policy = "admin.cms.write" })
    sushi.api.route("GET", "/admin/partials/cms/categories/table", deps.admin.categories_table_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/categories/upsert", deps.admin.categories_upsert_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/categories/delete", deps.admin.categories_delete_partial, { policy = "admin.cms.write" })

    sushi.web.page("/admin/cms", "plugins/official/cms/cms.html", {
        title = "CMS",
        policy = "admin.cms.read",
        assets = { bundles = { "workspace" } },
    })

    sushi.cli.command("cms", "CMS CRUD command", deps.cli.cms_dispatch, { policy = "cli.cms.execute" })
end

return M
```

- [ ] **Step 5: Run loader tests and commit**

Run: `cargo test -p sushi-core cms_plugin_registration_contract_is_stable -q`  
Expected: PASS

```bash
git add plugins/official/cms crates/sushi-core/src/lua/loader.rs
git commit -m "feat(cms): scaffold official cms plugin and registration map"
```

### Task 4: Implement Shared Utils and DB Access Layer

**Files:**
- Create: `plugins/official/cms/lua/infra/db.lua`
- Create: `plugins/official/cms/lua/utils/slug.lua`
- Create: `plugins/official/cms/lua/utils/validate.lua`
- Create: `plugins/official/cms/lua/utils/markdown.lua`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing utility contract tests**

```rust
#[test]
fn cms_utils_contract_is_stable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let slug = std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/slug.lua")).unwrap();
    let validate = std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/validate.lua")).unwrap();
    let markdown = std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/markdown.lua")).unwrap();

    assert!(slug.contains("function M.normalize"));
    assert!(validate.contains("function M.validate_status"));
    assert!(markdown.contains("function M.to_html"));
}
```

- [ ] **Step 2: Run test to confirm utility functions are not implemented yet**

Run: `cargo test -p sushi-core cms_utils_contract_is_stable -q`  
Expected: FAIL on missing function markers.

- [ ] **Step 3: Implement DB wrapper and validators**

```lua
-- plugins/official/cms/lua/infra/db.lua
local M = {}

function M.query(sql, params)
    local ok, rows_or_err = pcall(function()
        return sushi.db.query(sql, params or {})
    end)
    if not ok then
        return nil, "storage_error", tostring(rows_or_err)
    end
    return rows_or_err
end

function M.execute(sql, params)
    local ok, result_or_err = pcall(function()
        return sushi.db.execute(sql, params or {})
    end)
    if not ok then
        return nil, "storage_error", tostring(result_or_err)
    end
    return result_or_err
end

return M
```

```lua
-- plugins/official/cms/lua/utils/validate.lua
local M = {}
local STATUS = { draft = true, published = true }

function M.validate_status(value)
    if not STATUS[value] then
        return nil, "invalid_status", "status must be draft or published"
    end
    return value
end

function M.require_non_empty(value, field)
    local text = tostring(value or "")
    if text == "" then
        return nil, "invalid_" .. field, field .. " cannot be empty"
    end
    return text
end

return M
```

```lua
-- plugins/official/cms/lua/utils/slug.lua
local M = {}

function M.normalize(text)
    local value = tostring(text or ""):lower()
    value = value:gsub("[^%w%s%-_]", "")
    value = value:gsub("[%s_]+", "-")
    value = value:gsub("%-+", "-")
    value = value:gsub("^%-", ""):gsub("%-$", "")
    return value
end

return M
```

```lua
-- plugins/official/cms/lua/utils/markdown.lua
local M = {}

local function escape_html(input)
    local out = tostring(input or "")
    out = out:gsub("&", "&amp;")
    out = out:gsub("<", "&lt;")
    out = out:gsub(">", "&gt;")
    out = out:gsub("\"", "&quot;")
    out = out:gsub("'", "&#39;")
    return out
end

function M.to_html(markdown)
    local escaped = escape_html(markdown)
    escaped = escaped:gsub("\r\n", "\n")
    escaped = escaped:gsub("\n\n+", "</p><p>")
    return "<p>" .. escaped .. "</p>"
end

return M
```

- [ ] **Step 4: Run utility contract tests**

Run: `cargo test -p sushi-core cms_utils_contract_is_stable -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/lua/infra/db.lua plugins/official/cms/lua/utils/slug.lua plugins/official/cms/lua/utils/validate.lua plugins/official/cms/lua/utils/markdown.lua crates/sushi-core/src/lua/loader.rs
git commit -m "feat(cms): add shared db and utility modules"
```

### Task 5: Implement Domain Modules (Page/Post/Category) with Business Rules

**Files:**
- Create: `plugins/official/cms/lua/domain/page.lua`
- Create: `plugins/official/cms/lua/domain/post.lua`
- Create: `plugins/official/cms/lua/domain/category.lua`
- Create/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`

- [ ] **Step 1: Write failing behavior tests for soft delete and category delete guard**

```rust
#[tokio::test]
async fn cms_soft_deleted_posts_are_hidden_from_list() {
    let harness = CmsHarness::boot().await;
    harness.create_category("news").await;
    harness
        .create_post("hello-world", "Hello World", "news", "published")
        .await;
    harness.delete_post("hello-world").await;

    let body = harness.get_json("/api/cms/posts").await;
    let items = body.get("items").and_then(Value::as_array).unwrap();
    assert!(items
        .iter()
        .all(|item| item.get("slug").and_then(Value::as_str) != Some("hello-world")));
}

#[tokio::test]
async fn cms_category_delete_conflicts_when_posts_exist() {
    let harness = CmsHarness::boot().await;
    harness.create_category("news").await;
    harness
        .create_post("news-post", "News Post", "news", "published")
        .await;

    let response = harness.delete_category("news").await;
    assert_eq!(response.status, 409);
    assert!(response
        .body
        .get("error")
        .and_then(Value::as_str)
        .unwrap()
        .contains("category"));
}
```

- [ ] **Step 2: Run new behavior tests**

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: FAIL because domain modules/handlers are not implemented yet.

- [ ] **Step 3: Implement page domain CRUD and visibility filtering**

```lua
-- plugins/official/cms/lua/domain/page.lua
local M = {}

function M.new(deps)
    local db = deps.db
    local validate = deps.validate
    local slug = deps.slug
    local page = {}

    function page.list()
        return db.query(
            "SELECT id, title, slug, status, created_at, updated_at FROM cms_pages WHERE deleted_at IS NULL ORDER BY updated_at DESC",
            {}
        )
    end

    function page.get_by_slug(value, opts)
        local only_published = opts and opts.only_published
        local sql = "SELECT id, title, slug, markdown_body, status FROM cms_pages WHERE slug = ?1 AND deleted_at IS NULL"
        if only_published then
            sql = sql .. " AND status = 'published'"
        end
        local rows = db.query(sql, { value })
        if not rows or #rows == 0 then
            return nil, "not_found", "page not found"
        end
        return rows[1]
    end

    function page.upsert(payload, original_slug)
        local title = validate.require_non_empty(payload.title, "title")
        local normalized_slug = slug.normalize(validate.require_non_empty(payload.slug, "slug"))
        local body = validate.require_non_empty(payload.markdown_body, "markdown_body")
        local status = validate.validate_status(payload.status or "draft")
        if not title or not normalized_slug or not body or not status then
            return nil, "invalid_input", "invalid page payload"
        end
        if original_slug and original_slug ~= "" then
            db.execute(
                "UPDATE cms_pages SET title = ?1, slug = ?2, markdown_body = ?3, status = ?4, updated_at = datetime('now') WHERE slug = ?5 AND deleted_at IS NULL",
                { title, normalized_slug, body, status, original_slug }
            )
        else
            db.execute(
                "INSERT INTO cms_pages (title, slug, markdown_body, status) VALUES (?1, ?2, ?3, ?4)",
                { title, normalized_slug, body, status }
            )
        end
        return page.get_by_slug(normalized_slug)
    end

    function page.soft_delete(value)
        db.execute(
            "UPDATE cms_pages SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE slug = ?1 AND deleted_at IS NULL",
            { value }
        )
        return true
    end

    return page
end

return M
```

- [ ] **Step 4: Implement category/post domain with delete guard**

```lua
-- plugins/official/cms/lua/domain/category.lua (critical guard)
function category.soft_delete(slug_value)
    local rows = db.query("SELECT id FROM cms_categories WHERE slug = ?1 AND deleted_at IS NULL", { slug_value })
    if not rows or #rows == 0 then
        return nil, "not_found", "category not found"
    end
    local category_id = rows[1].id
    local refs = db.query(
        "SELECT id FROM cms_posts WHERE category_id = ?1 AND deleted_at IS NULL LIMIT 1",
        { category_id }
    )
    if refs and #refs > 0 then
        return nil, "conflict_has_posts", "category still has posts"
    end
    db.execute(
        "UPDATE cms_categories SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
        { category_id }
    )
    return true
end
```

```lua
-- plugins/official/cms/lua/domain/post.lua (published filters)
function post.list(opts)
    local only_published = opts and opts.only_published
    local where = "p.deleted_at IS NULL"
    local params = {}
    if only_published then
        where = where .. " AND p.status = 'published'"
    end
    if opts and opts.category_slug and opts.category_slug ~= "" then
        where = where .. " AND c.slug = ?1"
        params = { opts.category_slug }
    end
    return db.query(
        "SELECT p.id, p.title, p.slug, p.excerpt, p.status, c.slug AS category_slug, c.name AS category_name " ..
        "FROM cms_posts p JOIN cms_categories c ON c.id = p.category_id " ..
        "WHERE " .. where .. " ORDER BY p.updated_at DESC",
        params
    )
end
```

- [ ] **Step 5: Run behavior tests**

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add plugins/official/cms/lua/domain/page.lua plugins/official/cms/lua/domain/post.lua plugins/official/cms/lua/domain/category.lua crates/sushi-core/tests/cms_plugin_behavior.rs
git commit -m "feat(cms): add domain rules for soft delete and category guard"
```

### Task 6: Implement API and Public SSR Interfaces

**Files:**
- Create: `plugins/official/cms/lua/interfaces/api.lua`
- Create: `plugins/official/cms/web/templates/public/page_detail.html`
- Create: `plugins/official/cms/web/templates/public/post_list.html`
- Create: `plugins/official/cms/web/templates/public/post_detail.html`
- Create: `plugins/official/cms/web/templates/public/category_detail.html`
- Modify/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`

- [ ] **Step 1: Add failing API contract tests**

```rust
#[tokio::test]
async fn cms_public_page_route_hides_draft_content() {
    let harness = CmsHarness::boot().await;
    harness
        .create_page("internal-roadmap", "Internal Roadmap", "draft")
        .await;

    let response = harness.get_raw("/app/pages/internal-roadmap").await;
    assert_eq!(response.status, 404);
}

#[tokio::test]
async fn cms_post_list_category_query_filters_rows() {
    let harness = CmsHarness::boot().await;
    harness.create_category("tech").await;
    harness.create_category("ops").await;
    harness
        .create_post("rust-tips", "Rust Tips", "tech", "published")
        .await;
    harness
        .create_post("oncall-notes", "Oncall Notes", "ops", "published")
        .await;

    let response = harness.get_raw("/app/posts?category=tech").await;
    assert_eq!(response.status, 200);
    assert!(response.text.contains("Rust Tips"));
    assert!(!response.text.contains("Oncall Notes"));
}
```

- [ ] **Step 2: Run tests to confirm handlers are missing**

Run: `cargo test -p sushi-core --test cms_plugin_behavior cms_public_page_route_hides_draft_content -q`  
Expected: FAIL because `interfaces/api.lua` handlers are not wired.

- [ ] **Step 3: Implement JSON API handlers with status mapping**

```lua
-- plugins/official/cms/lua/interfaces/api.lua
local M = {}

local function json_ok(status, data)
    return sushi.web.json(status, data)
end

local function json_error(kind, message)
    local status = 500
    if kind == "invalid_input" or kind == "invalid_status" then
        status = 400
    elseif kind == "not_found" then
        status = 404
    elseif kind == "conflict_has_posts" then
        status = 409
    elseif kind == "conflict" then
        status = 409
    end
    return sushi.web.json(status, { error = tostring(message or kind) })
end

function M.new(deps)
    local api = {}
    local page = deps.page
    local post = deps.post
    local category = deps.category
    local markdown = deps.markdown

    function api.pages_list()
        local rows, kind, msg = page.list()
        if not rows then
            return json_error(kind, msg)
        end
        return json_ok(200, { items = rows })
    end

    function api.public_post_list(args)
        local path = args[1] or ""
        local category_slug = path:match("[?&]category=([^&]+)")
        local rows, kind, msg = post.list({ only_published = true, category_slug = category_slug })
        if not rows then
            return json_error(kind, msg)
        end
        return sushi.web.render("plugins/official/cms/public/post_list.html", { items = rows, category = category_slug })
    end

    function api.public_page_detail(args)
        local path = args[1] or ""
        local slug = path:match("^/app/pages/([^%?]+)")
        if not slug then
            return sushi.web.json(404, { error = "not found" })
        end
        local item, kind, msg = page.get_by_slug(slug, { only_published = true })
        if not item then
            return json_error(kind, msg)
        end
        return sushi.web.render("plugins/official/cms/public/page_detail.html", {
            title = item.title,
            content_html = markdown.to_html(item.markdown_body),
        })
    end

    return api
end

return M
```

- [ ] **Step 4: Add public templates with escaped HTML slots**

```html
<!-- plugins/official/cms/web/templates/public/post_detail.html -->
{% extends "base.html" %}
{% block title %}{{ title }}{% endblock %}
{% block content %}
  <article class="max-w-3xl mx-auto py-8">
    <header class="mb-4">
      <h1 class="text-3xl font-semibold">{{ title }}</h1>
      <p class="text-sm text-slate-500">Category: {{ category_name }}</p>
    </header>
    <section class="prose prose-slate max-w-none">
      {{ content_html | safe }}
    </section>
  </article>
{% endblock %}
```

- [ ] **Step 5: Run API behavior tests**

Run: `cargo test -p sushi-core --test cms_plugin_behavior cms_post_list_category_query_filters_rows -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add plugins/official/cms/lua/interfaces/api.lua plugins/official/cms/web/templates/public crates/sushi-core/tests/cms_plugin_behavior.rs
git commit -m "feat(cms): add api handlers and public ssr pages"
```

### Task 7: Implement Admin Workspace (Single Entry) and Partials

**Files:**
- Create: `plugins/official/cms/lua/interfaces/admin.lua`
- Create: `plugins/official/cms/web/templates/cms.html`
- Create: `plugins/official/cms/web/templates/fragments/page_rows.html`
- Create: `plugins/official/cms/web/templates/fragments/post_rows.html`
- Create: `plugins/official/cms/web/templates/fragments/category_rows.html`
- Create: `plugins/official/cms/web/templates/fragments/flash.html`
- Create: `plugins/official/cms/web/static/cms.js`
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write failing admin tests for `/admin/cms` and table partial**

```rust
#[tokio::test]
async fn admin_cms_workspace_page_renders() {
    let app = build_app(None).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/cms")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_bearer_token()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_cms_category_delete_returns_flash_on_conflict() {
    let app = build_app(None).await;
    let admin = admin_bearer_token();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/cms/categories/upsert")
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("name=News&slug=news"))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/cms/posts/upsert")
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "title=Post&slug=post&markdown_body=body&status=published&category_slug=news",
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/partials/cms/categories/delete")
                .header("authorization", format!("Bearer {admin}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("slug=news"))
                .unwrap(),
        )
        .await
        .unwrap();

    let html = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("cannot be deleted"));
}
```

- [ ] **Step 2: Run tests and verify missing admin handlers**

Run: `cargo test -p sushi-admin --test admin_web admin_cms_workspace_page_renders -q`  
Expected: FAIL with 404 for `/admin/cms`.

- [ ] **Step 3: Implement admin partial handlers**

```lua
-- plugins/official/cms/lua/interfaces/admin.lua
local M = {}

function M.new(deps)
    local admin = {}
    local page = deps.page
    local post = deps.post
    local category = deps.category

    local function flash(level, message)
        return sushi.web.render("plugins/official/cms/fragments/flash.html", {
            level = tostring(level or "success"),
            message = tostring(message or ""),
        })
    end

    function admin.pages_table_partial()
        local rows = page.list() or {}
        return sushi.web.render("plugins/official/cms/fragments/page_rows.html", { items = rows })
    end

    function admin.categories_delete_partial(args)
        local body = args[2] or ""
        local slug = body:match("slug=([^&]+)") or ""
        local ok, kind, msg = category.soft_delete(slug)
        if not ok then
            if kind == "conflict_has_posts" then
                return flash("error", "Category still has posts and cannot be deleted")
            end
            return flash("error", tostring(msg))
        end
        return flash("success", "Category deleted")
    end

    return admin
end

return M
```

- [ ] **Step 4: Implement workspace template and Alpine module**

```html
<!-- plugins/official/cms/web/templates/cms.html -->
{% extends "base.html" %}
{% set active_section = "plugins" %}
{% block title %}CMS — Sushi Admin{% endblock %}
{% block content %}
<section class="admin-module" data-admin-workspace-module="cms" x-data="cmsPage()">
  <div class="ui-page-header">
    <h1 class="ui-title">CMS</h1>
    <p class="ui-subtitle">Manage pages, posts, and categories.</p>
  </div>
  <div id="cms-feedback" class="ui-feedback"></div>
  <div class="ui-card">
    <header class="ui-card-header">
      <h2 class="text-lg font-semibold">Pages</h2>
    </header>
    <table class="ui-table">
      <tbody id="cms-page-table"
             hx-get="/admin/partials/cms/pages/table"
             hx-trigger="load, cms:pages:refresh from:body"
             hx-swap="innerHTML">
      </tbody>
    </table>
  </div>
</section>
{% endblock %}
```

```javascript
// plugins/official/cms/web/static/cms.js
(() => {
  window.cmsPage = function cmsPage() {
    return {
      refreshPages() {
        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url: '/admin/partials/cms/pages/table',
            target: '#cms-page-table',
            errorMessage: 'Unable to refresh page table.',
          });
        }
      },
    };
  };
})();
```

- [ ] **Step 5: Run admin integration tests**

Run: `cargo test -p sushi-admin --test admin_web admin_cms_workspace_page_renders -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web admin_cms_category_delete_returns_flash_on_conflict -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add plugins/official/cms/lua/interfaces/admin.lua plugins/official/cms/web/templates/cms.html plugins/official/cms/web/templates/fragments plugins/official/cms/web/static/cms.js crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(cms): add admin workspace and partial handlers"
```

### Task 8: Implement CLI Interface and Final End-to-End Verification

**Files:**
- Create: `plugins/official/cms/lua/interfaces/cli.lua`
- Modify: `plugins/official/cms/lua/bootstrap/register.lua`
- Modify/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing CLI behavior tests**

```rust
#[tokio::test]
async fn cms_cli_dispatch_supports_page_list() {
    let harness = CmsHarness::boot().await;
    harness.create_page("about", "About", "published").await;

    let output = harness
        .run_cli(vec!["page".to_string(), "list".to_string()])
        .await;
    assert!(output.contains("about"));
}
```

- [ ] **Step 2: Run behavior test**

Run: `cargo test -p sushi-core --test cms_plugin_behavior cms_cli_dispatch_supports_page_list -q`  
Expected: FAIL because CLI dispatch is not implemented.

- [ ] **Step 3: Implement CLI dispatcher with explicit usage**

```lua
-- plugins/official/cms/lua/interfaces/cli.lua
local M = {}

local function usage()
    return "Usage: sushi run cms <page|post|category> <list|get|create|update|delete> [args]"
end

function M.new(deps)
    local cli = {}
    local page = deps.page
    local post = deps.post
    local category = deps.category

    function cli.cms_dispatch(args)
        local resource = args[1]
        local action = args[2]
        if not resource or not action then
            return usage()
        end
        if resource == "page" and action == "list" then
            local rows = page.list() or {}
            if #rows == 0 then
                return "No pages found."
            end
            local lines = {}
            for i = 1, #rows do
                lines[#lines + 1] = rows[i].slug .. " [" .. rows[i].status .. "]"
            end
            return table.concat(lines, "\n")
        end
        if resource == "category" and action == "list" then
            local rows = category.list() or {}
            if #rows == 0 then
                return "No categories found."
            end
            local lines = {}
            for i = 1, #rows do
                lines[#lines + 1] = rows[i].slug
            end
            return table.concat(lines, "\n")
        end
        if resource == "post" and action == "list" then
            local rows = post.list({ only_published = false }) or {}
            if #rows == 0 then
                return "No posts found."
            end
            local lines = {}
            for i = 1, #rows do
                lines[#lines + 1] = rows[i].slug .. " [" .. rows[i].status .. "]"
            end
            return table.concat(lines, "\n")
        end
        return usage()
    end

    return cli
end

return M
```

- [ ] **Step 4: Expand behavior tests for published-only public visibility**

```rust
#[tokio::test]
async fn cms_public_post_detail_hides_draft_posts() {
    let harness = CmsHarness::boot().await;
    harness.create_category("tech").await;
    harness
        .create_post("draft-post", "Draft Post", "tech", "draft")
        .await;

    let response = harness.get_raw("/app/posts/draft-post").await;
    assert_eq!(response.status, 404);
}
```

- [ ] **Step 5: Run complete verification**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

Run: `cargo test --workspace -q`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add plugins/official/cms/lua/interfaces/cli.lua plugins/official/cms/lua/bootstrap/register.lua crates/sushi-core/tests/cms_plugin_behavior.rs crates/sushi-core/src/lua/loader.rs
git commit -m "feat(cms): add cli dispatch and finalize cms behavior coverage"
```

### Task 9: Graph and Release Hygiene

**Files:**
- Modify: `graphify-out/graph.json`
- Modify: `graphify-out/GRAPH_REPORT.md`

- [ ] **Step 1: Update graph after code changes**

Run: `graphify update .`  
Expected: `graph.json` and `GRAPH_REPORT.md` updated successfully.

- [ ] **Step 2: Confirm no unstaged implementation files remain**

Run: `git status --short`  
Expected: only expected changed files (or clean tree after commits).

- [ ] **Step 3: Final commit for graph updates (if changed)**

```bash
git add graphify-out/graph.json graphify-out/GRAPH_REPORT.md
git commit -m "chore(graphify): refresh knowledge graph after cms plugin changes"
```

---

## Self-Review

### 1) Spec Coverage Check

- Official plugin with Page/Post/Category modules: covered by Tasks 3, 4, 6, 7, 8.
- Admin single entry `/admin/cms` with partial CRUD: covered by Task 7.
- Public `/app` SSR pages and published-only visibility: covered by Tasks 2, 6, 8.
- CLI interfaces: covered by Task 8 (`sushi run cms ...` dispatcher).
- Soft delete and category-delete guard: covered by Task 5 tests and domain logic.
- Migration and boot wiring: covered by Task 1.
- Policy registration and plugin contracts: covered by Task 3.
- Required verification commands and workspace run: covered by Task 8.

No uncovered spec requirement remains.

### 2) Placeholder Scan

- No placeholder markers or unresolved implementation notes are present.

### 3) Type/Signature Consistency

- API dispatch path split is consistent between `router.rs` and `plugin/manager.rs`.
- CLI registration uses `cms` command and `cli.cms.execute` policy consistently across manifest/registration/tests.
- Domain interfaces (`list`, `get_by_slug`, `upsert`, `soft_delete`) are referenced consistently by API/admin/CLI modules.
