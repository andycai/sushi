# sushi.fs

受限文件系统 API，仅在插件 `plugin.toml` 配置了 `[file_browser]` 时注入。

## 可用性

- **需要权限** - 插件必须声明 `file_browser` 配置。
- **能力约束** - 每个 root 的 `capabilities` 独立控制可执行操作。
- **路径解析** - `plugin.toml` 里 `[[file_browser.roots]].path` 若为相对路径，会相对 `config.toml` 的 `[file_browser].root_dir` 解析。

## 元数据

- `sushi.fs.route_prefix` (string): 当前文件浏览器路由前缀（默认 `/app/files`）。
- `sushi.fs.roots()` -> `table[]`: 返回所有 root 的配置快照：
  - `id`
  - `title`
  - `path`（canonical 绝对路径）
  - `capabilities`（`can_*` 布尔开关）

## 方法

### `sushi.fs.list(root_id, rel_path)`

列出目录内容。

### `sushi.fs.read_text(root_id, rel_path)`

读取文本文件（受 `text_extensions` 白名单约束）。

### `sushi.fs.write_text(root_id, rel_path, content)`

写入文本文件（仅覆盖已有文件）。

### `sushi.fs.create_text(root_id, rel_path, initial_content?)`

创建新的文本文件（`create_new`，目标已存在会报冲突）。

### `sushi.fs.mkdir(root_id, rel_path)`

创建目录。

### `sushi.fs.rename(root_id, from_rel_path, to_rel_path)`

重命名文件或目录（目标已存在会报冲突；目录目标不允许位于源目录内部）。

### `sushi.fs.delete(root_id, rel_path)`

删除文件或空目录。

### `sushi.fs.write_upload(root_id, rel_path, bytes)`

以二进制写入上传文件（目标已存在会报冲突）。

### `sushi.fs.prepare_download(root_id, rel_path)`

准备下载元数据，返回：

- `root_id`
- `rel_path`
- `file_name`
- `size`

### `sushi.fs.read_download(root_id, rel_path)`

读取下载内容，返回：

- `root_id`
- `rel_path`
- `file_name`
- `size`
- `content` (Lua string bytes)

## 常见错误码

`sushi.fs` 出错时会抛出 Lua runtime error，前缀为稳定错误码：

- `invalid_path`
- `root_not_found`
- `permission_denied`
- `forbidden_hidden`
- `forbidden_symlink`
- `not_text_file`
- `not_found`
- `conflict`
- `not_empty_dir`
- `io_error`
