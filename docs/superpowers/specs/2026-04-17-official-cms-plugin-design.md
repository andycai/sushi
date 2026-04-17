# Official CMS Plugin (MVP) Design

- Date: 2026-04-17
- Scope: `plugins/official/cms/` official Lua plugin for Sushi
- Status: Approved in-session (architecture/data model/contracts/testing)

## 1. Context and Goal

Sushi needs an official CMS plugin that provides three core modules:

- Page
- Post
- Category

This plugin must provide:

- Admin management UI
- Public frontend pages
- CLI interfaces

The implementation must follow Sushi plugin standards:

- Tiered official plugin layout (`plugins/official/<name>/...`)
- Lua module layering (not monolithic `init.lua`)
- Plugin-local templates/static assets
- Unified API/admin/CLI registration through plugin bootstrap

## 2. Scope

### 2.1 In Scope (MVP)

1. CRUD for Page/Post/Category
2. Publish state management (`draft` / `published`) for Page and Post
3. Public SSR pages under unified `/app` prefix:
   - `/app/pages/:slug`
   - `/app/posts`
   - `/app/posts/:slug`
   - `/app/categories/:slug`
4. Single admin entry workspace at `/admin/cms` with three tabs:
   - Page
   - Post
   - Category
5. CLI CRUD command groups:
   - `cms page ...`
   - `cms post ...`
   - `cms category ...`
6. Soft delete for all three resources

### 2.2 Out of Scope (Explicitly Deferred)

- Tags
- SEO metadata fields
- Scheduled publishing
- Revision history / versioning
- Trash/recycle-bin UI
- Full-text search
- Import/export
- Multi-language content model

## 3. Key Product Decisions (Locked)

1. Architecture: single official plugin, multi-file layered modules (same style as official `kv-store`)
2. `Post -> Category`: one-to-many (each Post belongs to exactly one Category)
3. Status model: `draft` / `published`
4. Public path prefix: `/app`
5. Content format: Markdown stored in DB
6. Public visibility: only `published` and non-deleted content is visible on `/app/**`
7. Delete strategy: soft delete (`deleted_at`)
8. Category deletion rule: reject delete when non-deleted Posts still reference Category
9. Admin IA: single workspace entry (`/admin/cms`), not three separate global pages

## 4. Architecture Design

## 4.1 Plugin Layout

```text
plugins/official/cms/
├── plugin.toml
├── init.lua
├── lua/
│   ├── infra/
│   │   └── db.lua
│   ├── domain/
│   │   ├── page.lua
│   │   ├── post.lua
│   │   └── category.lua
│   ├── interfaces/
│   │   ├── api.lua
│   │   ├── admin.lua
│   │   └── cli.lua
│   ├── utils/
│   │   ├── markdown.lua
│   │   ├── validate.lua
│   │   └── slug.lua
│   └── bootstrap/
│       └── register.lua
└── web/
    ├── templates/
    │   ├── cms.html
    │   ├── fragments/
    │   │   ├── page_rows.html
    │   │   ├── post_rows.html
    │   │   ├── category_rows.html
    │   │   └── flash.html
    │   └── public/
    │       ├── page_detail.html
    │       ├── post_list.html
    │       ├── post_detail.html
    │       └── category_detail.html
    └── static/
        └── cms.js
```

## 4.2 Layer Responsibilities

- `infra/db.lua`
  - SQL execution wrappers and row mapping
  - Shared filtering defaults (`deleted_at IS NULL`)
- `domain/*.lua`
  - Business rules and validation orchestration
  - Status transition checks
  - Category delete guard enforcement
- `interfaces/api.lua`
  - JSON request parsing and response mapping (`sushi.web.json`)
- `interfaces/admin.lua`
  - Admin partial handlers and template rendering
  - Workspace tab data assembly
- `interfaces/cli.lua`
  - CLI command argument parsing + stdout messaging
- `bootstrap/register.lua`
  - All route/page/command/policy registration in one place

`init.lua` stays a composition root only.

## 5. Data Model Design

## 5.1 Tables

### `cms_pages`

- `id` INTEGER PK
- `title` TEXT NOT NULL
- `slug` TEXT NOT NULL UNIQUE
- `markdown_body` TEXT NOT NULL
- `status` TEXT NOT NULL CHECK (`draft` | `published`)
- `created_at` TEXT NOT NULL
- `updated_at` TEXT NOT NULL
- `deleted_at` TEXT NULL

### `cms_categories`

- `id` INTEGER PK
- `name` TEXT NOT NULL
- `slug` TEXT NOT NULL UNIQUE
- `description` TEXT NULL
- `created_at` TEXT NOT NULL
- `updated_at` TEXT NOT NULL
- `deleted_at` TEXT NULL

### `cms_posts`

- `id` INTEGER PK
- `title` TEXT NOT NULL
- `slug` TEXT NOT NULL UNIQUE
- `excerpt` TEXT NULL
- `markdown_body` TEXT NOT NULL
- `status` TEXT NOT NULL CHECK (`draft` | `published`)
- `category_id` INTEGER NOT NULL FK -> `cms_categories(id)`
- `created_at` TEXT NOT NULL
- `updated_at` TEXT NOT NULL
- `deleted_at` TEXT NULL

## 5.2 Query/Integrity Rules

- Default reads in plugin layer exclude soft-deleted rows.
- Unique checks include soft-deleted records for MVP (slug remains reserved once used).
- Category delete rule:
  - If any non-deleted Post exists with same `category_id`, return conflict (409 semantics).
- Public `/app/**` queries must include:
  - `status = 'published'`
  - `deleted_at IS NULL`

## 6. API / Admin / Frontend / CLI Contracts

## 6.1 Public SSR Routes (`/app`)

- `GET /app/pages/:slug` -> render single page
- `GET /app/posts` -> render post list (optional `?category=<slug>`)
- `GET /app/posts/:slug` -> render post detail
- `GET /app/categories/:slug` -> render category detail + published posts under that category

All public routes:

- Return 404 for missing/draft/deleted content
- Render Markdown to safe HTML before template output

## 6.2 JSON API Routes (`/api/cms`)

### Pages

- `GET /api/cms/pages`
- `POST /api/cms/pages`
- `PUT /api/cms/pages/*` (where `*` is page slug)
- `DELETE /api/cms/pages/*` (soft delete; where `*` is page slug)

### Posts

- `GET /api/cms/posts`
- `POST /api/cms/posts`
- `PUT /api/cms/posts/*` (where `*` is post slug)
- `DELETE /api/cms/posts/*` (soft delete; where `*` is post slug)

### Categories

- `GET /api/cms/categories`
- `POST /api/cms/categories`
- `PUT /api/cms/categories/*` (where `*` is category slug)
- `DELETE /api/cms/categories/*` (soft delete + guarded by relation rule; where `*` is category slug)

Response style:

- Success/failure wrapped via `sushi.web.json(status, payload)`

## 6.3 Admin Routes and Page

- Workspace page:
  - `sushi.web.page("/admin/cms", "plugins/official/cms/cms.html", ...)`
- Partial endpoints (HTMX-style refresh/actions):
  - `GET /admin/partials/cms/pages/table`
  - `POST /admin/partials/cms/pages/upsert`
  - `POST /admin/partials/cms/pages/delete`
  - `GET /admin/partials/cms/posts/table`
  - `POST /admin/partials/cms/posts/upsert`
  - `POST /admin/partials/cms/posts/delete`
  - `GET /admin/partials/cms/categories/table`
  - `POST /admin/partials/cms/categories/upsert`
  - `POST /admin/partials/cms/categories/delete`

Admin UX contract:

- One workspace view with Page/Post/Category tabs
- Partial responses include standard flash fragment for success/error feedback

## 6.4 CLI Commands

- `cms page list|get|create|update|delete`
- `cms post list|get|create|update|delete`
- `cms category list|get|create|update|delete`

MVP CLI behavior:

- Explicit flags for write operations (title/slug/markdown/status/category)
- Human-readable output
- Non-zero exit on validation/business errors

## 7. Policy and Permission Model

`plugin.toml` declares official plugin metadata and policy scopes. Proposed policy keys:

- API:
  - `api.cms.page.*`
  - `api.cms.post.*`
  - `api.cms.category.*`
- Admin:
  - `admin.cms.read`
  - `admin.cms.write`
- CLI:
  - `cli.cms.page.*`
  - `cli.cms.post.*`
  - `cli.cms.category.*`

`bootstrap/register.lua` binds each route/page/command with explicit `policy` option.

## 8. Error Handling Contract

- `400` invalid/missing input
- `404` resource not found (or hidden by delete/draft visibility rules)
- `409` slug conflict or category-delete blocked by existing posts
- `500` unexpected DB/render/runtime failures

Admin partial endpoints convert errors to flash feedback with clear actionable messages.

## 9. Testing Strategy

## 9.1 Core/Loader/Resource Tests

- Ensure plugin resources are plugin-local and loadable:
  - `cargo test -p sushi-core --test template_service -q`
- Add CMS-specific loader/registration characterization tests:
  - page registration
  - API route registration
  - CLI command registration
  - policy key wiring

## 9.2 Admin Integration Tests

- `cargo test -p sushi-admin --test admin_web -q`
- Validate `/admin/cms` access and partial CRUD flow
- Validate flash behavior on:
  - success
  - validation failure
  - category delete conflict

## 9.3 Behavior/Contract Tests (Plugin Level)

- Public routes expose only `published` and non-deleted content
- Soft-deleted entities are not returned by default list/read handlers
- Category delete fails when non-deleted post references category

## 9.4 Pre-Merge Validation

- `cargo test --workspace -q`

## 10. Delivery Plan Boundary

This design intentionally fits one implementation plan cycle for a single plugin subsystem.

Post-MVP enhancements (tags, SEO, scheduling, revisions, search, import/export, i18n) should each be separate follow-up specs and plans.
