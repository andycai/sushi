---
name: sushi-lua-plugin-delivery
description: Use when creating or refactoring Sushi Lua plugins that register routes/admin pages/CLI commands and require permission-safe database and templating patterns.
---

# Sushi Lua Plugin Delivery

## Overview

Use this skill to ship Lua plugins that match Sushi's parity model and production quality bar.

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

- Prefer `sushi.web.page` + template files under `web/templates/plugins/<name>/`.
- Put JS/CSS assets under `web/static/plugins/<name>/`.
- Avoid embedding full HTML UIs directly in Lua strings.
- Return feedback fragments compatible with shared UI (`data-ui-flash`, `data-level`, `data-message`).
- Use HTMX for server-first requests and partial refresh; use Alpine for local state only.
- Use `HX-Trigger` headers for deterministic refresh and close actions after successful mutations.
- Reuse shared `AdminUI` helpers instead of writing custom notification/table logic.

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
- `cargo test --workspace -q` passes.
