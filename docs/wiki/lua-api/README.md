# Lua API 总览

本文档列出 Lua 插件可以调用的所有 `sushi.*` 接口。

## API 命名空间

| 命名空间 | 方法数 | 权限要求 | 说明 |
|---------|-------|---------|------|
| `sushi.log` | 3 | 无 | 结构化日志 |
| `sushi.json` | 2 | 无 | JSON 编解码 |
| `sushi.config` | 1 | 无 | 配置访问（只读 stub） |
| `sushi.event` | 2 | 无 | 事件总线 |
| `sushi.auth` | 1 | 无 | JWT 验证 |
| `sushi.db` | 2 | 数据库权限 | 数据库查询执行 |
| `sushi.api` | 1 | `routes = true` | HTTP 路由注册 |
| `sushi.cli` | 1 | `commands = true` | CLI 命令注册 |
| `sushi.admin` | 1 | `admin = true` | Admin 页面注册 |
| `sushi.web` | 4 | `admin = true` 或 `routes = true` | Web 渲染与下载 envelope |
| `sushi.fs` | 11 | `file_browser` 配置存在时自动注入 | 受限文件系统访问 |

## 权限级别说明

### DatabasePermission

| 值 | 说明 |
|----|------|
| `false` / `None` | 无数据库访问 |
| `true` / `read` | 只读查询 |
| `"write"` | 读写（INSERT/UPDATE/DELETE） |
| `"admin"` | 完全访问（包括 DROP/ALTER） |

### 权限配置示例 (plugin.toml)

```toml
[permissions]
routes = true       # 可注册 HTTP 路由
commands = true     # 可注册 CLI 命令
admin = true        # 可扩展 Admin 面板
database = "write"  # 读写数据库
```

## 内部表（插件不应直接使用）

以下表是内部实现细节，可能随时变更：

- `sushi.__handlers` - 存储注册的处理函数
- `sushi.__pending_routes` - 待注册的路由
- `sushi.__pending_commands` - 待注册的 CLI 命令
- `sushi.__pending_pages` - 待注册的 Admin 页面
- `sushi.__event_handlers` - 事件处理器存储
