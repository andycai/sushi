# Sushi Admin Enterprise Visual Polish (daisyUI)

> Date: 2026-04-21  
> Status: Approved  
> Scope: Full admin experience (`web/templates/**` + official plugin templates)

## 1. Background

After the daisyUI migration, the current UI is functionally correct but visually plain.  
Compared with the previous polished version, the current experience has weaker enterprise character:

- weaker visual hierarchy in shell and workspace;
- inconsistent “production-grade” tone across admin and plugin modules;
- dense pages that look mechanically converted instead of intentionally designed.

This change focuses on visual/interaction quality upgrade while preserving all existing behavior contracts.

## 2. Goals

1. Re-establish a premium enterprise admin look-and-feel with clear hierarchy.
2. Unify visual language across core admin modules and official plugins (CMS/KV/File Browser).
3. Keep stack boundaries strict: **daisyUI + Tailwind utilities only** (no second component system).
4. Preserve all existing Rust/Lua route/permission/HTMX contracts.
5. Keep light/dark themes consistent, stable, and production-ready.

## 3. Scope and Non-Goals

### In Scope

- Global admin shell in `web/templates/base.html`.
- Core admin pages:
  - dashboard/users/roles/permissions/menus/plugins/config/logs/login
- Official plugin templates:
  - `plugins/official/cms/web/templates/**`
  - `plugins/official/kv-store/web/templates/**`
  - `plugins/official/file-browser/web/templates/**`
- Shared visual semantics in `web/static/css/input.css` (utility-first, no heavy custom component framework).

### Out of Scope

- No API or RBAC model redesign.
- No route path/contract changes.
- No data model or plugin runtime behavior changes.
- No new theme set beyond existing `light/dark`.

## 4. Design Direction

Chosen direction:

- **Brand-forward enterprise admin** (not consumer “marketing UI”, not raw utilitarian console).
- **Pure daisyUI + Tailwind utility strategy** (no heavy `@layer components` expansion).

Visual principles:

1. **Hierarchy First:** title/tooling/data/action levels are visually obvious in <2 seconds.
2. **Density with Clarity:** medium-high information density, but no cramped controls.
3. **Consistent Action Semantics:** primary/secondary/danger actions keep stable appearance and placement.
4. **Calm Motion:** subtle transitions only where they improve orientation.

## 5. Architecture of the Visual System

### 5.1 Shell-First Experience Contract

`web/templates/base.html` becomes the single source of shell experience:

- brand navigation panel (sidebar) with stronger section structure;
- standardized workspace stage;
- consistent top action region pattern for all modules;
- unified feedback host area (alerts/toasts/loading indicators).

### 5.2 Component Semantics (daisyUI-first)

Use daisyUI semantics as canonical vocabulary:

- layout containers via `card`, `tabs`, `drawer`, `join`;
- data presentation via `table`, `badge`, `stat`;
- interaction via `btn`, `input`, `select`, `textarea`, `modal`, `dropdown`;
- feedback via `alert`, loading states, and toasts.

No legacy `ui-*` class reintroduction.

### 5.3 Theme Behavior

- Keep `<html data-theme="light|dark">` global model.
- Preserve persisted toggle behavior.
- Ensure partial HTMX swaps never reset theme or visual state.

## 6. Module-Level Redesign Plan

### 6.1 Core Admin Modules

- **Dashboard:** executive console layout (KPI row + operational panels + quick actions).
- **List Pages (users/roles/permissions/menus/plugins):**
  - standardized page header
  - toolbar row (search/filter/actions)
  - table card with deterministic footer/pagination alignment
- **Forms/Editors:** consistent label/help/error rhythm and action bar alignment.
- **Login:** enterprise trust-focused entry screen (brand + secure access framing), not generic auth card.

### 6.2 Official Plugin Surfaces

- **CMS:** retain Overview/Library/Editor IA, upgrade hierarchy and panel rhythm.
- **KV Store:** clearer operations-first layout (query/edit/delete clarity and safer destructive action prominence).
- **File Browser:** harmonize with admin shell visual language while preserving file operations flow.

Constraint: plugin modules must visually feel native to the same product.

## 7. Interaction and Feedback Specification

1. HTMX partial updates must always show deterministic loading/empty/error visuals.
2. Success and failure feedback must use shared placement rules:
   - field-level errors near inputs
   - operation-level feedback in module header/alert zone
   - global lightweight toasts for completion notifications
3. Buttons, row hovers, selected tabs, and active nav states follow one interaction rhythm.
4. Motion budget:
   - shell/page transitions: 100-180ms
   - micro-state transitions: 80-140ms
   - no decorative animation loops

## 8. Accessibility and Production Readiness

- Maintain semantic heading order and landmark clarity.
- Preserve keyboard navigability in nav, tabs, forms, and tables.
- Ensure contrast remains AA-acceptable for both `light` and `dark`.
- Preserve focus-visible styles for all actionable controls.

## 9. Delivery Phases

### Phase 1 — Shell and Global Patterns

- Refine `base.html` shell hierarchy.
- Normalize global surface spacing and workspace container behavior.
- Align top-level feedback patterns.

### Phase 2 — Core Admin Page Unification

- Apply standardized patterns to dashboard + CRUD modules + login.

### Phase 3 — Official Plugin Harmonization

- CMS, KV, and File Browser visual convergence with admin language.

### Phase 4 — Regression and Polish Pass

- Cross-module consistency pass.
- Theme pass (`light` + `dark`).
- Interaction QA for HTMX partial flows.

## 10. Acceptance Criteria

All must be true:

1. All targeted admin and official plugin pages share one coherent enterprise visual language.
2. No route/policy/HTMX behavior regressions.
3. `light/dark` theme persistence and swap behavior remain correct.
4. No reintroduction of legacy `ui-*` utility system.
5. Core validations pass:
   - `cargo test -p sushi-admin --test admin_web -q`
   - `cargo test --workspace -q`

## 11. Risks and Mitigations

- **Risk:** Visual-only refactor accidentally breaks HTMX hooks/targets.  
  **Mitigation:** preserve IDs, `data-*`, `hx-*` contracts as immutable during template polish.

- **Risk:** Plugin pages drift from shell standards after local tweaks.  
  **Mitigation:** enforce one shared page skeleton pattern before plugin-specific styling.

- **Risk:** Dark theme readability regression during high-contrast polish.  
  **Mitigation:** run explicit dual-theme QA checklist per module.

---

This design is approved for planning and implementation.
