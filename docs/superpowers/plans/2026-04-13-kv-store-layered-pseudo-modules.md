# KV Store Layered Pseudo-Modules Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `plugins/kv-store/init.lua` into clear pseudo-modules (utils/infra/domain/interfaces/bootstrap) without changing Rust core or splitting files, while preserving external contracts.

**Architecture:** Keep one Lua file but introduce explicit namespace tables and one-way dependencies: interfaces -> domain -> infra -> utils. Add lightweight characterization tests in Rust (`loader.rs`) that pin architecture markers and external registration contracts, so refactor safety is enforced during future edits.

**Tech Stack:** Lua (plugin runtime), Rust tests in `sushi-core`, Axum-dispatched plugin routes, `sushi.web`/`sushi.db` bindings.

---

## Scope Check

This is one subsystem (`kv-store` plugin internal structure). No decomposition is needed. Event-bus runtime changes are explicitly out of scope and remain a separate future project.

## File Structure Map

### Modify

- `plugins/kv-store/init.lua` — reorganize into layered pseudo-modules and preserve API/Admin/CLI behavior.
- `crates/sushi-core/src/lua/loader.rs` — add/refine characterization tests that guard kv-store architecture + contract markers.

### Test

- `crates/sushi-core/src/lua/loader.rs` (test module path `lua::loader::tests::*`).

---

### Task 1: Add Baseline Characterization Test for Layered Namespace

**Files:**
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Modify: `plugins/kv-store/init.lua`
- Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Write failing test that requires layered namespace markers**

```rust
#[test]
fn kv_store_plugin_has_layered_namespace_tables() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/kv-store/init.lua");
    let source = std::fs::read_to_string(plugin_path).unwrap();

    assert!(source.contains("local kv = {"));
    assert!(source.contains("utils = {}"));
    assert!(source.contains("infra = { db = {} }"));
    assert!(source.contains("domain = { store = {} }"));
    assert!(source.contains("interfaces = { api = {}, admin = {}, cli = {} }"));
    assert!(source.contains("bootstrap = {}"));
}
```

- [ ] **Step 2: Run test and confirm it fails on current file**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_has_layered_namespace_tables -q`
Expected: FAIL because current `init.lua` is flat and does not contain the `local kv = {` namespace markers.

- [ ] **Step 3: Add namespace skeleton to `init.lua` (no behavior change yet)**

```lua
local kv = {
    utils = {},
    infra = { db = {} },
    domain = { store = {} },
    interfaces = { api = {}, admin = {}, cli = {} },
    bootstrap = {},
}

local json_encode = sushi.json.encode

-- temporary compatibility: existing flat functions can still exist below,
-- but all new/ported functions must attach to kv.* tables.
```

- [ ] **Step 4: Re-run the test and confirm pass**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_has_layered_namespace_tables -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs plugins/kv-store/init.lua
git commit -m "test(plugin): add layered namespace guard for kv-store refactor"
```

### Task 2: Move Utility + DB Access into `kv.utils` and `kv.infra.db`

**Files:**
- Modify: `plugins/kv-store/init.lua`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing test for utils/infra function anchors**

```rust
#[test]
fn kv_store_plugin_extracts_utils_and_db_adapter() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/kv-store/init.lua");
    let source = std::fs::read_to_string(plugin_path).unwrap();

    assert!(source.contains("kv.utils.html_escape = function"));
    assert!(source.contains("kv.utils.parse_form_urlencoded = function"));
    assert!(source.contains("kv.infra.db.query = function"));
    assert!(source.contains("kv.infra.db.execute = function"));
}
```

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_extracts_utils_and_db_adapter -q`
Expected: FAIL because helper/db functions are still mostly flat local functions.

- [ ] **Step 3: Port helper and db wrappers to pseudo-modules**

```lua
kv.utils.json_parse = function(raw)
    local ok, parsed = pcall(sushi.json.decode, raw)
    if ok then return parsed end
    return nil
end

kv.utils.html_escape = function(value)
    local text = tostring(value or "")
    text = text:gsub("&", "&amp;"):gsub("<", "&lt;"):gsub(">", "&gt;")
    text = text:gsub('"', "&quot;"):gsub("'", "&#39;")
    return text
end

kv.utils.parse_form_urlencoded = function(body)
    local out = {}
    local source = body or ""
    for key, value in string.gmatch(source, "([^&=]+)=?([^&]*)") do
        out[kv.utils.url_decode(key)] = kv.utils.url_decode(value)
    end
    return out
end

kv.infra.db.query = function(sql, params)
    local ok, rows = pcall(function() return sushi.db.query(sql, params) end)
    if not ok then return nil, "storage_error", tostring(rows) end
    return rows
end

kv.infra.db.execute = function(sql, params)
    local ok, err = pcall(function() return sushi.db.execute(sql, params) end)
    if not ok then return nil, "storage_error", tostring(err) end
    return true
end
```

- [ ] **Step 4: Run both characterization tests**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_has_layered_namespace_tables lua::loader::tests::kv_store_plugin_extracts_utils_and_db_adapter -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/kv-store/init.lua crates/sushi-core/src/lua/loader.rs
git commit -m "refactor(plugin): move kv-store helpers and db access to pseudo-modules"
```

### Task 3: Introduce `kv.domain.store` with Normalized Error Kinds

**Files:**
- Modify: `plugins/kv-store/init.lua`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing test for domain layer + error taxonomy markers**

```rust
#[test]
fn kv_store_plugin_defines_domain_store_and_error_kinds() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/kv-store/init.lua");
    let source = std::fs::read_to_string(plugin_path).unwrap();

    assert!(source.contains("kv.domain.store.list = function"));
    assert!(source.contains("kv.domain.store.get = function"));
    assert!(source.contains("kv.domain.store.upsert = function"));
    assert!(source.contains("kv.domain.store.delete = function"));
    assert!(source.contains("invalid_key"));
    assert!(source.contains("invalid_value"));
    assert!(source.contains("not_found"));
    assert!(source.contains("storage_error"));
}
```

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_defines_domain_store_and_error_kinds -q`
Expected: FAIL because domain contracts are not explicitly represented in the source yet.

- [ ] **Step 3: Add domain store API and route all KV data operations through it**

```lua
local function domain_error(kind, message)
    return nil, kind, message
end

kv.domain.store.list = function()
    local rows, kind, msg = kv.infra.db.query("SELECT key, value FROM kv_store ORDER BY key", nil)
    if not rows then return domain_error(kind or "storage_error", msg) end
    return rows
end

kv.domain.store.get = function(key)
    if not key or key == "" then return domain_error("invalid_key", "key cannot be empty") end
    local rows, kind, msg = kv.infra.db.query("SELECT value FROM kv_store WHERE key = ?1", { key })
    if not rows then return domain_error(kind or "storage_error", msg) end
    if #rows == 0 then return domain_error("not_found", "key not found") end
    return { key = key, value = rows[1].value }
end

kv.domain.store.upsert = function(key, value)
    if not key or key == "" then return domain_error("invalid_key", "key cannot be empty") end
    if value == nil or value == "" then return domain_error("invalid_value", "value cannot be empty") end
    local ok, kind, msg = kv.infra.db.execute(
        "INSERT INTO kv_store (key, value, updated_at) VALUES (?1, ?2, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
        { key, value }
    )
    if not ok then return domain_error(kind or "storage_error", msg) end
    return { key = key, value = value }
end

kv.domain.store.delete = function(key)
    if not key or key == "" then return domain_error("invalid_key", "key cannot be empty") end
    local ok, kind, msg = kv.infra.db.execute("DELETE FROM kv_store WHERE key = ?1", { key })
    if not ok then return domain_error(kind or "storage_error", msg) end
    return true
end
```

- [ ] **Step 4: Run characterization tests for Task 1-3**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_has_layered_namespace_tables lua::loader::tests::kv_store_plugin_extracts_utils_and_db_adapter lua::loader::tests::kv_store_plugin_defines_domain_store_and_error_kinds -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/kv-store/init.lua crates/sushi-core/src/lua/loader.rs
git commit -m "refactor(plugin): add kv-store domain store with normalized errors"
```

### Task 4: Refactor API/Admin/CLI Interfaces to Depend on Domain Layer

**Files:**
- Modify: `plugins/kv-store/init.lua`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing test that interface dispatchers are explicit and bootstrap-ready**

```rust
#[test]
fn kv_store_plugin_uses_interface_dispatchers() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/kv-store/init.lua");
    let source = std::fs::read_to_string(plugin_path).unwrap();

    assert!(source.contains("kv.interfaces.api.dispatch = function"));
    assert!(source.contains("kv.interfaces.api.delete_dispatch = function"));
    assert!(source.contains("kv.interfaces.admin.table_partial = function"));
    assert!(source.contains("kv.interfaces.admin.upsert_partial = function"));
    assert!(source.contains("kv.interfaces.admin.delete_partial = function"));
    assert!(source.contains("kv.interfaces.cli.kv_list = function"));
    assert!(source.contains("kv.interfaces.cli.kv_get = function"));
    assert!(source.contains("kv.interfaces.cli.kv_set = function"));
    assert!(source.contains("kv.interfaces.cli.kv_del = function"));
}
```

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_uses_interface_dispatchers -q`
Expected: FAIL because handlers are still mostly flat local functions.

- [ ] **Step 3: Move handler logic into `kv.interfaces.*` and map domain errors per channel**

```lua
local function api_error(kind, message)
    if kind == "invalid_key" or kind == "invalid_value" then
        return sushi.web.json(400, { error = message })
    elseif kind == "not_found" then
        return sushi.web.json(404, { error = message })
    end
    return sushi.web.json(500, { error = message or "internal error" })
end

kv.interfaces.api.dispatch = function(args)
    local path = args[1] or ""
    local body = args[2]

    if path == "/api/kv" and not body then
        local rows, kind, msg = kv.domain.store.list()
        if not rows then return api_error(kind, msg) end
        return json_encode(rows)
    end

    if path == "/api/kv" and body then
        local data = kv.utils.json_parse(body)
        if not data then return sushi.web.json(400, { error = "invalid json body" }) end
        local saved, kind, msg = kv.domain.store.upsert(data.key, data.value)
        if not saved then return api_error(kind, msg) end
        return json_encode(saved)
    end

    local key = path:match("^/api/kv/(.+)$")
    if key and not body then
        local item, kind, msg = kv.domain.store.get(key)
        if not item then return api_error(kind, msg) end
        return json_encode(item)
    end

    if key and body then
        local data = kv.utils.json_parse(body)
        if not data then return sushi.web.json(400, { error = "invalid json body" }) end
        local saved, kind, msg = kv.domain.store.upsert(key, data.value)
        if not saved then return api_error(kind, msg) end
        return json_encode(saved)
    end
    return sushi.web.json(404, { error = "not found" })
end

kv.interfaces.admin.table_partial = function()
    local rows, kind, msg = kv.domain.store.list()
    if not rows then
        return sushi.web.render("plugins/kv-store/partials/rows.html", {
            items = {},
            error_message = msg,
        })
    end
    return sushi.web.render("plugins/kv-store/partials/rows.html", { items = rows })
end

kv.interfaces.cli.kv_set = function(args)
    if not args[1] or not args[2] then return "Usage: sushi run kv-set <key> <value>" end
    local saved, kind, msg = kv.domain.store.upsert(args[1], args[2])
    if not saved then return "Error: " .. tostring(msg) end
    return "OK: " .. saved.key .. " = " .. saved.value
end
```

- [ ] **Step 4: Run characterization tests for Task 1-4 + existing kv-store HTML guard**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_has_layered_namespace_tables lua::loader::tests::kv_store_plugin_extracts_utils_and_db_adapter lua::loader::tests::kv_store_plugin_defines_domain_store_and_error_kinds lua::loader::tests::kv_store_plugin_uses_interface_dispatchers lua::loader::tests::kv_store_plugin_no_longer_embeds_html -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/kv-store/init.lua crates/sushi-core/src/lua/loader.rs
git commit -m "refactor(plugin): route kv-store api admin cli through layered interfaces"
```

### Task 5: Finalize Bootstrap Registration-Only Shape and Run Regression Verification

**Files:**
- Modify: `plugins/kv-store/init.lua`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing test for explicit bootstrap registration function and stable contracts**

```rust
#[test]
fn kv_store_plugin_bootstrap_registration_contract_is_stable() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/kv-store/init.lua");
    let source = std::fs::read_to_string(plugin_path).unwrap();

    assert!(source.contains("kv.bootstrap.register = function()"));
    assert!(source.contains("sushi.api.route(\"GET\", \"/api/kv\", kv.interfaces.api.dispatch)"));
    assert!(source.contains("sushi.api.route(\"DELETE\", \"/api/kv/*\", kv.interfaces.api.delete_dispatch)"));
    assert!(source.contains("sushi.web.page(\"/admin/kv\", \"plugins/kv-store/kv.html\", { title = \"KV Store\" })"));
    assert!(source.contains("sushi.cli.command(\"kv-set\", \"Set a KV entry (key + value)\", kv.interfaces.cli.kv_set)"));
    assert!(source.contains("function sushi.init()"));
    assert!(source.contains("kv.bootstrap.register()"));
}
```

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_bootstrap_registration_contract_is_stable -q`
Expected: FAIL until explicit `kv.bootstrap.register` wiring exists.

- [ ] **Step 3: Introduce bootstrap-only registration and remove leftover flat handlers**

```lua
kv.bootstrap.register = function()
    sushi.api.route("GET", "/api/kv", kv.interfaces.api.dispatch)
    sushi.api.route("GET", "/api/kv/*", kv.interfaces.api.dispatch)
    sushi.api.route("POST", "/api/kv", kv.interfaces.api.dispatch)
    sushi.api.route("PUT", "/api/kv/*", kv.interfaces.api.dispatch)
    sushi.api.route("DELETE", "/api/kv/*", kv.interfaces.api.delete_dispatch)

    sushi.api.route("GET", "/admin/partials/kv/table", kv.interfaces.admin.table_partial)
    sushi.api.route("POST", "/admin/partials/kv/upsert", kv.interfaces.admin.upsert_partial)
    sushi.api.route("POST", "/admin/partials/kv/delete", kv.interfaces.admin.delete_partial)

    sushi.web.page("/admin/kv", "plugins/kv-store/kv.html", { title = "KV Store" })

    sushi.cli.command("kv-list", "List all KV entries", kv.interfaces.cli.kv_list)
    sushi.cli.command("kv-get", "Get a KV entry by key", kv.interfaces.cli.kv_get)
    sushi.cli.command("kv-set", "Set a KV entry (key + value)", kv.interfaces.cli.kv_set)
    sushi.cli.command("kv-del", "Delete a KV entry by key", kv.interfaces.cli.kv_del)
end

function sushi.init()
    kv.bootstrap.register()
    sushi.log.info("kv-store plugin: registered API routes, admin page, and CLI commands")
end
```

- [ ] **Step 4: Run full targeted verification and a workspace regression sample**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_no_longer_embeds_html lua::loader::tests::kv_store_plugin_has_layered_namespace_tables lua::loader::tests::kv_store_plugin_extracts_utils_and_db_adapter lua::loader::tests::kv_store_plugin_defines_domain_store_and_error_kinds lua::loader::tests::kv_store_plugin_uses_interface_dispatchers lua::loader::tests::kv_store_plugin_bootstrap_registration_contract_is_stable -q`
Expected: PASS.

Run: `cargo test -p sushi-core -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/kv-store/init.lua crates/sushi-core/src/lua/loader.rs
git commit -m "refactor(plugin): finalize kv-store layered pseudo-module bootstrap"
```

## Post-Task Manual Smoke Checklist

- [ ] Run `cargo run -p sushi-cli -- run kv-set plan-check value-1` and confirm `OK: plan-check = value-1`.
- [ ] Run `cargo run -p sushi-cli -- run kv-get plan-check` and confirm `value-1`.
- [ ] Run `cargo run -p sushi-cli -- run kv-del plan-check` and confirm delete success.
- [ ] Start admin or API server and verify `/admin/kv` still loads and partial mutations work.

## Rollback Strategy

If any regression appears during Task 4-5, keep already-passing layered tests and revert only the latest commit, then re-apply interface migration in smaller slices (API first, then Admin, then CLI).
