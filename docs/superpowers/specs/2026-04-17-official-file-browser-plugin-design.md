# Official File Browser Plugin Design

- Date: 2026-04-17
- Scope: `plugins/official/file-browser/` + minimal runtime enhancements in `sushi-core`/`sushi-api`
- Status: Approved in-session (architecture, config contract, route contract, fs contract, security)

## 1. Goal and Scope

Sushi will ship an official File Browser plugin that exposes a public web UI for browsing configured directories and performing file operations.

Required product characteristics:

- Public web only (no admin menu integration, no CLI command surface, no standalone REST API product surface)
- Multi-root filesystem access, configured in plugin `plugin.toml`
- Capability flags per root (fine-grained operation control)
- Text file viewing/editing by extension whitelist
- Download support for both text and non-text files (capability-controlled)
- Anonymous access for MVP (future login/RBAC integration is planned later)

Out of scope for this iteration:

- Admin UI integration
- CLI integration
- Auth/RBAC enforcement on file operations
- Versioning/history/conflict merge
- Bulk operations and recycle bin

## 2. Locked Product Decisions

1. Public route prefix: `/app/files`
2. Plugin configuration source: plugin-local `plugin.toml`
3. Multiple roots are supported, each with independent capability flags
4. Text detection: extension whitelist only (no content sniffing)
5. Symlink policy: fully denied (not listed, not accessible)
6. Hidden path policy: deny dotfiles/dotdirs by default (not listed, not directly accessible)
7. Download policy: text and non-text are both downloadable when `can_download=true`
8. Security model for MVP: fully anonymous public access
9. Implementation approach: safety-critical filesystem operations in Rust runtime, orchestration/UI in official Lua plugin

## 3. Architecture

### 3.1 High-Level Split

- Runtime (`sushi-core` / `sushi-api`)
  - Adds a constrained filesystem API (`sushi.fs`) to Lua
  - Enforces canonical path sandboxing, hidden/symlink policy, and root-level capability checks
  - Supports public plugin routes for this plugin’s `/app/files/**` surface
- Plugin (`plugins/official/file-browser`)
  - Registers web routes and renders pages/partials
  - Maps user actions to `sushi.fs` calls
  - Applies per-root capability gating before invoking runtime operations

### 3.2 Why This Boundary

- Keeps security-critical filesystem boundary in Rust
- Preserves Lua plugin as first-class product layer (registration, views, behavior)
- Aligns with Sushi’s Rust/Lua parity model while avoiding unsafe filesystem logic in script-only code

## 4. Plugin Configuration Contract (`plugin.toml`)

The plugin declares file-browser behavior under `[file_browser]`.

```toml
[plugin]
name = "file-browser"
version = "0.1.0"
description = "Public file browser"
entry = "init.lua"
kind = "official"

[permissions]
routes = true
commands = false
admin = false
database = false

[file_browser]
route_prefix = "/app/files"
hide_dotfiles = true
deny_symlink = true
allow_download_default = false
text_extensions = ["txt", "md", "json", "toml", "yaml", "yml", "lua", "rs", "js", "ts", "html", "css"]

[[file_browser.roots]]
id = "docs"
title = "Documents"
path = "/srv/docs"

[file_browser.roots.capabilities]
can_list = true
can_view_text = true
can_edit_text = true
can_create_text = true
can_create_dir = true
can_rename = true
can_delete = false
can_upload = true
can_download = true
```

Startup-time validation rules:

- `roots[*].id` must be unique and match `[a-z0-9-_]+`
- `roots[*].path` must be absolute and exist
- Roots must not overlap (`/a` and `/a/b`) to avoid ambiguous capability evaluation
- `text_extensions` are normalized to lowercase extension tokens without dots
- Invalid config fails plugin init (fail-fast)

## 5. Public Route Contract

All routes are under `/app/files/**`.

- `GET /app/files`
  - Main browser shell page
- `GET /app/files/list/*`
  - Directory listing partial
- `GET /app/files/open/*`
  - Open file detail/editor partial for text files
- `POST /app/files/save/*`
  - Save edited text
- `POST /app/files/create-text`
  - Create `.txt` file
- `POST /app/files/create-dir`
  - Create directory
- `POST /app/files/rename/*`
  - Rename file/dir
- `POST /app/files/delete/*`
  - Delete file/dir (MVP: directory delete is empty-dir only)
- `POST /app/files/upload`
  - Upload file into target directory
- `GET /app/files/download/*`
  - Download file stream

Design notes:

- No independent `/api/file-browser/...` product namespace is introduced
- Browser page and operation handlers are all plugin routes
- Responses use HTML partials for UI refresh and UX feedback

## 6. Runtime Filesystem API (`sushi.fs`)

`sushi.fs` is introduced as constrained runtime capability for plugin use.

Proposed operations:

- `list(root_id, rel_path)`
- `read_text(root_id, rel_path)`
- `write_text(root_id, rel_path, content)`
- `create_text(root_id, rel_path, initial_content?)`
- `mkdir(root_id, rel_path)`
- `rename(root_id, from_rel_path, to_rel_path)`
- `delete(root_id, rel_path)`
- `prepare_download(root_id, rel_path)` + runtime download streaming binding
- upload handling API (single-shot or chunked; implementation decides based on existing body constraints)

Common safety rules enforced by runtime (non-bypassable):

- Relative path only (no absolute path, no `..`, no path escape variants)
- Canonical target must remain inside selected root
- Dot-prefixed segments denied when `hide_dotfiles=true`
- Symlink denied when `deny_symlink=true`
- Operation requires corresponding capability flag
- Text operations allowed only for configured text extensions

## 7. Capability Model

Per-root independent flags:

- `can_list`
- `can_view_text`
- `can_edit_text`
- `can_create_text`
- `can_create_dir`
- `can_rename`
- `can_delete`
- `can_upload`
- `can_download`

Enforcement strategy:

1. Plugin-side early deny for UX clarity and fast feedback
2. Runtime-side definitive deny for safety integrity

## 8. UI/Interaction Design

Main page sections:

- Root selector
- Path breadcrumb
- Directory/file table
- Right-side file action panel
- Text editor area (for whitelisted text files)
- Upload/create/rename/delete action controls (conditionally shown by capabilities)

Behavior:

- HTMX-driven partial refresh for list and action outcomes
- Alpine state for local interaction (selected row, editor draft, modal state)
- Shared flash/feedback fragment for operation success/failure
- Non-text files open as metadata row with download action (if allowed)

## 9. Error Model

Operation failures map to stable categories:

- `invalid_path` (malformed or escaping path)
- `not_found`
- `permission_denied` (capability false)
- `forbidden_hidden` (dot path denied)
- `forbidden_symlink`
- `not_text_file`
- `conflict` (rename/create collision)
- `not_empty_dir` (MVP delete constraint)
- `io_error` (runtime I/O boundary)

UX contract:

- Return actionable message in flash fragment
- Preserve list context and selected root/path when possible
- Avoid leaking sensitive host FS internals in user-facing errors

## 10. Security and Limits

MVP is intentionally anonymous; safety therefore depends on strict sandboxing and configuration quality.

Mandatory limits:

- Per-file size limits for text read/write
- Upload size limit
- Optional rate limit hooks at HTTP layer (future hardening)
- Download should not expose absolute host paths

## 11. Testing and Verification

### 11.1 Runtime (`sushi-core` / `sushi-api`)

- Unit tests for path validation and canonical boundary checks
- Symlink hidden-path denial tests
- Capability gating tests per operation
- Public route dispatch tests (bypass auth only for explicitly marked routes)

### 11.2 Plugin (`plugins/official/file-browser`)

- Loader contract tests for directory structure and manifest parsing
- Integration tests for list/open/save/create/rename/delete/upload/download behavior
- UI integration checks for partial refresh and flash messaging

### 11.3 Required verification commands

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

## 12. Future Phase (Post-MVP)

Planned enhancement:

- Integrate login + policy/RBAC model so anonymous mode can be replaced or narrowed
- Map file-browser operations to explicit policy keys
- Allow role-based root visibility and operation rights

