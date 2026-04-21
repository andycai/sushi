# AGENTS.md — Sushi Project

## Project Overview

Sushi is a modular platform built with **Rust** runtime + **Lua** plugins.  
It has three equal product surfaces:

- `admin` — web admin panel
- `api` — HTTP API server
- `cli` — command-line interface

Rust and Lua are equal citizens: features should be designed so both sides can participate.

## Name Meaning (Important)

In this project, **"sushi" means Su Shi (苏轼), not food sushi**.

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Web Framework | **Rust Axum** | Routing, middleware, request/response |
| Plugin Runtime | **mlua** | Lua 5.4 plugin integration |
| CLI | **clap** | Command/subcommand parsing |
| Frontend | **HTMX** + **Alpine.js** + **TailwindCSS v4** + **daisyUI** | Server-first UI + local state + shared component/theme system |

## Architecture

```text
sushi/
├── Cargo.toml
├── AGENTS.md
├── config.toml
├── crates/
│   ├── sushi/                  # App bootstrap binary
│   ├── sushi-core/             # Shared core: plugin loader, config, auth, templates
│   ├── sushi-api/              # Axum API server
│   ├── sushi-admin/            # Admin routes/handlers/tests
│   └── sushi-cli/              # CLI entry and command routing
├── plugins/
│   ├── official/
│   │   ├── cms/
│   │   ├── file-browser/
│   │   └── kv-store/
│   └── third_party/
│       └── _example/
├── web/
│   ├── templates/              # Base/admin HTML templates + fragments/partials
│   └── static/
│       ├── admin/              # Shared admin JS/CSS helpers
│       ├── css/                # Tailwind input + compiled style.css
│       └── js/                 # Local runtime deps (htmx/alpine/daisyui)
├── scripts/                    # CSS build/watch and frontend helper scripts
├── migrations/                 # SQLite schema migrations
├── docs/                       # Engineering docs/specs/wiki
├── data/                       # Local runtime DB files
├── openspec/                   # OpenSpec changes/tasks
└── graphify-out/               # Knowledge graph artifacts
```

## Component Notes

### admin

- HTML is rendered server-side by Axum templates.
- HTMX handles partial navigation/swaps.
- Alpine handles local UI state and interactions.
- Tailwind + daisyUI provide visual system and themes.

### api

- REST-style JSON endpoints by default.
- Routes can be Rust-defined or plugin-registered.
- Middleware and hooks are available for plugin extension.

### cli

- Clap derive-based command structure.
- Shares plugin runtime with admin/api.
- Plugins can register CLI commands.

## Plugin System (Lua via mlua)

### Principles

1. Rust and Lua capabilities should stay symmetric when possible.
2. One plugin = one folder with `plugin.toml` + `init.lua` entry.
3. Permissions are explicit in `plugin.toml`.
4. Plugin web resources are plugin-local, not repo-global.

### Manifest Example

```toml
[plugin]
name = "example_plugin"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true
database = false
```

### Plugin Web Resource Rules

- Templates/static assets must be inside plugin folder:
  - `plugins/<name>/web/templates/...`
  - `plugins/<name>/web/static/...`
- Do not place plugin assets under repo-global `web/templates/plugins/**` or `web/static/plugins/**`.
- Reference plugin static files by `/static/plugins/<name>/...`.
- Keep template rendering with logical names like `plugins/<name>/...`.
- Do not embed raw HTML strings in Lua; render template files via `sushi.web.render(...)`.

## Development Guidelines

### Standards References

- `docs/engineering/coding-standards.md`
- `docs/engineering/plugin-authoring-standards.md`
- `.agents/skills/sushi/`

### Rust Conventions

- Cargo workspace-first organization.
- `anyhow` for app errors, `thiserror` for library contracts.
- `tokio` async runtime.
- `serde` / `serde_json` for serialization.
- Keep route/command/plugin boundaries explicit and testable.

### Frontend Conventions (admin)

- Stack boundaries:
  - HTMX = server requests and partial swaps
  - Alpine = local state/actions
  - Tailwind v4 + daisyUI = styling and theme semantics
- Do not introduce React/Vue/Svelte/jQuery.
- Preserve stable `id`, `data-*`, `hx-target`, and fragment route contracts during refactors.
- Partial endpoints must return fragment HTML (not full page shell).
- Do not use `alert()`/`confirm()`/`hx-confirm`; use modal/drawer patterns.
- Tailwind is the only utility layer; do not reintroduce legacy `ui-*` systems.
- Keep single compiled stylesheet at `web/static/css/style.css` from `web/static/css/input.css`.
- Use daisyUI component semantics (`btn`, `card`, `table`, `alert`, `modal`, `drawer`, ...).
- Global theme state lives at `<html data-theme="light|dark">` and persists via `localStorage`.
- HTMX partial swaps must not reset theme state.
- No CDN dependencies for runtime/style assets.

### Validation Baseline

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

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
```

## Git Commit Conventions

- Conventional Commit format: `type(scope): summary`
- Recommended types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `style`
- Keep commits focused; do not mix unrelated changes.
- Do not include attribution trailers (`Co-Authored-By`, `Signed-off-by`, `Generated-by`, etc.).

## Graphify

This project uses a graphify knowledge graph in `graphify-out/`.

- Before architecture/codebase Q&A, read `graphify-out/GRAPH_REPORT.md`.
- If `graphify-out/wiki/index.md` exists, navigate it before raw-file deep dives.
- After modifying code files, run `graphify update .` to refresh graph artifacts.


<claude-mem-context>
# Memory Context

# [sushi] recent context, 2026-04-21 11:10pm GMT+8

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 50 obs (14,503t read) | 751,692t work | 98% savings

### Apr 21, 2026
211 8:50p ✅ CSS build pipeline migrated to pnpm + Tailwind v4 npm plugin
212 " 🟣 daisyUI CSS compilation working — 253KB output with theme system
213 8:51p ✅ First commit for daisyUI frontend rewrite project
214 " 🟣 Theme toggle with localStorage persistence added to base template
215 8:53p 🟣 Login page rewritten with daisyUI component classes
216 8:54p 🟣 Dashboard content fragment converted to daisyUI components
217 9:22p 🔵 UI component class system inventory
218 9:24p 🔵 Hybrid UI class systems found in templates
219 " 🔄 Flash templates fixed to use DaisyUI alert classes
220 9:27p ✅ CSS architecture refactored to use Tailwind v4 @apply components
221 9:28p 🔴 kv_store_plugin_no_longer_embeds_html test needs update for DaisyUI flash
222 " 🔴 kv_store_plugin_no_longer_embeds_html test assertion updated for DaisyUI flash
223 9:29p 🟣 Full workspace test suite passes after DaisyUI rewrite
224 9:30p 🔄 shadow-sm-body invalid class replaced with DaisyUI card-body
225 " 🔴 CMS editor textarea classes deduplicated
226 9:32p 🔄 Admin and plugin templates fully migrated to DaisyUI — committed
227 9:33p 🟣 File Browser plugin template gets theme toggle and DaisyUI CSS
228 9:40p 🔵 Claude-Mem observer session initialized
229 9:41p 🔵 DaisyUI rewrite analyzing existing ui-* component patterns
230 " ✅ DaisyUI rewrite: bulk replacement of ui-* classes with Tailwind/DaisyUI equivalents
231 9:42p ✅ DaisyUI rewrite: replaced remaining ui-* CSS classes in JavaScript files
232 " 🔵 DaisyUI rewrite: CSS component library still exists in input.css alongside migrated templates
233 9:43p 🟣 DaisyUI rewrite: complete migration of ui-* component classes from templates and JS
234 " 🟣 DaisyUI rewrite: all tests pass after component class migration
235 " 🔵 DaisyUI rewrite: 51 files modified across admin, CMS, and KV-store plugins
236 9:44p 🔄 DaisyUI rewrite committed to feature branch
237 9:45p 🔵 DaisyUI rewrite feature branch progress: 10 commits completed
238 9:49p 🔄 DaisyUI rewrite completed for sushi-daisyui-rewrite feature branch
239 " ✅ DaisyUI rewrite: added admin shell CSS component definitions
240 9:50p 🔴 DaisyUI rewrite: fixed Tailwind v4 CSS compilation error - removed invalid 'group' @apply
241 " 🔴 DaisyUI rewrite: fixed second Tailwind v4 CSS compilation error - replaced DaisyUI component classes
242 9:56p 🔴 Admin login endpoint returns 401 Unauthorized
243 9:57p 🔵 Admin user created via seed, exists in SQLite database
244 10:15p 🔴 HTMX login error response no longer returns 401 status code
245 10:18p 🔴 HTMX login error 200 status fix committed to feat/daisyui-frontend-rewrite
246 10:19p 🔵 Graphify post-commit hook causes cascading unstaged changes
247 " 🔵 Login page uses HTMX swap pattern with dedicated error div target
248 10:25p 🔴 Admin partial routes now registered with correct "admin" policy surface
249 10:26p 🔴 KV store plugin updated to use admin.kv.manage policy for all routes
250 10:27p 🔴 Added test verifying admin partial routes get correct policy surface
251 10:31p 🔴 Admin surface policy routing fix committed to feat/daisyui-frontend-rewrite
252 10:37p 🔄 Fixed admin pagination controls to use daisyUI join-horizontal pattern
253 10:55p 🔴 Local dev server homepage returning 404
254 10:56p 🔵 Root path / missing from admin router configuration
255 " 🔴 Added root path routes for homepage redirect
256 " 🔴 Root path redirect fix verified by tests
257 11:01p ✅ Added alpine.js, tailwindcss, htmx, daisyUI to AGENTS.md tech restrictions
258 11:02p ⚖️ Arch decision: restrict alpine.js, tailwindcss, htmx, daisyUI in project guidelines
259 " ✅ AGENTS.md frontend stack documented with full constraints for HTMX/Alpine/Tailwindv4/daisyUI
260 11:06p ✅ AGENTS.md deleted from project root

Access 752k tokens of past work via get_observations([IDs]) or mem-search skill.
</claude-mem-context>