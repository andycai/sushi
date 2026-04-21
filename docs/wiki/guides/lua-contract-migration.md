# Lua Contract Migration Guide (V2)

This guide explains how to migrate Sushi plugins from direct registration APIs to the
contract-first kernel introduced in V2.

## Why this change

- Keep Rust runtime APIs stable while plugin surface grows.
- Move plugin capability declaration to one unified shape.
- Enforce deny-by-default capability visibility from manifest + runtime governance.

## Breaking change summary

Legacy registration style:

- `sushi.api.route(...)`
- `sushi.cli.command(...)`
- `sushi.admin.page(...)`
- `sushi.web.page(...)`

V2 contract-first style:

```lua
sushi.capability.register({
  surface = "api",
  method = "GET",
  path = "/api/items",
  handler = handlers.items_list,
  policy = "api.items.read"
})
```

## Core rules

- Every registration goes through `sushi.capability.register({...})`.
- `surface` is mandatory (`api`, `web`, `cli`, `db`, `event`, `fs`).
- Policy keys must stay inside plugin-declared policy scopes.
- API route cannot set `policy` and `public = true` at the same time.

## Surface payloads

### API route

```lua
sushi.capability.register({
  surface = "api",
  method = "POST",
  path = "/api/items",
  handler = handlers.items_create,
  policy = "api.items.write"
})
```

### Admin/web page

```lua
sushi.capability.register({
  surface = "web",
  kind = "page",
  path = "/admin/items",
  title = "Items",
  template = "plugins/official/items/items.html",
  handler = function()
    return sushi.web.render("plugins/official/items/items.html")
  end,
  policy = "admin.items.read"
})
```

### CLI command

```lua
sushi.capability.register({
  surface = "cli",
  name = "items",
  description = "Items command",
  handler = handlers.items_cli,
  policy = "cli.items.execute"
})
```

## Migration checklist

1. Replace direct registrations with `sushi.capability.register` payloads.
2. Keep route/command/page paths and command names unchanged.
3. Keep policy keys unchanged unless intentionally re-scoped.
4. Re-run plugin behavior tests and route coverage tests.
5. Verify plugin manifest permissions still match declared capabilities.

## Security model

Capabilities blocked by permissions or governance state are not injected into Lua.
This is deny-by-default at injection time.
