# sushi.api (Contract-First Route Registration)

In V2, API routes should be declared through `sushi.capability.register`.

## Register an API route

```lua
sushi.capability.register({
  surface = "api",
  method = "GET",
  path = "/api/items",
  handler = handlers.items_list,
  policy = "api.items.read"
})
```

## Payload fields

- `surface` (string, required): must be `"api"`.
- `method` (string, required): HTTP method.
- `path` (string, required): route path.
- `handler` (function|string, required): Lua function or handler key.
- `policy` (string, optional): policy key bound to this route.
- `public` (boolean, optional): mark route as public.

## Constraints

- `policy` and `public = true` cannot both be set.
- Policy key must match plugin policy scopes.
- Route permissions are deny-by-default when plugin permission is absent.

## Response shape

Route handlers should return:

```lua
return {
  status = 200,
  body = { data = "ok" }
}
```
