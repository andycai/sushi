# sushi.db

`sushi.db` exposes SQL capabilities to Lua plugins when database permission is granted.

## Runtime methods

- `sushi.db.query(sql, params?)`
- `sushi.db.execute(sql, params?)`

## Contract declaration (for capability metadata)

```lua
sushi.capability.register({
  surface = "db",
  kind = "query",
  name = "posts_read"
})
```

## Permission model

- No DB permission: `sushi.db` is not injected.
- Read permission: query-only practical use.
- Write/admin permission: mutating SQL allowed.

All DB visibility follows deny-by-default injection rules.
