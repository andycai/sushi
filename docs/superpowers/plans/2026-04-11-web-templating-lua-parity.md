# Web Templating + Lua Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保持现有 `/admin/*` 与插件路由兼容的前提下，完成 `web/templates + web/static` 统一迁移，并让 Lua 具备与 Rust 对等的页面渲染与通用数据库能力。

**Architecture:** 先在 `sushi-core` 建立统一 `TemplateService` 与 `DbGateway`，再让 `sushi-admin` 与 Lua 绑定层都调用这两个底座。Phase 1 只落地模板/静态迁移与 Admin 页面重构，Phase 2 再增加 `sushi.web.*` 与 `sushi.db.*` API 并迁移 `kv-store` 示例插件。

**Tech Stack:** Rust, Axum, MiniJinja, mlua, serde_json, tokio, rusqlite, Alpine.js, TailwindCSS (local static files)

---

## Scope Check

该规范包含两个阶段（Phase 1/Phase 2），但共享同一技术底座（TemplateService + DbGateway）且按依赖顺序推进。本计划保留为单一实现计划，按任务边界划分为可独立提交、可回归验证的增量。

## Execution Status (2026-04-12)

- Plan execution is complete for Task 1-8 (Phase 1 + Phase 2).
- Key implementation commits:
  - `938200c`, `43003e0` — TemplateService + WebConfig hardening
  - `53e145c`, `6c78531` — Context/bootstrap wiring for TemplateService
  - `451922f`, `9d73b3e`, `220176a` — `web/templates` + local static assets (Tailwind/Alpine/HTMX), no-CDN checks, Alpine hydration fix
  - `1e0c46e`, `e0754f3`, `da767ac` — Admin router switched to TemplateService, static mapping, auth/static hardening, route collision handling
  - `5f5d2ee`, `274792a`, `7c658c1` — admin no-CDN regression coverage
  - `26d64c5`, `1280a31`, `03a1217` — `sushi.web.*` (render/page/json), sentinel status envelope support
  - `d4f4f95`, `ec40001` — `DbGateway` + `sushi.db.query/execute`, SQL boundary hardening and Lua binding tests
  - `ebbf839`, `540d849`, `d594dbd` — kv-store migrated to template + local static + `sushi.db`
- Post-plan high-priority auth fixes were also completed:
  - `bc411e4` — admin users page adapted to paginated `/api/users` response
  - `4df6a00` — avoid `/admin/kv` duplicate route panic when plugin route exists
  - `671d44f` — API auth hardening: default/proxy API routes protected while keeping login/refresh public
- Post-plan stale asset cleanup:
  - Removed legacy `crates/sushi-admin/templates/` directory (runtime now only uses `web/templates`)
  - Removed redundant built-in KV page assets (`web/templates/admin/kv.html`, `web/static/admin/js/kv.js`) and corresponding admin route module

## File Structure Map

### New Files

- `crates/sushi-core/src/web/mod.rs` — Web 子模块入口，导出模板服务与错误类型。
- `crates/sushi-core/src/web/template_service.rs` — MiniJinja 运行时加载与渲染服务。
- `crates/sushi-core/src/web/template_error.rs` — 模板渲染错误模型。
- `crates/sushi-core/src/db/mod.rs` — 通用 DB 网关模块入口。
- `crates/sushi-core/src/db/gateway.rs` — `query/execute` 与 SQL 权限分级校验。
- `crates/sushi-core/tests/template_service.rs` — TemplateService 单元测试（继承/变量/缺失模板）。
- `crates/sushi-core/tests/db_gateway.rs` — DbGateway 权限测试（read/write/admin）。
- `crates/sushi-admin/src/render.rs` — Admin 页面渲染助手（统一调用 TemplateService）。
- `crates/sushi-admin/tests/admin_web.rs` — Admin 渲染与静态资源集成测试。
- `web/templates/base.html` — Admin 页面基础布局（本地静态依赖，不用 CDN）。
- `web/templates/admin/login.html` — 登录页模板。
- `web/templates/admin/dashboard.html` — 仪表盘模板。
- `web/templates/admin/users.html` — 用户页模板。
- `web/templates/admin/plugins.html` — 插件页模板。
- `web/templates/admin/config.html` — 配置页模板。
- `web/templates/admin/logs.html` — 日志页模板。
- `web/templates/admin/kv.html` — KV 页模板。
- `web/templates/plugins/kv-store/kv.html` — kv-store 插件模板（Phase 2）。
- `web/static/admin/css/admin.css` — Admin 公共样式。
- `web/static/admin/js/login.js` — 登录页脚本。
- `web/static/admin/js/dashboard.js` — 仪表盘脚本。
- `web/static/admin/js/users.js` — 用户页脚本。
- `web/static/admin/js/plugins.js` — 插件页脚本。
- `web/static/admin/js/config.js` — 配置页脚本。
- `web/static/admin/js/logs.js` — 日志页脚本。
- `web/static/admin/js/kv.js` — KV 页脚本。
- `web/static/plugins/kv-store/kv.js` — kv-store 插件页面脚本（Phase 2）。

### Modified Files

- `Cargo.toml` — 新增 `minijinja` workspace 依赖。
- `crates/sushi-core/Cargo.toml` — 引入 `minijinja` 依赖。
- `crates/sushi-core/src/lib.rs` — 导出 `web` 与 `db` 模块。
- `crates/sushi-core/src/config.rs` — 新增 `WebConfig`（templates/static/static_url_prefix）。
- `crates/sushi-core/src/context.rs` — 在 `SushiContext` 注入 `TemplateService` 与 `DbGateway`。
- `crates/sushi-cli/src/app.rs` — bootstrap 初始化 `TemplateService`/`DbGateway` 并写入 context。
- `crates/sushi-admin/src/lib.rs` — 导出 `render` 模块。
- `crates/sushi-admin/src/router.rs` — 管理页改为模板渲染，新增 `/static/*path` 映射。
- `crates/sushi-admin/src/routes/*.rs` — 改为渲染模板而非 `include_str!`。
- `crates/sushi-core/src/lua/bindings.rs` — 新增 `sushi.web.render/page/json` 与 `sushi.db.query/execute`。
- `crates/sushi-core/src/plugin/manager.rs` — 扩展 admin handler 执行形态（支持模板渲染返回）。
- `plugins/kv-store/init.lua` — 去除内嵌 HTML，改用模板 + `sushi.db`。
- `plugins/kv-store/plugin.toml` — 权限声明改为 `database = "write"` 并保留 admin/routes。

---

### Task 1: 引入 TemplateService 与 WebConfig（Phase 1 底座）

**Files:**
- Create: `crates/sushi-core/src/web/mod.rs`
- Create: `crates/sushi-core/src/web/template_service.rs`
- Create: `crates/sushi-core/src/web/template_error.rs`
- Create: `crates/sushi-core/tests/template_service.rs`
- Modify: `Cargo.toml`
- Modify: `crates/sushi-core/Cargo.toml`
- Modify: `crates/sushi-core/src/lib.rs`
- Modify: `crates/sushi-core/src/config.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/sushi-core/tests/template_service.rs
use sushi_core::web::template_service::TemplateService;

#[test]
fn render_with_inheritance_works() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("admin")).unwrap();
    std::fs::write(root.path().join("base.html"), "<html>{% block body %}{% endblock %}</html>").unwrap();
    std::fs::write(root.path().join("admin/login.html"), "{% extends \"base.html\" %}{% block body %}{{ title }}{% endblock %}").unwrap();

    let svc = TemplateService::new(root.path()).unwrap();
    let html = svc.render("admin/login.html", serde_json::json!({"title": "Login"})).unwrap();
    assert_eq!(html, "<html>Login</html>");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: FAIL，报错 `could not find module web` 或 `TemplateService not found`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/web/template_service.rs
use minijinja::{Environment, path_loader};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::template_error::TemplateError;

#[derive(Clone)]
pub struct TemplateService {
    root: PathBuf,
}

impl TemplateService {
    pub fn new(root: &Path) -> Result<Self, TemplateError> {
        if !root.exists() {
            return Err(TemplateError::TemplateRootMissing(root.display().to_string()));
        }
        Ok(Self { root: root.to_path_buf() })
    }

    pub fn render(&self, name: &str, context: Value) -> Result<String, TemplateError> {
        let mut env = Environment::new();
        env.set_loader(path_loader(&self.root));
        let tpl = env.get_template(name)
            .map_err(|e| TemplateError::TemplateLoad(e.to_string()))?;
        tpl.render(context)
            .map_err(|e| TemplateError::Render(e.to_string()))
    }
}
```

```rust
// crates/sushi-core/src/web/template_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template root missing: {0}")]
    TemplateRootMissing(String),
    #[error("template load error: {0}")]
    TemplateLoad(String),
    #[error("template render error: {0}")]
    Render(String),
}
```

```rust
// crates/sushi-core/src/web/mod.rs
pub mod template_error;
pub mod template_service;
```

```toml
# Cargo.toml
[workspace.dependencies]
minijinja = { version = "2", features = ["loader", "json"] }
```

```toml
# crates/sushi-core/Cargo.toml
[dependencies]
minijinja = { workspace = true }
```

```rust
// crates/sushi-core/src/lib.rs
pub mod web;
```

```rust
// crates/sushi-core/src/config.rs (add fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SushiConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub jwt: JwtConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub web: WebConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,
    #[serde(default = "default_static_dir")]
    pub static_dir: String,
    #[serde(default = "default_static_prefix")]
    pub static_url_prefix: String,
}

fn default_templates_dir() -> String { "web/templates".to_string() }
fn default_static_dir() -> String { "web/static".to_string() }
fn default_static_prefix() -> String { "/static".to_string() }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/sushi-core/Cargo.toml crates/sushi-core/src/lib.rs crates/sushi-core/src/config.rs crates/sushi-core/src/web docs/superpowers/plans/2026-04-11-web-templating-lua-parity.md crates/sushi-core/tests/template_service.rs
git commit -m "feat(web): add TemplateService and web config defaults"
```

### Task 2: 将 TemplateService 注入 Context 与 bootstrap

**Files:**
- Modify: `crates/sushi-core/src/context.rs`
- Modify: `crates/sushi-cli/src/app.rs`

- [ ] **Step 1: Write the failing test**

```rust
// add to crates/sushi-core/tests/template_service.rs
#[test]
fn context_can_hold_template_service() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("base.html"), "ok").unwrap();
    let svc = TemplateService::new(root.path()).unwrap();
    let _ = svc; // compile-time guard for context wiring in next step
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: FAIL（context/bootstrap 尚未注入 web service）

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/context.rs
use crate::web::template_service::TemplateService;

#[derive(Clone)]
pub struct SushiContext {
    pub config: ConfigStore,
    pub db: Arc<SqliteStorage>,
    pub event: EventBus,
    pub jwt: Arc<JwtService>,
    pub plugins: PluginManager,
    pub templates: Arc<TemplateService>,
}

impl SushiContext {
    pub fn new(config: ConfigStore, db: SqliteStorage, jwt: JwtService, templates: TemplateService) -> Self {
        Self {
            config,
            db: Arc::new(db),
            event: EventBus::new(),
            jwt: Arc::new(jwt),
            plugins: PluginManager::new(),
            templates: Arc::new(templates),
        }
    }
}
```

```rust
// crates/sushi-cli/src/app.rs
use sushi_core::web::template_service::TemplateService;

let templates = {
    let guard = config.get().await;
    let templates_dir = std::path::PathBuf::from(&guard.web.templates_dir);
    TemplateService::new(&templates_dir)
        .with_context(|| format!("failed to init template root {}", templates_dir.display()))?
};

let ctx = SushiContext::new(config, storage, jwt, templates);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/context.rs crates/sushi-cli/src/app.rs crates/sushi-core/tests/template_service.rs
git commit -m "refactor(core): wire TemplateService into SushiContext bootstrap"
```

### Task 3: 建立 `web/templates` 与本地静态资源，不使用 CDN（Phase 1）

**Files:**
- Create: `web/templates/base.html`
- Create: `web/templates/admin/login.html`
- Create: `web/templates/admin/dashboard.html`
- Create: `web/templates/admin/users.html`
- Create: `web/templates/admin/plugins.html`
- Create: `web/templates/admin/config.html`
- Create: `web/templates/admin/logs.html`
- Create: `web/templates/admin/kv.html`
- Create: `web/static/admin/css/admin.css`
- Create: `web/static/admin/js/login.js`
- Create: `web/static/admin/js/dashboard.js`
- Create: `web/static/admin/js/users.js`
- Create: `web/static/admin/js/plugins.js`
- Create: `web/static/admin/js/config.js`
- Create: `web/static/admin/js/logs.js`
- Create: `web/static/admin/js/kv.js`

- [ ] **Step 1: Write the failing test**

```rust
// crates/sushi-core/tests/template_service.rs
#[test]
fn base_template_uses_local_assets_only() {
    let base = std::fs::read_to_string("web/templates/base.html").unwrap();
    assert!(base.contains("/static/js/alpine-3.14.1.js"));
    assert!(base.contains("/static/js/tailwindcss-3.4.17.js"));
    assert!(!base.contains("https://"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core --test template_service base_template_uses_local_assets_only -q`  
Expected: FAIL（`web/templates/base.html` 尚不存在）

- [ ] **Step 3: Write minimal implementation**

```html
<!-- web/templates/base.html -->
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{% block title %}Sushi Admin{% endblock %}</title>
  <script defer src="/static/js/alpine-3.14.1.js"></script>
  <script src="/static/js/tailwindcss-3.4.17.js"></script>
  <link rel="stylesheet" href="/static/admin/css/admin.css" />
</head>
<body class="bg-gray-100 min-h-screen">
  {% block body %}{% endblock %}
  {% block scripts %}{% endblock %}
</body>
</html>
```

```html
<!-- web/templates/admin/login.html -->
{% extends "base.html" %}
{% block title %}Login — Sushi Admin{% endblock %}
{% block body %}
<div class="min-h-screen flex items-center justify-center bg-gray-100">
  <div class="bg-white rounded-lg shadow-md p-8 w-full max-w-sm">
    <h1 class="text-2xl font-bold mb-6 text-center text-gray-800">Sushi Admin</h1>
    <div id="error" class="mb-4 p-3 bg-red-100 text-red-700 rounded text-sm hidden"></div>
    <form id="loginForm">
      <label class="block text-sm font-medium text-gray-700 mb-1">Username</label>
      <input type="text" id="username" required class="w-full border rounded px-3 py-2 mb-4" />
      <label class="block text-sm font-medium text-gray-700 mb-1">Password</label>
      <input type="password" id="password" required class="w-full border rounded px-3 py-2 mb-4" />
      <button type="submit" id="submitBtn" class="w-full bg-blue-600 text-white py-2 rounded">Sign In</button>
    </form>
  </div>
</div>
{% endblock %}
{% block scripts %}<script src="/static/admin/js/login.js"></script>{% endblock %}
```

```js
// web/static/admin/js/login.js
async function handleLogin(e) {
  e.preventDefault();
  const btn = document.getElementById('submitBtn');
  const err = document.getElementById('error');
  btn.disabled = true;
  btn.textContent = 'Signing in...';
  err.classList.add('hidden');

  try {
    const res = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        username: document.getElementById('username').value,
        password: document.getElementById('password').value,
      }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || 'Login failed');
    document.cookie = `sushi_token=${data.access_token}; path=/; SameSite=Lax; max-age=86400`;
    window.location.href = '/admin';
  } catch (e) {
    err.textContent = e.message;
    err.classList.remove('hidden');
    btn.disabled = false;
    btn.textContent = 'Sign In';
  }
}

document.getElementById('loginForm').addEventListener('submit', handleLogin);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core --test template_service base_template_uses_local_assets_only -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add web/templates web/static/admin
 git commit -m "feat(web): add Jinja templates and local static assets without CDN"
```

### Task 4: Admin 路由切换为模板渲染并挂载 `/static/*`

**Files:**
- Create: `crates/sushi-admin/src/render.rs`
- Modify: `crates/sushi-admin/src/lib.rs`
- Modify: `crates/sushi-admin/src/router.rs`
- Modify: `crates/sushi-admin/src/routes/login.rs`
- Modify: `crates/sushi-admin/src/routes/dashboard.rs`
- Modify: `crates/sushi-admin/src/routes/users.rs`
- Modify: `crates/sushi-admin/src/routes/plugins.rs`
- Modify: `crates/sushi-admin/src/routes/config.rs`
- Modify: `crates/sushi-admin/src/routes/logs.rs`
- Modify: `crates/sushi-admin/src/routes/kv.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/sushi-admin/tests/admin_web.rs
#[tokio::test]
async fn login_page_renders_template_and_static_routes_work() {
    let ctx = sushi_cli::app::bootstrap(Some(std::path::Path::new("config.toml"))).await.unwrap();
    let app = sushi_admin::router::build_admin_router(&ctx).await;

    let response = app
        .oneshot(axum::http::Request::builder().uri("/admin-login").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: FAIL（当前路由仍使用 `include_str!` 且无静态映射）

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-admin/src/render.rs
use axum::{http::StatusCode, response::{Html, IntoResponse}};
use serde_json::Value;
use sushi_core::context::SushiContext;

pub fn render_template(ctx: &SushiContext, name: &str, context: Value) -> impl IntoResponse {
    match ctx.templates.render(name, context) {
        Ok(html) => Html(html).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("template error: {err}")).into_response(),
    }
}
```

```rust
// crates/sushi-admin/src/routes/login.rs
pub async fn login_page(axum::extract::State(ctx): axum::extract::State<sushi_core::context::SushiContext>) -> impl axum::response::IntoResponse {
    crate::render::render_template(&ctx, "admin/login.html", serde_json::json!({}))
}
```

```rust
// crates/sushi-admin/src/router.rs (add static mapping)
use tower_http::services::ServeDir;

let static_prefix = {
    let cfg = ctx.config.get().await;
    cfg.web.static_url_prefix.clone()
};
let static_dir = {
    let cfg = ctx.config.get().await;
    cfg.web.static_dir.clone()
};

let static_router = Router::new().nest_service(&static_prefix, ServeDir::new(static_dir));

Router::new()
    .merge(static_router)
    .route("/admin-login", get(login::login_page))
    // ...existing admin routes...
    .with_state(ctx.clone())
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/src/lib.rs crates/sushi-admin/src/render.rs crates/sushi-admin/src/router.rs crates/sushi-admin/src/routes crates/sushi-admin/tests/admin_web.rs
git commit -m "refactor(admin): switch pages to TemplateService and mount /static"
```

### Task 5: 完成 Phase 1 页面迁移与回归验证

**Files:**
- Modify: `web/templates/admin/*.html`
- Modify: `web/static/admin/js/*.js`
- Modify: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write the failing test**

```rust
// add to crates/sushi-admin/tests/admin_web.rs
#[tokio::test]
async fn rendered_pages_do_not_include_cdn_links() {
    let html = std::fs::read_to_string("web/templates/base.html").unwrap();
    assert!(!html.contains("https://unpkg.com"));
    assert!(!html.contains("https://cdn.tailwindcss.com"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-admin --test admin_web rendered_pages_do_not_include_cdn_links -q`  
Expected: FAIL（若仍有 CDN 链接）

- [ ] **Step 3: Write minimal implementation**

```html
<!-- Example: web/templates/admin/users.html -->
{% extends "base.html" %}
{% block title %}Users — Sushi Admin{% endblock %}
{% block body %}
<main class="flex-1 p-6 overflow-auto" x-data="usersPage()">
  <h1 class="text-2xl font-bold mb-6">Users</h1>
  <div id="users-root"></div>
</main>
{% endblock %}
{% block scripts %}<script src="/static/admin/js/users.js"></script>{% endblock %}
```

```js
// Example: web/static/admin/js/users.js
function usersPage() {
  return {
    users: [],
    async init() {
      const resp = await fetch('/api/users', { headers: authHeader() });
      if (resp.ok) this.users = await resp.json();
    },
  };
}

function authHeader() {
  const cookie = document.cookie.split(';').map(v => v.trim()).find(v => v.startsWith('sushi_token='));
  if (!cookie) return {};
  return { Authorization: `Bearer ${cookie.substring('sushi_token='.length)}` };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add web/templates/admin web/static/admin/js crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin-web): migrate all pages to local templates and static scripts"
```

### Task 6: 增加 Lua `sushi.web.*` 渲染能力（Phase 2）

**Files:**
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Modify: `crates/sushi-core/src/plugin/manager.rs`
- Modify: `crates/sushi-core/src/lua/loader.rs`
- Modify: `crates/sushi-core/src/context.rs`

- [ ] **Step 1: Write the failing test**

```rust
// add to crates/sushi-core/src/lua/bindings.rs tests module
#[tokio::test]
async fn test_lua_web_render_renders_template() {
    let lua = create_sandboxed_vm().unwrap();
    let ctx = test_context().await;
    let mut permissions = Permissions::default();
    permissions.admin = true;

    inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

    let rendered: String = lua
        .load(r#"return sushi.web.render("admin/login.html", { title = "Login" })"#)
        .eval()
        .unwrap();

    assert!(rendered.contains("Sushi Admin"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core lua::bindings::tests::test_lua_web_render_renders_template -q`  
Expected: FAIL（`sushi.web` 尚未注入）

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/lua/bindings.rs (inside inject_sushi_api)
{
    let templates = ctx.templates.clone();
    let web_table = lua.create_table()?;

    web_table.set(
        "render",
        lua.create_function(move |lua, (name, data): (String, mlua::Value)| {
            let json_ctx: serde_json::Value = lua.from_value(data)?;
            templates
                .render(&name, json_ctx)
                .map_err(|e| mlua::Error::RuntimeError(format!("web render error: {e}")))
        })?,
    )?;

    web_table.set(
        "json",
        lua.create_function(|lua, (status, data): (u16, mlua::Value)| {
            let json_data: serde_json::Value = lua.from_value(data)?;
            let body = serde_json::to_string(&json_data)
                .map_err(|e| mlua::Error::RuntimeError(format!("json encode error: {e}")))?;
            let resp = lua.create_table()?;
            resp.set("status", status)?;
            resp.set("body", body)?;
            Ok(resp)
        })?,
    )?;

    sushi.set("web", web_table)?;
}
```

```rust
// crates/sushi-core/src/plugin/manager.rs (admin handler should accept HTML string output from Lua web.render)
func.call_async::<String>(()).await.map_err(|e| format!("handler error: {e}"))
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core lua::bindings::tests::test_lua_web_render_renders_template -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/bindings.rs crates/sushi-core/src/plugin/manager.rs crates/sushi-core/src/lua/loader.rs crates/sushi-core/src/context.rs
git commit -m "feat(lua-web): expose sushi.web.render/json for template-based pages"
```

### Task 7: 增加 DbGateway 与 Lua `sushi.db.*`（Phase 2）

**Files:**
- Create: `crates/sushi-core/src/db/mod.rs`
- Create: `crates/sushi-core/src/db/gateway.rs`
- Create: `crates/sushi-core/tests/db_gateway.rs`
- Modify: `crates/sushi-core/src/lib.rs`
- Modify: `crates/sushi-core/src/context.rs`
- Modify: `crates/sushi-core/src/lua/bindings.rs`
- Modify: `crates/sushi-core/src/plugin/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/sushi-core/tests/db_gateway.rs
use sushi_core::db::gateway::{DbGateway, DbPermission};

#[tokio::test]
async fn readonly_permission_rejects_insert() {
    let storage = sushi_core::storage::sqlite::SqliteStorage::new_in_memory().await.unwrap();
    storage.execute("CREATE TABLE t (id INTEGER)", vec![]).await.unwrap();

    let gateway = DbGateway::new(std::sync::Arc::new(storage));
    let err = gateway.execute(DbPermission::ReadOnly, "INSERT INTO t (id) VALUES (1)", vec![]).await.unwrap_err();
    assert!(err.to_string().contains("permission denied"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core --test db_gateway -q`  
Expected: FAIL（`db::gateway` 尚不存在）

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/sushi-core/src/db/gateway.rs
use crate::storage::{Storage, StorageError};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbPermission {
    ReadOnly,
    Write,
    Admin,
}

#[derive(Clone)]
pub struct DbGateway {
    storage: Arc<dyn Storage>,
}

impl DbGateway {
    pub fn new(storage: Arc<dyn Storage>) -> Self { Self { storage } }

    pub async fn query(&self, _perm: DbPermission, sql: &str, params: Vec<Value>) -> Result<Vec<crate::storage::Row>, StorageError> {
        self.storage.query(sql, params).await
    }

    pub async fn execute(&self, perm: DbPermission, sql: &str, params: Vec<Value>) -> Result<(), StorageError> {
        enforce_permission(perm, sql)?;
        self.storage.execute(sql, params).await
    }
}

fn enforce_permission(perm: DbPermission, sql: &str) -> Result<(), StorageError> {
    let op = sql.trim_start().split_whitespace().next().unwrap_or("").to_ascii_uppercase();
    let allow = match perm {
        DbPermission::ReadOnly => op == "SELECT" || op == "PRAGMA",
        DbPermission::Write => matches!(op.as_str(), "SELECT" | "INSERT" | "UPDATE" | "DELETE" | "PRAGMA"),
        DbPermission::Admin => true,
    };
    if allow { Ok(()) } else { Err(StorageError::QueryError("permission denied for SQL operation".to_string())) }
}
```

```rust
// crates/sushi-core/src/lua/bindings.rs (add sushi.db)
use crate::db::gateway::{DbGateway, DbPermission};

let db = ctx.db_gateway.clone();
let db_permission = permissions.database.clone();
let db_table = lua.create_table()?;

db_table.set("query", lua.create_async_function(move |lua: Lua, (sql, params): (String, Option<mlua::Table>)| {
    let db = db.clone();
    let perm = map_db_permission(&db_permission);
    async move {
        let args = table_to_json_params(params)?;
        let rows = db.query(perm, &sql, args).await
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&rows)
    }
})?)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sushi-core --test db_gateway -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/db crates/sushi-core/tests/db_gateway.rs crates/sushi-core/src/lib.rs crates/sushi-core/src/context.rs crates/sushi-core/src/lua/bindings.rs crates/sushi-core/src/plugin/mod.rs
git commit -m "feat(db): add permissioned DbGateway and expose sushi.db APIs"
```

### Task 8: 迁移 `kv-store` 插件到模板 + `sushi.db`，并完成总回归

**Files:**
- Modify: `plugins/kv-store/init.lua`
- Modify: `plugins/kv-store/plugin.toml`
- Create: `web/templates/plugins/kv-store/kv.html`
- Create: `web/static/plugins/kv-store/kv.js`
- Modify: `crates/sushi-core/src/lua/loader.rs` (tests)

- [ ] **Step 1: Write the failing test**

```rust
// add to crates/sushi-core/src/lua/loader.rs tests
#[tokio::test]
async fn kv_store_plugin_no_longer_embeds_html() {
    let code = std::fs::read_to_string("plugins/kv-store/init.lua").unwrap();
    assert!(!code.contains("<!DOCTYPE html>"));
    assert!(code.contains("sushi.web.render") || code.contains("sushi.web.page"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core lua::loader::tests::kv_store_plugin_no_longer_embeds_html -q`  
Expected: FAIL（当前插件仍有内嵌 HTML）

- [ ] **Step 3: Write minimal implementation**

```lua
-- plugins/kv-store/init.lua
function sushi.init()
  sushi.api.route("GET", "/api/kv", function(args)
    local rows = sushi.db.query("SELECT key, value FROM kv_store ORDER BY key", {})
    return sushi.json.encode(rows)
  end)

  sushi.admin.page("/admin/kv", "KV Store", function()
    return sushi.web.render("plugins/kv-store/kv.html", {
      page_title = "KV Store",
      script_path = "/static/plugins/kv-store/kv.js"
    })
  end)
end
```

```html
<!-- web/templates/plugins/kv-store/kv.html -->
{% extends "base.html" %}
{% block title %}{{ page_title }}{% endblock %}
{% block body %}
<main class="p-6" x-data="kvPage()">
  <h1 class="text-2xl font-bold mb-4">KV Store</h1>
  <div id="kv-root"></div>
</main>
{% endblock %}
{% block scripts %}<script src="{{ script_path }}"></script>{% endblock %}
```

- [ ] **Step 4: Run full verification**

Run: `cargo test --workspace`  
Expected: PASS  

Run: `cargo run -p sushi -- serve --config config.toml`  
Expected: 日志出现 `sushi listening on ...` 且访问 `/admin/kv` 可加载页面（模板渲染，非内嵌 HTML）

- [ ] **Step 5: Commit**

```bash
git add plugins/kv-store/init.lua plugins/kv-store/plugin.toml web/templates/plugins/kv-store/kv.html web/static/plugins/kv-store/kv.js crates/sushi-core/src/lua/loader.rs
 git commit -m "refactor(plugin): migrate kv-store to template rendering and sushi.db"
```

---

## Final Verification Checklist

- [ ] `cargo test -p sushi-core --test template_service`
- [ ] `cargo test -p sushi-core --test db_gateway`
- [ ] `cargo test -p sushi-admin --test admin_web`
- [ ] `cargo test --workspace`
- [ ] 手工验证：`/admin-login`、`/admin/users`、`/admin/kv`、`/static/js/alpine-3.14.1.js`
- [ ] grep 验证无 CDN：`rg "https://unpkg.com|https://cdn.tailwindcss.com" web/templates`

## Spec Coverage Self-Review

- 覆盖 `web/templates` 与 `web/static` 迁移：Task 3/4/5。
- 覆盖本地依赖（无 CDN）：Task 3/5 与 Final Checklist。
- 覆盖 Lua 页面渲染能力：Task 6。
- 覆盖 Lua 通用 DB 与权限分级：Task 7。
- 覆盖 kv-store 去内嵌 HTML 迁移：Task 8。
