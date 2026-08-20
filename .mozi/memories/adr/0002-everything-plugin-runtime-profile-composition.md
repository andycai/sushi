# ADR 0002: Ordered profiles with full-entry replacement

- Status: Accepted
- Date: 2026-08-18

## Context

Sushi product composition is currently encoded in bootstrap, Router merges, and the static Clap command enum. A configurable plugin tree needs deterministic layering, source diagnostics, and a safe way to override shipped defaults.

Recursive deep merge would make array handling, field deletion, source attribution, and schema evolution ambiguous. Required platform plugins also need stronger lifecycle rules than optional feature plugins.

## Decision

Compose runtime entries in this order:

1. bundles in profile declaration order;
2. profile overlays;
3. explicit launch-time overlays.

Entries are addressed by stable `PluginInstanceId`. An overlay replaces the complete matching entry and configuration rather than recursively merging fields. Required entries cannot be toggled through ordinary Admin or CLI governance APIs. The launcher retains bootstrap-safe inspect and recovery commands.

The first Rust plugin implementation uses statically linked builtin factories. Dynamic Rust library ABI, WASM, remote installation, and full Router hot swapping are outside the initial contract.

## Consequences

- Effective composition is deterministic and can be dumped with exact provenance.
- Overrides must restate fields they retain.
- Invalid or unknown overlay targets fail before plugin activation.
- Required identity, policy, governance, and recovery capabilities remain recoverable after profile errors.

## Related

- [Implemented single-path runtime kernel](../../notes/implemented/architecture/2026-08-18-everything-plugin-runtime-single-path-kernel.md)
