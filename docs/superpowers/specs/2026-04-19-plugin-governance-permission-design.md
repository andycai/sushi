# Plugin Governance and Runtime Permission Control Design

Date: 2026-04-19  
Status: Proposed (validated in brainstorming)  
Scope: `crates/sushi-core`, `crates/sushi-admin`, `crates/sushi-api`, `crates/sushi-cli`, `migrations/*`, `web/templates/admin/*`

## 1. Problem Statement

Current plugin permission control is developer-friendly but production-unfriendly:

- Third-party plugins self-declare permissions in `plugin.toml`.
- Runtime currently trusts manifest-declared capabilities as effective capabilities.
- Operators cannot centrally disable risky plugins without restart-level workarounds.
- Existing `plugin_state` table (`migrations/001_init.sql`) is not the source of runtime gating.

This creates a governance gap in production: operational control should belong to platform admins, not plugin authors.

## 2. User-Confirmed Decisions

The user confirmed the following constraints:

1. **Immediate effect**: plugin enable/disable must take effect without service restart.
2. **Manifest as upper bound**: admin controls can only reduce permissions/capabilities, never elevate beyond manifest declarations.
3. **V1 scope**: plugin-level toggle only (`enabled/disabled`), no capability-level or policy-level override yet.
4. **Backend-controlled governance**: admin is the authority for runtime activation state.

## 3. Approaches Considered

### A. Runtime soft gate (recommended for V1)

- Keep plugin loading flow mostly intact.
- Add governance check before dispatching plugin API/Admin/CLI handlers.
- Disabled plugins remain loaded in memory but are unreachable.

Pros:
- Low risk and fast delivery.
- No dynamic route rebuild complexity.
- Immediate effect achievable with minimal architecture churn.

Cons:
- Not a true unload; Lua VM may remain resident.

### B. Hot unload/reload plugin runtime

- Dynamically remove and re-add plugin VMs, handlers, and route mappings.

Pros:
- Clean runtime semantics.

Cons:
- High complexity and concurrency risk.
- Requires robust route and state lifecycle orchestration.

### C. Per-plugin process isolation

- Plugins run as separate processes with RPC boundaries.

Pros:
- Strongest isolation and resource governance.

Cons:
- Major architecture shift, not suitable for this refactor scope.

Decision: **Approach A for V1**, with extension points for B-level capabilities in later phases.

## 4. Multi-Layer Permission Model (Target State)

This design introduces layered governance while implementing only Layer 1 in V1.

### L0. Identity Layer

- Canonical plugin ID: `tier/name` (for example: `official/kv-store`, `third_party/demo`).
- Avoid collisions for same plugin `name` across tiers.

### L1. Activation Layer (V1)

- Admin-managed runtime state: `enabled=true/false`.
- If `enabled=false`, plugin is denied on all surfaces (API/Admin/CLI).
- This layer has highest precedence for runtime reachability.

### L2. Capability Envelope Layer (V2 reserved)

- Capability dimensions: `routes`, `commands`, `admin`, `database`.
- Future effective rule: `effective = manifest_capability ∩ admin_capability`.

### L3. Policy Binding Layer (existing)

- Keep unified policy model and plugin policy keys.
- Even when policy allows access, `enabled=false` still denies execution.

### L4. Execution/Data Safety Layer (future)

- Resource and I/O governance (quota, stricter DB/file constraints) as a later enhancement.

## 5. Data Model Design

## 5.1 Existing table reuse

Reuse `plugin_state` as the governance source of truth and evolve schema:

- Existing fields: `name`, `enabled`, `loaded`, `version`, `loaded_at`
- Proposed additions:
  - `plugin_id TEXT UNIQUE` (canonical `tier/name`)
  - `source_kind TEXT` (`official` or `third_party`)
  - `updated_by TEXT`
  - `updated_at TEXT`
  - `reason TEXT`

Compatibility strategy:
- Keep `name` for compatibility/readability.
- Backfill `plugin_id` during migration/bootstrap for existing rows.

## 5.2 Audit trail table

Add `plugin_state_events`:

- `id`, `plugin_id`, `name`, `action` (`enable`/`disable`), `old_enabled`, `new_enabled`
- `actor`, `reason`, `created_at`

Purpose:
- Traceability for production operations.
- Supports incident investigation and change accountability.

## 6. Runtime Architecture

## 6.1 New governance components

- `PluginStateRepository` (new): reads/writes plugin state and emits normalized state DTOs.
- `PluginGate` (new): shared runtime gate for execution checks.

## 6.2 Gate insertion points

Add gate checks in `PluginManager` runtime dispatch path:

- `dispatch_api_handler(...)`
- `call_admin_handler(...)`
- `call_cli_handler(...)`

Behavior:
- If plugin disabled: return `plugin_disabled` result before Lua handler invocation.
- If plugin enabled but not loaded: return `plugin_not_loaded`/`init_failed` semantic result.

## 6.3 Immediate-effect strategy

- Admin update writes `plugin_state.enabled`.
- Runtime gate reads from authoritative state on dispatch path.
- Default V1 strategy: read-through or very short TTL cache with fail-closed behavior.

Fail-closed rule:
- If state cannot be resolved due to transient repository failure, deny execution.

## 6.4 Why no hot route rebuild in V1

- Axum route graph stays static after startup.
- Governance is enforced at handler dispatch boundary.
- Avoids dynamic router mutation and consistency pitfalls.

## 7. API/Admin/CLI Contract Changes (V1)

## 7.1 Admin APIs

Add runtime state mutation endpoint:

- `PATCH /admin/api/plugins/{plugin_id}/state`
  - Body: `{ "enabled": true|false, "reason": "..." }`
  - Authz: `admin.plugins.manage`

Enhance existing list endpoint:

- `GET /admin/api/plugins`
  - Include: `plugin_id`, `source_kind`, `enabled`, `loaded`, `effective_status`, `updated_at`, `updated_by`

## 7.2 Admin UI

Enhance plugin registry view (`/admin/plugins`):

- Enable/disable action controls.
- Status badges:
  - `Active` (`enabled && loaded`)
  - `Disabled` (`!enabled`)
  - `Init Failed` (`enabled && !loaded`)
- Optional reason capture on state transitions.

## 7.3 CLI

Extend plugin command surface:

- `sushi plugin status [plugin_id]`
- `sushi plugin enable <plugin_id> --reason "..."`
- `sushi plugin disable <plugin_id> --reason "..."`

Authorization keys:
- read/list/status: `cli.plugins.read`
- mutate enable/disable: `cli.plugins.manage` (new key)

## 8. Error Semantics and Observability

Surface-specific deny behavior when disabled:

- API: HTTP `403` + JSON error code `plugin_disabled`
- Admin plugin page calls: denied response with clear user-facing message
- CLI command invocation: non-zero exit with `plugin_disabled` text

Structured logging fields for all governance decisions:

- `plugin_id`, `plugin_name`, `source_kind`, `surface`, `action`, `actor`, `reason`, `result`

## 9. Migration and Rollout Plan

### Phase 1: Data + read path

- Add migration for `plugin_state` extension and `plugin_state_events`.
- Bootstrap upsert discovered plugins into governance table.
- Keep runtime behavior unchanged.

### Phase 2: Runtime gate behind feature switch

- Add dispatch gate checks.
- Optional config flag: `plugins.runtime_gate`.

### Phase 3: Admin/CLI mutators

- Add API endpoint and CLI commands to mutate enable state.
- Add admin UI controls.

### Phase 4: Default-on and cleanup

- Turn `plugins.runtime_gate` default on.
- Remove legacy paths that bypass governance checks.

## 10. Testing Strategy

### 10.1 Unit tests

- Gate denies disabled plugin across API/Admin/CLI dispatch entry points.
- Gate behavior for enabled but unloaded plugin.
- Plugin ID parsing/normalization (`tier/name`).

### 10.2 Integration tests

- Toggle plugin to disabled via admin API and verify immediate deny without restart.
- Re-enable and verify recovery without restart.
- Verify role restrictions on state mutation endpoint (`admin.plugins.manage`).

### 10.3 Regression tests

- Existing plugin discovery and manifest parsing remain stable.
- Existing policy binding behavior remains intact.
- Existing admin plugin workspace and static mounting remain functional.

### 10.4 Acceptance criteria

1. State toggle reaches runtime behavior change without process restart.
2. Disabled plugin cannot execute API/Admin/CLI handlers.
3. Every mutation is auditable (`who`, `when`, `why`, `what`).
4. No path can elevate permissions beyond manifest envelope.
5. Workspace tests pass: `cargo test --workspace -q`.

## 11. Risks and Mitigations

- **Risk:** stale state in long-lived cache.
  - **Mitigation:** V1 default read-through or very short TTL, fail-closed fallback.
- **Risk:** incomplete gate coverage on dispatch paths.
  - **Mitigation:** enforce gate in central `PluginManager` entry points only, not duplicated in routers.
- **Risk:** identity mismatch (`name` vs `tier/name`).
  - **Mitigation:** canonicalize on `plugin_id`; keep `name` as display metadata.

## 12. Out of Scope for V1

- Hot unload/reload of plugin runtime objects.
- Capability-level admin override UI.
- Policy-key-level per-plugin override UI.
- Plugin signing and publisher trust chain.
- Per-plugin resource quotas.

## 13. Implementation Handoff Notes

When moving to planning and implementation:

- Preserve existing official/third-party tier model.
- Keep manifest-declared permission semantics as upper bound contract.
- Treat governance state as operational control plane, not plugin self-declaration.
- Add migration and tests first, then gate, then admin/cli mutators.
