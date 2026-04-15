# Su Shi Minimal Ink Favicon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the project favicon to a minimal abstract "Su Shi + poetry" black/white/gray mark and remove obsolete PNG favicon loading behavior.

**Architecture:** Keep favicon delivery SVG-first using a transparent background and a compact abstract composition (ink dot + poetic line rhythm) optimized for 16x16 readability. Enforce the contract in tests: base template references SVG favicon and does not reference non-existent PNG fallback.

**Tech Stack:** SVG, MiniJinja HTML templates, Rust integration tests (`cargo test`), Sushi admin template pipeline.

---

## File Structure Map

- Modify: `web/static/favicon.svg` (new visual design)
- Modify: `web/templates/base.html` (favicon link contract)
- Modify: `crates/sushi-core/tests/template_service.rs` (contract tests for favicon refs)

---

### Task 1: Add Favicon Contract Tests (TDD)

**Files:**
- Modify: `crates/sushi-core/tests/template_service.rs`
- Test: `crates/sushi-core/tests/template_service.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn base_template_uses_svg_favicon_only() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root missing");
    let templates_dir = workspace_root.join("web").join("templates");

    let svc = TemplateService::new(&templates_dir).unwrap();
    let html = svc.render("base.html", serde_json::json!({})).unwrap();

    assert!(html.contains("favicon.svg"));
    assert!(!html.contains("favicon.png"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core base_template_uses_svg_favicon_only -q`
Expected: FAIL if `base.html` still references `favicon.png`.

- [ ] **Step 3: Implement minimal code to satisfy the test**

```html
<!-- web/templates/base.html (head) -->
<link rel="icon" type="image/svg+xml" href="{{ static_prefix }}/favicon.svg">
```

(Ensure no `favicon.png` link remains.)

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p sushi-core base_template_uses_svg_favicon_only -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/tests/template_service.rs web/templates/base.html
git commit -m "test(web): enforce svg-only favicon contract"
```

---

### Task 2: Implement Su Shi Minimal Ink SVG (V1)

**Files:**
- Modify: `web/static/favicon.svg`

- [ ] **Step 1: Replace favicon SVG with V1 composition**

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img" aria-label="Sushi favicon">
  <!-- Primary ink dot (poet spirit) -->
  <path d="M9.6 8.2c2.8-1.3 6 .4 6.7 3.3.9 3.6-1.5 6.5-4.9 7.1-3.7.7-6.8-2.3-6.6-5.8.2-2.1 1.7-3.8 3.8-4.6Z" fill="#111111"/>

  <!-- Poetry lines (rhythmic verse) -->
  <rect x="16.6" y="16.2" width="11" height="1.9" rx="0.95" fill="#6B7280"/>
  <rect x="16.6" y="20.0" width="8"  height="1.9" rx="0.95" fill="#6B7280"/>
  <rect x="16.6" y="23.8" width="5"  height="1.9" rx="0.95" fill="#6B7280"/>
</svg>
```

- [ ] **Step 2: Validate SVG quickly**

Run: `python - <<'PY'\nimport xml.etree.ElementTree as ET\nET.parse('web/static/favicon.svg')\nprint('ok')\nPY`
Expected: prints `ok`.

- [ ] **Step 3: Commit**

```bash
git add web/static/favicon.svg
git commit -m "feat(brand): redesign favicon as su shi minimal ink mark"
```

---

### Task 3: Regression Verification (Admin + Core)

**Files:**
- Modify: none (verification only unless failures require small fix)

- [ ] **Step 1: Run focused core tests**

Run: `cargo test -p sushi-core -q`
Expected: PASS.

- [ ] **Step 2: Run admin tests**

Run: `cargo test -p sushi-admin -q`
Expected: PASS.

- [ ] **Step 3: Browser/manual smoke check**

Run app and confirm:
- Tab icon renders as monochrome abstract mark.
- DevTools network has no `GET /static/favicon.png` request.

Suggested run command:

```bash
cargo run -p sushi -- serve
```

- [ ] **Step 4: Final checkpoint commit (only if verification introduced fixes)**

```bash
git add -A
git commit -m "chore(web): finalize su shi favicon rollout"
```

---

## Spec Coverage Self-Review

- **Su Shi semantic correction:** handled via abstract poetry motif in Task 2.
- **Abstract-only style:** no portrait or figurative elements in SVG.
- **Black/white/gray palette:** explicit color values in Task 2.
- **Minimal implementation scope:** only favicon asset, template reference contract, and tests.
- **No PNG fallback:** enforced in Task 1 test and template update.

## Placeholder / Consistency Self-Check

- No TODO/TBD placeholders.
- Concrete file paths, commands, and expected outcomes provided for each step.
- Naming consistent with approved design language (Su Shi, minimal ink, SVG-first).
