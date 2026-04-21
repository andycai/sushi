# sushi.event

`sushi.event` is the event bus namespace for Lua plugins.

## Runtime methods

- `sushi.event.on(event, callback)`
- `sushi.event.emit(event, data)`

## Contract declaration (for capability metadata)

```lua
sushi.capability.register({
  surface = "event",
  kind = "emit",
  event = "content.published"
})
```

## Notes

- Use namespaced event names to avoid collisions.
- Keep event payloads JSON-serializable.
- Event handler behavior should be idempotent where possible.
