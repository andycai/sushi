# AGENTS.md — Sushi Project

## Project Overview

Sushi is a modular application platform built with **Rust** as the core runtime and **Lua** as a first-class plugin language. The project consists of three equal components: **admin** (web admin panel), **api** (HTTP API server), and **cli** (command-line interface). Rust and Lua share equal status across all components — any capability expressible in Rust is also expressible in Lua plugins, and vice versa.

## Name Meaning (Important)

In this project, **"sushi" means Su Shi (苏轼), not food sushi**.

Su Shi (1037-1101), style name Zizhan (子瞻), also known as Hezhong (和仲), and by the sobriquets Tieguan Daoren (铁冠道人) and Dongpo Jushi (东坡居士), commonly called Su Dongpo (苏东坡), was from Meizhou Meishan (present-day Meishan, Sichuan). He was a Northern Song writer, calligrapher, painter, and a historical figure in water-management governance. Together with his father Su Xun and younger brother Su Zhe, they are known as the "Three Su" (三苏).

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Web Framework | **Rust Axum** | HTTP routing, middleware, request/response handling |
| Plugin Runtime | **mlua** (https://github.com/mlua-rs/mlua) | Lua 5.4 integration for plugin development |
| CLI | **clap** | Command-line argument parsing and subcommand routing |
| Frontend | **Alpine.js** + **TailwindCSS** | Reactive UI for admin panel |

## Architecture

```
sushi/
├── Cargo.toml              # Workspace root
├── AGENTS.md
├── crates/
│   ├── sushi-core/         # Shared core: plugin loader, config, types
│   ├── sushi-api/          # Axum-based API server
│   ├── sushi-admin/        # Admin panel (Axum + Alpine.js + TailwindCSS)
│   └── sushi-cli/          # CLI tool powered by clap
├── plugins/                # Lua plugins (equal citizen to Rust)
│   ├── example_plugin/
│   │   ├── init.lua        # Plugin entry point
│   │   └── plugin.toml     # Plugin manifest
│   └── ...
└── ui/                     # Frontend assets for admin
    ├── src/
    │   ├── index.html
    │   ├── app.js          # Alpine.js application
    │   └── styles.css      # TailwindCSS entry
    └── tailwind.config.js
```

### Component Details

#### admin — Web Admin Panel
- Serves an HTML UI built with **Alpine.js** and **TailwindCSS** via Axum
- Alpine.js handles client-side reactivity; TailwindCSS handles styling
- All admin operations are routed through the api layer
- Admin-specific Rust handlers and Lua plugins are both supported
- Frontend assets live under `ui/` and are embedded at compile time (e.g., via `include_dir!` or `rust-embed`)

#### api — HTTP API Server
- Built on **Axum** with tower middleware layers
- Routes can be defined in Rust (compile-time) or registered dynamically via Lua plugins
- Plugin hooks: `on_request`, `on_response`, `on_route_register`, `on_middleware`
- RESTful conventions; JSON request/response by default
- Lua plugins can register custom routes, middleware, and response handlers

#### cli — Command-Line Interface
- Built with **clap** derive macros for subcommand parsing
- Shares the same plugin system as admin and api
- Lua plugins can register custom CLI subcommands that execute Lua logic
- Plugin hooks: `on_command`, `on_cli_init`

## Plugin System (Lua via mlua)

### Design Principles

1. **Rust and Lua are equal citizens.** Every component (admin, api, cli) exposes the same plugin interface regardless of whether the implementation is in Rust or Lua.
2. **Plugins are self-contained.** Each plugin lives in its own directory under `plugins/` with a `plugin.toml` manifest and Lua source files.
3. **Safety by default.** Lua runs in a sandboxed mlua environment. Plugins must declare permissions in their manifest.

### Plugin Manifest (plugin.toml)

```toml
[plugin]
name = "example_plugin"
version = "0.1.0"
description = "An example Lua plugin"
entry = "init.lua"

[permissions]
routes = true       # Can register HTTP routes
commands = true     # Can register CLI commands
admin = true        # Can extend admin panel
database = false    # Cannot access database directly
```

### Plugin Entry Point (init.lua)

```lua
-- Plugins receive a `sushi` context object
function sushi.init()
    -- Register a custom API route
    sushi.api.route("GET", "/hello", function(req)
        return { status = 200, body = { message = "Hello from Lua!" } }
    end)

    -- Register a CLI subcommand
    sushi.cli.command("greet", "Print a greeting", function(args)
        print("Hello from Lua plugin!")
    end)
end
```

### Plugin API Surface

The `sushi` context exposes these namespaces to Lua plugins:

| Namespace | Methods | Component |
|-----------|---------|-----------|
| `sushi.api` | `route(method, path, handler)`, `middleware(handler)` | api |
| `sushi.admin` | `page(path, title, component)`, `widget(name, component)` | admin |
| `sushi.cli` | `command(name, desc, handler)`, `option(name, short, desc)` | cli |
| `sushi.config` | `get(key)`, `set(key, value)` | core |
| `sushi.log` | `info(msg)`, `warn(msg)`, `error(msg)` | core |
| `sushi.db` | `query(sql)`, `exec(sql)` (if permitted) | core |
| `sushi.event` | `on(event, handler)`, `emit(event, data)` | core |

## Development Guidelines

### Standards References

- Coding standards (admin/api/cli): `docs/engineering/coding-standards.md`
- Plugin authoring standards: `docs/engineering/plugin-authoring-standards.md`
- Plugin delivery skill (workflow + done criteria): `.agents/skills/sushi/sushi-lua-plugin-delivery/SKILL.md`
- Reusable project skills: `.agents/skills/sushi/`

### Rust Conventions

- **Workspace structure:** Use a Cargo workspace with shared dependencies
- **mlua integration:** Use `mlua` with `Lua::new()` and sandbox via `Lua::create_table()` for plugin isolation
- **Error handling:** Use `anyhow` for applications, `thiserror` for library crates
- **Async runtime:** tokio (required by Axum)
- **Serialization:** serde + serde_json for all JSON handling
- **Route registration:** Provide both a Rust macro API and a Lua function API that converge to the same router

### Lua Plugin Conventions

- One plugin = one directory under `plugins/`
- Entry point is always `init.lua` (configurable in manifest)
- Plugins must declare required permissions in `plugin.toml`
- Plugin admin templates and static assets must live inside the plugin directory:
  - `plugins/<plugin-name>/web/templates/...`
  - `plugins/<plugin-name>/web/static/...`
- Do not place plugin templates/static assets under repo-global `web/templates/plugins/**` or `web/static/plugins/**`.
- Plugin page registration and template rendering should continue to use logical template names like `plugins/<plugin-name>/...` (resolved by runtime template loader).
- Plugin static assets should be referenced through `/static/plugins/<plugin-name>/...` (mounted from plugin-local `web/static`).
- Plugin asset declarations use list-based config:
  - `plugin.toml` bundle definitions under `[admin.assets.bundles.<bundle>]` with `js = []` and `css = []`.
  - `sushi.web.page(..., { assets = { bundles = {...}, js = {...}, css = {...} } })`.
  - Asset paths must be plugin-local relative paths (no URL/absolute/`..` forms).
- Keep plugins stateless where possible; use `sushi.config` for persistent state
- Use `sushi.log` for all logging; never print directly to stdout
- Do not embed raw HTML strings in Lua source (for example `init.lua`); place markup in HTML template files and render via `sushi.web.render(...)`

### Plugin Resource Auto-Validation

- Every plugin resource change should include automated checks proving template/static resources are plugin-local and still load correctly.
- Minimum verification commands:
  - `cargo test -p sushi-core --test template_service -q`
  - `cargo test -p sushi-admin --test admin_web -q`
- Before merge, run full validation:
  - `cargo test --workspace -q`

### Frontend Conventions (admin)

- **Alpine.js** for all interactivity — no jQuery, no vanilla DOM manipulation
- **TailwindCSS** for all styling — no custom CSS files beyond Tailwind utilities
- HTML is rendered by Axum; Alpine.js hydrates on the client
- Keep `x-data` components small and focused
- Use `x-fetch` or Alpine's fetch patterns for API calls to the api layer

### General

- All user-facing text should be in English by default; i18n support via plugins is acceptable
- Configuration uses TOML format (consistent with plugin manifests)
- Database migrations are managed in Rust, not Lua
- Tests: Rust unit/integration tests + Lua plugin validation tests

### Git Commit Conventions

- Use Conventional Commit style: `type(scope): summary` (e.g. `feat(api): add kv list endpoint`).
- Recommended `type` values: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `style`.
- Keep each commit focused on a single logical change; avoid mixing unrelated changes.
- Commit summary should be concise, imperative, and typically <= 72 characters.
- In commit messages, **do not include signature/attribution trailers**, such as:
  - `Co-Authored-By:`
  - `Signed-off-by:`
  - `Generated-by:` or similar attribution lines

## Build & Run

```bash
# Build all components
cargo build --workspace

# Run API server
cargo run -p sushi-api

# Run admin panel
cargo run -p sushi-admin

# Run CLI
cargo run -p sushi-cli -- --help
cargo run -p sushi-cli -- greet    # Lua plugin command
```

## Key Dependencies (Cargo)

```toml
[workspace.dependencies]
axum = "0.8"
mlua = { version = "0.10", features = ["lua54", "vendored", "async", "send"] }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
anyhow = "1"
thiserror = "2"
tower = "0.5"
tower-http = { version = "0.6", features = ["fs", "cors", "trace"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

## graphify

This project has a graphify knowledge graph at graphify-out/.

Rules:
- Before answering architecture or codebase questions, read graphify-out/GRAPH_REPORT.md for god nodes and community structure
- If graphify-out/wiki/index.md exists, navigate it instead of reading raw files
- After modifying code files in this session, run `graphify update .` to keep the graph current (AST-only, no API cost)


<claude-mem-context>
# Memory Context

# [sushi] recent context, 2026-04-21 7:53pm GMT+8

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 50 obs (14,231t read) | 411,319t work | 97% savings

### Apr 21, 2026
145 5:16p 🟣 Permission gate regression tests all passing
146 5:17p 🟣 Permission gate enforcement committed to sushi-core
147 5:18p 🔴 Task 6 Permission Fix Committed - Loader Now Enforces Permission Gates
148 5:21p 🟣 Task 6 spec compliance verified: web/db/event/fs adapters integrated
149 5:22p 🟣 Task 6 complete: web/db/event/fs Lua contract adapters integrated with permission gates
150 5:25p 🔴 Task 6 Permission Gate Fix Committed and Fully Approved
151 5:26p 🔄 Tasks 1-6 Complete - Task 7 Dispatched for Official Plugin Migration
152 5:27p 🔄 Task 7: Migrate kv-store and file-browser Lua plugins to contract registration
153 5:28p 🔄 Task 7: Added failing test assertions for kv and file-browser contract migration
154 5:29p 🔵 Task 7: Fail-first test running for kv bootstrap contract migration
155 " 🔄 Task 7: TDD fail-first tests confirmed failing as expected
156 5:30p 🔄 Task 7: Migrated both Lua bootstrap files to contract registration
157 " 🔄 Task 7: kv-store contract migration test passes
158 5:31p 🔄 Task 7: All tests pass confirming successful migration
159 " 🔄 Task 7: Final state before commit - all files migrated and tests passing
160 5:32p 🔄 Task 7 COMPLETED: Committed kv and file-browser contract registration migration
161 " 🔄 Task 7 FINAL STATUS: All work completed and committed
162 5:33p 🔄 Task 7 Complete - KV Store and File Browser Migrated to Contract Registration
163 5:35p 🔄 Sushi plugin bootstrap contract registration migration
164 5:37p 🔴 Task 7 Fix Dispatched - Restore Route Coverage Assertions
165 " 🔄 Task 7: Migrate kv-store and file-browser plugins to contract registration
166 " 🔄 Added TDD assertions for contract registration in loader tests
167 5:39p 🔵 kv-store plugin already uses contract registration
168 " 🔵 Both plugins already migrated to contract registration
169 5:40p 🔄 Task 7 complete: loader tests committed with route coverage assertions
170 5:44p 🔴 Task 7 verification: KV and File-Browser plugin contract migration complete
171 5:47p 🔵 kv-store bootstrap handler patterns verified
172 5:48p 🔄 Task 7 completed with second commit adding file-browser route coverage
173 5:49p 🔄 KV store and file-browser migrated to capability.register
174 5:50p 🔄 Contract bootstrap tests completed with full route coverage
175 " 🔵 Contract API schema supports public routes via boolean flag
176 5:53p 🔵 Task 7 Final Code Review Scope Defined
177 5:54p 🔄 Plugin registration migrated to capability-based contract pattern
178 5:55p 🔵 kv-store and file-browser interface handlers verified
179 5:56p 🔴 Template service integration test failing - alpine.js version mismatch
180 5:57p 🔄 Simplified loader tests to component-wise assertions
181 5:59p 🔄 Task 7 refactored tests to component-wise assertions
185 6:04p 🔴 Test contract enforcement strengthened with combined method-path assertions
186 6:05p 🔴 Task 7 test fix committed: method-path contract coverage enforced
187 6:07p 🟣 Contract-based plugin registration enforced with comprehensive route coverage tests
188 " 🔵 base_template_uses_local_assets_only test fails — Alpine.js version string mismatch
189 6:11p 🟣 **kv-store and file-browser migrated to contract registration API**
190 6:12p 🔄 CMS plugin migrated to contract registration
191 6:16p ✅ Added cms_bootstrap_uses_contract_registration test to loader.rs
192 6:18p 🔄 CMS bootstrap migrated to contract registration
193 6:19p 🔵 cms_plugin_behavior test needs update for contract format
194 " ✅ Updated cms_plugin_behavior test for contract format
195 " 🔵 Contract registration tests pass
196 6:20p 🟣 Task 9: CMS Official Plugin Bootstrap Migration Complete
197 " 🟣 Task 9 Complete - CMS Plugin Contract Migration

Access 411k tokens of past work via get_observations([IDs]) or mem-search skill.
</claude-mem-context>