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

# [sushi] recent context, 2026-04-22 5:00pm GMT+8

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 50 obs (8,687t read) | 855,719t work | 99% savings

### Apr 22, 2026
411 11:41a 🔴 File Browser UI Bug Reports
412 11:43a 🔵 File Browser UI Architecture
413 " 🔴 Fixed Editor Panel Flexbox Layout
414 11:44a 🔴 Fixed Directory Tree Layout on Expand
415 " 🔴 File Browser UI Bugs Fixed and Verified
416 11:45a ✅ File Browser Bug Fixes Ready
417 " ✅ Full Test Suite Passes
418 11:47a 🔴 Full Workspace Test Suite Passes
419 11:52a 🔴 File browser UI bugs identified and being fixed
420 " 🔵 Sushi web server runs on 127.0.0.1:3008
421 " 🔵 Sushi project structure discovered
422 11:53a 🔵 File browser UI is running and accessible
423 " 🔵 File browser shows runtime error on file open
424 12:01p 🔴 Directory tree rendering bug
425 12:02p 🔵 File browser JS uses extractTreeChildrenMarkup
426 12:03p 🔵 File browser roots behave differently
427 12:04p 🔴 Fixed directory tree rendering in file browser
428 12:21p ✅ Directory tree UI compactness improvements
429 " 🔵 File browser plugin current structure
430 12:22p ✅ Toolbar buttons converted to SVG icons
431 " ✅ Patch applied to file_browser.html
432 12:23p ✅ Directory tree made more compact
433 " 🟣 File/folder name character limit implemented
434 12:24p 🔵 Tests pass after compact UI changes
435 " 🔵 UI verification screenshot captured
436 " 🔵 UI verification screenshot reviewed
437 12:25p ✅ Development server restarted for testing
438 " 🔵 File browser plugin successfully loaded and UI verified
439 1:34p 🔴 Fix wallet creation screen UI issue
440 " 🔴 Fixed file browser list view layout
441 1:35p 🔴 Enhanced file browser CSS layout rules
442 " 🔴 Fixed CSS attribute selector syntax
443 " ✅ File browser plugin tests passed
444 1:41p 🔴 File browser plugin layout fixes verified
445 " 🔄 File browser compact layout refinements
446 1:42p 🔵 File browser server running on localhost:3008
448 1:49p 🔄 File browser HTML structure migrated from ul/li to div
449 " 🔄 Removed legacy group class from new div structure
450 1:50p 🔴 CSS hover-based download button visibility
451 2:08p 🔴 UI fixes for row spacing and hover state
452 " 🔴 Fixed row spacing and hover states in file browser CSS
453 2:09p 🔴 File browser CSS fixes verified with visual testing
454 2:10p 🔴 Added hover background for file browser node label
455 2:15p 🔴 Enhanced file browser hover states with smooth transitions
456 2:18p 🔴 Committed file browser UI fixes
457 2:20p 🟣 Refactored admin partial route authorization to use policy-based access control
458 2:32p ✅ Sushi API router significantly expanded with new endpoints
459 2:34p 🔴 Admin plugin routes now use admin policy surface
460 " ✅ Graphify knowledge graph auto-rebuilt after commit
461 " ✅ AGENTS.md updated alongside router fix and graph rebuild

Access 856k tokens of past work via get_observations([IDs]) or mem-search skill.
</claude-mem-context>