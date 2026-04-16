# Unified Policy RBAC Design (Admin/API/CLI/Plugins)

Date: 2026-04-16  
Status: Proposed (validated in brainstorming)  
Scope: `crates/sushi-core`, `crates/sushi-admin`, `crates/sushi-api`, `crates/sushi-cli`, `plugins/*`, `migrations/*`

## 1. Problem Statement

Current RBAC checks are fragmented and partially hardcoded:

- Admin access control is primarily mapped inside `crates/sushi-admin/src/router.rs`.
- Workspace module permission mapping is duplicated in `crates/sushi-admin/src/routes/workspace.rs`.
- API middleware currently enforces auth and a few special cases but does not use one unified policy source.
- CLI command execution has no unified RBAC gate shared with admin/api.
- Plugin route/page/command registration does not attach first-class policy keys that can be centrally governed.

This causes high maintenance cost, high drift risk, and poor plugin author ergonomics.

## 2. User-Confirmed Decisions

The user approved the following constraints:

1. Unify RBAC for **admin + api + cli + plugin registered targets**.
2. Use **DB-driven policy model** as the source of truth.
3. **No backward-compatibility slug mapping**; move directly to the new model.
4. Use **one-shot switch-over** (no dual-run compatibility period).
5. Use a **hybrid plugin model**:
   - `plugin.toml` defines allowed policy scope boundaries.
   - Lua registration defines concrete policy keys per target.
6. Policy key naming format is `surface.resource.action` (for example: `admin.users.read`, `api.users.write`, `cli.plugin.list.read`).

## 3. Approaches Considered

### A. DB source of truth + declaration sync + runtime cache (recommended)

- Declarations from core/plugin code are synchronized to DB.
- Runtime authorization uses an in-memory compiled matcher built from DB data.
- Keeps operational auditability and runtime performance.

### B. Pure DB manual binding

- Everything configured manually in DB/admin UI.
- Strong configurability but high human maintenance overhead and plugin drift risk.

### C. Pure code registry

- Fast to build but fails the goal of centralized operations and dynamic governance.

Decision: **Approach A**.

## 4. Target Architecture

Introduce a unified policy authorization domain in `sushi-core`:

- `PolicyKey`: normalized key model (`surface`, `resource`, `action`, plus canonical `key`).
- `PolicyBinding`: target-to-policy requirement mapping (HTTP route, CLI command, admin page/workspace target).
- `Authorizer`: runtime checker for request/command authorization.
- `PolicyRepository`: DB reader/writer for policies, role grants, bindings, and plugin scopes.
- `CompiledPolicySnapshot`: startup/runtime cached matcher for fast checks.

### 4.1 Trust and ownership model

- DB remains the authoritative policy state.
- Core code and plugins declare desired policy metadata and bindings.
- Startup sync writes declarations into DB idempotently.
- Runtime uses compiled snapshot from DB.

### 4.2 Surfaces and targets

- `admin` surface:
  - Full pages, partial endpoints, workspace content routes, plugin workspace pages, workspace assets endpoint.
- `api` surface:
  - Built-in Rust routes and plugin API routes.
- `cli` surface:
  - Built-in CLI subcommands and plugin CLI commands.

## 5. Data Model

Add new tables in a migration (proposed `006_unified_policy_v2.sql`):

1. `policy_keys`
   - `id`, `key` (unique), `surface`, `resource`, `action`, `name`, `description`, `is_system`, timestamps.
2. `role_policy_keys`
   - `role_id`, `policy_key_id`, `created_at`, composite PK (`role_id`, `policy_key_id`).
3. `policy_bindings`
   - Binds a target to one required policy key.
   - Fields include:
     - identity: `id`, `surface`, `target_type`, `target_ref`
     - matcher dimensions: `method`, `path_pattern`, `command_name`
     - policy relation: `policy_key_id`
     - ownership metadata: `owner_type` (`system` / `plugin`), `owner_id`, `is_system`
     - timestamps
4. `plugin_policy_scopes`
   - `plugin_name`, `scope_pattern`, `created_at`.
   - Used to validate plugin-declared runtime policy keys.

### 5.1 Existing RBAC tables

- Existing `permissions` and `role_permissions` are no longer runtime authorization sources.
- Keep old tables for one release for audit/rollback safety.
- Runtime reads only new unified policy tables.

## 6. Execution Flow

### 6.1 Startup flow

1. Run migration `006_unified_policy_v2.sql`.
2. Seed built-in policy keys and built-in bindings idempotently.
3. Load plugins; validate runtime policy keys against manifest scope boundaries.
4. Persist plugin bindings and plugin scopes.
5. Compile policy snapshot and publish `Authorizer`.
6. Only then start serving traffic.

Failure mode: fail closed (service does not start if critical policy initialization fails).

### 6.2 Request flow (admin/api)

1. JWT verification and token-type check.
2. Resolve role from claims.
3. Build authorization target (`surface`, `method`, `path`).
4. `Authorizer::check_http(...)` evaluates against compiled bindings + role grants.
5. Return `403` on deny, pass-through on allow.

### 6.3 CLI flow

1. Resolve effective CLI principal role (initial design: `--role` or `SUSHI_CLI_ROLE`, default `admin`).
2. Build command target.
3. `Authorizer::check_command(...)`.
4. Denied commands fail with clear policy error output.

### 6.4 Plugin registration flow

- `plugin.toml` declares allowed policy scope patterns.
- Lua registration includes concrete policy key for each route/page/command.
- Loader validates concrete policy key against allowed scope patterns.
- Invalid policy declarations fail plugin load with precise error context.

## 7. File-Level Refactor Plan

## 7.1 `crates/sushi-admin`

- `crates/sushi-admin/src/router.rs`
  - Remove hardcoded permission resolver (`required_admin_permission` and route maps).
  - Keep auth middleware responsibilities to token validation + unified authorizer call.
- `crates/sushi-admin/src/routes/workspace.rs`
  - Remove `permission_for_module`.
  - Add path-aware authorization guard for `/admin/api/workspace/assets?path=...`.

## 7.2 `crates/sushi-core`

- Add new auth modules:
  - `src/auth/policy.rs`
  - `src/auth/authorizer.rs`
  - `src/auth/repository_policy.rs` (or extend current `rbac.rs`)
- `src/context.rs`
  - Inject and expose `Authorizer`.
- `src/auth/middleware.rs`
  - Replace special-case role gate with unified `api` surface policy checks.
- Plugin system:
  - `src/plugin/mod.rs`: manifest scope model extension.
  - `src/lua/bindings.rs`: registration APIs accept optional `policy` option.
  - `src/lua/loader.rs`: read/store policy metadata, validate scope conformance.
  - `src/plugin/manager.rs`: retain policy key metadata per registration binding.

## 7.3 `crates/sushi-api`

- `crates/sushi-api/src/router.rs`
  - Keep route composition; rely on unified middleware authorizer for policy decisions.

## 7.4 `crates/sushi-cli`

- `crates/sushi/src/main.rs`
  - Add CLI role context input for authorization principal.
- `crates/sushi-cli/src/commands/run.rs`
  - Enforce command-level policy checks before plugin command invocation.
- `crates/sushi-cli/src/commands/plugin.rs`
  - Apply policy checks for plugin management commands as CLI targets.

## 7.5 Plugins

- `plugins/*/plugin.toml`
  - Add allowed policy scopes section.
- Lua registrations
  - Add per-target policy key options (`sushi.api.route`, `sushi.web.page`, `sushi.cli.command`).

## 8. Safety and Failure Semantics

- System is fail-closed for policy initialization and plugin policy scope violations.
- No runtime fallback to old hardcoded permission map.
- Startup errors must include plugin name, target, declared key, and rejected scope.
- Authorization denials should be explicit and consistent (`403` for web/API, command error for CLI).

## 9. Testing Strategy

### 9.1 Core authorization tests

- Key parsing and normalization.
- Binding matcher behavior (static path and wildcard patterns).
- Role grant resolution.
- Plugin scope validation rules.

### 9.2 Admin tests

- Replace hardcoded permission expectation assertions with unified policy key assertions.
- Verify `workspace` and `workspace/assets` authorization behavior.
- Verify plugin workspace and plugin pages API authorization behavior.

### 9.3 API tests

- Built-in API route allow/deny by role with unified keys.
- Plugin API route allow/deny by role.
- Auth endpoints (`login`, `refresh`) remain public as configured.

### 9.4 CLI tests

- Command authorization for built-in and plugin commands.
- Role-based command deny cases with clear errors.

### 9.5 Regression gate

Minimum verification:

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

## 10. Rollout and Rollback

### 10.1 One-shot rollout

- Deploy code + DB migration together.
- Do not enable traffic before policy snapshot compile succeeds.

### 10.2 Rollback

- Roll back application binary and restore database snapshot taken before migration.
- Avoid partial rollback (new code with old DB or old code with new-only policy state).

## 11. Non-Goals

- No compatibility mode mapping old permission slugs to new keys.
- No dual auth engine operation.
- No admin UI redesign in this change set.

## 12. Acceptance Criteria

1. Admin/API/CLI all authorize via the same authorizer and DB policy source.
2. `crates/sushi-admin/src/router.rs` and `crates/sushi-admin/src/routes/workspace.rs` no longer contain business permission hardcoded maps.
3. Plugin routes/pages/commands can declare policy keys and are validated against manifest scope boundaries.
4. Built-in roles (`admin`, `editor`, `viewer`) function under new key model with expected allow/deny behavior.
5. Workspace, plugin pages, and plugin APIs enforce policy correctly with no route-level bypass.
