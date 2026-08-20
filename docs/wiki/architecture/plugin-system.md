# 插件系统

Sushi 将 Rust builtin 与 Lua package 都视为由 profile 选择的运行时插件。宿主只保留配置、权限强制、生命周期、迁移、运行时隔离和 transport dispatch 等可信边界。

## 插件来源

| Source | 交付方式 | 信任来源 |
|---|---|---|
| `builtin:<key>` | 编译期静态链接的 `BuiltinPluginFactory` | 宿主注册的 factory key |
| `lua:official/<name>` | `plugins/official/<name>` | 宿主管理的 official 路径 |
| `lua:third_party/<name>` | `plugins/third_party/<name>` | 宿主管理的 third-party 路径 |

插件不能在 `plugin.toml` 中自报 trust tier。未知 builtin、绝对 Lua 路径、`..`、缺失 manifest 和重复 Lua source 都在启动前失败。

## Lua package 结构

```text
plugins/<tier>/<name>/
├── plugin.toml
├── init.lua
├── migrations/
├── lua/
└── web/
    ├── templates/
    └── static/
```

`plugin.toml` 必须包含顶层 `schema_version = 1`。旧 schema、缺失版本和未来版本均 fail closed。

## 权限交集

Lua 插件的有效权限是以下四层的交集：

1. source/path 对应的宿主信任上限；
2. manifest `[permissions]` 请求；
3. profile `[entries.grants]` 的收窄字段；
4. 管理员显式 `approved = true`。

```toml
[[entries]]
id = "example.default"
source = "lua:third_party/example"
enabled = true
required = false

[entries.grants]
approved = true
routes = true
database = "read"
```

未设置 `approved = true` 时，宿主不会执行插件入口，也不会发布 route、command、Admin、event、task、auth 或数据库等任何 effect；required 条目会在打开数据库前失败。Grant 只能降低 manifest 请求，不能提升权限。

## 注册模型

- API route、Admin page、CLI command、menu、template root、static root 和 event subscription 进入 owner-scoped staged `CapabilityRegistry`。
- 注册完成且冲突/权限检查通过后，runtime 原子发布 immutable `CapabilitySnapshot`。
- Lua VM 只在 capability commit 时发布；失败 activation 不留下半注册状态。
- 后台任务进入独立的 owner-scoped `TaskRegistry`，但与 capability 共享同一个 `PluginInstanceId` 生命周期。
- Task 直到 capability/VM 发布成功后才启动；disable、reload 和 host shutdown 会取消对应 generation。

旧 `sushi.api`、`sushi.cli`、`sushi.admin`、`sushi.web` 与 `sushi.event.on` 是写入同一 contract registry 的语法 adapter，不是第二套注册系统。新插件应优先使用 `sushi.capability.register({...})`。

## 组合与治理

- 产品组合只来自 profile、bundle 和 overlay；默认 profile 缺失时不会扫描目录兜底。
- `required = true` 的系统插件不能通过普通 Admin/CLI toggle 禁用，只能修改 profile 后受控重启。
- optional Lua 插件可通过治理接口无重启 disable/enable。
- `plugin_state.enabled` 保存 optional 插件的治理意图；`loaded` 表示本次 activation 是否成功。

详细的内核拓扑见 [插件运行时](plugin-runtime.md)，状态转换见 [插件生命周期指南](../guides/plugin-lifecycle.md)。

## 相关文档

- [Profile 组合指南](../guides/profile-composition.md)
- [插件生命周期指南](../guides/plugin-lifecycle.md)
- [Lua API 参考](../lua-api/README.md)
- [插件编写规范](../../engineering/plugin-authoring-standards.md)
