---
name: sushi-release-verification
description: Use when preparing Sushi changes for commit or merge to enforce a consistent verification gate across admin, API, CLI, and plugin surfaces.
---

# Sushi Release Verification

## Overview

Run this checklist before claiming work is complete.

## Verification Gate

1. Rust tests:
   - `cargo test -p sushi-admin --test admin_web -q`
   - `cargo test --workspace -q`
2. Frontend safety checks:
   - `rg -n "hx-confirm|alert\(|confirm\(" web/templates web/static --glob '!web/static/js/htmx.min-2.0.8.js'`
3. Syntax checks for touched JS files:
   - `node --check <file>`
4. Confirm no unrelated file edits:
   - `git status --short`

## Quality Questions

- Did this change preserve Rust/Lua parity expectations?
- Are permissions and auth boundaries still explicit?
- Are admin mutations deterministic (feedback + refresh + state persistence)?
- Are docs/skills updated if behavior conventions changed?

## Done Criteria

- All required checks pass.
- Any skipped check is explicitly documented with reason and risk.

