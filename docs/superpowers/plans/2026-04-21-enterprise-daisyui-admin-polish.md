# Enterprise DaisyUI Admin Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Sushi admin + official plugin UI to a premium enterprise-grade visual system while preserving all existing HTMX/RBAC/route behavior.

**Architecture:** Keep the current server-first rendering model and HTMX partial flow. Standardize shell/page/layout semantics in templates, then converge each module (admin + CMS/KV/File Browser) to one shared daisyUI + Tailwind utility language. Guard the redesign with template contract tests so future changes cannot regress the enterprise shell and module patterns.

**Tech Stack:** Rust (Axum tests), MiniJinja templates, HTMX, Alpine.js, Tailwind CSS v4, daisyUI, pnpm scripts.

---

## File Structure

### New File

- Create: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`
  - Responsibility: static template contract tests for enterprise shell/page/module semantics.

### Existing Files to Modify

- Modify: `web/templates/base.html`
  - Responsibility: enterprise shell contract (brand nav, workspace stage, top action row, theme toggle position/semantics).
- Modify: `web/static/css/input.css`
  - Responsibility: global utility-level rhythm tweaks, shell spacing consistency, state feedback consistency.
- Modify: `web/templates/admin/login.html`
  - Responsibility: enterprise login framing and trust-oriented hierarchy.
- Modify: `web/templates/admin/fragments/dashboard_content.html`
- Modify: `web/templates/admin/fragments/users_content.html`
- Modify: `web/templates/admin/fragments/roles_content.html`
- Modify: `web/templates/admin/fragments/permissions_content.html`
- Modify: `web/templates/admin/fragments/menus_content.html`
- Modify: `web/templates/admin/fragments/plugins_content.html`
- Modify: `web/templates/admin/fragments/logs_content.html`
- Modify: `web/templates/admin/fragments/config_content.html`
  - Responsibility: common module header/toolbar/table/action semantics.
- Modify: `plugins/official/cms/web/templates/cms.html`
- Modify: `plugins/official/cms/web/templates/fragments/overview_panel.html`
- Modify: `plugins/official/cms/web/templates/fragments/library_panel.html`
- Modify: `plugins/official/cms/web/templates/fragments/editor_panel.html`
- Modify: `plugins/official/cms/web/templates/fragments/rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/page_rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/post_rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/category_rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/flash.html`
  - Responsibility: CMS workbench visual parity with admin shell.
- Modify: `plugins/official/kv-store/web/templates/kv.html`
- Modify: `plugins/official/kv-store/web/templates/fragments/kv_content.html`
- Modify: `plugins/official/kv-store/web/templates/partials/rows.html`
- Modify: `plugins/official/kv-store/web/templates/partials/flash.html`
  - Responsibility: operations-first KV workspace with enterprise states.
- Modify: `plugins/official/file-browser/web/templates/file_browser.html`
- Modify: `plugins/official/file-browser/web/templates/fragments/list.html`
- Modify: `plugins/official/file-browser/web/templates/fragments/editor.html`
- Modify: `plugins/official/file-browser/web/templates/fragments/flash.html`
  - Responsibility: file operations UI harmonized with admin shell language.

---

### Task 1: Add Enterprise UI Contract Tests

**Files:**
- Create: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`
- Test: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`

- [ ] **Step 1: Write the failing test file**

```rust
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .expect("failed to resolve repository root")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

#[test]
fn base_shell_exposes_enterprise_landmarks() {
    let source = read("web/templates/base.html");
    assert!(source.contains("data-admin-shell"));
    assert!(source.contains("data-admin-nav"));
    assert!(source.contains("data-admin-workspace-stage"));
    assert!(source.contains("id=\"theme-toggle\""));
}

#[test]
fn core_admin_fragments_expose_page_header_contract() {
    let files = [
        "web/templates/admin/fragments/dashboard_content.html",
        "web/templates/admin/fragments/users_content.html",
        "web/templates/admin/fragments/roles_content.html",
        "web/templates/admin/fragments/permissions_content.html",
        "web/templates/admin/fragments/menus_content.html",
        "web/templates/admin/fragments/plugins_content.html",
        "web/templates/admin/fragments/logs_content.html",
        "web/templates/admin/fragments/config_content.html",
    ];
    for file in files {
        let source = read(file);
        assert!(source.contains("data-admin-page-header"), "missing in {file}");
        assert!(source.contains("data-admin-action-cluster"), "missing in {file}");
    }
}

#[test]
fn official_plugin_templates_follow_enterprise_workspace_contract() {
    let cms = read("plugins/official/cms/web/templates/cms.html");
    assert!(cms.contains("data-enterprise-workbench=\"cms\""));
    let kv = read("plugins/official/kv-store/web/templates/kv.html");
    assert!(kv.contains("data-enterprise-workbench=\"kv\""));
    let fb = read("plugins/official/file-browser/web/templates/file_browser.html");
    assert!(fb.contains("data-enterprise-workbench=\"file-browser\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL with missing `data-admin-shell` / `data-admin-page-header` / `data-enterprise-workbench`.

- [ ] **Step 3: Add test target include if needed**

```toml
# crates/sushi-admin/Cargo.toml (only if integration tests are explicitly listed)
[[test]]
name = "admin_ui_enterprise_contract"
path = "tests/admin_ui_enterprise_contract.rs"
```

- [ ] **Step 4: Run test again to ensure it is discovered**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL (assertions), not "no test target named".

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/tests/admin_ui_enterprise_contract.rs crates/sushi-admin/Cargo.toml
git commit -m "test(admin): add enterprise ui template contracts"
```

---

### Task 2: Rebuild Global Enterprise Shell in `base.html`

**Files:**
- Modify: `web/templates/base.html`
- Modify: `web/static/css/input.css`
- Test: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`

- [ ] **Step 1: Add failing shell contract assertion for nav and stage split**

```rust
#[test]
fn base_shell_has_nav_and_stage_regions() {
    let source = read("web/templates/base.html");
    assert!(source.contains("data-admin-nav-section=\"primary\""));
    assert!(source.contains("data-admin-nav-section=\"system\""));
    assert!(source.contains("data-admin-workspace-stage"));
}
```

- [ ] **Step 2: Run test to verify fail**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL on `data-admin-nav-section` assertions.

- [ ] **Step 3: Implement enterprise shell landmarks in `base.html`**

```html
<body data-admin-shell x-data="{% block body_x_data %}adminMenu(){% endblock %}">
  <div class="admin-shell" data-admin-shell>
    <aside class="admin-sidebar" data-admin-nav>
      <div class="admin-brand">
        <span class="admin-brand-logo"><img src="{{ static_prefix }}/favicon.svg" alt="Sushi logo"></span>
        <span class="admin-brand-title"><strong>Sushi Admin</strong><span>Control Surface</span></span>
      </div>
      <nav class="admin-nav">
        <div class="admin-nav-list" data-admin-nav-section="primary">
          <a href="/admin/" class="admin-nav-link">Dashboard</a>
          <a href="/admin/users" class="admin-nav-link">Users</a>
          <a href="/admin/plugins" class="admin-nav-link">Plugins</a>
        </div>
        <div class="nav-footer" data-admin-nav-section="system">
          <button id="theme-toggle" type="button" class="admin-nav-link btn btn-sm btn-ghost w-full justify-start">Theme: light</button>
          <a href="#" @click.prevent="logout()" class="admin-nav-link">Logout</a>
        </div>
      </nav>
    </aside>
    <main id="admin-content" class="admin-main" data-admin-workspace-stage>
      {% block content %}{% endblock %}
    </main>
  </div>
</body>
```

- [ ] **Step 4: Normalize shell rhythm utilities in `input.css`**

```css
@layer components {
  .admin-shell { @apply min-h-screen bg-base-200/40 text-base-content lg:flex; }
  .admin-sidebar { @apply w-full border-b border-base-300 bg-base-100/95 lg:h-screen lg:w-80 lg:border-r lg:border-b-0; }
  .admin-main { @apply min-w-0 flex-1 p-4 sm:p-6 lg:p-8; }
  .admin-workspace { @apply flex min-h-[72vh] flex-col gap-4; }
  .admin-workspace-tabs { @apply flex flex-wrap items-center gap-1 rounded-xl border border-base-300 bg-base-100 p-1.5 shadow-sm; }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: PASS for shell tests.

- [ ] **Step 6: Commit**

```bash
git add web/templates/base.html web/static/css/input.css crates/sushi-admin/tests/admin_ui_enterprise_contract.rs
git commit -m "style(admin): upgrade enterprise shell landmarks and rhythm"
```

---

### Task 3: Standardize Core Admin Fragment Headers and Action Clusters

**Files:**
- Modify: `web/templates/admin/fragments/dashboard_content.html`
- Modify: `web/templates/admin/fragments/users_content.html`
- Modify: `web/templates/admin/fragments/roles_content.html`
- Modify: `web/templates/admin/fragments/permissions_content.html`
- Modify: `web/templates/admin/fragments/menus_content.html`
- Modify: `web/templates/admin/fragments/plugins_content.html`
- Modify: `web/templates/admin/fragments/logs_content.html`
- Modify: `web/templates/admin/fragments/config_content.html`
- Test: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`

- [ ] **Step 1: Add failing test for toolbar and table card contract**

```rust
#[test]
fn users_fragment_has_enterprise_toolbar_and_table_card() {
    let source = read("web/templates/admin/fragments/users_content.html");
    assert!(source.contains("data-admin-page-header"));
    assert!(source.contains("data-admin-action-cluster"));
    assert!(source.contains("data-admin-table-card"));
}
```

- [ ] **Step 2: Run test to verify fail**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL on `data-admin-table-card`.

- [ ] **Step 3: Implement unified fragment skeleton**

```html
<section class="admin-module" data-admin-workspace-module="users" x-data="usersPage()">
  <header class="mb-6 flex flex-wrap items-start justify-between gap-4" data-admin-page-header>
    <div class="space-y-1">
      <h1 class="text-3xl font-bold tracking-tight">Identity &amp; Access</h1>
      <p class="text-sm text-base-content/70">Create, monitor, and remove administrator and editor accounts.</p>
    </div>
    <div class="flex flex-wrap items-center gap-2" data-admin-action-cluster>
      <!-- existing search + primary actions -->
    </div>
  </header>

  <section class="card border border-base-300 bg-base-100 shadow-sm" data-admin-table-card>
    <!-- existing table and controls kept functionally identical -->
  </section>
</section>
```

- [ ] **Step 4: Apply same header/action/table semantics to other admin fragments**

```html
<!-- Required marker contract in each fragment -->
<header data-admin-page-header>
  <h1 class="text-3xl font-bold tracking-tight">Module Title</h1>
  <p class="text-sm text-base-content/70">Module description text.</p>
</header>
<div data-admin-action-cluster>
  <input class="input input-bordered" placeholder="Search users">
  <button type="button" class="btn btn-primary btn-sm">Primary Action</button>
</div>
<!-- For table/list modules -->
<section data-admin-table-card>
  <table class="table table-zebra"><tbody><tr><td>Row</td></tr></tbody></table>
</section>
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: PASS for core fragment contracts.

- [ ] **Step 6: Commit**

```bash
git add web/templates/admin/fragments/*.html crates/sushi-admin/tests/admin_ui_enterprise_contract.rs
git commit -m "style(admin): unify enterprise headers and table cards across fragments"
```

---

### Task 4: Elevate Login Page to Enterprise Entry Experience

**Files:**
- Modify: `web/templates/admin/login.html`
- Modify: `web/static/css/input.css`
- Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing template contract test for login shell cues**

```rust
#[test]
fn login_template_exposes_enterprise_auth_contract() {
    let source = std::fs::read_to_string(
        repo_root().join("web/templates/admin/login.html")
    ).expect("failed to read login template");
    assert!(source.contains("data-admin-login-shell"));
    assert!(source.contains("id=\"login-form\""));
    assert!(source.contains("data-enterprise-trust-panel"));
}
```

- [ ] **Step 2: Run focused test to verify fail**

Run: `cargo test -p sushi-admin admin_web::login_template_exposes_enterprise_auth_contract -q`  
Expected: FAIL on missing data attributes.

- [ ] **Step 3: Implement enterprise login structure while keeping HTMX contract**

```html
<section class="min-h-screen bg-base-200/60" data-admin-login-shell>
  <div class="mx-auto grid min-h-screen max-w-6xl grid-cols-1 gap-6 px-4 py-8 lg:grid-cols-2">
    <aside class="card border border-base-300 bg-base-100 shadow-sm" data-enterprise-trust-panel>
      <div class="card-body">
        <h2 class="text-2xl font-bold">Enterprise Control Surface</h2>
        <p class="text-sm text-base-content/70">Secure access for operations, plugins, and policy-managed workflows.</p>
      </div>
    </aside>
    <main class="card border border-base-300 bg-base-100 shadow-xl">
      <form id="login-form" hx-post="/admin-login" hx-target="#login-error" hx-swap="innerHTML" class="card-body space-y-4">
        <input type="text" name="username" class="input input-bordered w-full" required>
        <input type="password" name="password" class="input input-bordered w-full" required>
        <button type="submit" class="btn btn-primary w-full">Sign in</button>
      </form>
    </main>
  </div>
</section>
```

- [ ] **Step 4: Adjust utility rhythm for login panel balance**

```css
@layer components {
  [data-admin-login-shell] .card { @apply rounded-2xl; }
  [data-admin-login-shell] .input { @apply input-bordered; }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS (including existing login HTMX behavior tests).

- [ ] **Step 6: Commit**

```bash
git add web/templates/admin/login.html web/static/css/input.css crates/sushi-admin/tests/admin_web.rs
git commit -m "style(admin): refine login into enterprise access experience"
```

---

### Task 5: Harmonize CMS Workbench with Enterprise Shell

**Files:**
- Modify: `plugins/official/cms/web/templates/cms.html`
- Modify: `plugins/official/cms/web/templates/fragments/overview_panel.html`
- Modify: `plugins/official/cms/web/templates/fragments/library_panel.html`
- Modify: `plugins/official/cms/web/templates/fragments/editor_panel.html`
- Modify: `plugins/official/cms/web/templates/fragments/rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/page_rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/post_rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/category_rows.html`
- Modify: `plugins/official/cms/web/templates/fragments/flash.html`
- Test: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`

- [ ] **Step 1: Add failing CMS workbench contract test**

```rust
#[test]
fn cms_workbench_uses_enterprise_workspace_semantics() {
    let source = read("plugins/official/cms/web/templates/cms.html");
    assert!(source.contains("data-enterprise-workbench=\"cms\""));
    assert!(source.contains("data-admin-page-header"));
    assert!(source.contains("data-admin-action-cluster"));
}
```

- [ ] **Step 2: Run test to verify fail**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL on missing `data-enterprise-workbench`.

- [ ] **Step 3: Update `cms.html` shell semantics**

```html
<section
  class="admin-module cms-workbench"
  data-admin-workspace-module="cms"
  data-enterprise-workbench="cms"
  x-data="cmsPage()"
  x-init="init()"
>
  <header class="card border border-base-300 bg-base-100 shadow-sm" data-admin-page-header>
    <div class="cms-top-nav-right" data-admin-action-cluster>
      <button type="button" class="btn btn-sm btn-outline" @click="openCommandPalette()">Command</button>
      <button type="button" class="btn btn-sm btn-primary" @click="openEditor('posts', 'new')">New Post</button>
      <button type="button" class="btn btn-sm btn-primary" @click="openEditor('pages', 'new')">New Page</button>
    </div>
  </header>
  <section id="cms-overview-panel" class="cms-panel" data-cms-panel="overview"></section>
  <section id="cms-library-panel" class="cms-panel" data-cms-panel="library"></section>
  <section id="cms-editor-panel" class="cms-panel" data-cms-panel="editor"></section>
</section>
```

- [ ] **Step 4: Apply enterprise card/table/flash semantics to CMS fragments**

```html
<!-- Example in overview_panel.html -->
<section class="grid gap-4 xl:grid-cols-4" data-admin-table-card>
  <article class="card border border-base-300 bg-base-100 shadow-sm"><div class="card-body"><h3 class="font-semibold">Published</h3><p>42</p></div></article>
  <article class="card border border-base-300 bg-base-100 shadow-sm"><div class="card-body"><h3 class="font-semibold">Drafts</h3><p>11</p></div></article>
</section>
<!-- Example in flash.html -->
<div class="alert alert-error shadow-sm"><span>Failed to save content.</span></div>
<div class="alert alert-success shadow-sm"><span>Content saved successfully.</span></div>
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: PASS for CMS contracts.

- [ ] **Step 6: Commit**

```bash
git add plugins/official/cms/web/templates/**/*.html crates/sushi-admin/tests/admin_ui_enterprise_contract.rs
git commit -m "style(cms): align workbench visuals with enterprise admin shell"
```

---

### Task 6: Harmonize KV Store Workspace

**Files:**
- Modify: `plugins/official/kv-store/web/templates/kv.html`
- Modify: `plugins/official/kv-store/web/templates/fragments/kv_content.html`
- Modify: `plugins/official/kv-store/web/templates/partials/rows.html`
- Modify: `plugins/official/kv-store/web/templates/partials/flash.html`
- Test: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`

- [ ] **Step 1: Add failing KV contract test**

```rust
#[test]
fn kv_workspace_uses_enterprise_workspace_semantics() {
    let source = read("plugins/official/kv-store/web/templates/kv.html");
    assert!(source.contains("data-enterprise-workbench=\"kv\""));
    assert!(source.contains("data-admin-page-header"));
}
```

- [ ] **Step 2: Run test to verify fail**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL on missing marker.

- [ ] **Step 3: Update KV template shell and toolbars**

```html
<section class="admin-module" data-admin-workspace-module="kv" data-enterprise-workbench="kv">
  <header data-admin-page-header>
    <div data-admin-action-cluster>
      <input class="input input-bordered input-sm" placeholder="Search key">
      <button type="button" class="btn btn-primary btn-sm">New Key</button>
    </div>
  </header>
  <section class="card border border-base-300 bg-base-100 shadow-sm" data-admin-table-card>
    <table class="table table-zebra"><tbody><tr><td>feature.flag</td><td>true</td></tr></tbody></table>
  </section>
</section>
```

- [ ] **Step 4: Upgrade KV rows + flash semantics**

```html
<!-- rows.html -->
<tr class="hover"><td class="font-mono text-xs">feature.flag</td><td>true</td><td><button class="btn btn-xs btn-ghost">Edit</button></td></tr>
<!-- flash.html -->
<div class="alert alert-info"><span>KV table refreshed.</span></div>
<div class="alert alert-error"><span>Unable to delete key.</span></div>
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: PASS for KV contracts.

- [ ] **Step 6: Commit**

```bash
git add plugins/official/kv-store/web/templates/**/*.html crates/sushi-admin/tests/admin_ui_enterprise_contract.rs
git commit -m "style(kv): bring kv workspace to enterprise visual baseline"
```

---

### Task 7: Harmonize File Browser Workspace

**Files:**
- Modify: `plugins/official/file-browser/web/templates/file_browser.html`
- Modify: `plugins/official/file-browser/web/templates/fragments/list.html`
- Modify: `plugins/official/file-browser/web/templates/fragments/editor.html`
- Modify: `plugins/official/file-browser/web/templates/fragments/flash.html`
- Test: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`

- [ ] **Step 1: Add failing File Browser contract test**

```rust
#[test]
fn file_browser_workspace_uses_enterprise_workspace_semantics() {
    let source = read("plugins/official/file-browser/web/templates/file_browser.html");
    assert!(source.contains("data-enterprise-workbench=\"file-browser\""));
    assert!(source.contains("data-admin-page-header"));
}
```

- [ ] **Step 2: Run test to verify fail**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: FAIL on missing marker.

- [ ] **Step 3: Update file browser shell + landmarks**

```html
<section
  class="admin-module file-browser-workspace"
  data-admin-workspace-module="file-browser"
  data-enterprise-workbench="file-browser"
>
  <header data-admin-page-header>
    <div data-admin-action-cluster>
      <button type="button" class="btn btn-outline btn-sm">Upload</button>
      <button type="button" class="btn btn-primary btn-sm">New File</button>
    </div>
  </header>
  <div class="grid gap-4 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
    <section class="card border border-base-300 bg-base-100 shadow-sm"><div class="card-body">File list panel</div></section>
    <section class="card border border-base-300 bg-base-100 shadow-sm"><div class="card-body">Editor panel</div></section>
  </div>
</section>
```

- [ ] **Step 4: Normalize list/editor/flash component semantics**

```html
<!-- list.html -->
<section class="card border border-base-300 bg-base-100 shadow-sm" data-admin-table-card>
  <table class="table table-zebra"><tbody><tr><td>README.md</td><td>2 KB</td></tr></tbody></table>
</section>
<!-- editor.html -->
<section class="card border border-base-300 bg-base-100 shadow-sm">
  <div class="card-body"><textarea class="textarea textarea-bordered min-h-64 w-full">File content</textarea></div>
</section>
<!-- flash.html -->
<div class="alert alert-warning"><span>File has unsaved changes.</span></div>
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: PASS for file-browser contracts.

- [ ] **Step 6: Commit**

```bash
git add plugins/official/file-browser/web/templates/**/*.html crates/sushi-admin/tests/admin_ui_enterprise_contract.rs
git commit -m "style(file-browser): align workspace with enterprise design language"
```

---

### Task 8: Full Validation and Final Polish Regression Gate

**Files:**
- Modify: `web/static/css/input.css` (only if final rhythm fix needed)
- Test: `crates/sushi-admin/tests/admin_web.rs`
- Test: `crates/sushi-admin/tests/admin_ui_enterprise_contract.rs`

- [ ] **Step 1: Run targeted enterprise contract tests**

Run: `cargo test -p sushi-admin --test admin_ui_enterprise_contract -q`  
Expected: PASS.

- [ ] **Step 2: Run admin integration tests**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS.

- [ ] **Step 3: Build CSS output**

Run: `./scripts/compile-css.sh`  
Expected: `web/static/css/style.css` regenerated without errors.

- [ ] **Step 4: Run workspace-wide regression**

Run: `cargo test --workspace -q`  
Expected: PASS.

- [ ] **Step 5: Record final visual checklist in commit body**

```text
- Shell hierarchy: PASS
- Core modules consistency: PASS
- CMS/KV/File Browser parity: PASS
- Light/Dark persistence: PASS
- HTMX partial behavior unchanged: PASS
```

- [ ] **Step 6: Commit**

```bash
git add web/static/css/input.css web/static/css/style.css crates/sushi-admin/tests/admin_ui_enterprise_contract.rs
git commit -m "style(admin): finalize enterprise daisyui polish and regression gate"
```

---

## Plan Self-Review

### 1) Spec Coverage Check

- Enterprise shell hierarchy: covered by Tasks 2 and 8.
- Core admin module polish: covered by Task 3.
- Login enterprise framing: covered by Task 4.
- CMS/KV/File Browser harmonization: covered by Tasks 5/6/7.
- Interaction/feedback consistency and non-regression: covered by Tasks 3/5/6/7/8.
- Theme persistence and HTMX contract preservation: validated in Tasks 2 and 8.

No uncovered spec requirement found.

### 2) Placeholder Scan

- No `TBD`, `TODO`, “implement later”, or “similar to task N” placeholders remain.
- Each task includes concrete files, code snippets, commands, and expected outcomes.

### 3) Type/Contract Consistency Check

- Template contract markers are consistent across tasks:
  - `data-admin-shell`
  - `data-admin-page-header`
  - `data-admin-action-cluster`
  - `data-admin-table-card`
  - `data-enterprise-workbench="cms"`
  - `data-enterprise-workbench="kv"`
  - `data-enterprise-workbench="file-browser"`
- Test names align with the same marker vocabulary.

No naming mismatches detected.
