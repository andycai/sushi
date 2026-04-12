---
name: sushi-admin-feature-delivery
description: Use when implementing or refactoring Sushi admin pages with HTMX partials, Alpine state, shared ui-kit components, and persistent table behavior.
---

# Sushi Admin Feature Delivery

## Overview

Use this skill to implement admin pages that follow Sushi's production patterns instead of one-off page logic.

## When to Use

- Adding a new `/admin/*` page.
- Refactoring a legacy admin page to HTMX + Alpine.
- Adding table/list CRUD flows that require partial refresh.

## Required Pattern

1. Add route in `crates/sushi-admin/src/router.rs`.
2. Put page handlers in `crates/sushi-admin/src/routes/<feature>.rs`.
3. Put templates in `web/templates/admin/` (or `web/templates/plugins/<name>/` for plugin pages).
4. Put static JS in `web/static/admin/js/` (or `web/static/plugins/<name>/`).
5. Use HTMX for request + partial swap (`hx-get/hx-post`, partial tbody endpoint).
6. Use Alpine for UI state (modal/drawer/loading/form state).
7. Reuse `window.AdminUI` helpers for table state, feedback parsing, and partial refresh.

## UI Rules

- No `alert()`, no `confirm()`, no `hx-confirm`.
- Use shared modal/drawer components.
- Preserve table filter/sort/pagination state with `storageKey`.
- Always show loading, empty, and error states.

## Done Criteria

- Mutation response provides visible flash/toast feedback.
- List area refreshes deterministically after success.
- `cargo test -p sushi-admin --test admin_web -q` passes.
- `cargo test --workspace -q` passes.

