# 插件系统

## 设计原则

1. **Rust 和 Lua 平权** - 所有功能既可用 Rust 实现也可用 Lua 实现
2. **沙箱隔离** - 每个 Lua 插件运行在独立的 VM 中
3. **权限控制** - 插件必须声明权限才能访问敏感 API
4. **声明式** - 插件通过 manifest (plugin.toml) 声明元数据和权限
5. **运行时治理优先** - 生产环境是否激活由平台治理状态决定，而不是由插件自行决定

## 插件结构

```
plugins/
└── example_plugin/
    ├── plugin.toml      # 插件清单
    └── init.lua         # 入口文件
```

## plugin.toml 格式

```toml
[plugin]
name = "example_plugin"
version = "0.1.0"
description = "An example plugin"
entry = "init.lua"        # 可选，默认 init.lua

[permissions]
routes = true            # HTTP 路由注册
commands = true          # CLI 命令注册
admin = true             # Admin 页面扩展
database = "write"       # 数据库权限
```

## 权限说明

| 权限 | 类型 | 说明 |
|-----|------|------|
| `routes` | bool | 注册 HTTP API 路由 |
| `commands` | bool | 注册 CLI 子命令 |
| `admin` | bool | 扩展 Admin 管理面板 |
| `database` | string | 数据库访问级别 |

> `plugin.toml` 中的 `[permissions]` 是**能力上限声明**（ceiling），用于限制插件最多能申请哪些能力；它不等于“强制启用令”。
> 即使 manifest 声明了 `routes = true`，平台仍可在运行时将该插件禁用，插件将不会被分发执行。

### DatabasePermission 值

| 值 | 说明 |
|----|------|
| `false` / `None` | 无数据库访问 |
| `true` / `"read"` | 只读查询 |
| `"write"` | 读写（INSERT/UPDATE/DELETE） |
| `"admin"` | 完全访问（包括 DROP/ALTER） |

## 插件生命周期

```
启动扫描
    │
    ▼
解析 plugin.toml
    │
    ▼
权限上限验证
    │
    ▼
读取平台治理状态（plugin_state.enabled）
    │
    ▼
enabled=true: 创建独立 Lua VM
enabled=false: 跳过 init，标记未加载
    │
    ▼
注入 sushi API
    │
    ▼
调用 init.lua:sushi.init()
    │
    ▼
收集注册的路由/命令/页面
    │
    ▼
插件激活
```

## 运行时治理模型（V1）

- 统一真相来源：`plugin_state.enabled`（数据库）是插件激活状态的运行时权威来源。
- 即时生效：Admin/CLI 切换启用状态后，API/Admin/CLI 分发路径立即按新状态执行，无需重启服务。
- 生产控制面：
  - Admin API：`PATCH /admin/api/plugins/{plugin}/state`
  - CLI：`sushi plugin status|enable|disable`
- 拒绝语义：插件被禁用时，运行时会阻断分发，返回显式拒绝（例如 API 返回 `403 plugin_disabled`）。

## Lua VM 隔离

- 每个插件创建独立的 `Lua` 实例
- 通过 `mlua` 沙箱限制危险操作
- 只暴露 `sushi.*` 命名空间
- `os.execute`, `io.*`, `require` 等被禁用

## Plugin Trait (Rust)

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    async fn init(&self, ctx: &SushiContext) -> Result<(), PluginError>;
}
```

Rust 插件实现此 trait，Lua 插件通过 `LuaPlugin` wrapper 适配。

## 相关文档

- [Lua API 参考](../lua-api/README.md)
- [插件开发指南](../guides/plugin-development.md)
