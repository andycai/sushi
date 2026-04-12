---
name: sushi-cli-command-delivery
description: Use when adding or updating Sushi CLI subcommands so argument design, bootstrap wiring, output semantics, and plugin command integration stay consistent.
---

# Sushi CLI Command Delivery

## Overview

Use this skill for new `sushi` subcommands and for behavior changes to existing commands.

## Required Pattern

1. Add/adjust command args in `crates/sushi-cli/src/commands/<command>.rs`.
2. Wire command dispatch in `crates/sushi/src/main.rs` and `crates/sushi-cli/src/commands/mod.rs` if needed.
3. Reuse shared runtime initialization through `bootstrap(...)`.
4. Validate arguments early and return actionable error messages.

## Output Rules

- Success output is concise and script-friendly.
- Errors include what failed and, when possible, the next action.
- Avoid noisy debug output in normal command flow.

## Plugin Integration

- Ensure plugin commands remain discoverable and isolated from core command failures.
- Keep command names stable unless there is a migration plan.

## Done Criteria

- Command help text is clear (`--help` output reviewed).
- Command works with default config and explicit config path.
- `cargo test --workspace -q` passes.

