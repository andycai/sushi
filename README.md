# Sushi

A modular platform combining a Rust runtime with Lua plugin extensibility, exposing three equal product surfaces: a web admin panel, an HTTP API server, and a CLI tool.

## Overview

Sushi is named after Su Shi (苏轼), the renowned Song dynasty poet — because a good platform should be as elegant and enduring as great literature.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Runtime | Rust + Tokio async |
| Web Framework | Axum 0.8 |
| Plugin Runtime | mlua (Lua 5.4) |
| CLI Parsing | Clap 4 |
| Database | SQLite (rusqlite, bundled) |
| Frontend | HTMX + Alpine.js + TailwindCSS v4 + daisyUI |
| Templating | Minijinja (Rust) |
| Auth | JWT + Argon2 password hashing |
| Serialization | Serde + Serde_JSON |

## Architecture

```
sushi/
├── Cargo.toml              # Rust workspace
├── config.toml            # Runtime configuration
├── crates/
│   ├── sushi/             # Main app bootstrap binary
│   ├── sushi-core/         # Shared core library
│   ├── sushi-api/          # Axum HTTP API server
│   ├── sushi-admin/        # Admin web panel
│   └── sushi-cli/          # CLI entry point
├── plugins/
│   └── official/           # Built-in plugins
│       ├── cms/           # CMS plugin (pages, posts, categories)
│       ├── file-browser/  # File browser plugin
│       └── kv-store/      # Key-value store plugin
├── web/
│   ├── templates/          # HTML templates (Minijinja)
│   └── static/             # CSS, JS, assets
├── migrations/             # SQLite schema migrations
└── data/                   # Runtime data (SQLite DB)
```

## Crates

- **sushi-core** — Shared library: auth, config, database, Lua plugin loader, permission engine
- **sushi-api** — Axum HTTP API server with extensive REST endpoints
- **sushi-admin** — Admin web panel router and HTML rendering
- **sushi-cli** — CLI tool with subcommands: serve, run, plugin, config, seed

## Plugin System

Plugins are written in Lua with a `plugin.toml` manifest:

```toml
schema_version = 1

[plugin]
name = "kv-store"
version = "0.2.0"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true
database = "admin"

[policies]
scopes = ["api.kv.*", "admin.kv.manage", "cli.kv.*"]
```

`schema_version` is the required manifest contract version; missing or unsupported versions fail closed before activation. Trust comes from the host-selected source path and profile grants, not from plugin-declared metadata. Official plugins live in `plugins/official/`. The plugin loader is entirely Rust-based (via mlua), giving Lua plugins access to Rust services through a permission-gated module system.

## Database

SQLite with migrations in `migrations/` covering: base schema, KV store, RBAC, menus, policy system, CMS, and plugin governance.

## Getting Started

### Prerequisites

- Rust 1.75+
- Node.js (for TailwindCSS v4 compilation)

### Build

```bash
# Build all crates
cargo build --workspace

# Build with debug info
cargo build --workspace --profile dev
```

### Run

```bash
# Start the API server
cargo run -p sushi-api

# Start the admin panel
cargo run -p sushi-admin

# Start with all features (serve command)
cargo run -p sushi -- serve

# CLI help
cargo run -p sushi-cli -- --help
```

### Configuration

Edit `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 3008

[database]
path = "data/sushi.db"

[jwt]
secret = "change-me-in-production-at-least-32-chars"
access_ttl = 3600
refresh_ttl = 604800

[plugins]
directory = "plugins"
```

## Key Features

- **Hybrid Rust+Lua Runtime** — Both languages are equal citizens; Lua plugins access Rust services
- **Plugin Architecture** — One plugin = folder with `plugin.toml` + `init.lua`; explicit permissions
- **Three Interfaces** — Admin (web UI), API (REST JSON), CLI — all extensible via plugins
- **RBAC/Policy System** — Role-based access control with policy scopes
- **Server-Side Rendered UI** — Minijinja templates + HTMX for SPA-like experience
- **JWT Authentication** — With refresh tokens and Argon2 password hashing
- **Built-in Plugins** — File browser, KV store, CMS

## Documentation

- [AGENTS.md](AGENTS.md) — Project documentation and context for AI agents
- `docs/` — Engineering docs and wiki
- `migrations/` — Database schema documentation (SQL files)
