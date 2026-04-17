# Official CMS Admin Editorial Workbench Design

Date: 2026-04-18  
Status: Approved for planning (pending implementation plan)  
Scope: Official CMS plugin admin experience (`/admin/cms` and related admin flows)

## 1. Context and Problem Statement

The current CMS admin surface is functionally incomplete and visually inconsistent with the quality bar expected of a daily content tool. It feels like a generic table utility instead of a writing workspace. This causes low trust and poor adoption.

Primary complaint from stakeholders: the interface looks amateur and is hard to use for real publishing work.

## 2. Product Direction (Confirmed)

- Visual direction: modern editorial workbench (not a utility dashboard skin)
- Default landing experience: **Overview dashboard**
- Editing mode: **single-column immersive editor**
- Interaction priority: **keyboard-first workflow and shortcuts**
- Success criterion: team sentiment shifts from “ugly/hard to use” to “good enough to use long-term”
- Delivery strategy: **Approach B** (front-end full redesign + necessary back-end/API adjustments)

## 3. Goals and Non-Goals

### Goals

1. Create a coherent CMS admin IA: Overview → Editor → Library.
2. Replace “multi-table cram page” with role-specific surfaces.
3. Make writing and publishing faster through command palette + shortcut system.
4. Keep plugin architecture compliant (Lua plugin, template-local assets, policy-safe routes).
5. Preserve existing CMS domain behavior (page/post/category, soft delete, category guard).

### Non-Goals (for this phase)

1. Real-time collaborative editing.
2. Full version history and rollback UI.
3. Public-site theming redesign.
4. Rebuilding the entire admin design system outside CMS scope.

## 4. Information Architecture

Top navigation replaces left sidebar for CMS workflows.

Primary CMS navigation:

- `Overview` (default)
- `Posts`
- `Pages`
- `Categories`

Surface responsibilities:

- Overview: “what should I do now?”
- Posts/Pages/Categories: searchable, filterable libraries
- Editor: dedicated immersive writing mode for create/edit

## 5. UX Blueprint

### 5.1 Top Navigation

- Left: brand + CMS primary tabs
- Center: global search / command launcher affordance
- Right: quick create (post/page), user menu
- Mobile: condensed menu + persistent search/action entry

### 5.2 Overview Dashboard (default `/admin/cms`)

Cards/sections:

1. **Today’s queue**: drafts, pending publish actions, recent edits.
2. **Recently edited**: quick resume links (open directly in editor).
3. **Content health**: issues such as uncategorized posts, stale drafts.
4. **Fast actions**: New Post / New Page / Jump to Library.

Design behavior:

- Every card leads to a next action.
- Empty states provide explicit “do this now” actions.
- Keyboard-focusable widgets for non-mouse users.

### 5.3 Immersive Editor

Single-column writing-first surface:

- Main area: title + markdown body
- Optional collapsible side drawer: slug, status, category, excerpt, metadata
- Persistent slim status bar: save state, publish action, shortcut hints

Interaction design:

- Minimize visual chrome while writing
- Keep critical publishing controls always reachable
- Preserve cursor and scroll position when returning to draft

### 5.4 Library Views (Posts / Pages / Categories)

Unified list framework:

- Search + filter + sort toolbar
- Row-level quick actions: edit, status change, delete
- Batch-ready architecture (UI prepared; bulk ops can be enabled incrementally)

Consistency:

- Shared table patterns across all three modules
- Consistent status badges, empty states, and destructive action confirmations

## 6. Keyboard and Command Interaction Model

### Global Shortcuts

- `Cmd/Ctrl + K`: open command palette
- `G O`: go to Overview
- `G P`: go to Posts
- `G A`: create new post

### Editor Shortcuts

- `Cmd/Ctrl + S`: save
- `Cmd/Ctrl + Enter`: publish
- `Cmd/Ctrl + Shift + P`: open status transition action
- `Esc`: close side drawer / contextual panel

### Library Shortcuts

- `/`: focus search
- `J / K`: move row selection
- `E`: edit selected item
- `Del`: delete selected item (still requires confirmation)

### Command Palette

Unified action endpoint for:

- Navigation commands
- Content actions (create, open, publish)
- Quick open by title/slug

Palette output format should support consistent action/result rendering from back-end APIs.

## 7. Visual and UI Principles

1. Editor-first aesthetics over admin-panel aesthetics.
2. Clear typography hierarchy (title/body/meta) optimized for writing.
3. Lower visual noise: lighter borders, stronger spacing rhythm.
4. Consistent feedback language: inline state + toast, no fragmented status signaling.
5. Destructive actions must look explicit and high-friction.

## 8. Technical Design (Approach B)

## 8.1 Front-end Architecture

Refactor CMS UI into modular templates/components under plugin-local paths:

- `overview` fragments
- `library` fragments (posts/pages/categories)
- `editor` fragment(s)
- shared feedback and command palette partials

JS module responsibilities (`cms.js` split as needed):

- routing/state orchestration within CMS workspace
- keyboard dispatcher
- command palette controller
- optimistic save/publish feedback orchestration

### 8.2 Back-end/API Adjustments (necessary scope only)

Keep existing domain modules (`page`, `post`, `category`), add orchestration APIs:

1. **Overview aggregate endpoint**  
   Returns dashboard counters + recent edits + health signals.

2. **Unified editor save endpoint shape**  
   Harmonizes response contracts used by editor regardless of content type.

3. **Status transition endpoint(s)**  
   Explicit state-change API with clear error messages and policy checks.

4. **Command palette query endpoint**  
   Returns action candidates + jump targets.

Policy key format remains compliant with existing `surface.resource.action`.

### 8.3 Compatibility Constraints

- Continue using plugin-local templates/static only.
- Do not embed raw HTML inside Lua source.
- Preserve `/app/*` public routes behavior.
- Keep soft-delete and category-delete guards unchanged semantically.

## 9. Error Handling and Edge Cases

1. Save failures are non-blocking and visible in persistent editor status.
2. Validation errors are field-level first, toast second.
3. Unsaved changes prompt on leave.
4. Publish failure shows explicit actionable reason.
5. Delete actions always require confirmation.
6. API/network failure surfaces retry affordances without losing draft text.

## 10. Testing and Verification Strategy

### Unit / Contract

- Keyboard dispatcher command mapping tests
- Command palette response contract tests
- Editor save/status response envelope tests

### Integration

- `/admin/cms` loads Overview by default
- Shortcut flows trigger expected navigation/actions
- Post/Page/Category CRUD from redesigned views
- Status transitions and guard errors render correctly

### Regression

- Existing CMS domain rules remain intact
- Existing admin auth/policy checks remain enforced
- Public `/app` routes still render correctly

## 11. Acceptance Criteria

Functional:

1. Users can create/edit/delete posts/pages/categories from redesigned UI.
2. Overview dashboard is default and actionable.
3. Immersive editor workflow works with keyboard-first controls.
4. Command palette supports navigation + content actions.

Quality:

1. Visual consistency with modern editorial workbench direction.
2. No major UX dead-end states (all empty/error states suggest next action).
3. Internal team feedback indicates clear improvement in “looks” and “usability”.

## 12. Delivery Boundaries

This design intentionally focuses on CMS admin experience only.  
Future enhancements (version history, collaboration, workflow automation) should build on this architecture rather than be mixed into this phase.

---

## Spec Self-Review

### Placeholder Scan
- No `TODO`/`TBD` placeholders remain.

### Internal Consistency
- IA, shortcut model, and API adjustments align with Approach B and confirmed user choices.

### Scope Check
- Focused on one subsystem (CMS admin).  
- Broader admin-wide redesign is explicitly out of scope.

### Ambiguity Check
- Default landing, nav placement, editor mode, and priority interaction model are all explicit.
