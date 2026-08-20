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
│       ├── migrations/
│       ├── lua/
│       └── web/
└── third_party/
    └── <plugin-name>/
        ├── plugin.toml
        ├── init.lua
        ├── migrations/
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
schema_version = 1

[plugin]
name = "my-plugin"
version = "0.1.0"
description = "What this plugin provides"
entry = "init.lua"

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
- `schema_version`: required manifest contract version. Current version is `1`; missing or unsupported versions are rejected before plugin activation.
- Plugin trust tier is derived from the host-selected source path and profile policy. `plugin.toml` has no trust-tier field.

### 3.3 Permission minimization

- For `third_party` plugins: request only what is required.
- `database = "write"` (or above) must have explicit business need.
- Host trust ceilings, manifest requests, profile grants, and administrator approval are intersected. A plugin cannot self-declare a higher trust tier.
- Every enabled Lua profile entry must set `approved = true` under `[entries.grants]`. Without that explicit administrator approval, the host does not execute the plugin entrypoint or publish any capability, event subscription, background task, authentication use, or database access. A required unapproved entry fails before the database is opened.
- Grant fields can only reduce manifest requests. Omitting `routes`, `commands`, or `admin` after approval keeps the manifest request; setting one to `false` removes it. `database` clamps the requested database level.
- Runtime activation (`enabled` / `disabled`) is controlled by platform governance state, not by plugin self-declaration.
- `plugin.toml` permissions are declaration-time capability ceilings and cannot force runtime enablement.

Profile example:

```toml
[[entries]]
id = "my-plugin.default"
source = "lua:third_party/my-plugin"
enabled = true
required = false

[entries.grants]
approved = true
routes = true
commands = true
database = "read"
```

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
- Register capabilities during `sushi.init()` only.
- Keep registration idempotent and deterministic.
- Plugins register capabilities only via `sushi.capability.register({...})`.
- Legacy direct registration APIs remain readable only for one compatibility window. Each used adapter (`sushi.api.route`, `sushi.admin.page`, `sushi.cli.command`, `sushi.web.page`) emits one host warning naming the plugin and migration target; shipped plugins must not depend on them.
- Capability visibility is deny-by-default at injection time.

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

### 4.5 数据库迁移

- 只有受宿主信任且由 profile 选择的 `plugins/official/<name>` source 可以声明 migration；第三方插件不能通过修改 manifest 元数据获得该能力。
- migration 文件放在插件本地 `migrations/*.sql`，按文件名中的数字前缀确定全局执行顺序，例如 `010_create_notes.sql`。
- migration 需要 manifest 的数据库写权限，以及 profile 中显式的 `[entries.grants] approved = true` 和 `database = "write" | "admin"`。
- 历史 migration 文件发布后不可修改；runtime 会校验 SHA-256 checksum，不一致时 fail closed。
- 单个 migration 的 SQL 与 catalog 记录在同一数据库事务中；执行失败不得留下部分 schema 或记录。
- migration 只向前执行，不提供自动 down migration；发布回滚必须通过新的 forward migration 修复数据结构。

### 4.6 Background tasks

- Register owner-scoped work with `sushi.task.spawn(name, callback)` or `sushi.task.interval(name, interval_ms, callback)` during `sushi.init()`.
- Task names must be non-empty, contain no control characters, and be unique within one plugin activation.
- Registered tasks remain deferred until capability and VM publication succeeds. Failed activation starts no task.
- Plugin disable/reload and host shutdown cancel owner tasks. A task that ignores cancellation is aborted after the host timeout.
- Do not create untracked work through hidden executors or host escape hatches. Work that must survive plugin disable belongs to a host service, not a plugin task.

### 4.7 Admin UI Convention

- Page registration uses contract payloads through `sushi.capability.register({...})`:
  - `surface = "web"`
  - `kind = "page"`
  - `path`, `title`, `template`, `handler`, `policy`
- If the page requires JS/CSS, keep asset declaration in `plugin.toml` and render via `sushi.web.render(...)` in the page handler.
- Partial endpoints should return template-rendered fragments.
- Feedback fragments should follow shared flash protocol (`data-ui-flash`, `data-level`, `data-message`).
- Navigation convention for scalable plugin IA:
  - Keep global sidebar stable (`Plugins` is the global entry).
  - Prefer plugin workspace pages under `/admin/plugins/<plugin>` for plugin-level navigation.
  - Additional plugin pages should usually stay inside plugin workspace flows; avoid adding many global sidebar entries.

## 5. HTTP/API Contract Conventions

- Register API routes via `sushi.capability.register({ surface = "api", ... })`.
- Keep endpoint naming stable (`/api/<domain>`).
- Use consistent method semantics:
  - `GET` list/read
  - `POST` create/action
  - `PUT/PATCH` update
  - `DELETE` remove
- For not found / validation / conflict, return clear `4xx` semantics.
- Do not set both `policy` and `public = true` in the same route contract.

## 6. CLI Contract Conventions

- Register commands with `sushi.capability.register({ surface = "cli", ... })`.
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
