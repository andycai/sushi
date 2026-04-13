# Shared Admin UI Module Checklist

Use this as the single source of truth for any admin or plugin-admin CRUD/list page.

## 1) Page Composition

- Page must include: title/subtitle header, search input, primary add/create action, feedback container, and a carded table region.
- Table must include: loading row, empty state, and actions column.
- Add a contextual callout describing operational guidance (for example naming conventions, safety notes, or permission scope).

## 2) Interaction Model

- Use drawer for create/edit; use modal for delete confirmation.
- Never use native `alert()`, `confirm()`, or `hx-confirm`.
- Show busy states for all mutation submissions and disable destructive buttons while pending.

## 3) HTMX + Partial Contract

- Provide partial endpoints for list + mutations:
  - `GET /admin/partials/<feature>/table`
  - `POST /admin/partials/<feature>/create`
  - `POST /admin/partials/<feature>/{id}/update`
  - `DELETE /admin/partials/<feature>/{id}`
- Mutation responses must return flash fragments (`data-ui-flash`) and set `HX-Trigger` for deterministic refresh/close actions.
- Frontend success checks must require both:
  - `event.detail.successful === true`
  - feedback level is not error/danger

## 4) Alpine + Shared UI Toolkit

- Use `window.AdminUI.createDataTable` with persistent state key:
  - `storageKey: 'admin.<feature>.table.v1'`
- Enable client search/sort/pagination and show visible/filtered/total counters.
- Reuse shared helpers: `consumeFeedback`, `isErrorFeedback`, `hasHxTrigger`, `refreshPartial`, `notify`.

## 5) Backend Contract

- Add reserved-path protection for new admin/partial routes in `crates/sushi-admin/src/router.rs`.
- Add/extend permission mapping for read/write endpoints.
- Validate all form inputs server-side and return actionable error messages.
- Keep bootstrap/seed data idempotent and deduplicated for repeated startup/migration runs.

## 6) Verification Gate

- Add/adjust route + partial tests in `crates/sushi-admin/tests/admin_web.rs`.
- Required checks before claiming done:
  - `cargo test -p sushi-admin --test admin_web -q`
  - `cargo test --workspace -q`
