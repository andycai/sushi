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

