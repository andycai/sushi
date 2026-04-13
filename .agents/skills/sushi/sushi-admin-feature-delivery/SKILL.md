---
name: sushi-admin-feature-delivery
description: Use when implementing or refactoring Sushi admin pages with HTMX partials, Alpine state, shared ui-kit components, and persistent table behavior.
---

# Sushi Admin Feature Delivery

## Overview

Use this skill to implement admin pages that follow Sushi's production patterns instead of one-off page logic.

## Quality Bar (Non-Negotiable)

- New admin modules MUST match the `permissions` page quality level.
- Do not ship tree/list pages as ad-hoc Alpine loops with direct `fetch` CRUD buttons.
- If a module supports create/update/delete, it must use the full server-first pattern (page + partial table + drawer + confirm modal + feedback trigger loop).
- Apply the shared checklist in `../_shared/admin-ui-module-checklist.md`.

## When to Use

- Adding a new `/admin/*` page.
- Refactoring a legacy admin page to HTMX + Alpine.
- Adding table/list CRUD flows that require partial refresh.

## Required Pattern

1. Add route in `crates/sushi-admin/src/router.rs`.
2. Put page handlers in `crates/sushi-admin/src/routes/<feature>.rs`.
3. Put templates in `web/templates/admin/` (or `web/templates/plugins/<name>/` for plugin pages).
4. Put static JS in `web/static/admin/js/` (or `web/static/plugins/<name>/`).
5. Add partial routes for table and mutations (example: `/admin/partials/<feature>/table`, `/create`, `/{id}/update`, `/{id}`).
6. Use HTMX for request + partial swap (`hx-get/hx-post/hx-delete`, tbody endpoint, `hx-trigger` refresh events).
7. Use Alpine for UI state (drawer/modal/loading/form state), not for replacing server rendering.
8. Reuse `window.AdminUI` helpers for table state, feedback parsing, trigger checks, and partial refresh.
9. Keep JSON API endpoints for machine consumers; do not force UI rendering through JSON endpoints only.

## Shared Checklist

- Follow `../_shared/admin-ui-module-checklist.md` in full.
- Treat that file as the primary maintenance target; avoid duplicating checklist logic here.

## UI Rules

- No `alert()`, no `confirm()`, no `hx-confirm`.
- Use shared modal/drawer components.
- Preserve table filter/sort/pagination state with `storageKey`.
- Always show loading, empty, and error states.
- Prefer badges/callouts/monospace labels already in `admin.css`; avoid introducing one-off visual language.

## Backend Contract Rules

- Add reserved-path protection for new admin/partial endpoints in `router.rs` collision list.
- Add/extend permission mapping for new read/write endpoints.
- Mutation endpoints should validate input and return actionable messages (not generic 500 text).
- For seed/config style data used by navigation, keep migration and runtime bootstrap idempotent and deduplicating.
- Keep reserved-path and permission mapping updates in the same PR as route additions.

## Done Criteria

- Mutation response provides visible flash/toast feedback.
- List area refreshes deterministically after success.
- Route + partial coverage is added/updated in `crates/sushi-admin/tests/admin_web.rs`.
- `cargo test -p sushi-admin --test admin_web -q` passes.
- `cargo test --workspace -q` passes.
