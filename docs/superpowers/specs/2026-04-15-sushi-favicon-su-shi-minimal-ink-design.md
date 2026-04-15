# Sushi Favicon (Su Shi) Minimal Ink Design (2026-04-15)

## 1. Goal

Redesign `favicon.svg` to represent **Su Shi (苏轼)** rather than food sushi, using a **minimal black/white/gray ink-poetry visual language** inspired by GitHub-like restraint.

## 2. Confirmed Design Decisions

- Semantic meaning: `sushi` = Su Shi (poet), not sushi food.
- Visual style: abstract only (no portrait, no figurative face/body).
- Color direction: black/white/gray ink style.
- Product tone: concise, modern, minimal, high-recognition at tiny favicon sizes.

## 3. Scope

### In scope

- Redesign `web/static/favicon.svg`.
- Keep favicon implementation SVG-first.
- Maintain compatibility with current template references to SVG favicon.

### Out of scope

- No PNG/favicon pack generation in this task.
- No logo system or full brand redesign.
- No animation.
- No textual poetry glyphs/characters in favicon.

## 4. Visual Concept

Use a two-symbol abstraction:

1. **Primary ink dot** (`墨点`): represents the poet's core spirit and "one point contains meaning".
2. **Poetry lines** (`诗行`): 2-3 short horizontal strokes with rhythm and truncation, representing verse structure.

The icon relies on composition and whitespace rather than illustration.

## 5. Composition and Geometry Rules

Canvas baseline: `32x32` with transparent background.

Global rules:

- Preserve generous negative space (target >= 55% visual whitespace).
- Keep shapes bold enough for 16x16 rendering.
- Avoid thin decorative detail that disappears at small scale.

Candidate variants (to draft in SVG):

### V1 (Recommended)

- Primary ink dot near left-upper center.
- Dot radius approx `6.2` equivalent visual mass, slightly organic edge (not mathematically perfect circle feel).
- Three poetry lines in right/lower area with descending lengths (`11 / 8 / 5`), line gap approx `3.2`.
- Color mapping:
  - Dot: `#111111`
  - Lines: `#6B7280`

### V2 (More literary, lighter)

- Dot slightly smaller (approx `5.6` visual radius).
- Same concept with sparser stroke spacing and lighter overall density.
- Risk: can become too faint at 16x16.

### V3 (More product-modern)

- Stronger geometric alignment between dot and first line.
- Slightly thicker lines and tighter grouping.
- Better micro-size readability but less poetic softness.

## 6. Selection Policy

Default output should use **V1** because it best balances:

- tiny-size recognizability,
- poetic abstraction,
- GitHub-like minimal restraint.

V2/V3 can be kept as optional alternates for later A/B review.

## 7. Technical Constraints

- File path: `web/static/favicon.svg`.
- Use only SVG primitives/path; keep structure maintainable.
- Keep transparent background.
- No external dependencies.

## 8. Quality and Acceptance Criteria

The redesign is accepted when:

1. Browser tab favicon visibly reads as "ink dot + verse lines" at 16x16/32x32.
2. No food-sushi visual metaphor remains.
3. Visual tone is minimal and restrained (no busy decoration).
4. No favicon-related 404 regressions are introduced.
5. Existing admin tests remain green.

## 9. Verification Plan

- Manual visual check in browser tab at normal and zoomed views.
- Confirm no favicon 404 in DevTools network panel.
- Run: `cargo test -p sushi-admin -q`.

## 10. Risks and Mitigations

- **Risk:** Too much detail disappears at small sizes.
  - **Mitigation:** keep large simple masses and strong contrast.

- **Risk:** Icon appears generic abstract mark without poetic cue.
  - **Mitigation:** retain clear "dot + rhythmic line" relationship.

- **Risk:** Over-fitting to desktop tab rendering only.
  - **Mitigation:** validate at multiple favicon sizes.

## 11. Implementation Boundary

Only the favicon asset is changed in this design cycle. No additional template or routing changes are required.
