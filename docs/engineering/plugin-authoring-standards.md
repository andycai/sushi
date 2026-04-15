# Sushi Lua Plugin Authoring Standards

This guide defines how to build production-grade Lua plugins for Sushi.

Related references:

- Project-wide operating conventions: `AGENTS.md`
- Execution workflow checklist: `.agents/skills/sushi/sushi-lua-plugin-delivery/SKILL.md`

## 1. Plugin Philosophy

- Plugins are first-class modules, not scripting afterthoughts.
- A plugin should be self-contained: manifest, Lua code, admin surface (optional), and static/template resources.
- Capabilities must be declared and enforced through permissions.

## 2. Required Directory Layout

Each plugin must live under one of the tier roots:

```text
plugins/
├── official/
│   └── <plugin-name>/
│       ├── plugin.toml
│       ├── init.lua
│       ├── lua/
│       └── web/
└── third_party/
    └── <plugin-name>/
        ├── plugin.toml
        ├── init.lua
        ├── lua/
        └── web/
```

If plugin provides admin UI:

- Template files:
  - `plugins/official/<plugin-name>/web/templates/...`
  - `plugins/third_party/<plugin-name>/web/templates/...`
- Static files:
  - `plugins/official/<plugin-name>/web/static/...`
  - `plugins/third_party/<plugin-name>/web/static/...`
- Do **not** place plugin assets in repository-global paths:
  - `web/templates/plugins/**`
  - `web/static/plugins/**`

Legacy flat layout `plugins/<plugin-name>/` is not supported and causes startup failure.

Do not inline large HTML strings in Lua for admin pages.

## 3. Manifest Rules (`plugin.toml`)

### 3.1 Required fields

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "What this plugin provides"
entry = "init.lua"
kind = "third_party" # or "official"

[permissions]
routes = true
commands = true
admin = true
database = "read" # or "write" / false
```

### 3.2 Naming

- `name`: lowercase kebab-case, stable once released.
- `version`: semantic versioning (`MAJOR.MINOR.PATCH`).
- `description`: concise, behavior-oriented.
- `kind`: must match directory category (`official` plugins under `plugins/official/`, `third_party` plugins under `plugins/third_party/`).

### 3.3 Permission minimization

- For `third_party` plugins: request only what is required.
- `database = "write"` (or above) must have explicit business need.
- For `official` plugins: runtime enforces full permissions (`routes`, `commands`, `admin`, `database = "admin"`), regardless of manifest declaration.

### 3.4 Admin asset bundles

Plugins with admin JS/CSS must declare list-based bundles in `plugin.toml`:

```toml
[admin.assets.bundles.workspace]
js = ["kv.js", "shared/table.js"]
css = ["kv.css"]
```

Rules:

- `js` and `css` use **lists** (not scalar strings).
- Paths are relative to tiered plugin static root:
  - `plugins/official/<name>/web/static/`
  - `plugins/third_party/<name>/web/static/`
- `sushi.web.page(..., { assets = { bundles = {...}, js = {...}, css = {...} } })`
  may combine bundle names with page-local lists.
- Paths must be plugin-local relative paths only (no `http://`, `https://`, `//`, absolute path, or `..`).

## 4. Lua Implementation Rules

### 4.1 Entry and Registration

- Entry point is `function sushi.init()`.
- Register routes/commands/pages during `sushi.init()` only.
- Keep registration idempotent and deterministic.

### 4.2 Logging and Observability

- Use `sushi.log.info/warn/error` for operational logs.
- Do not `print(...)` in production plugin paths.
- Log enough context to diagnose failures (route/command/key IDs, not secrets).

### 4.3 Input and Error Handling

- Validate all user input (HTTP body/query/path/form/CLI args).
- Return explicit, actionable error messages.
- For API handlers, prefer `sushi.web.json(status, payload)` to encode status + body semantics.

### 4.4 Database Access

- Use `sushi.db.query(...)` for reads and `sushi.db.execute(...)` for writes.
- Keep SQL parameterized; never concatenate untrusted input directly into SQL.
- Handle database errors and return safe user feedback.

### 4.5 Admin UI Convention

- Page registration:
  - `sushi.web.page("/admin/<path>", "plugins/<tier>/<name>/<page>.html", { ... })`
- If the page requires JS/CSS, declare `assets = { bundles = {...}, js = {...}, css = {...} }` in `sushi.web.page(...)`.
- Partial endpoints should return template-rendered fragments.
- Feedback fragments should follow shared flash protocol (`data-ui-flash`, `data-level`, `data-message`).
- Navigation convention for scalable plugin IA:
  - Keep global sidebar stable (`Plugins` is the global entry).
  - Prefer plugin workspace pages under `/admin/plugins/<plugin>` for plugin-level navigation.
  - Additional plugin pages should usually stay inside plugin workspace flows; avoid adding many global sidebar entries.

## 5. HTTP/API Contract Conventions

- Keep endpoint naming stable (`/api/<domain>`).
- Use consistent method semantics:
  - `GET` list/read
  - `POST` create/action
  - `PUT/PATCH` update
  - `DELETE` remove
- For not found / validation / conflict, return clear `4xx` semantics.

## 6. CLI Contract Conventions

- Register commands with `sushi.cli.command(name, description, handler)`.
- Return concise success/error messages.
- Validate args early and return usage hints on missing/invalid args.

## 7. Security and Data Safety

- Never expose secrets in HTML, JSON responses, logs, or CLI output.
- Escape untrusted values when generating HTML strings.
- Keep permission boundaries strict; do not build hidden side channels around denied permissions.

## 8. Testing and Verification Checklist

Before merge/release, verify:

- Plugin loads without runtime errors.
- Registered routes/pages/commands are reachable.
- Invalid input returns correct error responses.
- Admin mutations show feedback and refresh list correctly.
- Database operations behave correctly for read/write paths.
- Resource-isolation checks pass:
  - `cargo test -p sushi-core --test template_service -q`
  - `cargo test -p sushi-admin --test admin_web -q`
- Workspace tests pass:
  - `cargo test --workspace -q`

## 9. Backward Compatibility

- Treat API route names and payload shape as public contract once used by UI/automation.
- Breaking changes require a major version bump and migration notes.
