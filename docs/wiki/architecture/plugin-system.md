# 插件系统

## 设计原则

1. **Rust 和 Lua 平权** - 所有功能既可用 Rust 实现也可用 Lua 实现
2. **沙箱隔离** - 每个 Lua 插件运行在独立的 VM 中
3. **权限控制** - 插件必须声明权限才能访问敏感 API
4. **声明式** - 插件通过 manifest (plugin.toml) 声明元数据和权限

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
权限验证
    │
    ▼
创建独立 Lua VM
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
