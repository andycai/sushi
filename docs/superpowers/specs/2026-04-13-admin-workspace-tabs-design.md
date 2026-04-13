# Admin Workspace Tabs + HTMX Partial Navigation Design

Date: 2026-04-13  
Status: Approved (design)  
Owner: Admin UI / Platform

## 1) Background and Problem

The current admin console still relies on full-page navigation for left sidebar menu clicks. This causes:

- Full page reloads for routine operator workflows.
- No multi-module working context (cannot keep multiple modules open side-by-side as tabs).
- Loss of working continuity when switching modules.

The target behavior is:

- Left menu click loads right-side content with HTMX partial updates.
- The right-side workspace supports multiple tabs.
- URL always reflects active tab.
- Existing tab is re-activated (not duplicated) when clicking the same module.
- Workspace state is restored after refresh.

## 2) Goals and Non-Goals

### Goals

1. Replace full-page admin module switching with HTMX partial loading in the right content workspace.
2. Add tabbed workspace with module-level tabs.
3. Keep URL synchronized with active tab using History API.
4. Persist tab state in localStorage and restore on refresh.
5. Keep Dashboard (`/admin/`) as pinned, non-closable tab.
6. Preserve existing RBAC behavior.

### Non-Goals

1. No unsaved-changes interception in this phase.
2. No conversion to a JSON-only SPA architecture.
3. No RBAC model redesign.
4. No redesign of business logic inside modules (Users/Roles/etc.) beyond workspace integration.

## 3) UX Contract

1. Clicking a sidebar item opens or activates a tab, then loads content in that tab panel via HTMX.
2. Clicking an already opened module only activates that existing tab.
3. Dashboard tab is always present and cannot be closed.
4. Non-dashboard tabs are closable.
5. Closing active tab activates the most recently active remaining tab; if none, fallback to Dashboard.
6. URL changes to the active tab path (e.g., `/admin/users`).
7. Browser back/forward restores active tab/content through `popstate`.
8. Refresh restores open tabs and active tab from localStorage.

## 4) Architecture Overview

## 4.1 Frontend Runtime (Alpine + HTMX)

Introduce a shared workspace runtime in `web/static/admin/js/workspace.js`:

- Tab registry/state:
  - `tabs: Array<{ path, label, closable, module }>`
  - `activePath: string`
  - `recentStack: string[]` (for close fallback behavior)
- Persistence:
  - localStorage key: `admin.workspace.v1`
- Navigation API:
  - `openTab(path, label, module)`
  - `activateTab(path, push = true)`
  - `closeTab(path)`
  - `loadTabContent(path, module)`
  - `restoreWorkspace()`
  - `persistWorkspace()`

Sidebar integration in `menu.js`:

- Keep menu-fetch, tree expand/collapse, icons.
- Replace direct navigation behavior with workspace API invocation.
- Menu click contract:
  - If item has children: expand/collapse only.
  - If leaf item with route: `AdminWorkspace.openFromMenu(item)`.

## 4.2 Shell Composition

The admin shell composes menu + workspace state at the layout level:

- A single shell Alpine component drives:
  - sidebar menu tree behavior
  - workspace tab strip
  - active panel loading

Login page remains independent from workspace shell.

## 4.3 Backend Partial Endpoints

Add a dedicated workspace partial endpoint:

- `GET /admin/workspace/:module`

Allowed module names (explicit whitelist):

- `dashboard`
- `users`
- `roles`
- `permissions`
- `plugins`
- `kv`
- `config`
- `logs`
- `menus`

This endpoint returns only module content HTML (no base layout, no sidebar).

## 5) Template Strategy

Keep current full-page templates for direct URL access compatibility.

Add workspace partial templates:

- `web/templates/admin/workspace/dashboard.html`
- `web/templates/admin/workspace/users.html`
- `web/templates/admin/workspace/roles.html`
- `web/templates/admin/workspace/permissions.html`
- `web/templates/admin/workspace/plugins.html`
- `web/templates/admin/workspace/kv.html` (or plugin workspace partial target)
- `web/templates/admin/workspace/config.html`
- `web/templates/admin/workspace/logs.html`
- `web/templates/admin/workspace/menus.html`

Reuse existing inner content/partials where possible to avoid duplication.

## 6) URL and History Model

1. On tab activation:
   - Update active tab state.
   - `history.pushState({ path, module }, '', path)` unless activation originated from `popstate`.
2. On initial load:
   - If current location maps to a supported module, open/activate it.
   - Always ensure Dashboard tab exists.
3. On `window.popstate`:
   - Resolve target path to module.
   - Activate/open tab without issuing another `pushState`.
   - Load content if not already loaded.

## 7) Persistence Model

Storage key: `admin.workspace.v1`  
Payload:

```json
{
  "tabs": [
    { "path": "/admin/", "label": "Dashboard", "closable": false, "module": "dashboard" },
    { "path": "/admin/users", "label": "Users", "closable": true, "module": "users" }
  ],
  "activePath": "/admin/users",
  "recentStack": ["/admin/", "/admin/users"]
}
```

Validation rules on restore:

1. Drop tabs whose path/module are no longer allowed.
2. Re-insert Dashboard tab if missing.
3. Ensure only one tab per path (dedupe).
4. If activePath invalid, fallback to Dashboard.

## 8) RBAC and Security Contract

No permission model change.

`/admin/workspace/:module` maps to existing read permissions:

- `dashboard` -> `dashboard.view`
- `users` -> `users.view`
- `roles` -> `roles.view`
- `permissions` -> `permissions.view`
- `plugins` -> `plugins.view`
- `kv` -> `kv.manage` (current behavior)
- `config` -> `config.view`
- `logs` -> `logs.view`
- `menus` -> `menus.view`

Unauthorized access returns `403`; unknown module returns `404`.

## 9) Error Handling and Degradation

1. Workspace partial load failure (`403/404/500`) renders a standard inline error card in the active tab panel:
   - status-aware message
   - retry action
2. If HTMX is unavailable, fallback to full-page navigation using module route.
3. If localStorage is unavailable, runtime remains functional without persistence.

## 10) Delivery Phases

### Phase 1 (High-frequency modules)

- `dashboard`, `users`, `plugins`, `kv`
- Ship workspace shell + tabs + URL/history + persistence + partial endpoint base

### Phase 2 (Remaining modules)

- `roles`, `permissions`, `menus`, `logs`, `config`

Each phase ships in isolated commits and remains backward-compatible with full-page routes.

## 11) Testing Strategy

### Backend

1. Router tests for `/admin/workspace/:module`:
   - whitelist pass
   - unknown module 404
   - RBAC denied 403
2. Permission mapping tests align with `required_admin_permission`.

### Frontend (manual + scripted where feasible)

1. Menu click opens tabs without full-page reload.
2. Same-path click re-activates existing tab only.
3. Dashboard tab is pinned and non-closable.
4. Close behavior picks previous recent tab.
5. URL sync works on open/activate/close.
6. Back/forward restores correct active content.
7. Refresh restores tabs/active state.
8. Partial load error card + retry works.

## 12) Acceptance Criteria

Feature is complete only when all conditions are true:

1. Sidebar menu uses HTMX partial workspace loading for configured modules.
2. Tab workspace supports open/switch/close with no duplicate path tabs.
3. Dashboard tab is fixed and non-closable.
4. URL always matches active tab.
5. Browser history navigation works with workspace tabs.
6. Workspace state restores from localStorage after refresh.
7. Existing RBAC checks remain effective for workspace partial routes.
8. Direct route access to `/admin/<module>` remains functional.

## 13) Risks and Mitigations

1. **Risk:** Double-initialization of module scripts after HTMX swap.  
   **Mitigation:** module init contract (`initModule(container)`) and idempotent initialization guards.

2. **Risk:** Divergence between full-page templates and workspace partial templates.  
   **Mitigation:** maximize reuse of existing partials; keep page-level wrappers thin.

3. **Risk:** History stack inconsistency during rapid tab operations.  
   **Mitigation:** strict `pushState` policy and `popstate` path-to-tab resolver.

