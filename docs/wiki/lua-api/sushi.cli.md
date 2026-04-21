# sushi.cli (Contract-First Command Registration)

In V2, CLI commands should be declared through `sushi.capability.register`.

## Register a CLI command

```lua
sushi.capability.register({
  surface = "cli",
  name = "greet",
  description = "Print greeting",
  handler = handlers.greet,
  policy = "cli.greet.execute"
})
```

## Payload fields

- `surface` (string, required): must be `"cli"`.
- `name` (string, required): command name.
- `description` (string, required): command help text.
- `handler` (function|string, required): command handler.
- `policy` (string, optional): policy key for command execution.

## Conventions

- Prefer stable command names.
- Validate arguments early and emit actionable errors.
- Keep output concise and machine-friendly where possible.
