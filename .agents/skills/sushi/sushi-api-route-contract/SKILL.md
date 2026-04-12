---
name: sushi-api-route-contract
description: Use when adding or changing Sushi API routes to keep REST semantics, validation, status codes, and Rust/Lua plugin response behavior consistent.
---

# Sushi API Route Contract

## Overview

Use this skill when route changes may affect admin UI, CLI consumers, or plugin-based handlers.

## Contract Checklist

- Route naming is resource-oriented (`/api/<resource>`).
- HTTP method matches action (`GET/POST/PUT/PATCH/DELETE`).
- Input validation happens before DB or side effects.
- Error status is explicit:
  - `400` invalid payload
  - `404` missing resource
  - `409` conflict (if applicable)
  - `5xx` server/runtime failure

## Rust + Lua Parity

- If route is intended for plugin usage, verify Lua handler behavior is equivalent.
- For Lua JSON responses, use `sushi.web.json(status, payload)` where status must be preserved by API dispatch.
- Do not silently change response shape consumed by existing pages or scripts.

## Observability

- Log route failures with enough context for triage (without leaking secrets).
- Keep user-facing error payloads stable and concise.

## Done Criteria

- Existing route tests pass.
- New/changed route has at least one test or explicit manual verification note.
- `cargo test --workspace -q` passes.

