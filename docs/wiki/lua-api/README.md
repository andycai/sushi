# Lua API Overview (V2 Contract-First)

Sushi Lua plugins now use a contract-first registration model.

## Registration entrypoint

Use `sushi.capability.register({...})` for all plugin capability declarations.

```lua
local function register(surface, payload)
  payload.surface = surface
  sushi.capability.register(payload)
end
```

## Runtime namespaces

| Namespace | Purpose | Injection rule |
| --- | --- | --- |
| `sushi.capability` | Capability registration | Always injected |
| `sushi.log` | Structured logs | Always injected |
| `sushi.json` | JSON encode/decode | Always injected |
| `sushi.config` | Config access (read-only stub) | Always injected |
| `sushi.event` | Event bus | Always injected |
| `sushi.auth` | Token verification | Always injected |
| `sushi.db` | Database query/execute | Injected when DB permission is granted |
| `sushi.web` | HTML render/JSON/download helpers | Injected when admin or routes permission is granted |
| `sushi.fs` | File browser capabilities | Injected when file-browser roots are configured |

## Contract surfaces

| Surface | Required fields |
| --- | --- |
| `api` | `method`, `path`, `handler` |
| `web` | `kind = "page"`, `path`, `handler` |
| `cli` | `name`, `description`, `handler` |
| `db` | `kind`, `name` |
| `event` | `kind`, `event` |
| `fs` | `kind`, `root` |

See `docs/wiki/guides/lua-contract-migration.md` for migration details.
