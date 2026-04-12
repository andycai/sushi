# Sushi Coding Standards (Admin / API / CLI)

This document defines implementation and review standards for the Rust runtime and web/admin surface.

## 1. Scope and Principles

- Scope: `crates/sushi-admin`, `crates/sushi-api`, `crates/sushi-cli`, and shared contracts from `crates/sushi-core`.
- Core principle: Rust and Lua are equal citizens. New capability should be designed so it can be exposed to Lua plugins unless there is a security reason not to.
- Keep behavior explicit and observable: clear route contracts, structured errors, and traceable logs.
- Prefer small, focused modules over large mixed-responsibility files.

## 2. Workspace-Wide Rust Conventions

### 2.1 Code Style

- Use `rustfmt` defaults; do not commit hand-formatted style outliers.
- Use `snake_case` for functions/modules, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Keep public interfaces (`pub`) minimal.
- Avoid `unwrap()` / `expect()` in runtime paths. Prefer `?` with context-rich errors.

### 2.2 Errors and Result Handling

- Application crates may use `anyhow::Result` with `.context(...)` on IO, config, network, and DB boundaries.
- Library-like modules should return typed errors when a stable contract is required.
- User-facing endpoints should return actionable error text; internal details remain in logs.

### 2.3 Async and State

- Use `tokio` async APIs end-to-end for IO operations.
- Do not block the async runtime with sync file/db/network operations.
- Shared mutable state must have clear ownership and lock boundaries.

### 2.4 Security Baseline

- Validate all external input (forms, path params, query/body payload).
- Never bypass permission checks when exposing admin, route, command, or database capabilities.
- Treat plugin responses as untrusted data at boundaries.

## 3. Admin Standards (`crates/sushi-admin` + `web/`)

### 3.1 Routing and Handlers

- Route definitions belong in `crates/sushi-admin/src/router.rs`.
- Feature handlers belong in `crates/sushi-admin/src/routes/<feature>.rs`.
- Avoid overlapping method/path routes.
- For table/list pages, provide a dedicated partial route for body refresh (HTMX target).

### 3.2 Templating and Static Assets

- HTML templates live under `web/templates/`.
- CSS/JS/images live under `web/static/`.
- No inline page-specific JS in templates except minimal bootstrapping.
- No CDN dependencies in templates.

### 3.3 Frontend Interaction Pattern

- Stack: HTMX for requests/partial swaps, Alpine.js for local state, TailwindCSS + shared admin styles for presentation.
- Do not use `alert()` / `confirm()` / `hx-confirm`; use modal/drawer components.
- Reuse shared helpers in `web/static/admin/js/ui-kit.js` for:
  - table behavior (filter/sort/pagination)
  - feedback/toast parsing
  - partial refresh helpers
  - persisted state via `localStorage`

### 3.4 UX and State

- Keep search/sort/page/pageSize/filter state persistent for high-frequency tables.
- Always provide visible loading/empty/error states.
- Mutations (create/update/delete) must provide explicit success/failure feedback and deterministic list refresh.

## 4. API Standards (`crates/sushi-api`)

### 4.1 Route Design

- Use RESTful naming and HTTP methods.
- Keep response shape consistent for similar resources.
- For plugin-returned JSON envelopes (`sushi.web.json`), preserve status semantics at boundary parsing.

### 4.2 Request Validation

- Validate path/query/body before touching storage.
- Reject malformed input with `4xx`; reserve `5xx` for server/runtime failures.

### 4.3 Response Rules

- Prefer JSON for API endpoints.
- Set explicit status codes (e.g., `201` for create, `404` for missing resource).
- Keep errors stable and parseable for UI/CLI consumers.

## 5. CLI Standards (`crates/sushi-cli`, `crates/sushi`)

### 5.1 Command Shape

- Define subcommands with `clap` derive and clear argument docs.
- Command names should be short and verb-oriented.
- Return user-friendly output for success and failure paths.

### 5.2 Runtime Behavior

- Reuse `bootstrap(...)` for shared initialization (config, db, templates, plugins).
- Avoid command-specific duplicated boot logic.
- Ensure plugin command execution failures are isolated and clearly surfaced.

## 6. Testing Requirements

- For admin routing/templating behavior, add/update tests in `crates/sushi-admin/tests/admin_web.rs`.
- For plugin/Lua runtime boundary behavior, add/update tests in `crates/sushi-core` tests where applicable.
- Minimum validation before merge:
  - `cargo test -p sushi-admin --test admin_web -q`
  - `cargo test --workspace -q`
- For UI regressions, manually verify affected pages (load, mutate, refresh, pagination/filter correctness).

## 7. Review Checklist (PR Gate)

- Does the change follow existing file/module boundaries?
- Are input validation and permission checks explicit?
- Are status codes and error messages consistent?
- Does admin UI avoid inline hacks and reuse shared components/helpers?
- Are tests updated for the new behavior and passing?
- Is there any duplicated logic that should be centralized first?

