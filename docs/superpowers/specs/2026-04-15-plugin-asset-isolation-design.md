# Plugin Asset Isolation Design (No Global-File Coupling)

Date: 2026-04-15  
Status: Approved Design (pre-implementation)

## 1. Context and Problem

Current admin module loading relies on global template logic in `web/templates/base.html` to decide which JS files to inject.  
This creates a hard coupling between plugin pages and non-plugin files, which violates the desired plugin boundary:

- Plugin developers should complete plugin development only inside `plugins/<name>/`.
- Plugin developers should not need permission to edit files outside plugin directories.
- A single plugin may require multiple JS/CSS assets per page, not one hardcoded module file.

## 2. Goals

1. **Full plugin asset isolation**: plugin templates and static assets are stored and maintained only under plugin directories.
2. **Declarative multi-asset loading**: each plugin page can configure multiple JS/CSS files.
3. **No base.html plugin hardcoding**: remove plugin-specific script decisions from `base.html`.
4. **Deterministic runtime loading**: same behavior for initial page load and workspace tab switches.
5. **Strict validation and enforcement**: CI/test gate blocks invalid paths and legacy global plugin asset usage.

## 3. Non-Goals

- Backward compatibility with legacy plugin asset layout (`web/templates/plugins/**`, `web/static/plugins/**`).
- Runtime fallback to old loading behavior.
- Introducing external CDN asset loading for plugin pages.

## 4. Chosen Approach

Adopt a **hybrid declarative model**:

- `plugin.toml` defines reusable asset bundles.
- `sushi.web.page(..., opts)` references bundles and may append page-local assets.

This balances reuse and flexibility for large plugins with multiple pages.

## 5. Resource Ownership Model

### 5.1 Physical file locations (plugin-owned only)

- Templates: `plugins/<plugin-name>/web/templates/...`
- Static assets: `plugins/<plugin-name>/web/static/...`

### 5.2 Logical references (runtime-facing)

- Lua template names remain logical:
  - `plugins/<plugin-name>/...`
- Static asset URLs remain logical:
  - `/static/plugins/<plugin-name>/...`

### 5.3 Access boundary

Plugin development must not require edits outside `plugins/<plugin-name>/`.  
Platform maintainers may evolve runtime internals, but plugin authors only touch plugin-owned files.

## 6. Configuration and Registration Contract

### 6.1 `plugin.toml` bundle declaration

`plugin.toml` declares reusable admin asset bundles under plugin scope.

Example:

```toml
[admin.assets.bundles.workspace]
js = ["kv.js", "shared/table.js"]
css = ["kv.css"]

[admin.assets.bundles.editor]
js = ["shared/editor.js"]
css = ["shared/editor.css"]
```

### 6.2 `sushi.web.page` per-page declaration

Each page can reference bundles and append page-local files:

```lua
sushi.web.page("/admin/kv", "plugins/kv-store/kv.html", {
  title = "KV Store",
  assets = {
    bundles = {"workspace", "editor"},
    js = {"pages/kv-extra.js"},
    css = {"pages/kv-extra.css"}
  }
})
```

### 6.3 Resolution and ordering

Final asset list for a page is resolved as:

1. Expand `bundles` in listed order.
2. Append page-local `js` and `css`.
3. Deduplicate while preserving first-seen order.
4. Normalize each path to `/static/plugins/<plugin>/<relative-path>`.

## 7. Runtime Architecture

## 7.1 Backend metadata

Plugin page registration stores:

- `path`
- `title`
- `handler_key`
- `resolved_assets` (`js[]`, `css[]`)

## 7.2 Asset API for workspace/runtime loader

Provide endpoint:

- `GET /admin/api/workspace/assets?path=/admin/<route>`

Response:

```json
{
  "js": ["/static/plugins/kv-store/kv.js"],
  "css": ["/static/plugins/kv-store/kv.css"]
}
```

Permission follows page access policy; unauthorized paths return authorization errors.

## 7.3 Frontend loading flow

### Initial load

1. Generic loader detects current admin path.
2. Loader requests workspace asset list.
3. Loader injects CSS first, then JS (strict sequence).
4. Only after assets are ready, initialize page interactivity.

### Workspace tab switch

1. Preload target path assets via same API.
2. If successful, request HTMX fragment content.
3. After swap, dispatch module-ready event for Alpine/feature hooks.

### Deduplication

- Global loaded asset registry prevents duplicate `<script>` or `<link>` injection.
- Re-opening a tab does not re-execute already loaded scripts.

### Error handling

- If any asset fails to load:
  - block module-ready transition for that page,
  - show explicit error panel with failed URL,
  - log structured error in console.

## 8. Security and Validation Rules

Reject during plugin load/init:

- Missing bundle key references.
- Asset paths with `..`, absolute paths, or protocol URLs (`http://`, `https://`, `//`).
- Declared assets that do not exist in `plugins/<name>/web/static`.

Reject at repo validation/test level:

- New plugin assets under legacy global directories:
  - `web/templates/plugins/**`
  - `web/static/plugins/**`

## 9. Migration Strategy (No Backward Compatibility)

1. Move all plugin resources into plugin-owned directories.
2. Update plugin page registrations to `assets` declarations.
3. Remove plugin-specific script mapping from `base.html`.
4. Enable strict validation gates.
5. Fail plugin initialization when declaration/layout is invalid.

Legacy mode is intentionally unsupported after migration.

## 10. Testing and Acceptance

## 10.1 Automated tests

- Template resolution from plugin roots.
- Plugin static route serving for `/static/plugins/<name>/...`.
- Workspace assets API correctness (order + dedupe + auth behavior).
- Frontend integration test for page interactive readiness (prevent undefined handler errors).

## 10.2 Mandatory verification commands

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

## 10.3 Regression acceptance criteria

- No plugin-specific module map in `base.html`.
- Plugin pages load in both direct navigation and workspace tabs without Alpine undefined-function errors.
- Plugin developers can add/modify page assets entirely within `plugins/<name>/`.

## 11. Risks and Mitigations

- **Risk**: broken plugin pages due to incomplete asset declaration.  
  **Mitigation**: strict load-time validation with explicit diagnostics.

- **Risk**: race conditions between script loading and HTMX swap.  
  **Mitigation**: enforce preload-before-swap sequence and module-ready event boundary.

- **Risk**: hidden dependency on previous global script registration.  
  **Mitigation**: remove legacy path and add test asserting no plugin script map in base template.

## 12. Implementation Boundary

This document is design-only.  
Implementation planning and execution follow in the next phase via a dedicated implementation plan.
