---
name: sushi-lua-plugin-delivery
description: Use when creating or refactoring Sushi Lua plugins that register routes/admin pages/CLI commands and require permission-safe database and templating patterns.
---

# Sushi Lua Plugin Delivery

## Overview

Use this skill to ship Lua plugins that match Sushi's parity model and production quality bar.

Reference alignment:

- Project conventions: `AGENTS.md`
- Plugin standard details: `docs/engineering/plugin-authoring-standards.md`

## Quality Bar (Non-Negotiable)

- Plugin admin pages must not be demo-grade; they should look and behave like first-party admin modules.
- For CRUD/list screens, follow the same interaction model as core admin pages (for example `permissions`): searchable table, drawer editor, confirm modal, toast/flash feedback.
- Never regress to monolithic inline HTML blobs inside Lua when a template + static asset structure is available.
- Apply the shared checklist in `../_shared/admin-ui-module-checklist.md` for plugin-admin pages.

## Scaffold Rules

- Plugin directory: `plugins/<name>/`.
- Required files: `plugin.toml`, `init.lua`.
- Manifest must declare only minimum needed permissions.

## Implementation Rules

- Register capabilities inside `sushi.init()`.
- Use `sushi.log.*` (not `print`) for runtime logging.
- Validate path/body/form/arg input before business logic.
- Use parameterized SQL via `sushi.db.query/execute`.

## Admin UI Rules

- Prefer `sushi.web.page` + template files under `plugins/<name>/web/templates/`.
- Put JS/CSS assets under `plugins/<name>/web/static/`.
- Use logical template names `plugins/<name>/...` in Lua (`sushi.web.page`, `sushi.web.render`), even though files are stored in plugin-local `web/templates`.
- Use static URLs under `/static/plugins/<name>/...` (runtime mounts plugin-local `web/static` automatically).
- Avoid embedding full HTML UIs directly in Lua strings.
- Return feedback fragments compatible with shared UI (`data-ui-flash`, `data-level`, `data-message`).
- Use HTMX for server-first requests and partial refresh; use Alpine for local state only.
- Use `HX-Trigger` headers for deterministic refresh and close actions after successful mutations.
- Reuse shared `AdminUI` helpers instead of writing custom notification/table logic.

## Resource Isolation & Auto-Validation

- Treat plugin web resources as isolated ownership: no new plugin templates/static files under repository-level `web/templates/plugins/...` or `web/static/plugins/...`.
- Keep plugin paths internally consistent:
  - Files: `plugins/<name>/web/templates/...`, `plugins/<name>/web/static/...`
  - Lua references: `plugins/<name>/...` template names, `/static/plugins/<name>/...` asset URLs
- For any plugin resource migration or new plugin admin UI, run and record:
  - `cargo test -p sushi-core --test template_service -q`
  - `cargo test -p sushi-admin --test admin_web -q`

## Shared Checklist

- Follow `../_shared/admin-ui-module-checklist.md` for all CRUD/list plugin-admin pages.
- Plugin-specific additions (manifest permissions, route registration, Lua binding usage) are layered on top of the shared checklist, not replacements.

## Plugin Page Pattern (Recommended Default)

1. Plugin registers page route via `sushi.web.page(...)`.
2. Main template renders shell + table container + feedback region.
3. Partial endpoint returns table rows/snippets only (HTMX swap target).
4. Mutation endpoints return flash fragment + `HX-Trigger` events.
5. Page JS module handles: open create/edit, open delete confirm, busy states, feedback consume, fallback refresh.

## Compatibility Rules

- Keep plugin route and command contracts stable.
- If breaking behavior is unavoidable, bump plugin version and include migration notes.
- Keep UI behavior stable too (keyboard flow, form names, event names, trigger names) to avoid breaking operator muscle memory.

## Done Criteria

- Plugin loads cleanly and registers expected capabilities.
- Admin/API/CLI paths are manually smoke-tested.
- Plugin admin page supports loading, empty, validation-error, and success states.
- Plugin resources are stored in `plugins/<name>/web/...` and pass the targeted resource-validation tests.
- `cargo test --workspace -q` passes.
