# CMS + File Browser UI Harmonization Design (2026-04-22)

## Background

This design covers three coordinated UI tasks:

1. Fix CMS admin dark-theme regressions where list/content surfaces still render with hardcoded light backgrounds.
2. Redesign the CMS public frontend with daisyUI and a modern premium visual language.
3. Redesign the File Browser public frontend with daisyUI, including layout optimization while preserving all existing behavior contracts.

A strict theme boundary is required:

- Admin theme: still controlled by `sushi-theme` (`light|dark`) on the admin shell.
- Public frontend theme (CMS + File Browser): always fixed to light and must not be affected by admin theme toggles.

## Goals

- Eliminate obvious hardcoded light-only backgrounds in CMS admin views, especially list-related surfaces, so dark mode is visually consistent.
- Rebuild CMS public pages with a cohesive daisyUI-first visual system focused on modern, high-end editorial experience.
- Rebuild File Browser public page UI with improved information architecture (toolbar / explorer / editor / status), while keeping all interactions and data hooks stable.
- Keep route contracts, data contracts, and JS/Lua behavior unchanged.

## Non-Goals

- No backend behavior changes in Lua/Rust handlers.
- No route or API contract changes.
- No feature additions to CMS domain logic or File Browser permissions model.
- No replacement of HTMX/Alpine workflow.

## Current Constraints and Invariants

### Theme Invariants

- Admin pages may remain light/dark and continue using `localStorage['sushi-theme']`.
- CMS public pages and File Browser public page are fixed-light UIs and must not read/write `sushi-theme`.

### Contract Invariants

- Keep all existing HTMX endpoints and fragment composition behavior.
- Preserve critical DOM hooks used by JS, especially in File Browser:
  - `data-fb-action`
  - `data-fb-node`
  - `data-fb-children-for`
  - `#fb-list`, `#fb-editor`, `#fb-search-panel`, `#fb-search-results`, `#fb-context-menu`, `#fb-context-upload-input`
- Preserve admin enterprise markers and page landmarks (for current contract tests):
  - `data-enterprise-workbench`
  - `data-admin-workspace-module`
  - `data-admin-page-header`
  - `data-admin-action-cluster`
  - `data-admin-table-card`

## Approach Overview

### Option Chosen

Balanced refactor (recommended option):

- Systematically replace hardcoded CMS admin light colors with theme-semantic classes and variables.
- Redesign CMS public templates into a daisyUI-driven premium light interface.
- Redesign File Browser public templates and CSS into daisyUI-driven premium light interface with better layout hierarchy.
- Keep JS behavior and endpoint contracts intact.

This balances delivery speed, visual quality, and regression risk.

## Detailed Design

## 1) CMS Admin Dark-Theme Fix

### Target Files

- `plugins/official/cms/web/static/cms.css`
- CMS admin fragments under `plugins/official/cms/web/templates/fragments/*.html` when class semantics need adjustment.

### Changes

- Replace hardcoded light backgrounds (`#fff`, `#f8fbff`, similar) with theme-aware surfaces using `base-*` / semantic colors.
- Normalize row hover and selected states to semantic color layers that adapt in dark mode.
- Normalize auxiliary surfaces (markdown toolbar, preview panel, command shell, toast) to theme-aware colors.
- Keep current structure and behavior in `cms.js`; no event or endpoint changes.

### Expected Result

- In dark mode, list rows/cards/panels no longer show white patches.
- In light mode, appearance remains polished and coherent with enterprise shell.

## 2) CMS Public Frontend Redesign (Fixed Light)

### Target Files

- `plugins/official/cms/web/templates/public/base_public.html`
- `plugins/official/cms/web/templates/public/home.html`
- `plugins/official/cms/web/templates/public/post_list.html`
- `plugins/official/cms/web/templates/public/post_detail.html`
- `plugins/official/cms/web/templates/public/page_list.html`
- `plugins/official/cms/web/templates/public/page_detail.html`
- `plugins/official/cms/web/templates/public/category_detail.html`
- `plugins/official/cms/web/templates/public/partials/post_feed.html`
- New public CSS asset: `plugins/official/cms/web/static/cms_public.css`

### Changes

- Force public shell to fixed light theme (`<html data-theme="light">`) without reading/writing `sushi-theme`.
- Rework public shell layout into daisyUI semantic components (navbar, cards, badges, alerts, buttons, input groups).
- Keep HTMX target/trigger contracts unchanged for category filtering and related feeds.
- Rebuild typography, spacing, and gradient/background system for a modern premium editorial tone.

### Expected Result

- Public CMS looks materially upgraded and consistent.
- Theme remains light regardless of admin theme state.
- Existing partial reload behavior remains stable.

## 3) File Browser Public Frontend Redesign + Layout Optimization (Fixed Light)

### Target Files

- `plugins/official/file-browser/web/templates/file_browser.html`
- `plugins/official/file-browser/web/templates/fragments/list.html`
- `plugins/official/file-browser/web/templates/fragments/editor.html`
- `plugins/official/file-browser/web/templates/fragments/flash.html`
- `plugins/official/file-browser/web/static/file_browser.css`

### Changes

- Convert shell and panels to daisyUI semantic building blocks.
- Optimize information architecture:
  - Top control region (root selector, global actions, state hints)
  - Left explorer region (tree + quick actions + search)
  - Right editor region (breadcrumbs + editor card + status row)
  - Contextual notifications and menus with clearer visual hierarchy
- Keep all `data-fb-*` hooks and key IDs intact for `file_browser.js` compatibility.
- Keep behavior flows unchanged: tree navigation, open/save, right-click menu, upload, download, search.
- Remove dependency on admin theme storage for this public surface.

### Expected Result

- Stronger usability and readability for large file trees.
- Modern, high-quality UI while preserving all current capabilities.

## Data Flow and Behavior Compatibility

## CMS Public

- Request/response and rendered data fields remain unchanged.
- HTMX endpoints and swap targets remain unchanged.
- Alpine state functions remain unchanged where used.

## File Browser

- JS still controls all behavior through the same selectors and data attributes.
- Template structure may be reorganized visually, but behavior-affecting hooks are preserved.

## Risk Analysis and Mitigations

- Risk: breaking File Browser JS due to missing hooks.
  - Mitigation: explicit checklist for all required `id`/`data-fb-*` selectors before test run.
- Risk: dark-mode regressions in CMS admin due to mixed style sources.
  - Mitigation: grep scan for hardcoded light colors in CMS plugin CSS/templates and verify dark-mode surfaces manually.
- Risk: visual regressions in fragment-driven areas.
  - Mitigation: smoke-check all HTMX partial paths after redesign.

## Validation Plan

### Automated

Run:

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

### Manual

- CMS admin in dark mode:
  - library table rows, overview cards, editor markdown preview, command panel, toasts
  - verify no obvious white hardcoded blocks
- CMS public frontend:
  - verify pages always stay light after toggling admin theme
  - verify category filter and related feed HTMX updates
- File Browser public frontend:
  - root switch, tree expand/collapse, search, context menu actions, upload, save, download
  - verify fixed-light style independent from admin theme

## Implementation Environment

Implementation must start from a worktree under repository-local `.worktrees/` as requested by user.

Planned execution location pattern:

- `.worktrees/<feature-branch-name>`

All coding, testing, and commits for this task will be done in that worktree.

## Deliverables

- Updated CMS admin styles with dark-theme-safe surfaces.
- Redesigned CMS public templates and supporting CSS (fixed-light daisyUI style).
- Redesigned File Browser public templates and CSS with optimized layout (fixed-light daisyUI style).
- Passing baseline test suite above.
