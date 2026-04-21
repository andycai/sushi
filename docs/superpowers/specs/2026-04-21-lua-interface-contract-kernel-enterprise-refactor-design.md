# Lua Interface Contract Kernel Enterprise Refactor Design

Date: 2026-04-21
Status: Proposed (validated in brainstorming)
Scope: `crates/sushi-core`, `crates/sushi-admin`, `crates/sushi-api`, `crates/sushi-cli`, `plugins/official/*`, `docs/wiki/lua-api/*`, `docs/engineering/plugin-authoring-standards.md`

## 1. Goal and Context

Sushi needs an enterprise-grade Lua interface architecture that can support large-scale plugin development without frequent Rust-layer API churn.

Current behavior already includes runtime governance and permission-aware namespace injection, but the Lua export surface is concentrated in a large binding implementation and evolves function-by-function. This creates scaling risks:

- Capability growth increases coupling in core Rust binding code.
- Permission checks and registration metadata handling are spread across multiple flows.
- Plugin developers depend on concrete exported functions rather than a stable capability contract.

This refactor establishes a contract-driven kernel so future plugin capability expansion is primarily schema/config evolution instead of repeated Rust export rewrites.

## 2. Confirmed Decisions (Locked)

This design is locked to the user-confirmed decisions:

1. **Compatibility strategy**: `B` (one-time upgrade). Existing plugin interfaces may change; no long-term dual-track compatibility layer.
2. **Delivery boundary**: `L1`.
   - Core export refactor complete.
   - Permission/governance model complete.
   - Official plugins migrated.
   - Full tests and docs complete.
   - Third-party migration guidance included.
3. **Permission default**: `P1` deny-by-default.
   - Unauthorized capabilities are not injected and are not visible to Lua.
4. **Interface style**: `I1` capability-oriented contract model.
   - Stable registration contracts replace continual addition of ad hoc exported functions.
5. **Coverage**: Full surface in this cycle.
   - `api`, `admin`, `cli`, `web`, `db`, `event`, `fs`.

## 3. Approaches Considered

### A. Contract Kernel + phased internal cutover (recommended)

- Build a unified contract kernel first.
- Migrate all surfaces into the contract/registry model.
- Cut runtime dispatch to contract-derived bindings.

Pros:
- Clean architecture and clear boundaries.
- Best long-term stability for plugin ecosystem.
- Future capability expansion mostly schema-first.

Cons:
- Broad refactor footprint.
- Requires strict migration validation and regression gates.

### B. Surface-by-surface refactor first, unify later

Pros:
- Smaller immediate slices.

Cons:
- Temporary double standards.
- Additional later unification cost.
- Higher risk of drift.

### C. DSL/codegen-first

Pros:
- Excellent long-term ergonomics.

Cons:
- Large upfront investment.
- Exceeds L1 delivery focus.

Decision: **Approach A**.

## 4. Target Architecture

### 4.1 Architectural model

Adopt **Contract Kernel + Capability Adapters**.

- Lua plugins register capability intent through stable contracts.
- Rust validates, authorizes, normalizes, and registers capabilities centrally.
- Runtime dispatch consumes normalized capability bindings from a single registry source.

### 4.2 Layered runtime design

1. **Contract Schema Layer**
   - Strongly typed registration contracts for `api/admin/cli/web/db/event/fs`.
2. **Permission & Policy Engine**
   - Single decision entrypoint for authorization and scope checks.
3. **Capability Registry Layer**
   - Stores `RegisteredCapability` records and metadata.
4. **Runtime Dispatch Adapter Layer**
   - Bridges registry bindings into API/Admin/CLI execution paths.

### 4.3 Injection strategy (P1)

Injection is capability-aware and deny-by-default:

- Lua VM receives only baseline safe core plus allowed capability handles.
- Unauthorized capabilities are not present in global exports.
- This avoids runtime probing side channels and reduces misuse paths.

### 4.4 Evolution rule

Future capabilities should be added by extending contract schema and adapter interpretation first. Rust exported function signatures should remain stable by default.

## 5. Component Refactor Plan (Module Boundaries)

`crates/sushi-core/src/lua/bindings.rs` is split into focused modules:

- `crates/sushi-core/src/lua/contract/mod.rs`
  - Contract root types, version markers, common validation/result types.
- `crates/sushi-core/src/lua/contract/schema/api.rs`
- `crates/sushi-core/src/lua/contract/schema/admin.rs`
- `crates/sushi-core/src/lua/contract/schema/cli.rs`
- `crates/sushi-core/src/lua/contract/schema/web.rs`
- `crates/sushi-core/src/lua/contract/schema/db.rs`
- `crates/sushi-core/src/lua/contract/schema/event.rs`
- `crates/sushi-core/src/lua/contract/schema/fs.rs`
- `crates/sushi-core/src/lua/permission/engine.rs`
  - Central capability decision engine.
- `crates/sushi-core/src/lua/registry/mod.rs`
  - Capability registry and normalized binding output.
- `crates/sushi-core/src/lua/injector/mod.rs`
  - Lua global injection and registration entrypoint only.
- `crates/sushi-core/src/lua/adapters/api.rs`
- `crates/sushi-core/src/lua/adapters/admin.rs`
- `crates/sushi-core/src/lua/adapters/cli.rs`
- `crates/sushi-core/src/lua/adapters/web.rs`
- `crates/sushi-core/src/lua/adapters/db.rs`
- `crates/sushi-core/src/lua/adapters/event.rs`
- `crates/sushi-core/src/lua/adapters/fs.rs`
- `crates/sushi-core/src/lua/errors.rs`
  - Stable error codes and conversion helpers.

`bindings.rs` becomes a thin assembler of these modules.

## 6. Permission and Governance Model

### 6.1 Unified decision formula

Effective authorization is determined by:

`effective = manifest_ceiling ∩ runtime_governance ∩ surface_policy`

### 6.2 Three-stage enforcement

1. **Visibility Gate (inject-time)**
   - Controls whether capability is exposed in Lua.
2. **Registration Gate (register-time)**
   - Validates schema, permission ceilings, governance state, policy scopes.
3. **Dispatch Gate (execution-time)**
   - Lightweight re-check before handler execution to enforce immediate runtime state changes.

### 6.3 Governance behavior

- Plugin disabled state (`plugin_state.enabled = false`) must deny API/Admin/CLI dispatch immediately.
- Dispatch deny semantics remain explicit and stable (`plugin_disabled`, etc.).

### 6.4 Error contract and audit dimensions

All denials and registration failures must produce deterministic codes and structured context:

- `plugin_id`
- `capability`
- `surface`
- `reason_code`
- `actor` (where available)
- `timestamp`

Target stable reason code family:

- `capability_not_visible`
- `registration_denied`
- `policy_scope_violation`
- `plugin_disabled`
- `plugin_not_loaded`

## 7. Data Flow and Lifecycle

1. **Scan/Load**
   - Discover plugin, parse manifest, compute effective baseline, create sandboxed VM.
2. **Inject**
   - Permission engine computes visible capability set and injects only allowed handles.
3. **Register**
   - Plugin submits capability contracts in `sushi.init()`.
   - Registry validates and normalizes entries.
4. **Persist/Bind**
   - Persist policy-related metadata where required.
   - Build runtime binding snapshots from registry.
5. **Dispatch**
   - Runtime routes/commands/pages resolve via registry-derived bindings.
   - Dispatch gate enforces latest governance state.
6. **Observe**
   - Structured logs and metrics across inject/register/dispatch paths.

## 8. Official Plugin Migration Scope (L1)

Official plugins must be migrated in this cycle:

- `plugins/official/kv-store`
- `plugins/official/file-browser`
- `plugins/official/cms`

Migration target:

- New contract registration style across all used surfaces.
- No dependency on legacy internal pending table assumptions.
- Existing production behavior preserved except intentional interface break defined by this design.

Third-party output:

- Publish migration guide documenting contract migration steps and breaking points.

## 9. Testing and Verification Strategy

### 9.1 Contract schema tests

- Positive/negative cases per surface schema.
- Conflict validation (for example policy/public incompatibility where applicable).
- Stable error code assertions.

### 9.2 Permission matrix tests

Dimension coverage:

- plugin kind
- permissions by surface
- runtime enabled/disabled
- policy scope allowed/denied
- dispatch surface

Critical assertion:

- Under P1, unauthorized capabilities are absent from Lua exports.

### 9.3 Registry and binding tests

- Registry entries must contain normalized metadata.
- Invalid contracts must never reach runtime dispatch binding tables.

### 9.4 Integration tests

- API/Admin/CLI dispatch deny/allow semantics are consistent.
- Disabled plugin behavior remains immediate and explicit.
- Policy bindings remain consistent with registry state.

### 9.5 Golden migration tests

For each official plugin:

- Baseline behavior snapshot before migration.
- Post-migration behavior comparison.
- No unintended functional regressions.

### 9.6 Required verification commands

Minimum release gate:

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

Add contract-focused suite naming convention:

- `cargo test -p sushi-core lua_contract_ -q` (suite prefix target)

## 10. Execution Slices and Rollback

### Slice 1: Contract kernel foundation

- Introduce schema, permission engine, registry, injector modules.
- No dispatch cutover yet.

### Slice 2: Capability registration migration

- Migrate all surfaces to contract registration path.
- Keep temporary parity checks against legacy registration metadata.

### Slice 3: Dispatch cutover

- API/Admin/CLI consume registry-derived bindings as source of truth.
- Enable unified reason codes and audit fields.

### Slice 4: Official plugin migration + golden validation

- Migrate official plugins and pass behavior parity gates.

### Slice 5: Legacy cleanup + documentation finalization

- Remove legacy registration surfaces and stale internals.
- Finalize docs and migration guide.

Rollback principles:

- Each slice is independently revertible.
- Dispatch cutover includes a short-lived internal guard for operational rollback.
- Rollback prioritizes restoring runtime behavior while preserving data consistency.

## 11. Risks and Mitigations

- **Risk: Broad one-shot refactor introduces regression clusters.**
  - Mitigation: Slice-based rollout with explicit gates and golden comparisons.
- **Risk: Permission logic divergence across surfaces.**
  - Mitigation: Single permission engine used by all adapters.
- **Risk: Migration uncertainty for third-party plugins.**
  - Mitigation: Publish explicit migration guide and deterministic error codes.
- **Risk: Runtime governance drift from contract state.**
  - Mitigation: Dispatch-time re-check against authoritative runtime governance.

## 12. Out of Scope for L1

- Long-term dual runtime support for legacy Lua export contracts.
- Automated codemod tooling for third-party plugins.
- CI admission gate blocking non-compliant third-party plugin packages.
- Process-level plugin isolation architecture.

## 13. Definition of Done

L1 is complete only when all conditions are met:

1. Contract kernel is implemented and is the single registration authority.
2. Full surface coverage achieved: `api/admin/cli/web/db/event/fs`.
3. P1 deny-by-default is enforced at injection visibility level.
4. Runtime governance + policy checks are unified and auditable.
5. Official plugins are migrated and pass golden/regression checks.
6. Required test gates pass.
7. Lua API and plugin authoring docs are updated with migration guidance.

