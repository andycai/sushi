# Plugin Governance Runtime Control (V1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add backend-governed plugin enable/disable control that takes effect immediately at runtime for API/Admin/CLI without service restart.

**Architecture:** Extend `plugin_state` into the runtime governance source of truth, introduce a dedicated plugin state repository in `sushi-core`, and enforce a central dispatch gate in `PluginManager`. Wire new admin + CLI control surfaces to mutate plugin state, then map disabled-plugin dispatch errors to explicit deny responses across API/Admin/CLI.

**Tech Stack:** Rust (Tokio/Axum/Clap/mlua), SQLite migrations, existing Sushi policy model, Alpine.js + HTMX admin UI.

---

## File Structure Map

- Create: `migrations/008_plugin_governance_v1.sql` — plugin state schema extension, event audit table, policy key/binding seeds for manage commands.
- Create: `crates/sushi-core/src/plugin/state_repository.rs` — CRUD for plugin runtime state + audit writes.
- Modify: `crates/sushi-core/src/plugin/mod.rs` — export new repository module.
- Modify: `crates/sushi-core/src/plugin/manager.rs` — add governance fields (`plugin_id`, `source_kind`, `enabled`), central runtime gate, state mutation APIs.
- Modify: `crates/sushi-core/src/context.rs` — construct `PluginManager` with storage-backed repository.
- Modify: `crates/sushi-cli/src/app.rs` — run migration `008`, register plugin identity/kind metadata, skip init when plugin disabled at startup.
- Modify: `crates/sushi-admin/src/routes/plugins.rs` — add plugin state mutation API handler.
- Modify: `crates/sushi-admin/src/router.rs` — register patch route and reserved path.
- Modify: `crates/sushi-admin/src/routes/workspace.rs` — map disabled plugin page calls to `403`.
- Modify: `crates/sushi-api/src/router.rs` — map disabled plugin API errors to `403` with `plugin_disabled` code.
- Modify: `crates/sushi-cli/src/commands/plugin.rs` — add `status/enable/disable` subcommands.
- Modify: `crates/sushi-cli/src/commands/run.rs` — map disabled-plugin command failure to explicit CLI error.
- Modify: `web/templates/admin/partials/plugins_rows.html` — add enable/disable control and state badges.
- Modify: `web/static/admin/js/plugins.js` — add toggle action + refresh flow with toast feedback.
- Modify/Test: `crates/sushi-core/src/plugin/manager.rs` tests — dispatch gate behavior.
- Create/Test: `crates/sushi-core/src/plugin/state_repository.rs` tests — state upsert + toggle + audit.
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs` — plugin state API auth + mutation behavior.
- Modify/Test: `crates/sushi-api/src/router.rs` tests — disabled plugin API route returns `403`.
- Modify/Test: `crates/sushi-cli/src/commands/plugin.rs` tests — new subcommand parsing and authorization targets.

---

### Task 1: Add Migration 008 and Wire It Into Bootstrap/Test Harnesses

**Files:**
- Create: `migrations/008_plugin_governance_v1.sql`
- Modify: `crates/sushi-cli/src/app.rs`
- Modify: `crates/sushi-admin/tests/admin_web.rs`
- Modify: `crates/sushi-api/src/router.rs`

- [ ] **Step 1: Write failing wiring edits first**

```rust
// crates/sushi-cli/src/app.rs
const PLUGIN_GOVERNANCE_MIGRATION_SQL: &str =
    include_str!("../../../migrations/008_plugin_governance_v1.sql");

storage
    .run_migrations(PLUGIN_GOVERNANCE_MIGRATION_SQL)
    .await
    .context("failed to run plugin governance migrations")?;
```

```rust
// crates/sushi-admin/tests/admin_web.rs
const PLUGIN_GOVERNANCE_MIGRATION_SQL: &str =
    include_str!("../../../migrations/008_plugin_governance_v1.sql");

storage
    .run_migrations(PLUGIN_GOVERNANCE_MIGRATION_SQL)
    .await
    .expect("failed to run migration 008_plugin_governance_v1");
```

```rust
// crates/sushi-api/src/router.rs (test module constants)
const PLUGIN_GOVERNANCE_MIGRATION_SQL: &str =
    include_str!("../../../migrations/008_plugin_governance_v1.sql");
```

- [ ] **Step 2: Run tests to confirm missing migration failure**

Run: `cargo test -p sushi-admin --test admin_web plugins_api_returns_list_payload -q`  
Expected: FAIL with `include_str!("../../../migrations/008_plugin_governance_v1.sql")` file-not-found compile error.

- [ ] **Step 3: Create migration file with schema + policy seeds**

```sql
-- migrations/008_plugin_governance_v1.sql
ALTER TABLE plugin_state ADD COLUMN plugin_id TEXT;
ALTER TABLE plugin_state ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'third_party';
ALTER TABLE plugin_state ADD COLUMN updated_by TEXT;
ALTER TABLE plugin_state ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now'));
ALTER TABLE plugin_state ADD COLUMN reason TEXT NOT NULL DEFAULT '';

UPDATE plugin_state
SET plugin_id = name
WHERE plugin_id IS NULL OR trim(plugin_id) = '';

CREATE UNIQUE INDEX IF NOT EXISTS idx_plugin_state_plugin_id
    ON plugin_state(plugin_id);

CREATE TABLE IF NOT EXISTS plugin_state_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plugin_id TEXT NOT NULL,
    name TEXT NOT NULL,
    action TEXT NOT NULL,
    old_enabled INTEGER NOT NULL,
    new_enabled INTEGER NOT NULL,
    actor TEXT,
    reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO policy_keys (key, surface, resource, action, name, description, is_system) VALUES
    ('admin.plugins.manage', 'admin', 'plugins', 'manage', 'Manage Admin Plugins', 'Enable or disable plugin runtime state from admin.', 1),
    ('cli.plugins.manage', 'cli', 'plugins', 'manage', 'Manage Plugins From CLI', 'Enable or disable plugins from CLI.', 1);

INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
SELECT r.id, pk.id
FROM roles r
JOIN policy_keys pk ON pk.key IN ('admin.plugins.manage', 'cli.plugins.manage')
WHERE r.slug = 'admin';

WITH seeded_bindings (
    surface,
    target_type,
    target_ref,
    method,
    path_pattern,
    command_name,
    policy_key,
    owner_type,
    owner_id,
    is_system
) AS (
    VALUES
    ('admin', 'http_route', '/admin/api/plugins/{plugin}/state', 'PATCH', '/admin/api/plugins/{plugin}/state', NULL, 'admin.plugins.manage', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:status', NULL, NULL, 'plugin:status', 'cli.plugins.read', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:enable', NULL, NULL, 'plugin:enable', 'cli.plugins.manage', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:disable', NULL, NULL, 'plugin:disable', 'cli.plugins.manage', 'system', 'builtin', 1)
)
INSERT OR IGNORE INTO policy_bindings (
    surface,
    target_type,
    target_ref,
    method,
    path_pattern,
    command_name,
    policy_key_id,
    owner_type,
    owner_id,
    is_system
)
SELECT
    seeded_bindings.surface,
    seeded_bindings.target_type,
    seeded_bindings.target_ref,
    seeded_bindings.method,
    seeded_bindings.path_pattern,
    seeded_bindings.command_name,
    pk.id,
    seeded_bindings.owner_type,
    seeded_bindings.owner_id,
    seeded_bindings.is_system
FROM seeded_bindings
JOIN policy_keys pk ON pk.key = seeded_bindings.policy_key;

INSERT OR IGNORE INTO _sushi_migrations (id, name)
VALUES (8, '008_plugin_governance_v1');
```

- [ ] **Step 4: Run migration-focused tests**

Run: `cargo test -p sushi-admin --test admin_web plugins_api_returns_list_payload -q`  
Expected: PASS

Run: `cargo test -p sushi-api test_plugin_api_dispatch_applies_status_envelope -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add migrations/008_plugin_governance_v1.sql crates/sushi-cli/src/app.rs crates/sushi-admin/tests/admin_web.rs crates/sushi-api/src/router.rs
git commit -m "feat(core): add plugin governance migration and bootstrap wiring"
```

### Task 2: Implement Plugin State Repository in sushi-core

**Files:**
- Create: `crates/sushi-core/src/plugin/state_repository.rs`
- Modify: `crates/sushi-core/src/plugin/mod.rs`

- [ ] **Step 1: Write failing repository tests first**

```rust
// crates/sushi-core/src/plugin/state_repository.rs
#[tokio::test]
async fn plugin_state_upsert_and_toggle_round_trip() {
    let sqlite = Arc::new(SqliteStorage::new_in_memory().await.unwrap());
    sqlite
        .run_migrations(include_str!("../../../../migrations/001_init.sql"))
        .await
        .unwrap();
    sqlite
        .run_migrations(include_str!("../../../../migrations/008_plugin_governance_v1.sql"))
        .await
        .unwrap();

    let storage: Arc<dyn Storage> = sqlite;
    let repo = PluginStateRepository::new(storage);

    let created = repo
        .upsert_discovered_plugin("official/kv-store", "kv-store", "official", "1.0.0")
        .await
        .unwrap();
    assert!(created.enabled);

    let disabled = repo
        .set_enabled("kv-store", false, Some("admin"), Some("incident response"))
        .await
        .unwrap();
    assert!(!disabled.enabled);
}
```

- [ ] **Step 2: Run test and confirm unresolved symbols fail**

Run: `cargo test -p sushi-core plugin_state_upsert_and_toggle_round_trip -q`  
Expected: FAIL with unresolved `PluginStateRepository`.

- [ ] **Step 3: Implement repository + model parsing**

```rust
// crates/sushi-core/src/plugin/state_repository.rs
use crate::storage::Storage;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct StoredPluginState {
    pub plugin_id: String,
    pub name: String,
    pub source_kind: String,
    pub enabled: bool,
    pub loaded: bool,
    pub version: String,
    pub updated_by: Option<String>,
    pub updated_at: String,
    pub reason: String,
}

pub struct PluginStateRepository {
    storage: Arc<dyn Storage>,
}

impl PluginStateRepository {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    pub async fn upsert_discovered_plugin(
        &self,
        plugin_id: &str,
        name: &str,
        source_kind: &str,
        version: &str,
    ) -> Result<StoredPluginState, String> {
        self.storage
            .execute(
                r#"
                INSERT INTO plugin_state (plugin_id, name, source_kind, enabled, loaded, version, updated_at)
                VALUES (?1, ?2, ?3, 1, 0, ?4, datetime('now'))
                ON CONFLICT(name) DO UPDATE SET
                    plugin_id = excluded.plugin_id,
                    source_kind = excluded.source_kind,
                    version = excluded.version,
                    updated_at = datetime('now')
                "#,
                vec![
                    Value::String(plugin_id.to_string()),
                    Value::String(name.to_string()),
                    Value::String(source_kind.to_string()),
                    Value::String(version.to_string()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;
        self.get_by_name(name)
            .await?
            .ok_or_else(|| format!("plugin state row missing after upsert: {name}"))
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<StoredPluginState>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT plugin_id, name, source_kind, enabled, loaded, version, updated_by, updated_at, reason
                FROM plugin_state
                WHERE name = ?1
                "#,
                vec![Value::String(name.to_string())],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().next().map(row_to_state).transpose()
    }

    pub async fn set_loaded(&self, name: &str, loaded: bool) -> Result<(), String> {
        self.storage
            .execute(
                r#"
                UPDATE plugin_state
                SET loaded = ?2,
                    loaded_at = CASE WHEN ?2 = 1 THEN datetime('now') ELSE loaded_at END,
                    updated_at = datetime('now')
                WHERE name = ?1
                "#,
                vec![Value::String(name.to_string()), Value::Bool(loaded)],
            )
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn set_enabled(
        &self,
        name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<StoredPluginState, String> {
        let before = self
            .get_by_name(name)
            .await?
            .ok_or_else(|| format!("plugin not found: {name}"))?;

        let actor_value = actor.unwrap_or("").trim().to_string();
        let reason_value = reason.unwrap_or("").trim().to_string();

        self.storage
            .execute(
                r#"
                UPDATE plugin_state
                SET enabled = ?2,
                    updated_by = ?3,
                    reason = ?4,
                    updated_at = datetime('now')
                WHERE name = ?1
                "#,
                vec![
                    Value::String(name.to_string()),
                    Value::Bool(enabled),
                    Value::String(actor_value.clone()),
                    Value::String(reason_value.clone()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.storage
            .execute(
                r#"
                INSERT INTO plugin_state_events (plugin_id, name, action, old_enabled, new_enabled, actor, reason)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                vec![
                    Value::String(before.plugin_id.clone()),
                    Value::String(before.name.clone()),
                    Value::String(if enabled { "enable" } else { "disable" }.to_string()),
                    Value::Bool(before.enabled),
                    Value::Bool(enabled),
                    Value::String(actor_value),
                    Value::String(reason_value),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.get_by_name(name)
            .await?
            .ok_or_else(|| format!("plugin state row missing after update: {name}"))
    }
}

fn row_to_state(row: HashMap<String, Value>) -> Result<StoredPluginState, String> {
    let plugin_id = row
        .get("plugin_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let name = row
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing plugin_state.name".to_string())?
        .to_string();
    let source_kind = row
        .get("source_kind")
        .and_then(Value::as_str)
        .unwrap_or("third_party")
        .to_string();
    let enabled = row
        .get("enabled")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        != 0;
    let loaded = row
        .get("loaded")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        != 0;
    let version = row
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let updated_by = row
        .get("updated_by")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned);
    let updated_at = row
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let reason = row
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Ok(StoredPluginState {
        plugin_id,
        name,
        source_kind,
        enabled,
        loaded,
        version,
        updated_by,
        updated_at,
        reason,
    })
}
```

```rust
// crates/sushi-core/src/plugin/mod.rs
pub mod manager;
pub mod state_repository;
```

- [ ] **Step 4: Run new repository tests**

Run: `cargo test -p sushi-core plugin_state_upsert_and_toggle_round_trip -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin/state_repository.rs crates/sushi-core/src/plugin/mod.rs
git commit -m "feat(core): add plugin state repository for governance"
```

### Task 3: Add Central Runtime Gate in PluginManager

**Files:**
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Modify: `crates/sushi-core/src/context.rs`

- [ ] **Step 1: Add failing manager tests for disabled dispatch**

```rust
#[tokio::test]
async fn plugin_disabled_gate_blocks_api_admin_and_cli_dispatch() {
    let manager = PluginManager::new();
    let lua = mlua::Lua::new();

    let sushi = lua.create_table().unwrap();
    let handlers = lua.create_table().unwrap();
    sushi.set("__handlers", handlers.clone()).unwrap();
    lua.globals().set("sushi", sushi).unwrap();

    let passthrough = lua
        .create_async_function(|_, ()| async { Ok("ok".to_string()) })
        .unwrap();
    handlers.set("h", passthrough).unwrap();

    manager.register_vm("notes", lua).await;
    manager
        .register_api_handler("GET", "/api/notes", "notes", "h")
        .await;
    manager
        .register_admin_handler("/admin/notes", "notes", "Notes", "h")
        .await;
    manager
        .register_cli_handler("notes-run", "notes", "h")
        .await;

    manager
        .set_plugin_enabled("notes", false, Some("admin"), Some("test"))
        .await
        .unwrap();

    let api = manager
        .call_api_handler("GET", "/api/notes", None)
        .await
        .unwrap()
        .unwrap_err();
    let admin = manager
        .call_admin_handler("/admin/notes")
        .await
        .unwrap()
        .unwrap_err();
    let cli = manager
        .call_cli_handler("notes-run", &[])
        .await
        .unwrap()
        .unwrap_err();

    assert!(api.contains("plugin_disabled"));
    assert!(admin.contains("plugin_disabled"));
    assert!(cli.contains("plugin_disabled"));
}
```

- [ ] **Step 2: Run test to verify missing API fails**

Run: `cargo test -p sushi-core plugin_disabled_gate_blocks_api_admin_and_cli_dispatch -q`  
Expected: FAIL with unresolved `set_plugin_enabled`.

- [ ] **Step 3: Implement manager governance fields + gate APIs**

```rust
// crates/sushi-core/src/plugin/manager.rs (struct additions)
use super::state_repository::PluginStateRepository;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInfo {
    pub plugin_id: String,
    pub source_kind: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub loaded: bool,
    pub permissions: PluginPermissionsView,
}

#[derive(Clone, Default)]
pub struct PluginManager {
    vms: Arc<RwLock<HashMap<String, mlua::Lua>>>,
    api_handlers: Arc<RwLock<HashMap<(String, String), ApiHandlerBinding>>>,
    cli_handlers: Arc<RwLock<HashMap<String, CliHandlerBinding>>>,
    admin_handlers: Arc<RwLock<HashMap<String, AdminHandlerBinding>>>,
    plugin_info: Arc<RwLock<HashMap<String, PluginInfo>>>,
    plugin_static_roots: Arc<RwLock<HashMap<String, PathBuf>>>,
    state_repo: Option<Arc<PluginStateRepository>>,
}

impl PluginManager {
    pub fn new_with_storage(storage: Arc<dyn crate::storage::Storage>) -> Self {
        let mut manager = Self::default();
        manager.state_repo = Some(Arc::new(PluginStateRepository::new(storage)));
        manager
    }

    pub async fn set_plugin_enabled(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<PluginInfo, String> {
        if let Some(repo) = &self.state_repo {
            let state = repo
                .set_enabled(plugin_name, enabled, actor, reason)
                .await?;
            let mut info = self.plugin_info.write().await;
            let item = info
                .get_mut(plugin_name)
                .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
            item.enabled = state.enabled;
            return Ok(item.clone());
        }

        let mut info = self.plugin_info.write().await;
        let item = info
            .get_mut(plugin_name)
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
        item.enabled = enabled;
        Ok(item.clone())
    }

    async fn guard_plugin_enabled(&self, plugin_name: &str) -> Result<(), String> {
        if let Some(repo) = &self.state_repo {
            if let Some(state) = repo.get_by_name(plugin_name).await? {
                if !state.enabled {
                    return Err(format!("plugin_disabled: plugin '{plugin_name}' is disabled"));
                }
            }
        }

        let info = self.plugin_info.read().await;
        if let Some(plugin) = info.get(plugin_name) {
            if !plugin.enabled {
                return Err(format!("plugin_disabled: plugin '{plugin_name}' is disabled"));
            }
        }
        Ok(())
    }
}
```

```rust
// crates/sushi-core/src/plugin/manager.rs (dispatch hooks)
self.guard_plugin_enabled(&plugin_name).await?;
```

```rust
// crates/sushi-core/src/context.rs
let storage: Arc<dyn Storage> = db.clone();
let db_gateway = DbGateway::new(storage.clone(), DbPermission::Admin);

Self {
    config,
    db,
    db_gateway,
    event: EventBus::new(),
    jwt: Arc::new(jwt),
    authorizer: Arc::new(Authorizer::new(CompiledPolicySnapshot::default())),
    plugins: PluginManager::new_with_storage(storage),
    templates: Arc::new(templates),
    logs: Arc::new(LogService::new()),
}
```

- [ ] **Step 4: Run manager tests**

Run: `cargo test -p sushi-core plugin_disabled_gate_blocks_api_admin_and_cli_dispatch -q`  
Expected: PASS

Run: `cargo test -p sushi-core register_manifest_is_visible_in_plugin_list -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin/manager.rs crates/sushi-core/src/context.rs
git commit -m "feat(core): enforce runtime plugin enabled gate in manager"
```

### Task 4: Persist Identity/State During Plugin Bootstrap and Respect Disabled-on-Startup

**Files:**
- Modify: `crates/sushi-cli/src/app.rs`
- Modify: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing bootstrap test for disabled startup skip**

```rust
// crates/sushi-core/src/lua/loader.rs (test module)
#[tokio::test]
async fn disabled_plugin_is_not_invoked_after_scan_registration() {
    let ctx = test_context().await;

    let manifest = PluginManifest {
        plugin: crate::plugin::PluginMeta {
            name: "notes".to_string(),
            version: "0.1.0".to_string(),
            description: "notes".to_string(),
            entry: "init.lua".to_string(),
        },
        permissions: crate::plugin::Permissions::default(),
        policies: crate::plugin::PluginPoliciesConfig::default(),
        admin: None,
        file_browser: None,
    };

    ctx.plugins
        .register_plugin_manifest_with_permissions_and_identity(
            &manifest,
            &crate::plugin::Permissions::default(),
            "third_party/notes",
            crate::plugin::PluginKind::ThirdParty,
        )
        .await;

    ctx.plugins
        .set_plugin_enabled("notes", false, Some("admin"), Some("seed"))
        .await
        .unwrap();

    assert_eq!(
        ctx.plugins
            .list_plugins()
            .await
            .into_iter()
            .find(|p| p.name == "notes")
            .unwrap()
            .enabled,
        false
    );
}
```

- [ ] **Step 2: Run test to verify missing registration API fails**

Run: `cargo test -p sushi-core disabled_plugin_is_not_invoked_after_scan_registration -q`  
Expected: FAIL with unresolved `register_plugin_manifest_with_permissions_and_identity`.

- [ ] **Step 3: Implement bootstrap identity registration + disabled skip**

```rust
// crates/sushi-cli/src/app.rs (plugin registration loop)
ctx.plugins
    .register_plugin_manifest_with_permissions_and_identity(
        plugin.manifest(),
        plugin.effective_permissions(),
        plugin.path_id(),
        plugin.kind(),
    )
    .await;

let enabled = ctx
    .plugins
    .list_plugins()
    .await
    .into_iter()
    .find(|item| item.name == plugin_name)
    .map(|item| item.enabled)
    .unwrap_or(true);

if !enabled {
    tracing::info!("plugin {plugin_name} is disabled by governance state; skip init");
    ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
    continue;
}
```

```rust
// crates/sushi-core/src/plugin/manager.rs
pub async fn register_plugin_manifest_with_permissions_and_identity(
    &self,
    manifest: &PluginManifest,
    effective_permissions: &Permissions,
    plugin_id: &str,
    kind: crate::plugin::PluginKind,
) {
    if let Some(repo) = &self.state_repo {
        let _ = repo
            .upsert_discovered_plugin(
                plugin_id,
                &manifest.plugin.name,
                kind.tier_name(),
                &manifest.plugin.version,
            )
            .await;
    }

    let enabled = if let Some(repo) = &self.state_repo {
        repo.get_by_name(&manifest.plugin.name)
            .await
            .ok()
            .flatten()
            .map(|state| state.enabled)
            .unwrap_or(true)
    } else {
        true
    };

    let mut plugin_info = self.plugin_info.write().await;
    plugin_info.insert(
        manifest.plugin.name.clone(),
        PluginInfo {
            plugin_id: plugin_id.to_string(),
            source_kind: kind.tier_name().to_string(),
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            description: manifest.plugin.description.clone(),
            enabled,
            loaded: false,
            permissions: PluginPermissionsView {
                routes: effective_permissions.routes,
                commands: effective_permissions.commands,
                admin: effective_permissions.admin,
                database: db_permission_name(&effective_permissions.database).to_string(),
            },
        },
    );
}
```

- [ ] **Step 4: Run bootstrap/loader related tests**

Run: `cargo test -p sushi-core disabled_plugin_is_not_invoked_after_scan_registration -q`  
Expected: PASS

Run: `cargo test -p sushi-cli --lib -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-cli/src/app.rs crates/sushi-core/src/lua/loader.rs crates/sushi-core/src/plugin/manager.rs
git commit -m "feat(core): register plugin identity state and skip disabled startup init"
```

### Task 5: Add Admin State Mutation Endpoint and UI Controls

**Files:**
- Modify: `crates/sushi-admin/src/routes/plugins.rs`
- Modify: `crates/sushi-admin/src/router.rs`
- Modify: `web/templates/admin/partials/plugins_rows.html`
- Modify: `web/static/admin/js/plugins.js`
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing admin integration tests**

```rust
#[tokio::test]
async fn admin_can_toggle_plugin_enabled_state() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/kv-store/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"enabled":false,"reason":"maintenance"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(disable_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn viewer_cannot_toggle_plugin_enabled_state() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/plugins/kv-store/state")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"enabled":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run tests to confirm route missing failure**

Run: `cargo test -p sushi-admin --test admin_web admin_can_toggle_plugin_enabled_state -q`  
Expected: FAIL with `404` (route not registered).

- [ ] **Step 3: Implement route handler + router entry + UI toggle action**

```rust
// crates/sushi-admin/src/routes/plugins.rs
use axum::Json;
use axum::Extension;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PluginStateUpdateRequest {
    pub enabled: bool,
    #[serde(default)]
    pub reason: String,
}

pub async fn plugin_state_update_api(
    Path(plugin): Path<String>,
    State(ctx): State<SushiContext>,
    Extension(auth): Extension<crate::router::AdminAuthContext>,
    Json(payload): Json<PluginStateUpdateRequest>,
) -> impl IntoResponse {
    match ctx
        .plugins
        .set_plugin_enabled(
            &plugin,
            payload.enabled,
            Some(&auth.role),
            Some(payload.reason.as_str()),
        )
        .await
    {
        Ok(plugin_info) => (StatusCode::OK, axum::Json(plugin_info)).into_response(),
        Err(err) if err.contains("plugin not found") => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({ "error": "plugin not found" })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": err })),
        )
            .into_response(),
    }
}
```

```rust
// crates/sushi-admin/src/router.rs
use axum::routing::patch;

.route(
    "/admin/api/plugins/{plugin}/state",
    patch(plugins::plugin_state_update_api),
)
```

```html
<!-- web/templates/admin/partials/plugins_rows.html -->
<td class="actions">
  <a
    class="ui-action-link"
    href="/admin/plugins/{{ p.name }}"
    data-workspace-path="/admin/plugins/{{ p.name }}"
    data-workspace-title="{{ p.name }} Workspace"
    @click.prevent="openWorkspace($el.dataset.workspacePath, $el.dataset.workspaceTitle)"
  >
    Workspace
  </a>
  {% if p.enabled %}
    <button type="button" class="ui-action-link danger" @click="togglePlugin('{{ p.name }}', false)">Disable</button>
  {% else %}
    <button type="button" class="ui-action-link" @click="togglePlugin('{{ p.name }}', true)">Enable</button>
  {% endif %}
</td>
```

```javascript
// web/static/admin/js/plugins.js
async togglePlugin(pluginName, enabled) {
  const target = String(pluginName || '').trim();
  if (!target) {
    return;
  }

  const response = await fetch(`/admin/api/plugins/${encodeURIComponent(target)}/state`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    credentials: 'same-origin',
    body: JSON.stringify({ enabled }),
  });

  if (!response.ok) {
    if (window.AdminUI && typeof window.AdminUI.notify === 'function') {
      window.AdminUI.notify({
        title: 'Plugin update failed',
        message: `Could not update plugin state for ${target}`,
        tone: 'danger',
      });
    }
    return;
  }

  if (window.AdminUI && typeof window.AdminUI.notify === 'function') {
    window.AdminUI.notify({
      title: enabled ? 'Plugin enabled' : 'Plugin disabled',
      message: `${target} updated successfully`,
      tone: 'success',
    });
  }

  document.body.dispatchEvent(new CustomEvent('plugins:refresh'));
}
```

- [ ] **Step 4: Run admin test + template checks**

Run: `cargo test -p sushi-admin --test admin_web admin_can_toggle_plugin_enabled_state viewer_cannot_toggle_plugin_enabled_state plugins_rows_template_opens_workspace_in_tab_when_available -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/src/routes/plugins.rs crates/sushi-admin/src/router.rs web/templates/admin/partials/plugins_rows.html web/static/admin/js/plugins.js crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): add plugin enable-disable controls and state api"
```

### Task 6: Extend CLI Plugin Commands for Governance Controls

**Files:**
- Modify: `crates/sushi-cli/src/commands/plugin.rs`

- [ ] **Step 1: Add failing command parsing test coverage**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: PluginCommand,
    }

    #[test]
    fn parse_enable_subcommand() {
        let cli = TestCli::try_parse_from(["plugin", "enable", "kv-store"]).unwrap();
        match cli.command {
            PluginCommand::Enable { plugin, .. } => assert_eq!(plugin, "kv-store"),
            _ => panic!("expected enable command"),
        }
    }
}
```

- [ ] **Step 2: Run test to confirm enum variant missing**

Run: `cargo test -p sushi-cli parse_enable_subcommand -q`  
Expected: FAIL with missing `Enable` variant.

- [ ] **Step 3: Add `status/enable/disable` and manager calls**

```rust
// crates/sushi-cli/src/commands/plugin.rs
#[derive(Subcommand)]
pub enum PluginCommand {
    /// List all discovered plugins
    List,
    /// Show plugin status
    Status {
        /// Plugin name (optional)
        plugin: Option<String>,
    },
    /// Enable plugin runtime dispatch
    Enable {
        /// Plugin name
        plugin: String,
        /// Optional reason
        #[arg(long)]
        reason: Option<String>,
    },
    /// Disable plugin runtime dispatch
    Disable {
        /// Plugin name
        plugin: String,
        /// Optional reason
        #[arg(long)]
        reason: Option<String>,
    },
}

pub async fn run(args: PluginArgs, role: &str) -> Result<()> {
    let ctx = crate::app::bootstrap(None).await?;

    match args.command {
        PluginCommand::List => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:list")
                .await?;
            for plugin in ctx.plugins.list_plugins().await {
                println!(
                    "{}\t{}\tenabled={}\tloaded={}",
                    plugin.name, plugin.version, plugin.enabled, plugin.loaded
                );
            }
        }
        PluginCommand::Status { plugin } => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:status")
                .await?;
            let plugins = ctx.plugins.list_plugins().await;
            for item in plugins.into_iter().filter(|p| {
                plugin
                    .as_ref()
                    .map(|name| p.name == *name)
                    .unwrap_or(true)
            }) {
                println!(
                    "{}\t{}\tenabled={}\tloaded={}\tsource={}",
                    item.name, item.version, item.enabled, item.loaded, item.source_kind
                );
            }
        }
        PluginCommand::Enable { plugin, reason } => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:enable")
                .await?;
            let state = ctx
                .plugins
                .set_plugin_enabled(&plugin, true, Some(role), reason.as_deref())
                .await
                .map_err(anyhow::Error::msg)?;
            println!("enabled {} (loaded={})", state.name, state.loaded);
        }
        PluginCommand::Disable { plugin, reason } => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:disable")
                .await?;
            let state = ctx
                .plugins
                .set_plugin_enabled(&plugin, false, Some(role), reason.as_deref())
                .await
                .map_err(anyhow::Error::msg)?;
            println!("disabled {} (loaded={})", state.name, state.loaded);
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run CLI command tests**

Run: `cargo test -p sushi-cli parse_enable_subcommand -q`  
Expected: PASS

Run: `cargo test -p sushi-cli --lib -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-cli/src/commands/plugin.rs
git commit -m "feat(cli): add plugin status enable disable commands"
```

### Task 7: Map Disabled Errors to Explicit API/Admin/CLI Responses

**Files:**
- Modify: `crates/sushi-api/src/router.rs`
- Modify: `crates/sushi-admin/src/router.rs`
- Modify: `crates/sushi-admin/src/routes/workspace.rs`
- Modify: `crates/sushi-cli/src/commands/run.rs`
- Modify/Test: `crates/sushi-api/src/router.rs` test module

- [ ] **Step 1: Add failing API test for disabled route behavior**

```rust
#[tokio::test]
async fn test_plugin_api_dispatch_returns_forbidden_when_plugin_disabled() {
    let lua = create_sandboxed_vm().unwrap();
    let sushi = lua.create_table().unwrap();
    let handlers = lua.create_table().unwrap();
    sushi.set("__handlers", handlers.clone()).unwrap();
    lua.globals().set("sushi", sushi).unwrap();

    let handler = lua
        .create_async_function(|_, ()| async { Ok("ok".to_string()) })
        .unwrap();
    handlers.set("h_disabled", handler).unwrap();

    let manager = PluginManager::new();
    manager.register_vm("plugin", lua).await;
    manager
        .register_api_handler_with_policy_and_public("GET", "/api/disabled", "plugin", "h_disabled", None, true)
        .await;
    manager
        .set_plugin_enabled("plugin", false, Some("admin"), Some("maintenance"))
        .await
        .unwrap();

    let state = PluginApiState {
        plugins: manager,
        auth_state: test_auth_state(),
        logs: Arc::new(LogService::new()),
        body_size_limit: 1024,
        route_map: Vec::new(),
    };

    let req = Request::builder()
        .method("GET")
        .uri("/api/disabled")
        .body(Body::empty())
        .unwrap();

    let response = plugin_api_dispatch(State(state), req).await.into_response();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run test and verify current 500 behavior fails assertion**

Run: `cargo test -p sushi-api test_plugin_api_dispatch_returns_forbidden_when_plugin_disabled -q`  
Expected: FAIL (returns `500` before mapping logic exists).

- [ ] **Step 3: Implement disabled-error mapping in API/Admin/workspace/CLI run**

```rust
// crates/sushi-api/src/router.rs
fn is_plugin_disabled_error(err: &str) -> bool {
    err.starts_with("plugin_disabled:")
}

// inside Some(Err(e)) arm of plugin_api_dispatch
if is_plugin_disabled_error(&e) {
    return (
        axum::http::StatusCode::FORBIDDEN,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::json!({
            "error": "plugin_disabled",
            "message": e,
        })
        .to_string(),
    )
        .into_response();
}
```

```rust
// crates/sushi-admin/src/router.rs and crates/sushi-admin/src/routes/workspace.rs
if err.starts_with("plugin_disabled:") {
    return (
        StatusCode::FORBIDDEN,
        "Plugin disabled by administrator",
    )
        .into_response();
}
```

```rust
// crates/sushi-cli/src/commands/run.rs
Some(Err(e)) if e.starts_with("plugin_disabled:") => {
    anyhow::bail!("plugin is disabled by administrator: {}", args.plugin_name)
}
```

- [ ] **Step 4: Run response behavior tests**

Run: `cargo test -p sushi-api test_plugin_api_dispatch_returns_forbidden_when_plugin_disabled -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web plugin_workspace_page_rejects_unknown_plugin -q`  
Expected: PASS

Run: `cargo test -p sushi-cli --lib -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-api/src/router.rs crates/sushi-admin/src/router.rs crates/sushi-admin/src/routes/workspace.rs crates/sushi-cli/src/commands/run.rs
git commit -m "fix(runtime): return explicit forbidden responses for disabled plugins"
```

### Task 8: End-to-End Verification and Documentation Sync

**Files:**
- Modify: `docs/wiki/architecture/plugin-system.md`
- Modify: `docs/engineering/plugin-authoring-standards.md`

- [ ] **Step 1: Update docs to reflect backend-governed runtime state**

```md
# docs/engineering/plugin-authoring-standards.md (add to permission section)
- Runtime activation (`enabled/disabled`) is controlled by platform governance state.
- `plugin.toml` permissions remain declaration-time upper bounds and cannot force runtime enablement.
```

```md
# docs/wiki/architecture/plugin-system.md (replace old statement)
- Plugin manifests declare capability envelopes.
- Production runtime activation is governed by admin/CLI plugin state controls.
```

- [ ] **Step 2: Run focused test suites**

Run: `cargo test -p sushi-core -q`  
Expected: PASS

Run: `cargo test -p sushi-api -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

Run: `cargo test -p sushi-cli --lib -q`  
Expected: PASS

- [ ] **Step 3: Run full workspace gate**

Run: `cargo test --workspace -q`  
Expected: PASS

- [ ] **Step 4: Commit docs + final verification delta**

```bash
git add docs/engineering/plugin-authoring-standards.md docs/wiki/architecture/plugin-system.md
git commit -m "docs(plugin): document runtime governance control model"
```
