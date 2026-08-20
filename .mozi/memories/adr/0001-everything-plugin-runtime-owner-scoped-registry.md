# ADR 0001: Owner-scoped capability registry

- Status: Accepted
- Date: 2026-08-18

## Context

Sushi currently stores Lua VMs and API/Admin/CLI bindings in independent maps owned by `PluginManager`. Registration is immediately visible, duplicate keys overwrite previous owners, and disabling a plugin only adds a dispatch-time guard. This cannot provide atomic activation, reliable rollback after partial initialization, or complete owner-level deactivation.

## Decision

Introduce a transport-neutral capability registry in `sushi-core` with:

- stable `PluginId`, `PluginInstanceId`, and `RegistrationId` identities;
- owner-scoped API route, Admin page, and CLI command registrations;
- staging transactions whose contents are invisible until commit;
- immutable snapshots published atomically after validation;
- fail-closed conflict detection that reports both owners;
- owner-level removal that publishes a new snapshot.

The first implementation remains inside `sushi-core`. `PluginManager` acts as a compatibility facade while existing VM dispatch maps remain in place. Physical crate extraction is deferred until the contract stabilizes.

## Consequences

- New runtime registration cannot exist without an owner.
- Plugin activation can later stage all effects and commit them together.
- Existing callers keep their current `PluginManager` APIs during migration.
- There is temporary duplication between snapshot metadata and legacy handler maps; later slices remove the legacy maps after dispatch migrates.

## Related

- [Implemented single-path runtime kernel](../../notes/implemented/architecture/2026-08-18-everything-plugin-runtime-single-path-kernel.md)
