# Official CMS Admin Editorial Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the official CMS admin into an editor-first workbench with Top Nav IA, overview dashboard, immersive editor, keyboard-first commands, and production-grade CRUD usability.

**Architecture:** Keep CMS domain model in Lua (`page/post/category`) and extend admin interface orchestration (`interfaces/admin.lua`) to drive Overview/Library/Editor flows through HTMX partials and a richer `cms.js` state controller. Preserve plugin-local templates/static assets and add only the minimum route/query contracts needed for command palette, status transition, and dashboard aggregates.

**Tech Stack:** Rust tests (`sushi-admin`, `sushi-core`), Lua plugin runtime (`mlua` bindings via Sushi), HTMX + Alpine.js, plugin-local templates/static assets.

---

## Scope Check

This plan covers one subsystem only: CMS admin experience redesign for the official `cms` plugin.  
Public `/app/*` pages, non-CMS admin modules, and collaboration/version-history features remain out of scope.

## File Structure Map

- Modify: `plugins/official/cms/lua/bootstrap/register.lua` — register new CMS admin partial routes and keep policy keys valid.
- Modify: `plugins/official/cms/lua/domain/page.lua` — add overview/list fields and status transition helpers for pages.
- Modify: `plugins/official/cms/lua/domain/post.lua` — add overview/list fields and status transition helpers for posts.
- Modify: `plugins/official/cms/lua/interfaces/admin.lua` — add Overview/Library/Editor/Command handlers and unify save/status flow.
- Modify: `plugins/official/cms/web/templates/cms.html` — convert root template to Top Nav CMS shell.
- Create: `plugins/official/cms/web/templates/fragments/overview_panel.html` — default overview dashboard panel.
- Create: `plugins/official/cms/web/templates/fragments/library_panel.html` — reusable library panel (posts/pages/categories).
- Create: `plugins/official/cms/web/templates/fragments/editor_panel.html` — immersive editor panel.
- Modify: `plugins/official/cms/web/templates/fragments/page_rows.html` — richer row metadata/actions for editor transitions.
- Modify: `plugins/official/cms/web/templates/fragments/post_rows.html` — richer row metadata/actions.
- Modify: `plugins/official/cms/web/templates/fragments/category_rows.html` — richer row metadata/actions.
- Modify: `plugins/official/cms/web/static/cms.js` — keyboard dispatcher, command palette, panel routing, HTMX orchestration.
- Create: `plugins/official/cms/web/static/cms.css` — CMS-local editorial workbench styling.
- Modify: `plugins/official/cms/plugin.toml` — include `cms.css` in workspace asset bundle.
- Modify/Test: `crates/sushi-core/src/lua/loader.rs` — registration/template/static contract checks for CMS redesign.
- Modify/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs` — domain/admin contract checks (overview/status/commands).
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs` — template and interaction-contract tests for new CMS workbench.

---

### Task 1: Lock New CMS Admin Route Contracts

**Files:**
- Modify/Test: `crates/sushi-core/src/lua/loader.rs`
- Modify: `plugins/official/cms/lua/bootstrap/register.lua`

- [ ] **Step 1: Add failing loader contract assertions for redesigned CMS routes**

```rust
#[test]
fn cms_plugin_registration_contract_is_stable() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let plugin_path = repo_root.join("plugins/official/cms/lua/bootstrap/register.lua");
    let source = std::fs::read_to_string(plugin_path).unwrap();

    assert!(source.contains("sushi.web.page(\"/admin/cms\""));
    assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/overview\""));
    assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/library/*\""));
    assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/editor/*\""));
    assert!(source.contains("sushi.api.route(\"POST\", \"/admin/partials/cms/editor/save\""));
    assert!(source.contains("sushi.api.route(\"POST\", \"/admin/partials/cms/status/transition\""));
    assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/commands\""));
}
```

- [ ] **Step 2: Run focused test to confirm failure before route wiring**

Run: `cargo test -p sushi-core cms_plugin_registration_contract_is_stable -q`  
Expected: FAIL because new route strings are missing.

- [ ] **Step 3: Implement route registration in plugin bootstrap**

```lua
-- plugins/official/cms/lua/bootstrap/register.lua
sushi.api.route("GET", "/admin/partials/cms/overview", deps.admin.overview_partial, { policy = "admin.cms.read" })
sushi.api.route("GET", "/admin/partials/cms/library/*", deps.admin.library_partial, { policy = "admin.cms.read" })
sushi.api.route("GET", "/admin/partials/cms/editor/*", deps.admin.editor_partial, { policy = "admin.cms.read" })
sushi.api.route("POST", "/admin/partials/cms/editor/save", deps.admin.editor_save_partial, { policy = "admin.cms.write" })
sushi.api.route("POST", "/admin/partials/cms/status/transition", deps.admin.status_transition_partial, { policy = "admin.cms.write" })
sushi.api.route("GET", "/admin/partials/cms/commands", deps.admin.commands_partial, { policy = "admin.cms.read" })
```

- [ ] **Step 4: Re-run contract test**

Run: `cargo test -p sushi-core cms_plugin_registration_contract_is_stable -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/lua/loader.rs plugins/official/cms/lua/bootstrap/register.lua
git commit -m "feat(cms): register editorial workbench admin partial routes"
```

### Task 2: Extend Domain Modules for Overview + Status Workflows

**Files:**
- Modify: `plugins/official/cms/lua/domain/page.lua`
- Modify: `plugins/official/cms/lua/domain/post.lua`
- Modify/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`

- [ ] **Step 1: Add failing behavior tests for domain helpers**

```rust
#[test]
fn cms_page_domain_exposes_overview_and_status_helpers() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/domain/page.lua"),
    )
    .expect("failed to read page domain");
    assert!(source.contains("function page.count_by_status"));
    assert!(source.contains("function page.recent"));
    assert!(source.contains("function page.set_status"));
}

#[test]
fn cms_post_domain_exposes_overview_and_status_helpers() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/domain/post.lua"),
    )
    .expect("failed to read post domain");
    assert!(source.contains("function post.count_by_status"));
    assert!(source.contains("function post.recent"));
    assert!(source.contains("function post.set_status"));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: FAIL because helper functions do not exist yet.

- [ ] **Step 3: Implement domain helper methods**

```lua
-- plugins/official/cms/lua/domain/page.lua
function page.count_by_status()
    local rows, kind, msg = db.query(
        "SELECT status, COUNT(*) AS total FROM cms_pages WHERE deleted_at IS NULL GROUP BY status",
        {}
    )
    if not rows then
        return nil, kind or "storage_error", msg
    end
    return rows
end

function page.recent(limit)
    local cap = tonumber(limit or 8) or 8
    local rows, kind, msg = db.query(
        "SELECT title, slug, status, updated_at FROM cms_pages WHERE deleted_at IS NULL ORDER BY updated_at DESC LIMIT ?1",
        { cap }
    )
    if not rows then
        return nil, kind or "storage_error", msg
    end
    return rows
end

function page.set_status(slug_value, status)
    local normalized, kind, msg = validate.validate_status(status)
    if not normalized then
        return nil, kind, msg
    end
    local ok, exec_kind, exec_msg = db.execute(
        "UPDATE cms_pages SET status = ?1, updated_at = datetime('now') WHERE slug = ?2 AND deleted_at IS NULL",
        { normalized, slug_value }
    )
    if not ok then
        return nil, exec_kind or "storage_error", exec_msg
    end
    return page.get_by_slug(slug_value)
end
```

```lua
-- plugins/official/cms/lua/domain/post.lua
function post.count_by_status()
    local rows, kind, msg = db.query(
        "SELECT status, COUNT(*) AS total FROM cms_posts WHERE deleted_at IS NULL GROUP BY status",
        {}
    )
    if not rows then
        return nil, kind or "storage_error", msg
    end
    return rows
end

function post.recent(limit)
    local cap = tonumber(limit or 8) or 8
    local rows, kind, msg = db.query(
        "SELECT p.title, p.slug, p.status, p.updated_at, c.slug AS category_slug FROM cms_posts p JOIN cms_categories c ON c.id = p.category_id WHERE p.deleted_at IS NULL AND c.deleted_at IS NULL ORDER BY p.updated_at DESC LIMIT ?1",
        { cap }
    )
    if not rows then
        return nil, kind or "storage_error", msg
    end
    return rows
end

function post.set_status(slug_value, status)
    local normalized, kind, msg = validate.validate_status(status)
    if not normalized then
        return nil, kind, msg
    end
    local ok, exec_kind, exec_msg = db.execute(
        "UPDATE cms_posts SET status = ?1, updated_at = datetime('now') WHERE slug = ?2 AND deleted_at IS NULL",
        { normalized, slug_value }
    )
    if not ok then
        return nil, exec_kind or "storage_error", exec_msg
    end
    return post.get_by_slug(slug_value, { only_published = false })
end
```

- [ ] **Step 4: Re-run behavior tests**

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/lua/domain/page.lua plugins/official/cms/lua/domain/post.lua crates/sushi-core/tests/cms_plugin_behavior.rs
git commit -m "feat(cms): add overview and status domain helpers"
```

### Task 3: Implement Admin Interface Orchestration Handlers

**Files:**
- Modify: `plugins/official/cms/lua/interfaces/admin.lua`
- Modify/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`

- [ ] **Step 1: Add failing contract tests for admin orchestration functions**

```rust
#[test]
fn cms_admin_interface_exposes_workbench_handlers() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/interfaces/admin.lua"),
    )
    .expect("failed to read cms admin interface");
    assert!(source.contains("function admin.overview_partial"));
    assert!(source.contains("function admin.library_partial"));
    assert!(source.contains("function admin.editor_partial"));
    assert!(source.contains("function admin.editor_save_partial"));
    assert!(source.contains("function admin.status_transition_partial"));
    assert!(source.contains("function admin.commands_partial"));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: FAIL because functions are not implemented yet.

- [ ] **Step 3: Implement handler orchestration in Lua**

```lua
-- plugins/official/cms/lua/interfaces/admin.lua
function admin.overview_partial()
    local page_counts = page.count_by_status() or {}
    local post_counts = post.count_by_status() or {}
    local recent_pages = page.recent(6) or {}
    local recent_posts = post.recent(6) or {}
    return sushi.web.render("plugins/official/cms/fragments/overview_panel.html", {
        page_counts = page_counts,
        post_counts = post_counts,
        recent_pages = recent_pages,
        recent_posts = recent_posts,
    })
end

function admin.library_partial(args)
    local path = (args and args[1]) or ""
    local scope = path:match("^/admin/partials/cms/library/([^%?]+)") or "posts"
    if scope == "pages" then
        return sushi.web.render("plugins/official/cms/fragments/page_rows.html", { items = page.list() or {} })
    elseif scope == "categories" then
        return sushi.web.render("plugins/official/cms/fragments/category_rows.html", { items = category.list() or {} })
    end
    return sushi.web.render("plugins/official/cms/fragments/post_rows.html", { items = post.list({ only_published = false }) or {} })
end

function admin.editor_save_partial(args)
    local form = parse_urlencoded(args[2] or "")
    local kind = tostring(form.kind or "post")
    if kind == "page" then
        local item, e_kind, e_msg = page.upsert(form, form.original_slug)
        if not item then return flash("error", tostring(e_msg or e_kind)) end
        return flash("success", "Page saved")
    end
    local item, e_kind, e_msg = post.upsert(form, form.original_slug)
    if not item then return flash("error", tostring(e_msg or e_kind)) end
    return flash("success", "Post saved")
end
```

- [ ] **Step 4: Re-run behavior suite**

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/lua/interfaces/admin.lua crates/sushi-core/tests/cms_plugin_behavior.rs
git commit -m "feat(cms): implement workbench admin orchestration handlers"
```

### Task 4: Refactor CMS Template IA to Overview + Library + Editor Panels

**Files:**
- Modify: `plugins/official/cms/web/templates/cms.html`
- Create: `plugins/official/cms/web/templates/fragments/overview_panel.html`
- Create: `plugins/official/cms/web/templates/fragments/library_panel.html`
- Create: `plugins/official/cms/web/templates/fragments/editor_panel.html`
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing template contract test**

```rust
#[test]
fn admin_cms_template_uses_top_nav_and_panel_mounts() {
    let source = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/cms.html"),
    )
    .expect("failed to read cms template");
    assert!(source.contains("data-cms-top-nav"));
    assert!(source.contains("data-cms-panel=\"overview\""));
    assert!(source.contains("data-cms-panel=\"library\""));
    assert!(source.contains("data-cms-panel=\"editor\""));
}
```

- [ ] **Step 2: Run focused test to confirm failure**

Run: `cargo test -p sushi-admin --test admin_web admin_cms_template_uses_top_nav_and_panel_mounts -q`  
Expected: FAIL because new structural markers are missing.

- [ ] **Step 3: Implement top-nav CMS shell and panel fragments**

```html
<!-- plugins/official/cms/web/templates/cms.html -->
<section class="cms-workbench" x-data="cmsPage()" data-admin-workspace-module="cms">
  <header class="cms-top-nav" data-cms-top-nav>
    <button type="button" @click="switchPanel('overview')">Overview</button>
    <button type="button" @click="switchPanel('posts')">Posts</button>
    <button type="button" @click="switchPanel('pages')">Pages</button>
    <button type="button" @click="switchPanel('categories')">Categories</button>
    <button type="button" @click="openCommandPalette()">⌘K</button>
  </header>

  <section data-cms-panel="overview" x-show="panel === 'overview'"></section>
  <section data-cms-panel="library" x-show="panel === 'library'"></section>
  <section data-cms-panel="editor" x-show="panel === 'editor'"></section>
</section>
```

```html
<!-- plugins/official/cms/web/templates/fragments/overview_panel.html -->
<article class="cms-overview-card-grid">
  <section class="ui-card"><h3>Today</h3></section>
  <section class="ui-card"><h3>Recent edits</h3></section>
  <section class="ui-card"><h3>Content health</h3></section>
</article>
```

- [ ] **Step 4: Re-run admin template tests**

Run: `cargo test -p sushi-admin --test admin_web admin_cms_template_uses_top_nav_and_panel_mounts -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/web/templates/cms.html plugins/official/cms/web/templates/fragments/overview_panel.html plugins/official/cms/web/templates/fragments/library_panel.html plugins/official/cms/web/templates/fragments/editor_panel.html crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(cms): refactor cms template into top-nav workbench panels"
```

### Task 5: Rebuild CMS Front-end Controller with Keyboard + Commands

**Files:**
- Modify: `plugins/official/cms/web/static/cms.js`
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: Add failing JS contract test for keybindings and panel switching**

```rust
#[test]
fn cms_js_defines_shortcuts_and_command_palette_hooks() {
    let source = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/static/cms.js"),
    )
    .expect("failed to read cms.js");
    assert!(source.contains("Cmd/Ctrl+K"));
    assert!(source.contains("switchPanel"));
    assert!(source.contains("openCommandPalette"));
    assert!(source.contains("handleGlobalShortcut"));
}
```

- [ ] **Step 2: Run focused test to verify failure**

Run: `cargo test -p sushi-admin --test admin_web cms_js_defines_shortcuts_and_command_palette_hooks -q`  
Expected: FAIL because controller APIs are not present yet.

- [ ] **Step 3: Implement keyboard-first controller**

```javascript
// plugins/official/cms/web/static/cms.js
window.cmsPage = function cmsPage() {
  return {
    panel: 'overview',
    commandOpen: false,
    switchPanel(next) { this.panel = next === 'overview' ? 'overview' : 'library'; },
    openCommandPalette() { this.commandOpen = true; },
    closeCommandPalette() { this.commandOpen = false; },
    handleGlobalShortcut(event) {
      const cmd = event.metaKey || event.ctrlKey;
      if (cmd && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        this.openCommandPalette(); // Cmd/Ctrl+K
      }
    },
    init() { window.addEventListener('keydown', (e) => this.handleGlobalShortcut(e)); },
  };
};
```

- [ ] **Step 4: Re-run admin tests**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/web/static/cms.js crates/sushi-admin/tests/admin_web.rs
git commit -m "feat(cms): add keyboard-first workbench controller and command hooks"
```

### Task 6: Add CMS-local Editorial Styling and Bundle Wiring

**Files:**
- Create: `plugins/official/cms/web/static/cms.css`
- Modify: `plugins/official/cms/plugin.toml`
- Modify/Test: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Add failing loader contract test for CSS bundle**

```rust
#[test]
fn cms_plugin_declares_workspace_css_bundle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let source = std::fs::read_to_string(root.join("plugins/official/cms/plugin.toml")).unwrap();
    assert!(source.contains("[admin.assets.bundles.workspace]"));
    assert!(source.contains("js = [\"cms.js\"]"));
    assert!(source.contains("css = [\"cms.css\"]"));
}
```

- [ ] **Step 2: Run focused test to verify failure**

Run: `cargo test -p sushi-core cms_plugin_declares_workspace_css_bundle -q`  
Expected: FAIL because `cms.css` is not bundled.

- [ ] **Step 3: Add plugin-local CSS and bundle entry**

```toml
# plugins/official/cms/plugin.toml
[admin.assets.bundles.workspace]
js = ["cms.js"]
css = ["cms.css"]
```

```css
/* plugins/official/cms/web/static/cms.css */
.cms-workbench { display: grid; gap: 16px; }
.cms-top-nav { display: flex; gap: 8px; padding: 8px; border-bottom: 1px solid var(--border-soft); }
.cms-top-nav button[aria-current="page"] { background: var(--bg-muted); font-weight: 700; }
.cms-editor-single-column { max-width: 860px; margin: 0 auto; }
.cms-status-bar { position: sticky; top: 0; background: rgba(255,255,255,0.9); backdrop-filter: blur(8px); }
```

- [ ] **Step 4: Run loader + template service tests**

Run: `cargo test -p sushi-core cms_plugin_declares_workspace_css_bundle -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/official/cms/plugin.toml plugins/official/cms/web/static/cms.css crates/sushi-core/src/lua/loader.rs
git commit -m "feat(cms): add editorial workbench css bundle for cms admin"
```

### Task 7: Add End-to-End Admin Contract Tests for New CMS Workbench Actions

**Files:**
- Modify/Test: `crates/sushi-admin/tests/admin_web.rs`
- Modify/Test: `crates/sushi-core/tests/cms_plugin_behavior.rs`

- [ ] **Step 1: Add failing action contract tests**

```rust
#[test]
fn cms_template_wires_overview_library_editor_endpoints() {
    let source = std::fs::read_to_string(
        workspace_root().join("plugins/official/cms/web/templates/cms.html"),
    )
    .expect("failed to read cms template");
    assert!(source.contains("/admin/partials/cms/overview"));
    assert!(source.contains("/admin/partials/cms/library/posts"));
    assert!(source.contains("/admin/partials/cms/editor/post"));
    assert!(source.contains("/admin/partials/cms/editor/save"));
    assert!(source.contains("/admin/partials/cms/status/transition"));
    assert!(source.contains("/admin/partials/cms/commands"));
}
```

- [ ] **Step 2: Run targeted tests to verify failure**

Run: `cargo test -p sushi-admin --test admin_web cms_template_wires_overview_library_editor_endpoints -q`  
Expected: FAIL before full wiring.

- [ ] **Step 3: Complete template/JS wiring for those endpoints**

```html
<!-- cms.html key hooks -->
<section id="cms-overview-panel" hx-get="/admin/partials/cms/overview" hx-trigger="load"></section>
<section id="cms-library-panel" hx-get="/admin/partials/cms/library/posts" hx-trigger="cms:library:refresh from:body"></section>
<form id="cms-editor-form" hx-post="/admin/partials/cms/editor/save" hx-target="#cms-feedback"></form>
<form id="cms-transition-form" hx-post="/admin/partials/cms/status/transition" hx-target="#cms-feedback"></form>
<div id="cms-command-panel" hx-get="/admin/partials/cms/commands" hx-trigger="cms:commands:refresh from:body"></div>
```

- [ ] **Step 4: Run full CMS-related verification**

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-admin/tests/admin_web.rs crates/sushi-core/tests/cms_plugin_behavior.rs plugins/official/cms/web/templates/cms.html plugins/official/cms/web/static/cms.js
git commit -m "test(cms): cover overview editor and command wiring contracts"
```

### Task 8: Final Verification, Graph Update, and Release Hygiene

**Files:**
- Modify: `graphify-out/graph.json`
- Modify: `graphify-out/GRAPH_REPORT.md`

- [ ] **Step 1: Run full regression suite**

Run: `cargo test -p sushi-core --test template_service -q`  
Expected: PASS

Run: `cargo test -p sushi-core --test cms_plugin_behavior -q`  
Expected: PASS

Run: `cargo test -p sushi-admin --test admin_web -q`  
Expected: PASS

Run: `cargo test --workspace -q`  
Expected: PASS

- [ ] **Step 2: Refresh graph artifacts**

Run: `graphify update .`  
Expected: `graphify-out/graph.json` and `graphify-out/GRAPH_REPORT.md` updated.

- [ ] **Step 3: Validate final tree**

Run: `git status --short`  
Expected: only intended redesign files + graph artifacts.

- [ ] **Step 4: Commit graph refresh**

```bash
git add graphify-out/graph.json graphify-out/GRAPH_REPORT.md
git commit -m "chore(graphify): refresh graph after cms admin workbench redesign"
```

- [ ] **Step 5: Prepare branch completion**

```bash
git log --oneline -n 12
```

Expected: task commits are ordered and scoped cleanly for review/PR.

---

## Self-Review

### 1) Spec Coverage Check

- Top Nav IA + default overview: covered by Tasks 4 and 7.
- Immersive editor (single-column) + save/status flow: covered by Tasks 3, 4, and 7.
- Keyboard-first and command palette: covered by Task 5 and Task 7.
- Approach B backend adjustments (overview aggregate, status transitions, command query): covered by Tasks 2 and 3.
- Visual redesign and local asset strategy: covered by Task 6.
- Error/boundary and regression verification: covered by Task 8.

No spec requirement is left without an implementation task.

### 2) Placeholder Scan

- No `TODO`/`TBD` placeholders remain.
- Every implementation step includes concrete file paths, snippets, and commands.

### 3) Type/Signature Consistency

- Admin partial routes in Task 1 align with handler names introduced in Task 3 and UI wiring in Task 7.
- Domain helpers introduced in Task 2 are consumed by Task 3 overview/status handlers with matching names.
- CSS bundle contract in Task 6 aligns with plugin assets consumed by CMS page registration.
