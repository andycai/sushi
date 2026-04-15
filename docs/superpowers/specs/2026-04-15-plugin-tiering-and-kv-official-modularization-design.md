# Plugin Tiering and KV Official Modularization Design (2026-04-15)

## 1. Goal and Scope

This design introduces a two-tier Lua plugin model:

- `official` plugins (full platform privileges)
- `third_party` plugins (permission-limited, manifest-driven)

It also migrates and rewrites `kv-store` as an official plugin with real multi-file Lua modules (not monolithic `init.lua`).

### In scope

- New plugin directory contract with category folders
- Runtime category + manifest kind validation
- Official permission override policy
- Secure Lua module loading for plugin-local multi-file structure
- KV store migration to `official/` and modular Lua file layout
- Template/static path contract updates for category-aware plugin assets
- Tests and migration behavior

### Out of scope

- Backward compatibility for legacy `plugins/<name>` layout
- Changing existing API/admin/CLI user-facing behavior of KV store
- New plugin marketplace/distribution workflow

## 2. Hard Decisions (Locked)

1. **Directory + manifest dual validation** is required.
2. **Legacy single-level plugin directories are abandoned immediately**.
3. **Legacy directory presence is a startup-fatal error**.
4. **Official plugins use runtime-enforced full permissions**.
5. **External paths include category segment** (e.g. `official/kv-store`).

## 3. Directory and Manifest Contract

## 3.1 Required directory layout

```text
plugins/
  official/
    <plugin-name>/
      plugin.toml
      init.lua
      lua/
      web/
  third_party/
    <plugin-name>/
      plugin.toml
      init.lua
      lua/
      web/
```

Legacy layout `plugins/<plugin-name>/` is invalid and causes startup failure.

## 3.2 Manifest contract

`plugin.toml` must include:

```toml
[plugin]
name = "kv-store"
version = "0.1.0"
entry = "init.lua"
kind = "official" # or "third_party"
```

Validation rule:

- directory category and `plugin.kind` must match exactly
- mismatch is fatal for startup

## 3.3 Plugin identity model

To avoid breaking internal plugin naming semantics while enabling category-aware assets:

- `plugin.name`: logical plugin name, unchanged (`kv-store`)
- `plugin_path_id`: derived runtime path identity (`official/kv-store` or `third_party/<name>`)

`plugin_path_id` is used for template/static route resolution.

## 4. Permissions Model

## 4.1 Official plugin effective permissions (forced)

Official plugins always run with effective permissions:

- `routes = true`
- `commands = true`
- `admin = true`
- `database = "admin"`

Runtime overrides manifest permissions for official plugins.

## 4.2 Third-party permissions

Third-party plugins keep current behavior:

- permissions come from `[permissions]` in `plugin.toml`
- runtime injects only approved Lua namespaces/APIs

## 4.3 Observability

Startup logs must include:

- plugin category
- manifest kind
- effective permissions
- explicit warning when official manifest permissions are overridden

## 5. Plugin Discovery and Startup Flow

1. Scan `plugins/official/*` and `plugins/third_party/*`.
2. Reject if any direct legacy plugin directory exists under `plugins/*` (excluding `official`, `third_party`).
3. Parse each `plugin.toml` and validate `plugin.kind` against category.
4. Compute effective permissions.
5. Initialize Lua VM, inject Sushi API with effective permissions.
6. Load plugin entry and module dependencies.
7. Register routes, commands, admin pages, template roots, and static roots.

Failure behavior:

- Any contract violation is startup-fatal.
- No compatibility fallback path is provided.

## 6. Secure Lua Modularization

Current sandbox disables `package`, `require`, `dofile`, and `loadfile`. To support multi-file plugins safely:

- Provide a runtime-managed module loader (`require` or `sushi.require`) scoped to current plugin root.
- Allow only plugin-local module paths.
- Disallow absolute paths, `..`, URL imports, and cross-plugin traversal.
- Cache loaded modules to avoid duplicate evaluation.
- Return deterministic load errors with plugin/module context.

This mechanism is shared by official and third-party plugins.

## 7. KV Store Migration Plan

## 7.1 Filesystem migration

From:

- `plugins/kv-store/*`

To:

- `plugins/official/kv-store/*`

## 7.2 Lua code modularization layout

```text
plugins/official/kv-store/
  init.lua
  lua/
    utils/
      json.lua
      form.lua
      html.lua
    infra/
      db.lua
    domain/
      store.lua
    interfaces/
      api.lua
      admin.lua
      cli.lua
    bootstrap/
      register.lua
```

`init.lua` becomes a thin bootstrap entry:

- import modules
- wire dependencies
- call registration
- log initialization result

## 7.3 Behavioral compatibility

Keep existing external behavior:

- API: `/api/kv`, `/api/kv/*`
- Admin page: `/admin/kv`
- CLI: `kv-list`, `kv-get`, `kv-set`, `kv-del`

Only internal structure and asset/template resolution paths change.

## 8. Template and Static Asset Path Contract

Category-aware resource keys become canonical:

- templates: `plugins/official/kv-store/...` / `plugins/third_party/<name>/...`
- static assets: `/static/plugins/official/kv-store/...` / `/static/plugins/third_party/<name>/...`

All resolver and loader code paths must consume `plugin_path_id` instead of bare plugin name when forming template/static references.

## 9. Error Handling Contract

Startup-fatal errors must be explicit and actionable:

- legacy plugin layout detected
- missing/invalid `plugin.kind`
- category-kind mismatch
- forbidden module path/import attempts
- module load failures (syntax/runtime import errors)

Errors should include:

- plugin directory
- plugin logical name
- category/kind values
- failing module path (if applicable)

## 10. Testing Strategy

## 10.1 Core/plugin loader tests

- scan supports only category-based structure
- legacy direct plugin folders trigger fatal error
- kind/category mismatch fails
- official permission override is enforced
- third-party permissions remain manifest-driven
- module loader blocks path traversal/absolute/URL imports
- module loader supports normal plugin-local module import and caching

## 10.2 Template/static resolution tests

- category-aware template key resolution works
- category-aware static asset URLs are generated correctly
- admin page assets resolve via category path IDs

## 10.3 KV regression tests

- existing KV API routes still return expected semantics
- admin partials still render and mutate correctly
- CLI commands still operate correctly

## 11. Rollout Sequence

1. Introduce category-aware discovery + manifest kind validation.
2. Introduce effective-permission computation rules.
3. Introduce secure plugin-local Lua module loader.
4. Update template/static resolvers to category-aware path IDs.
5. Migrate `kv-store` to `plugins/official/kv-store` and split into modules.
6. Update tests and docs.
7. Enforce startup-fatal legacy layout rejection.

## 12. Risks and Mitigations

- **Risk:** Broad path contract update impacts tests and admin assets.
  - **Mitigation:** switch resolvers first, then migrate plugin, then update fixtures.

- **Risk:** Lua module loader introduces subtle runtime failures.
  - **Mitigation:** add targeted loader tests (cache, recursion, invalid path, syntax errors).

- **Risk:** Startup-fatal legacy rejection breaks local environments unexpectedly.
  - **Mitigation:** clear release notes and explicit error messages with migration instructions.

## 13. Acceptance Criteria

- Only `plugins/official/*` and `plugins/third_party/*` are loadable.
- Any legacy direct plugin folder under `plugins/` aborts startup.
- `plugin.kind` is mandatory and must match folder category.
- Official plugins always run with forced full permissions.
- Third-party permissions remain restricted by manifest.
- KV store runs from `plugins/official/kv-store` with multi-file Lua modules.
- Category-aware template/static paths are consistently used and tested.
