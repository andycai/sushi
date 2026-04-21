# sushi.admin (Admin Surface in V2)

Admin pages are registered via the `web` contract surface in V2.

## Register an admin page

```lua
sushi.capability.register({
  surface = "web",
  kind = "page",
  path = "/admin/my-plugin",
  title = "My Plugin",
  template = "plugins/third_party/my-plugin/admin.html",
  handler = function()
    return sushi.web.render("plugins/third_party/my-plugin/admin.html")
  end,
  policy = "admin.my_plugin.read"
})
```

## Notes

- Keep admin route paths stable for bookmarks and navigation.
- Use policy keys (`admin.*`) for all non-public admin pages.
- Render through templates; avoid embedding raw HTML in Lua source.
