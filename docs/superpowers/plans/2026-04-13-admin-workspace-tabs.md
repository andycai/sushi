# Admin Workspace Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert admin sidebar navigation from full-page refresh to HTMX partial loading with a multi-tab workspace that deduplicates by path, syncs URL/history, and restores tab state after refresh.

**Architecture:** Keep existing `/admin/*` full-page routes for direct access, and add a parallel workspace partial route (`/admin/workspace/:module`) for right-panel rendering. Introduce a shell-level Alpine workspace controller that owns tabs/history/persistence, while existing module pages remain the source of truth for module UI and APIs.

**Tech Stack:** Rust (Axum), Tera templates, Alpine.js, HTMX, TailwindCSS (local static assets), SQLite-backed RBAC tests.

---

## Scope Check

This scope is one subsystem (admin shell navigation model), not multiple independent products. It can be delivered as one implementation plan with phased module migration:

1. Phase 1: dashboard/users/plugins/kv.
2. Phase 2: roles/permissions/menus/logs/config.

Each phase keeps direct route compatibility and is independently testable.

## File Structure Map

### Create

- `crates/sushi-admin/src/routes/workspace.rs` — module whitelist + workspace partial renderer.
- `web/static/admin/js/workspace.js` — tab state, URL/history sync, localStorage persistence, HTMX content loading.
- `web/templates/admin/workspace/dashboard.html`
- `web/templates/admin/workspace/users.html`
- `web/templates/admin/workspace/plugins.html`
- `web/templates/admin/workspace/kv.html`
- `web/templates/admin/workspace/roles.html`
- `web/templates/admin/workspace/permissions.html`
- `web/templates/admin/workspace/menus.html`
- `web/templates/admin/workspace/logs.html`
- `web/templates/admin/workspace/config.html`
- `web/templates/admin/fragments/dashboard_content.html`
- `web/templates/admin/fragments/users_content.html`
- `web/templates/admin/fragments/plugins_content.html`
- `web/templates/plugins/kv-store/fragments/kv_content.html`
- `web/templates/admin/fragments/roles_content.html`
- `web/templates/admin/fragments/permissions_content.html`
- `web/templates/admin/fragments/menus_content.html`
- `web/templates/admin/fragments/logs_content.html`
- `web/templates/admin/fragments/config_content.html`

### Modify

- `crates/sushi-admin/src/routes/mod.rs`
- `crates/sushi-admin/src/router.rs`
- `crates/sushi-admin/src/routes/users.rs`
- `crates/sushi-admin/src/routes/roles.rs`
- `crates/sushi-admin/src/routes/permissions.rs`
- `crates/sushi-admin/src/routes/menu.rs`
- `crates/sushi-admin/src/routes/logs.rs`
- `crates/sushi-admin/src/routes/config.rs`
- `crates/sushi-admin/src/routes/dashboard.rs`
- `crates/sushi-admin/src/routes/plugins.rs`
- `web/templates/base.html`
- `web/templates/admin/dashboard.html`
- `web/templates/admin/users.html`
- `web/templates/admin/plugins.html`
- `web/templates/plugins/kv-store/kv.html`
- `web/templates/admin/roles.html`
- `web/templates/admin/permissions.html`
- `web/templates/admin/menus.html`
- `web/templates/admin/logs.html`
- `web/templates/admin/config.html`
- `web/static/admin/js/menu.js`
- `web/static/admin/js/ui-kit.js`
- `web/static/admin/css/admin.css`

### Test

- `crates/sushi-admin/tests/admin_web.rs` — workspace route behavior, permission mapping coverage, template shell coverage.

---

### Task 1: Add Workspace Route Contract and RBAC Mapping

**Files:**
- Create: `crates/sushi-admin/src/routes/workspace.rs`
- Modify: `crates/sushi-admin/src/routes/mod.rs`
- Modify: `crates/sushi-admin/src/router.rs`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Write failing integration tests for workspace endpoint**

```rust
#[tokio::test]
async fn workspace_partial_unknown_module_returns_404() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/not-a-module")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workspace_partial_users_is_accessible_for_admin() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run targeted tests to confirm failure**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_unknown_module_returns_404 workspace_partial_users_is_accessible_for_admin -- --exact`  
Expected: FAIL with `404/route not found` for `/admin/workspace/*` because route is not registered.

- [ ] **Step 3: Implement workspace route + permission mapping**

```rust
// crates/sushi-admin/src/routes/workspace.rs
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use sushi_core::context::SushiContext;

pub async fn workspace_partial(
    Path(module): Path<String>,
    State(ctx): State<SushiContext>,
) -> impl IntoResponse {
    match module.as_str() {
        "dashboard" => crate::render::render_template(&ctx, "admin/workspace/dashboard.html").await,
        "users" => super::users::users_workspace_partial(State(ctx)).await.into_response(),
        "plugins" => crate::render::render_template(&ctx, "admin/workspace/plugins.html").await,
        "kv" => crate::render::render_template(&ctx, "admin/workspace/kv.html").await,
        "roles" => crate::render::render_template(&ctx, "admin/workspace/roles.html").await,
        "permissions" => crate::render::render_template(&ctx, "admin/workspace/permissions.html").await,
        "menus" => crate::render::render_template(&ctx, "admin/workspace/menus.html").await,
        "logs" => crate::render::render_template(&ctx, "admin/workspace/logs.html").await,
        "config" => crate::render::render_template(&ctx, "admin/workspace/config.html").await,
        _ => (axum::http::StatusCode::NOT_FOUND, "unknown workspace module").into_response(),
    }
}
```

```rust
// crates/sushi-admin/src/router.rs (add route + permission branch)
.route("/admin/workspace/{module}", get(workspace::workspace_partial))
```

```rust
// crates/sushi-admin/src/router.rs (inside required_admin_permission)
if method == "GET" && path.starts_with("/admin/workspace/") {
    return match path.trim_start_matches("/admin/workspace/") {
        "dashboard" => Some("dashboard.view"),
        "users" => Some("users.view"),
        "plugins" => Some("plugins.view"),
        "kv" => Some("kv.manage"),
        "roles" => Some("roles.view"),
        "permissions" => Some("permissions.view"),
        "menus" => Some("menus.view"),
        "logs" => Some("logs.view"),
        "config" => Some("config.view"),
        _ => None,
    };
}
```

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_unknown_module_returns_404 workspace_partial_users_is_accessible_for_admin -- --exact`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/src/routes/workspace.rs crates/sushi-admin/src/routes/mod.rs crates/sushi-admin/src/router.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): add workspace partial route and RBAC mapping"
```

### Task 2: Build Workspace Shell (Tabs + Panels) in Base Layout

**Files:**
- Modify: `web/templates/base.html`
- Create: `web/static/admin/js/workspace.js`
- Modify: `web/static/admin/css/admin.css`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing test for workspace shell markup**

```rust
#[tokio::test]
async fn dashboard_page_includes_workspace_shell() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("id=\"admin-workspace-tabs\""), "html: {html}");
    assert!(html.contains("id=\"admin-workspace-panels\""), "html: {html}");
}
```

- [ ] **Step 2: Run test and confirm it fails**

Run: `cargo test -p sushi-admin --test admin_web dashboard_page_includes_workspace_shell -- --exact`  
Expected: FAIL because base layout does not render workspace tabs/panels yet.

- [ ] **Step 3: Implement shell structure + runtime bootstrap**

```html
<!-- web/templates/base.html (inside main area) -->
<main id="admin-content" class="admin-main" x-data="adminWorkspaceShell()" x-init="init()">
  <section id="admin-workspace-tabs" class="admin-tabs">
    <template x-for="tab in tabs" :key="tab.path">
      <button type="button" class="admin-tab" :class="{ 'active': tab.path === activePath }" @click="activateTab(tab.path)">
        <span x-text="tab.label"></span>
        <span x-show="tab.closable" class="admin-tab-close" @click.stop="closeTab(tab.path)">x</span>
      </button>
    </template>
  </section>
  <section id="admin-workspace-panels">
    <div id="admin-workspace-active-panel"></div>
  </section>
  {% block content %}{% endblock %}
</main>
```

```js
// web/static/admin/js/workspace.js
(() => {
  const STORAGE_KEY = 'admin.workspace.v1';
  const DASHBOARD = { path: '/admin/', module: 'dashboard', label: 'Dashboard', closable: false };

  window.adminWorkspaceShell = function adminWorkspaceShell() {
    return {
      tabs: [DASHBOARD],
      activePath: '/admin/',
      init() { this.restoreWorkspace(); this.activateTab(window.location.pathname || '/admin/', false); },
      persistWorkspace() { localStorage.setItem(STORAGE_KEY, JSON.stringify({ tabs: this.tabs, activePath: this.activePath })); },
      restoreWorkspace() { /* restore + dedupe + fallback */ },
      activateTab(path, pushUrl = true) { /* set active + load panel + pushState */ },
      closeTab(path) { /* dashboard pinned; close others */ },
      openFromMenu(item) { this.openTab(item.route, item.label || item.route, this.moduleFromPath(item.route)); },
      openTab(path, label, module) { /* dedupe by path + activate */ },
      loadTabContent(path, module) { /* htmx.ajax GET /admin/workspace/{module} target panel */ },
      moduleFromPath(path) { /* /admin/users => users */ },
    };
  };
})();
```

- [ ] **Step 4: Re-run the shell test**

Run: `cargo test -p sushi-admin --test admin_web dashboard_page_includes_workspace_shell -- --exact`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/templates/base.html web/static/admin/js/workspace.js web/static/admin/css/admin.css crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): add workspace tab shell with Alpine runtime"
```

### Task 3: Integrate Sidebar Clicks with Workspace (No Full Reload)

**Files:**
- Modify: `web/static/admin/js/menu.js`
- Modify: `web/templates/base.html`
- Modify: `web/static/admin/js/ui-kit.js`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing test that workspace runtime is loaded globally**

```rust
#[tokio::test]
async fn admin_layout_loads_workspace_runtime_script() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("/admin/js/workspace.js"), "html: {html}");
}
```

- [ ] **Step 2: Run test to confirm failure**

Run: `cargo test -p sushi-admin --test admin_web admin_layout_loads_workspace_runtime_script -- --exact`  
Expected: FAIL because base scripts do not include `workspace.js`.

- [ ] **Step 3: Wire menu click interception + runtime handoff**

```js
// web/static/admin/js/menu.js
handleMenuClick(event, item) {
  if (this.hasChildren(item)) {
    event?.preventDefault?.();
    this.toggleExpand(item.id);
    return;
  }
  if (item?.route && window.adminWorkspaceBridge?.openFromMenu) {
    event?.preventDefault?.();
    window.adminWorkspaceBridge.openFromMenu(item);
  }
}
```

```js
// web/static/admin/js/workspace.js (bridge registration)
window.adminWorkspaceBridge = {
  openFromMenu: (item) => {
    const root = document.querySelector('#admin-content');
    if (root && root.__x) {
      root.__x.$data.openFromMenu(item);
    } else {
      window.location.href = item.route;
    }
  }
};
```

```html
<!-- web/templates/base.html -->
{% block scripts %}
  <script src="{{ static_prefix }}/admin/js/menu.js"></script>
  <script src="{{ static_prefix }}/admin/js/workspace.js"></script>
{% endblock %}
```

- [ ] **Step 4: Re-run targeted test**

Run: `cargo test -p sushi-admin --test admin_web admin_layout_loads_workspace_runtime_script -- --exact`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/static/admin/js/menu.js web/static/admin/js/workspace.js web/templates/base.html web/static/admin/js/ui-kit.js crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): route sidebar navigation into workspace tabs"
```

### Task 4: Deliver Phase 1 Workspace Partials (dashboard/users/plugins/kv)

**Files:**
- Create: `web/templates/admin/fragments/dashboard_content.html`
- Create: `web/templates/admin/fragments/users_content.html`
- Create: `web/templates/admin/fragments/plugins_content.html`
- Create: `web/templates/plugins/kv-store/fragments/kv_content.html`
- Create: `web/templates/admin/workspace/dashboard.html`
- Create: `web/templates/admin/workspace/users.html`
- Create: `web/templates/admin/workspace/plugins.html`
- Create: `web/templates/admin/workspace/kv.html`
- Modify: `web/templates/admin/dashboard.html`
- Modify: `web/templates/admin/users.html`
- Modify: `web/templates/admin/plugins.html`
- Modify: `web/templates/plugins/kv-store/kv.html`
- Modify: `crates/sushi-admin/src/routes/users.rs`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing tests for phase-1 workspace partial payloads**

```rust
#[tokio::test]
async fn workspace_partial_users_contains_users_table_marker() {
    let app = build_app(None).await;
    let token = admin_bearer_token();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-admin-workspace-module=\"users\""), "html: {html}");
    assert!(html.contains("id=\"users-table-body\""), "html: {html}");
}
```

- [ ] **Step 2: Run the phase-1 test and confirm failure**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_users_contains_users_table_marker -- --exact`  
Expected: FAIL because workspace templates/fragments are not wired.

- [ ] **Step 3: Extract content fragments and create workspace wrappers**

```html
<!-- web/templates/admin/workspace/users.html -->
<section data-admin-workspace-module="users" x-data="usersPage()">
  {% include "admin/fragments/users_content.html" %}
</section>
```

```html
<!-- web/templates/admin/users.html -->
{% extends "base.html" %}
{% set active_section = "users" %}
{% block content %}
  {% include "admin/fragments/users_content.html" %}
{% endblock %}
```

```rust
// crates/sushi-admin/src/routes/users.rs
pub async fn users_workspace_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let roles = repo.list_roles().await.unwrap_or_default();
    crate::render::render_template_with_context(
        &ctx,
        "admin/workspace/users.html",
        serde_json::json!({ "roles": roles }),
    )
    .await
}
```

- [ ] **Step 4: Re-run targeted phase-1 test**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_users_contains_users_table_marker -- --exact`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/templates/admin/fragments/dashboard_content.html web/templates/admin/fragments/users_content.html web/templates/admin/fragments/plugins_content.html web/templates/plugins/kv-store/fragments/kv_content.html web/templates/admin/workspace/dashboard.html web/templates/admin/workspace/users.html web/templates/admin/workspace/plugins.html web/templates/admin/workspace/kv.html web/templates/admin/dashboard.html web/templates/admin/users.html web/templates/admin/plugins.html web/templates/plugins/kv-store/kv.html crates/sushi-admin/src/routes/users.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): add workspace partial templates for high-frequency modules"
```

### Task 5: Deliver Phase 2 Workspace Partials (roles/permissions/menus/logs/config)

**Files:**
- Create: `web/templates/admin/fragments/roles_content.html`
- Create: `web/templates/admin/fragments/permissions_content.html`
- Create: `web/templates/admin/fragments/menus_content.html`
- Create: `web/templates/admin/fragments/logs_content.html`
- Create: `web/templates/admin/fragments/config_content.html`
- Create: `web/templates/admin/workspace/roles.html`
- Create: `web/templates/admin/workspace/permissions.html`
- Create: `web/templates/admin/workspace/menus.html`
- Create: `web/templates/admin/workspace/logs.html`
- Create: `web/templates/admin/workspace/config.html`
- Modify: `web/templates/admin/roles.html`
- Modify: `web/templates/admin/permissions.html`
- Modify: `web/templates/admin/menus.html`
- Modify: `web/templates/admin/logs.html`
- Modify: `web/templates/admin/config.html`
- Modify: `crates/sushi-admin/src/routes/roles.rs`
- Modify: `crates/sushi-admin/src/routes/permissions.rs`
- Modify: `crates/sushi-admin/src/routes/menu.rs`
- Modify: `crates/sushi-admin/src/routes/logs.rs`
- Modify: `crates/sushi-admin/src/routes/config.rs`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing test for phase-2 workspace module accessibility**

```rust
#[tokio::test]
async fn workspace_partial_roles_contains_role_table_marker() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/roles")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-admin-workspace-module=\"roles\""), "html: {html}");
    assert!(html.contains("id=\"roles-table-body\""), "html: {html}");
}
```

- [ ] **Step 2: Run targeted phase-2 test and confirm failure**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_roles_contains_role_table_marker -- --exact`  
Expected: FAIL because phase-2 workspace templates are missing.

- [ ] **Step 3: Add phase-2 fragments/templates and route render support**

```html
<!-- web/templates/admin/workspace/roles.html -->
<section data-admin-workspace-module="roles" x-data="rolesPage()">
  {% include "admin/fragments/roles_content.html" %}
</section>
```

```rust
// crates/sushi-admin/src/routes/workspace.rs (module arms retained)
"roles" => crate::render::render_template(&ctx, "admin/workspace/roles.html").await,
"permissions" => crate::render::render_template(&ctx, "admin/workspace/permissions.html").await,
"menus" => crate::render::render_template(&ctx, "admin/workspace/menus.html").await,
"logs" => crate::render::render_template(&ctx, "admin/workspace/logs.html").await,
"config" => crate::render::render_template(&ctx, "admin/workspace/config.html").await,
```

- [ ] **Step 4: Re-run targeted phase-2 test**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_roles_contains_role_table_marker -- --exact`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/templates/admin/fragments/roles_content.html web/templates/admin/fragments/permissions_content.html web/templates/admin/fragments/menus_content.html web/templates/admin/fragments/logs_content.html web/templates/admin/fragments/config_content.html web/templates/admin/workspace/roles.html web/templates/admin/workspace/permissions.html web/templates/admin/workspace/menus.html web/templates/admin/workspace/logs.html web/templates/admin/workspace/config.html web/templates/admin/roles.html web/templates/admin/permissions.html web/templates/admin/menus.html web/templates/admin/logs.html web/templates/admin/config.html crates/sushi-admin/src/routes/roles.rs crates/sushi-admin/src/routes/permissions.rs crates/sushi-admin/src/routes/menu.rs crates/sushi-admin/src/routes/logs.rs crates/sushi-admin/src/routes/config.rs crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): complete workspace partial coverage for remaining modules"
```

### Task 6: Finalize URL/History/Persistence + Regression Verification + Wiki Update

**Files:**
- Modify: `web/static/admin/js/workspace.js`
- Modify: `web/static/admin/css/admin.css`
- Modify: `docs/wiki/architecture/admin-panel.md`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing regression tests for workspace route RBAC + shell assets**

```rust
#[tokio::test]
async fn workspace_partial_requires_authentication() {
    let app = build_app(None).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/workspace/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
}
```

```rust
#[tokio::test]
async fn workspace_shell_uses_local_assets_only() {
    let app = build_app(None).await;
    let token = admin_bearer_token();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(!html.contains("https://"), "html: {html}");
    assert!(html.contains("/static/admin/js/workspace.js"), "html: {html}");
}
```

- [ ] **Step 2: Run targeted regression tests and confirm initial failures**

Run: `cargo test -p sushi-admin --test admin_web workspace_partial_requires_authentication workspace_shell_uses_local_assets_only -- --exact`  
Expected: FAIL before final shell and route hardening are complete.

- [ ] **Step 3: Harden runtime behavior and update wiki**

```js
// web/static/admin/js/workspace.js (required behaviors)
window.addEventListener('popstate', () => {
  const path = window.location.pathname || '/admin/';
  this.activateTab(path, false);
});

closeTab(path) {
  if (path === '/admin/') return;
  // remove + fallback to last visited tab or dashboard
}

persistWorkspace() {
  const payload = { tabs: this.tabs, activePath: this.activePath, recentStack: this.recentStack };
  localStorage.setItem('admin.workspace.v1', JSON.stringify(payload));
}
```

```md
<!-- docs/wiki/architecture/admin-panel.md -->
## Workspace Navigation Model

- Sidebar clicks open/activate workspace tabs instead of full-page navigation.
- URL is synchronized to active tab using History API.
- Dashboard tab (`/admin/`) is pinned and cannot be closed.
- Workspace state is persisted in `localStorage` under `admin.workspace.v1`.
- Module content is loaded from `GET /admin/workspace/:module` via HTMX.
```

- [ ] **Step 4: Run full verification**

Run: `cargo test -p sushi-admin --tests`  
Expected: PASS (all admin integration tests pass).

Run: `cargo check -p sushi-cli`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/static/admin/js/workspace.js web/static/admin/css/admin.css docs/wiki/architecture/admin-panel.md crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(admin): finalize workspace history/persistence and document architecture"
```

---

## Self-Review

1. **Spec coverage check**
   - HTMX partial workspace load: covered by Tasks 1, 4, 5.
   - Tab model (dedupe + pinned dashboard + close behavior): covered by Tasks 2, 3, 6.
   - URL/history sync: covered by Task 6.
   - localStorage restore: covered by Tasks 2 and 6.
   - RBAC mapping: covered by Task 1 and regression tests in Task 6.
   - Direct `/admin/*` compatibility: preserved through fragment extraction in Tasks 4 and 5.

2. **Placeholder scan**
   - No `TODO/TBD/implement later` placeholders remain.
   - Every task includes concrete file paths, code snippets, and commands.

3. **Type/signature consistency**
   - Workspace handler function name is consistently `workspace_partial`.
   - Alpine shell factory name is consistently `adminWorkspaceShell`.
   - localStorage key is consistently `admin.workspace.v1`.

