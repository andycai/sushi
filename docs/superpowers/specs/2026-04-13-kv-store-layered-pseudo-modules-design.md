# KV Store Plugin Layered Refactor (Pseudo-Modules) Design

- Date: 2026-04-13
- Scope: `plugins/kv-store/init.lua`
- Status: Approved (design), pending implementation

## 1. Background

Current `plugins/kv-store/init.lua` contains API handlers, admin handlers, CLI handlers, SQL access, form parsing, JSON parsing, and registration logic in one flat file. This makes behavior hard to scan and change safely.

At the same time, the current Lua sandbox disables `require`/`package`, and plugin VMs are isolated, so true multi-file Lua module composition is not available yet without Rust core changes.

## 2. Goals

1. Refactor `kv-store` to a layered, modular structure inside a single `init.lua`.
2. Make each layer logically independent with clear interfaces.
3. Keep external behavior stable (routes/commands/template paths), while allowing minor wording and consistency improvements.
4. Prepare the code shape so future true multi-file extraction is straightforward.

## 3. Constraints and Non-Goals

### Constraints

- Do not modify Rust core or sandbox behavior.
- Do not split plugin Lua into multiple files in this phase.
- Keep plugin manifest and permission model unchanged.

### Non-Goals

- Do not implement cross-plugin event bus improvements in this phase.
- Do not introduce new user-facing features.
- Do not change public endpoint or command names.

## 4. Proposed Architecture (Single File, Layered)

Inside `plugins/kv-store/init.lua`, define one top-level local table (for example `kv`) and organize by pseudo-modules:

1. `kv.utils`
   - Pure utility helpers (string/url/html/json/form parsing helpers).
   - No domain/business decisions.

2. `kv.infra.db`
   - Database adapter wrappers around `sushi.db.query` / `sushi.db.execute`.
   - Centralized `pcall` and storage error normalization.

3. `kv.domain.store`
   - Core KV business logic (`list/get/upsert/delete`, validation rules).
   - Depends on `kv.infra.db` only.
   - No direct HTTP/Admin/CLI formatting.

4. `kv.interfaces.api`
   - Route dispatch and HTTP semantics.
   - Converts inputs to domain calls and maps domain errors to API responses.

5. `kv.interfaces.admin`
   - Admin partial/page behavior (`table/upsert/delete`, flash messaging, template rendering).
   - Uses domain layer; keeps UI-specific output local.

6. `kv.interfaces.cli`
   - CLI argument checks and output text.
   - Uses domain layer for all data changes.

7. `kv.bootstrap`
   - Registration only (`sushi.init` route/page/command wiring).
   - No business logic.

## 5. Dependency Direction

Strict one-way dependency:

`interfaces/* -> domain.store -> infra.db -> utils`

And:

`bootstrap -> interfaces/*`

Rules:

- `domain.store` must not call `sushi.web.render` or CLI-specific formatting.
- `interfaces.*` must not run raw SQL.
- `sushi.init` should be declarative registration only.

## 6. Interface Contracts

## 6.1 Domain contract (internal)

- `store.list() -> rows | nil, err_kind, err_msg`
- `store.get(key) -> row_or_nil | nil, err_kind, err_msg`
- `store.upsert(key, value) -> true | nil, err_kind, err_msg`
- `store.delete(key) -> true | nil, err_kind, err_msg`

`err_kind` is normalized categories such as:

- `invalid_key`
- `invalid_value`
- `not_found`
- `storage_error`

## 6.2 API contract (external behavior preserved)

Keep existing endpoints and semantics:

- `GET /api/kv`
- `POST /api/kv`
- `GET /api/kv/{key}`
- `PUT /api/kv/{key}`
- `DELETE /api/kv/{key}`

Status mapping:

- `invalid_* -> 400`
- `not_found -> 404`
- `storage_error -> 500`

Payload shape remains compatible with current plugin behavior.

## 6.3 Admin contract (external behavior preserved)

Keep existing admin endpoints and template usage:

- `GET /admin/partials/kv/table`
- `POST /admin/partials/kv/upsert`
- `POST /admin/partials/kv/delete`
- page registration: `/admin/kv` -> `plugins/kv-store/kv.html`

Flash output continues to follow shared protocol (`data-ui-flash`, `data-level`, `data-message`).

## 6.4 CLI contract (external behavior preserved)

Keep command names:

- `kv-list`
- `kv-get`
- `kv-set`
- `kv-del`

Allow minor wording cleanup while preserving meaning and usage expectations.

## 7. Data Flow

### API flow

`dispatch -> parse/validate -> domain.store -> response map (status + body)`

### Admin flow

`form parse -> domain.store mutate -> flash response`

`table read -> domain.store.list -> template render`

### CLI flow

`arg parse/validate -> domain.store -> terminal output`

## 8. Error Handling Strategy

- `infra.db` emits technical storage errors only.
- `domain.store` normalizes technical and validation errors into stable `err_kind` values.
- `interfaces.*` translate `err_kind` to channel-specific output (HTTP code, flash text, CLI text).
- This removes duplicated ad-hoc error branches currently spread across handlers.

## 9. Compatibility and Risk

### Compatibility commitments

- Keep route paths unchanged.
- Keep CLI command names unchanged.
- Keep template/static paths unchanged.
- Keep permission requirements unchanged.

### Main risk

- Reordering and refactoring a single large Lua file can cause accidental behavior drift.

### Mitigations

- Keep function-level changes incremental by layer.
- Preserve existing branch conditions where behavior is relied upon.
- Run targeted regression checks after refactor.

## 10. Verification Plan

1. Static structure check
   - `init.lua` visibly partitioned into pseudo-modules.
   - SQL appears only in `infra/db` + `domain/store` sections.
   - `sushi.init` contains registration only.

2. Runtime behavior checks
   - API CRUD paths remain functional.
   - Admin table/upsert/delete partials remain functional.
   - CLI commands remain functional.

3. Existing automated guard
   - Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_no_longer_embeds_html -q`

4. Manual smoke flow
   - create key
   - update key
   - delete key
   - query missing key

## 11. Future Evolution (Out of Scope for This Iteration)

When Rust core later supports safe plugin-local module loading, this design can be split with minimal churn into:

- `utils.lua`
- `infra/db.lua`
- `domain/store.lua`
- `interfaces/api.lua`
- `interfaces/admin.lua`
- `interfaces/cli.lua`
- slim `init.lua` bootstrap

Event-bus-based plugin collaboration can be addressed in a separate design/implementation cycle.
