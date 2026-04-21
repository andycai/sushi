# sushi.web

`sushi.web` provides runtime helpers for rendering and response envelopes.
Page registration itself is contract-first via `sushi.capability.register`.

## Methods

### `sushi.web.render(template_name, context?)`

Render HTML from a template.

### `sushi.web.json(status, data)`

Build a JSON response envelope.

### `sushi.web.download(file_name, mime, body_bytes)`

Build a file download envelope.

## Register a page (contract-first)

```lua
sushi.capability.register({
  surface = "web",
  kind = "page",
  path = "/admin/report",
  title = "Report",
  template = "plugins/official/report/report.html",
  handler = function()
    return sushi.web.render("plugins/official/report/report.html", {
      title = "Report"
    })
  end,
  policy = "admin.report.read"
})
```

## Notes

- `kind` must be `"page"` for page registration.
- `handler` can be a function or an existing handler key string.
- Use plugin-local template paths only.
