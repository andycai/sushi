# sushi.fs

`sushi.fs` provides restricted filesystem operations for file-browser plugins.

## Runtime methods

- `sushi.fs.roots()`
- `sushi.fs.list(root_id, rel_path)`
- `sushi.fs.read_text(root_id, rel_path)`
- `sushi.fs.write_text(root_id, rel_path, content)`
- `sushi.fs.create_text(root_id, rel_path, initial_content?)`
- `sushi.fs.mkdir(root_id, rel_path)`
- `sushi.fs.rename(root_id, from_rel_path, to_rel_path)`
- `sushi.fs.delete(root_id, rel_path)`
- `sushi.fs.write_upload(root_id, rel_path, bytes)`
- `sushi.fs.prepare_download(root_id, rel_path)`
- `sushi.fs.read_download(root_id, rel_path)`

## Contract declaration (for capability metadata)

```lua
sushi.capability.register({
  surface = "fs",
  kind = "root",
  root = "docs"
})
```

## Safety rules

- All paths are scoped to configured roots.
- Hidden/symlink and capability restrictions are enforced.
- Use `sushi.web.download(...)` for HTTP download responses.
