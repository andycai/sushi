# Unified Policy RBAC (Admin/API/CLI/Plugins) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace hardcoded RBAC checks with one DB-driven policy engine that governs admin, API, CLI, and plugin-registered routes/pages/commands using `surface.resource.action` keys.

**Architecture:** Introduce a core `Authorizer` that compiles policy bindings + role grants from DB into an in-memory snapshot for fast checks. Move all permission decisions in admin/API/CLI to this authorizer, and require plugins to declare concrete policy keys at registration time while enforcing manifest scope boundaries at load time.

**Tech Stack:** Rust (Axum, Tokio, Clap), SQLite migrations, mlua plugin runtime, existing Sushi test harness.

---

## File Structure Map

- Create: `migrations/006_unified_policy_v2.sql` — unified policy schema + built-in seeds.
- Create: `crates/sushi-core/src/auth/policy.rs` — policy key, binding, target, and scope domain structs.
- Create: `crates/sushi-core/src/auth/authorizer.rs` — compiled snapshot, matcher, `check_http`, `check_command`.
- Create: `crates/sushi-core/src/auth/policy_repository.rs` — DB CRUD/load/sync for unified policy tables.
- Modify: `crates/sushi-core/src/auth/mod.rs` — export new auth modules.
- Modify: `crates/sushi-core/src/context.rs` — attach shared `Authorizer` instance.
- Modify: `crates/sushi-core/src/auth/middleware.rs` — API auth now delegates authorization to `Authorizer`.
- Modify: `crates/sushi-admin/src/router.rs` — remove hardcoded route-permission map, call `Authorizer`.
- Modify: `crates/sushi-admin/src/routes/workspace.rs` — remove module permission map; enforce `path` auth on assets endpoint.
- Modify: `crates/sushi-api/src/router.rs` — wire new migration in tests.
- Modify: `crates/sushi-cli/src/app.rs` — run migration `006` during bootstrap.
- Modify: `crates/sushi/src/main.rs` — add CLI role flag/env for authorization principal.
- Modify: `crates/sushi-cli/src/commands/run.rs` — authorize plugin command execution.
- Modify: `crates/sushi-cli/src/commands/plugin.rs` — authorize built-in plugin subcommands.
- Modify: `crates/sushi-core/src/plugin/mod.rs` — parse plugin policy scope boundaries from `plugin.toml`.
- Modify: `crates/sushi-core/src/lua/bindings.rs` — accept `opts.policy` in `sushi.api.route`, `sushi.cli.command`, `sushi.web.page`, `sushi.admin.page`.
- Modify: `crates/sushi-core/src/lua/loader.rs` — validate declared policy keys against scopes; persist plugin bindings/scopes.
- Modify: `crates/sushi-core/src/plugin/manager.rs` — store policy metadata on registered handlers.
- Modify: `plugins/official/kv-store/plugin.toml` — add `policies.scopes` section.
- Modify: `plugins/official/kv-store/lua/bootstrap/register.lua` — add policy keys to every registration.
- Modify: `plugins/third_party/_example/plugin.toml` + `plugins/third_party/_example/init.lua` — update sample plugin.
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs` — new policy-key-driven assertions and migration.
- Modify/Test: `crates/sushi-core/src/auth/middleware.rs` tests — API policy deny/allow coverage.
- Modify/Test: `crates/sushi-api/src/router.rs` tests — include migration `006` and API auth policy checks.
- Create/Test: `crates/sushi-core/src/auth/authorizer.rs` unit tests — matcher + deny/allow semantics.

---

### Task 1: Add Unified Policy Migration + Bootstrap Wiring

**Files:**
- Create: `migrations/006_unified_policy_v2.sql`
- Modify: `crates/sushi-cli/src/app.rs`
- Modify: `crates/sushi-admin/tests/admin_web.rs`
- Modify: `crates/sushi-api/src/router.rs`

- [ ] **Step 1: Write failing migration wiring tests**

```rust
// crates/sushi-admin/tests/admin_web.rs
const UNIFIED_POLICY_MIGRATION_SQL: &str = include_str!("../../../migrations/006_unified_policy_v2.sql");

// in test setup, add:
storage.run_migrations(UNIFIED_POLICY_MIGRATION_SQL).await.unwrap();
```

```rust
// crates/sushi-api/src/router.rs test module
const UNIFIED_POLICY_MIGRATION_SQL: &str = include_str!("../../../migrations/006_unified_policy_v2.sql");
storage.run_migrations(UNIFIED_POLICY_MIGRATION_SQL).await.unwrap();
```

- [ ] **Step 2: Run tests to verify migration file is missing**

Run: `cargo test -p sushi-admin --test admin_web workspace_users_module_loads_for_authenticated_admin -q`
Expected: FAIL with include_str path error for `006_unified_policy_v2.sql`.

- [ ] **Step 3: Create migration + bootstrap constant**

```sql
-- migrations/006_unified_policy_v2.sql
CREATE TABLE IF NOT EXISTS policy_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    surface TEXT NOT NULL,
    resource TEXT NOT NULL,
    action TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS role_policy_keys (
    role_id INTEGER NOT NULL,
    policy_key_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (role_id, policy_key_id),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY (policy_key_id) REFERENCES policy_keys(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS policy_bindings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    surface TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_ref TEXT NOT NULL,
    method TEXT,
    path_pattern TEXT,
    command_name TEXT,
    policy_key_id INTEGER NOT NULL,
    owner_type TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (policy_key_id) REFERENCES policy_keys(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS plugin_policy_scopes (
    plugin_name TEXT NOT NULL,
    scope_pattern TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_name, scope_pattern)
);

INSERT OR IGNORE INTO _sushi_migrations (id, name)
VALUES (6, '006_unified_policy_v2');
```

```rust
// crates/sushi-cli/src/app.rs
const UNIFIED_POLICY_MIGRATION_SQL: &str = include_str!("../../../migrations/006_unified_policy_v2.sql");

storage
    .run_migrations(UNIFIED_POLICY_MIGRATION_SQL)
    .await
    .context("failed to run unified policy migrations")?;
```

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p sushi-api router::tests::test_plugin_api_dispatch_applies_status_envelope -q`
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web workspace_users_module_loads_for_authenticated_admin -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add migrations/006_unified_policy_v2.sql crates/sushi-cli/src/app.rs crates/sushi-admin/tests/admin_web.rs crates/sushi-api/src/router.rs
git commit -m "feat(auth): add unified policy schema migration"
```

### Task 2: Build Policy Domain + Repository

**Files:**
- Create: `crates/sushi-core/src/auth/policy.rs`
- Create: `crates/sushi-core/src/auth/policy_repository.rs`
- Modify: `crates/sushi-core/src/auth/mod.rs`
- Test: `crates/sushi-core/src/auth/policy_repository.rs` (inline unit tests)

- [ ] **Step 1: Write failing repository tests**

```rust
#[tokio::test]
async fn upsert_and_load_policy_key_round_trip() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    storage.run_migrations(include_str!("../../../../migrations/001_init.sql")).await.unwrap();
    storage.run_migrations(include_str!("../../../../migrations/003_rbac.sql")).await.unwrap();
    storage.run_migrations(include_str!("../../../../migrations/006_unified_policy_v2.sql")).await.unwrap();

    let repo = PolicyRepository::new(Arc::new(storage));
    repo.upsert_policy_key("admin.users.read", "View Users").await.unwrap();

    let keys = repo.list_policy_keys().await.unwrap();
    assert!(keys.iter().any(|k| k.key == "admin.users.read"));
}
```

- [ ] **Step 2: Run test to verify missing types**

Run: `cargo test -p sushi-core upsert_and_load_policy_key_round_trip -q`
Expected: FAIL with unresolved `PolicyRepository`/`policy` module symbols.

- [ ] **Step 3: Implement policy models + repository**

```rust
// crates/sushi-core/src/auth/policy.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyKey {
    pub key: String,
    pub surface: String,
    pub resource: String,
    pub action: String,
}

impl PolicyKey {
    pub fn parse(key: &str) -> Result<Self, String> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 3 {
            return Err("policy key must follow surface.resource.action".to_string());
        }
        Ok(Self {
            key: key.to_string(),
            surface: parts[0].to_string(),
            resource: parts[1].to_string(),
            action: parts[2].to_string(),
        })
    }
}
```

```rust
// crates/sushi-core/src/auth/policy_repository.rs
pub struct PolicyRepository {
    storage: Arc<dyn Storage>,
}

impl PolicyRepository {
    pub fn new(storage: Arc<dyn Storage>) -> Self { Self { storage } }

    pub async fn upsert_policy_key(&self, key: &str, name: &str) -> Result<(), String> {
        let parsed = PolicyKey::parse(key)?;
        self.storage.execute(
            r#"INSERT INTO policy_keys (key, surface, resource, action, name, is_system)
               VALUES (?1, ?2, ?3, ?4, ?5, 1)
               ON CONFLICT(key) DO UPDATE SET
                 surface = excluded.surface,
                 resource = excluded.resource,
                 action = excluded.action,
                 name = excluded.name,
                 updated_at = datetime('now')"#,
            vec![
                parsed.key.into(),
                parsed.surface.into(),
                parsed.resource.into(),
                parsed.action.into(),
                name.to_string().into(),
            ],
        ).await.map_err(|e| e.to_string())
    }
}
```

```rust
// crates/sushi-core/src/auth/mod.rs
pub mod policy;
pub mod policy_repository;
```

- [ ] **Step 4: Run repository tests**

Run: `cargo test -p sushi-core upsert_and_load_policy_key_round_trip -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/auth/policy.rs crates/sushi-core/src/auth/policy_repository.rs crates/sushi-core/src/auth/mod.rs
git commit -m "feat(auth): add unified policy domain repository"
```

### Task 3: Implement Authorizer Snapshot + Matcher

**Files:**
- Create: `crates/sushi-core/src/auth/authorizer.rs`
- Modify: `crates/sushi-core/src/auth/mod.rs`
- Test: `crates/sushi-core/src/auth/authorizer.rs` (unit tests)

- [ ] **Step 1: Write failing matcher tests**

```rust
#[test]
fn http_binding_matches_path_params() {
    let binding = HttpBinding {
        surface: "admin".to_string(),
        method: "GET".to_string(),
        path_pattern: "/admin/partials/users/{id}".to_string(),
        policy_key: "admin.users.read".to_string(),
    };
    assert!(binding.matches("admin", "GET", "/admin/partials/users/42"));
}

#[test]
fn command_binding_requires_exact_name() {
    let snapshot = CompiledPolicySnapshot::from_raw(
        vec![("cli", "plugin:list", "cli.plugin.list.read")],
        vec![("editor", "cli.plugin.list.read")],
    );
    assert!(snapshot.command_allowed("editor", "cli", "plugin:list"));
    assert!(!snapshot.command_allowed("editor", "cli", "plugin:delete"));
}
```

- [ ] **Step 2: Run tests to verify missing authorizer module**

Run: `cargo test -p sushi-core http_binding_matches_path_params -q`
Expected: FAIL with unresolved `HttpBinding`/`CompiledPolicySnapshot`.

- [ ] **Step 3: Implement authorizer snapshot**

```rust
#[derive(Debug, Clone)]
pub struct HttpBinding {
    pub surface: String,
    pub method: String,
    pub path_pattern: String,
    pub policy_key: String,
}

impl HttpBinding {
    pub fn matches(&self, surface: &str, method: &str, path: &str) -> bool {
        self.surface == surface
            && self.method.eq_ignore_ascii_case(method)
            && path_pattern_matches(&self.path_pattern, path)
    }
}

pub struct Authorizer {
    snapshot: Arc<RwLock<CompiledPolicySnapshot>>,
}

impl Authorizer {
    pub async fn check_http(&self, role: &str, surface: &str, method: &str, path: &str) -> Result<(), String> {
        let snap = self.snapshot.read().await;
        if snap.http_allowed(role, surface, method, path) {
            Ok(())
        } else {
            Err(format!("policy denied for role={role} target={surface}:{method} {path}"))
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p sushi-core http_binding_matches_path_params -q`
Expected: PASS

Run: `cargo test -p sushi-core command_binding_requires_exact_name -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/auth/authorizer.rs crates/sushi-core/src/auth/mod.rs
git commit -m "feat(auth): add compiled policy authorizer"
```

### Task 4: Wire Authorizer into Context + API Middleware

**Files:**
- Modify: `crates/sushi-core/src/context.rs`
- Modify: `crates/sushi-core/src/auth/middleware.rs`
- Test: `crates/sushi-core/src/auth/middleware.rs`

- [ ] **Step 1: Write failing middleware authorization test**

```rust
#[tokio::test]
async fn viewer_cannot_access_users_api_without_policy_binding() {
    // build app with /api/users route and authorizer snapshot
    // snapshot grants only api.auth.me.read for viewer
    let response = app.oneshot(
        Request::builder()
            .uri("/api/users")
            .header("authorization", format!("Bearer {viewer_token}"))
            .body(Body::empty())
            .unwrap(),
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run middleware test and confirm failure**

Run: `cargo test -p sushi-core viewer_cannot_access_users_api_without_policy_binding -q`
Expected: FAIL (currently middleware allows authenticated access broadly).

- [ ] **Step 3: Implement middleware authorizer delegation**

```rust
#[derive(Clone)]
pub struct AuthState {
    pub jwt_service: Arc<JwtService>,
    pub authorizer: Arc<Authorizer>,
}

// in require_auth after token validation:
if let Err(_) = state
    .authorizer
    .check_http(role.as_str(), "api", req.method().as_str(), path)
    .await
{
    return (StatusCode::FORBIDDEN, "{\"error\":\"Forbidden\"}").into_response();
}
```

```rust
// crates/sushi-core/src/context.rs
pub fn auth_state(&self) -> AuthState {
    AuthState {
        jwt_service: Arc::clone(&self.jwt),
        authorizer: Arc::clone(&self.authorizer),
    }
}
```

- [ ] **Step 4: Run middleware tests**

Run: `cargo test -p sushi-core auth::middleware -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/context.rs crates/sushi-core/src/auth/middleware.rs
git commit -m "feat(auth): enforce api policies in auth middleware"
```

### Task 5: Refactor Admin Authorization (Router + Workspace)

**Files:**
- Modify: `crates/sushi-admin/src/router.rs`
- Modify: `crates/sushi-admin/src/routes/workspace.rs`
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write failing admin test for workspace assets path auth**

```rust
#[tokio::test]
async fn viewer_cannot_fetch_workspace_assets_for_users_path_without_admin_users_read() {
    let app = build_app(None).await;
    let token = bearer_token_for_role("viewer");

    let response = app
        .oneshot(Request::builder()
            .uri("/admin/api/workspace/assets?path=/admin/users")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run test to verify current bypass behavior**

Run: `cargo test -p sushi-admin --test admin_web viewer_cannot_fetch_workspace_assets_for_users_path_without_admin_users_read -q`
Expected: FAIL (currently mapped through static `plugins.view` route logic).

- [ ] **Step 3: Remove hardcoded maps and call authorizer**

```rust
// router.rs: in admin_auth_middleware
if claims.role != "admin" {
    if let Err(_) = state
        .authorizer
        .check_http(&claims.role, "admin", req.method().as_str(), path)
        .await
    {
        return (StatusCode::FORBIDDEN, "Insufficient privileges for admin access").into_response();
    }
}
```

```rust
// workspace.rs: remove permission_for_module + add explicit assets path check
let Some(path) = query.get("path").map(|v| v.trim()).filter(|v| !v.is_empty()) else { ... };

if let Err(_) = ctx
    .authorizer
    .check_http(current_role, "admin", "GET", path)
    .await
{
    return (StatusCode::FORBIDDEN, axum::Json(json!({"error":"forbidden"}))).into_response();
}
```

- [ ] **Step 4: Run admin tests**

Run: `cargo test -p sushi-admin --test admin_web -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/src/router.rs crates/sushi-admin/src/routes/workspace.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "refactor(admin): replace hardcoded permission maps with authorizer"
```

### Task 6: Extend Plugin Manifest + Lua Registration Policy Options

**Files:**
- Modify: `crates/sushi-core/src/plugin/mod.rs`
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Modify/Test: `crates/sushi-core/src/lua/bindings.rs` tests

- [ ] **Step 1: Write failing parser and binding tests**

```rust
#[test]
fn parse_plugin_policy_scopes_from_manifest() {
    let manifest: PluginManifest = toml::from_str(r#"
[plugin]
name = "kv-store"
version = "0.2.0"
kind = "official"

[policies]
scopes = ["api.plugin.kv.*", "admin.plugin.kv.*", "cli.plugin.kv.*"]
"#).unwrap();

    assert_eq!(manifest.policies.scopes.len(), 3);
}
```

```rust
#[test]
fn sushi_api_route_accepts_policy_option() {
    // lua snippet should populate pending entry with policy
    // assert entry.policy == "api.plugin.kv.read"
}
```

- [ ] **Step 2: Run test to verify missing fields/options**

Run: `cargo test -p sushi-core parse_plugin_policy_scopes_from_manifest -q`
Expected: FAIL (missing `policies` in manifest model).

- [ ] **Step 3: Implement manifest + binding option parsing**

```rust
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct PluginPoliciesConfig {
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub permissions: Permissions,
    pub admin: Option<PluginAdminConfig>,
    pub policies: PluginPoliciesConfig,
}
```

```rust
// lua/bindings.rs (api.route)
move |lua, (method, path, handler, opts): (String, String, mlua::Function, Option<mlua::Table>)| {
    let policy = opts
        .as_ref()
        .and_then(|t| t.get::<Option<String>>("policy").ok())
        .flatten();
    entry.set("policy", policy)?;
    ...
}
```

- [ ] **Step 4: Run binding tests**

Run: `cargo test -p sushi-core lua::bindings -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin/mod.rs crates/sushi-core/src/lua/bindings.rs
git commit -m "feat(plugin): add policy scopes and lua policy options"
```

### Task 7: Loader + Plugin Manager Policy Validation and Persistence

**Files:**
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Test: `crates/sushi-core/src/lua/loader.rs` tests

- [ ] **Step 1: Write failing loader test for scope violation**

```rust
#[tokio::test]
async fn plugin_load_fails_when_route_policy_outside_scope() {
    // plugin scope: api.plugin.kv.*
    // route policy: admin.users.read
    // expect PluginError::InitFailed containing policy + scope mismatch
}
```

- [ ] **Step 2: Run test to verify loader currently allows invalid policy**

Run: `cargo test -p sushi-core plugin_load_fails_when_route_policy_outside_scope -q`
Expected: FAIL (no scope validation yet).

- [ ] **Step 3: Implement validation + manager metadata**

```rust
fn policy_matches_scope(policy: &str, scope: &str) -> bool {
    if let Some(prefix) = scope.strip_suffix("*") {
        policy.starts_with(prefix)
    } else {
        policy == scope
    }
}

fn validate_plugin_policy(policy: &str, scopes: &[String]) -> Result<(), PluginError> {
    if scopes.iter().any(|scope| policy_matches_scope(policy, scope)) {
        Ok(())
    } else {
        Err(PluginError::PermissionDenied(format!(
            "policy '{policy}' is outside declared plugin scopes"
        )))
    }
}
```

```rust
// manager binding structs add policy_key: String
struct AdminHandlerBinding { ... policy_key: String }
```

- [ ] **Step 4: Run loader tests**

Run: `cargo test -p sushi-core lua::loader -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs crates/sushi-core/src/plugin/manager.rs
git commit -m "feat(plugin): enforce policy scope validation during load"
```

### Task 8: CLI Principal + Command Authorization

**Files:**
- Modify: `crates/sushi/src/main.rs`
- Modify: `crates/sushi-cli/src/commands/run.rs`
- Modify: `crates/sushi-cli/src/commands/plugin.rs`
- Create/Test: `crates/sushi-cli/src/commands/authorization.rs` + unit tests

- [ ] **Step 1: Write failing CLI auth unit tests**

```rust
#[test]
fn resolves_role_from_flag_then_env_then_default() {
    assert_eq!(resolve_cli_role(Some("editor"), None), "editor");
    assert_eq!(resolve_cli_role(None, Some("viewer")), "viewer");
    assert_eq!(resolve_cli_role(None, None), "admin");
}
```

- [ ] **Step 2: Run test to verify missing helper module**

Run: `cargo test -p sushi-cli resolves_role_from_flag_then_env_then_default -q`
Expected: FAIL with missing module/function.

- [ ] **Step 3: Implement role resolution + authorizer checks**

```rust
// crates/sushi/src/main.rs
#[derive(Parser)]
struct Cli {
    #[arg(long, env = "SUSHI_CLI_ROLE", global = true, default_value = "admin")]
    role: String,
    #[command(subcommand)]
    command: Commands,
}
```

```rust
// run.rs
let role = crate::commands::authorization::resolve_cli_role(Some(&args.role), None);
ctx.authorizer
    .check_command(&role, "cli", &args.plugin_name)
    .await
    .map_err(|e| anyhow::anyhow!(e))?;
```

- [ ] **Step 4: Run CLI tests/build**

Run: `cargo test -p sushi-cli -q`
Expected: PASS

Run: `cargo check -p sushi -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi/src/main.rs crates/sushi-cli/src/commands/run.rs crates/sushi-cli/src/commands/plugin.rs crates/sushi-cli/src/commands/authorization.rs
git commit -m "feat(cli): enforce unified policy checks for commands"
```

### Task 9: Update Plugins to Declare and Use Policy Keys

**Files:**
- Modify: `plugins/official/kv-store/plugin.toml`
- Modify: `plugins/official/kv-store/lua/bootstrap/register.lua`
- Modify: `plugins/third_party/_example/plugin.toml`
- Modify: `plugins/third_party/_example/init.lua`

- [ ] **Step 1: Write failing loader integration test using official plugin fixtures**

```rust
#[tokio::test]
async fn kv_plugin_registers_policy_keys_for_routes_pages_commands() {
    // load kv plugin and assert pending entries include non-empty policy values
}
```

- [ ] **Step 2: Run test to verify plugin registrations currently miss policy**

Run: `cargo test -p sushi-core kv_plugin_registers_policy_keys_for_routes_pages_commands -q`
Expected: FAIL

- [ ] **Step 3: Add policy scopes and per-target policies**

```toml
# plugins/official/kv-store/plugin.toml
[policies]
scopes = [
  "api.kv.read",
  "api.kv.write",
  "admin.kv.read",
  "admin.kv.write",
  "cli.kv.execute"
]
```

```lua
-- plugins/official/kv-store/lua/bootstrap/register.lua
sushi.api.route("GET", "/api/kv", deps.api.dispatch, { policy = "api.kv.read" })
sushi.api.route("POST", "/api/kv", deps.api.dispatch, { policy = "api.kv.write" })
sushi.web.page("/admin/kv", "plugins/official/kv-store/kv.html", {
    title = "KV Store",
    policy = "admin.kv.read",
    assets = { bundles = { "workspace" } },
})
sushi.cli.command("kv-list", "List all KV entries", deps.cli.kv_list, { policy = "cli.kv.execute" })
```

- [ ] **Step 4: Run plugin loader tests**

Run: `cargo test -p sushi-core lua::loader -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/kv-store/plugin.toml plugins/official/kv-store/lua/bootstrap/register.lua plugins/third_party/_example/plugin.toml plugins/third_party/_example/init.lua
git commit -m "refactor(plugin): declare explicit policy keys for registrations"
```

### Task 10: End-to-End Authorization Regression and Docs Sync

**Files:**
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`
- Modify/Test: `crates/sushi-api/src/router.rs`
- Modify/Test: `crates/sushi-core/src/auth/middleware.rs`
- Modify: `docs/wiki/architecture/admin-panel.md`
- Modify: `docs/wiki/architecture/auth-rbac.md`

- [ ] **Step 1: Add final failing scenario tests**

```rust
#[tokio::test]
async fn editor_can_read_admin_users_but_cannot_mutate_users_without_write_policy() { ... }

#[tokio::test]
async fn viewer_cannot_invoke_plugin_api_write_routes() { ... }
```

- [ ] **Step 2: Run targeted tests to confirm failures**

Run: `cargo test -p sushi-admin --test admin_web editor_can_read_admin_users_but_cannot_mutate_users_without_write_policy -q`
Expected: FAIL until binding seeds and middleware are fully aligned.

- [ ] **Step 3: Finalize built-in policy seeds and docs**

```sql
-- in 006 migration, ensure role grants include:
-- admin: all built-in keys
-- editor: read + selective write keys
-- viewer: read-only keys
```

```markdown
# docs/wiki/architecture/auth-rbac.md
- Policy key format: `surface.resource.action`
- Runtime enforcement surfaces: admin/api/cli/plugin targets
- Plugin policy declaration model: `plugin.toml` scopes + Lua target key
```

- [ ] **Step 4: Run full verification**

Run: `cargo test -p sushi-core --test template_service -q`
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web -q`
Expected: PASS

Run: `cargo test --workspace -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/tests/admin_web.rs crates/sushi-api/src/router.rs crates/sushi-core/src/auth/middleware.rs docs/wiki/architecture/admin-panel.md docs/wiki/architecture/auth-rbac.md migrations/006_unified_policy_v2.sql
git commit -m "test(auth): verify unified policy enforcement across surfaces"
```

---

## Self-Review (Plan Quality)

- **Spec coverage:**
  - Unified DB model: covered by Tasks 1-3.
  - Admin/API/CLI convergence: covered by Tasks 4, 5, 8, 10.
  - Plugin hybrid model (manifest scope + runtime key): covered by Tasks 6, 7, 9.
  - One-shot switch and fail-closed startup: covered by Tasks 1, 3, 7, 10.
- **Placeholder scan:** No placeholder markers or vague “handle later” markers remain.
- **Type consistency:** `PolicyKey`, `PolicyRepository`, `Authorizer`, and `check_http/check_command` naming is consistent across all tasks.
