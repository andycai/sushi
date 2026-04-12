---
name: sushi-lua-plugin-delivery
description: Use when creating or refactoring Sushi Lua plugins that register routes/admin pages/CLI commands and require permission-safe database and templating patterns.
---

# Sushi Lua Plugin Delivery

## Overview

Use this skill to ship Lua plugins that match Sushi's parity model and production quality bar.

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

## Compatibility Rules

- Keep plugin route and command contracts stable.
- If breaking behavior is unavoidable, bump plugin version and include migration notes.

## Done Criteria

- Plugin loads cleanly and registers expected capabilities.
- Admin/API/CLI paths are manually smoke-tested.
- `cargo test --workspace -q` passes.

