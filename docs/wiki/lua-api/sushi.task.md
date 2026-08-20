# sushi.task

`sushi.task` registers background work owned by the current plugin instance. It is always injected, but registered work does not start until plugin activation commits successfully.

## Methods

### `sushi.task.spawn(name, callback)`

Runs `callback` once after activation. The callback receives no arguments.

```lua
function sushi.init()
  sushi.task.spawn("warm-cache", function()
    sushi.log.info("warming plugin cache")
  end)
end
```

### `sushi.task.interval(name, interval_ms, callback)`

Runs `callback` repeatedly on the given positive millisecond interval. A callback error is logged and stops that interval task.

```lua
function sushi.init()
  sushi.task.interval("refresh-index", 30000, function()
    sushi.log.info("refreshing plugin index")
  end)
end
```

## Lifecycle rules

- Register tasks during `sushi.init()`.
- Names must be non-empty, contain no control characters, and be unique within one activation.
- Activation failure discards deferred tasks without starting them.
- Disable, reload, or host shutdown cancels all tasks for that owner.
- Callbacks should return promptly and avoid blocking the Lua runtime.
