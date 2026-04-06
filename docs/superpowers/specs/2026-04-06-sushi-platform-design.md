# Sushi — 通用应用平台设计文档

> 日期: 2026-04-06
> 状态: Draft
> 技术栈: Rust Axum, mlua, clap, Alpine.js, TailwindCSS, SQLite

## 1. 项目定位

Sushi 是一个通用应用平台，Rust 提供高性能运行时，Lua 作为一等公民插件语言。项目包含三个等价组件：admin（后台管理）、api（HTTP API）、cli（命令行），三者同时支持 Rust 和 Lua 实现。

**关键决策：**
- 单体二进制部署（`sushi serve` / `sushi admin` / `sushi run`）
- SQLite 存储
- 目录扫描方式加载 Lua 插件
- Plugin-First 架构：插件系统是第一层，所有业务功能通过插件机制暴露

## 2. Crate 组织与二进制入口

```
sushi/
├── Cargo.toml                  # workspace 根
├── crates/
│   ├── sushi-core/             # Plugin trait, Lua VM, 事件总线, 存储抽象
│   ├── sushi-api/              # Axum 路由, 中间件, 依赖 sushi-core
│   ├── sushi-admin/            # Axum server + 嵌入式 UI, 依赖 sushi-core + sushi-api
│   ├── sushi-cli/              # clap 命令解析, 依赖 sushi-core
│   └── sushi/                  # 单体二进制入口, 依赖上述所有 crate
├── plugins/                    # Lua 插件目录（运行时扫描）
│   └── _example/
│       ├── plugin.toml
│       └── init.lua
├── ui/                         # Admin 前端源码
│   ├── src/
│   │   ├── index.html
│   │   ├── app.js              # Alpine.js 应用入口
│   │   └── styles.css          # TailwindCSS 入口
│   ├── package.json
│   └── tailwind.config.js
└── config.toml                 # Sushi 主配置文件
```

**CLI 入口：**

```bash
sushi serve          # 启动 API + Admin 服务
sushi serve --api    # 只启动 API
sushi serve --admin  # 只启动 Admin
sushi run <plugin>   # 运行单个 Lua 插件
sushi plugin list    # 列出已加载插件
sushi config get/set # 管理配置
```

## 3. 核心 Plugin 系统（sushi-core）

### 3.1 Plugin Trait

所有插件（Rust 或 Lua）统一通过 `Plugin` trait 定义：

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    async fn init(&self, ctx: &SushiContext) -> Result<(), PluginError>;
}
```

### 3.2 SushiContext

插件通过 `SushiContext` 注册能力：

```rust
pub struct SushiContext {
    api: ApiRegistry,      // 注册路由、中间件
    admin: AdminRegistry,  // 注册管理页面、widget
    cli: CliRegistry,      // 注册 CLI 子命令
    config: ConfigStore,   // 读写配置
    db: DatabasePool,      // SQLite 连接池
    event: EventBus,       // 发布/订阅事件
    log: Logger,           // 结构化日志
}
```

### 3.3 Lua Plugin Bridge

Lua 插件通过 `LuaPlugin` wrapper 适配 `Plugin` trait：

```rust
pub struct LuaPlugin {
    manifest: PluginManifest,    // 解析自 plugin.toml
    lua: Lua,                    // 独立的 Lua VM 实例（沙箱隔离）
}
```

`init` 时将 `SushiContext` 的各 registry 注入到 Lua 全局表，然后调用 `init.lua` 中的 `sushi.init()`。

### 3.4 插件加载流程

1. 启动时扫描 `plugins/` 目录
2. 解析每个 `plugin.toml`
3. 按依赖顺序排序（未来支持）
4. 为每个插件创建独立 Lua VM
5. 注入 `sushi` 上下文到全局表
6. 调用 `sushi.init()`
7. 收集注册的路由/命令/页面

### 3.5 权限沙箱

- 每个插件的 Lua VM 独立隔离
- 禁止 `os.execute`、`io` 等危险操作
- 只暴露 `sushi.*` 命名空间
- `plugin.toml` 中的 `permissions` 字段决定哪些 `sushi.*` 可用

**plugin.toml 示例：**

```toml
[plugin]
name = "example_plugin"
version = "0.1.0"
description = "An example Lua plugin"
entry = "init.lua"

[permissions]
routes = true       # 可以注册 HTTP 路由
commands = true     # 可以注册 CLI 命令
admin = true        # 可以扩展 Admin 面板
database = true     # 可以访问数据库
```

### 3.6 Lua 插件 API Surface

| Namespace | 方法 | 组件 |
|-----------|------|------|
| `sushi.api` | `route(method, path, handler)`, `middleware(handler)` | api |
| `sushi.admin` | `page(path, title, renderer)`, `widget(name, renderer)` | admin |
| `sushi.cli` | `command(name, desc, handler)`, `option(name, short, desc)` | cli |
| `sushi.config` | `get(key)`, `set(key, value)` | core |
| `sushi.log` | `info(msg)`, `warn(msg)`, `error(msg)` | core |
| `sushi.db` | `query(sql, params?)`, `execute(sql, params?)` | core |
| `sushi.event` | `on(event, handler)`, `emit(event, data)` | core |
| `sushi.auth` | `verify_token(token)`, `hash_password(pwd)` | core |

## 4. 数据存储层（SQLite）

### 4.1 存储抽象

```rust
pub trait Storage: Send + Sync {
    async fn execute(&self, query: &str, params: Vec<Value>) -> Result<(), StorageError>;
    async fn query(&self, query: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError>;
    async fn transaction<F, T>(&self, f: F) -> Result<T, StorageError>
    where F: FnOnce(&StorageConn) -> Result<T, StorageError>;
}
```

底层实现使用 `rusqlite`，通过 tokio task 隔离阻塞 IO。

### 4.2 Lua 侧访问

```lua
sushi.db.execute("CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, name TEXT)")
sushi.db.execute("INSERT INTO items (name) VALUES (?)", {"hello"})

local rows = sushi.db.query("SELECT * FROM items")
for _, row in ipairs(rows) do
    sushi.log.info("item: " .. row.name)
end
```

### 4.3 迁移管理

- 内置迁移表 `_sushi_migrations`
- 内置迁移文件放在 `migrations/` 目录（Rust 实现）
- V1 内置迁移：用户表、配置表、插件状态表
- 插件可以有独立的迁移文件（插件目录下 `migrations/`）

### 4.4 安全限制

- `database = true` 时 `sushi.db` 只读可用
- `database = "write"` 时写入可用（包含 CREATE/INSERT/UPDATE/DELETE）
- DROP/ALTER 需要 `database = "admin"` 权限
- 查询超时机制，防止插件执行耗时 SQL 阻塞主线程

## 5. 用户认证与授权

### 5.1 认证方案

- JWT（access_token + refresh_token），无状态
- 密码使用 Argon2 哈希
- Token 通过 Axum middleware（`FromRequestParts`）提取和验证

### 5.2 用户模型

```rust
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,   // Argon2
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum UserRole {
    Admin,    // 全部权限
    Editor,   // 内容管理权限
    Viewer,   // 只读权限
}
```

### 5.3 API 认证端点

| 方法 | 路径 | 功能 |
|------|------|------|
| POST | `/api/auth/login` | 验证密码，返回 access_token + refresh_token |
| POST | `/api/auth/refresh` | 用 refresh_token 换新的 access_token |
| POST | `/api/auth/logout` | 注销（可选：将 token 加入黑名单） |
| GET | `/api/auth/me` | 返回当前用户信息 |

### 5.4 插件中的认证

```lua
sushi.api.middleware(function(req)
    local token = req.headers["authorization"]
    if not token then
        return { status = 401, body = { error = "Unauthorized" } }
    end
    local user = sushi.auth.verify_token(token)
    if not user then
        return { status = 403, body = { error = "Invalid token" } }
    end
    req.user = user
end)
```

### 5.5 权限控制

- RBAC（基于角色的访问控制）
- 内置角色：Admin、Editor、Viewer
- 插件可以注册自定义权限和角色

## 6. Admin UI（Alpine.js + TailwindCSS）

### 6.1 布局结构

```
┌─────────────────────────────────────────────┐
│  Sushi Admin                     [User] ▾   │  顶部导航栏
├────────┬────────────────────────────────────┤
│        │                                    │
│ 仪表盘  │         主内容区域                  │
│ 插件    │                                    │
│ 用户    │                                    │
│ 配置    │                                    │
│ 日志    │                                    │
│ ────── │                                    │
│ 插件A  │                                    │  插件注册的页面
│ 插件B  │                                    │
│        │                                    │
├────────┴────────────────────────────────────┤
│  Footer: Sushi v0.1.0 | 插件加载: 3         │
└─────────────────────────────────────────────┘
```

### 6.2 内置页面

| 页面 | 路由 | 功能 |
|------|------|------|
| 仪表盘 | `/admin/` | 系统概览、插件状态、最近活动 |
| 插件管理 | `/admin/plugins` | 列表、启用/禁用、查看详情 |
| 用户管理 | `/admin/users` | CRUD 用户、分配角色 |
| 系统配置 | `/admin/config` | 编辑 config.toml 的可视化界面 |
| 日志查看 | `/admin/logs` | 实时日志流 |

### 6.3 插件扩展 Admin

```lua
sushi.admin.page("/admin/my-plugin", "我的插件", function()
    return [[
        <div x-data="{ items: [] }" x-init="fetch('/api/my-plugin/items').then(r=>r.json()).then(d=>items=d)">
            <table class="w-full">
                <template x-for="item in items">
                    <tr><td x-text="item.name"></td></tr>
                </template>
            </table>
        </div>
    ]]
end)

sushi.admin.widget("stats", function()
    return [[
        <div class="bg-blue-50 p-4 rounded">
            <span class="text-2xl font-bold" x-data x-text="$store.stats.count">0</span>
            <p class="text-gray-500">Total Items</p>
        </div>
    ]]
end)
```

### 6.4 构建与嵌入

- `ui/` 目录独立构建（npm + TailwindCSS CLI）
- 构建产物通过 `rust-embed` 编译进二进制
- 开发时通过 `sushi serve --admin-dev` 指向本地 `ui/src/` 实现热更新

## 7. 事件总线与插件间通信

### 7.1 EventBus

```rust
pub struct EventBus {
    subscribers: HashMap<String, Vec<Box<dyn EventHandler>>>,
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &Event) -> Result<(), EventError>;
}
```

### 7.2 内置事件

| 事件名 | 触发时机 | 数据 |
|--------|---------|------|
| `plugin.loaded` | 插件加载完成 | `{ name, version }` |
| `plugin.unloaded` | 插件卸载 | `{ name }` |
| `server.starting` | 服务启动前 | `{ port, mode }` |
| `server.started` | 服务启动后 | `{ port, mode }` |
| `user.created` | 用户创建 | `{ user_id, username }` |
| `user.login` | 用户登录 | `{ user_id, ip }` |
| `request.before` | 请求处理前 | `{ method, path, headers }` |
| `request.after` | 请求处理后 | `{ status, duration }` |

### 7.3 Lua 侧事件

```lua
-- 监听事件
sushi.event.on("user.created", function(data)
    sushi.log.info("New user: " .. data.username)
end)

-- 发射自定义事件
sushi.event.emit("my-plugin.data-changed", { item_id = 42 })
```

### 7.4 Lua 路由注册

```lua
sushi.api.route("GET", "/api/items", function(req)
    local rows = sushi.db.query("SELECT * FROM items")
    return { status = 200, body = rows }
end)

sushi.api.route("POST", "/api/items", function(req)
    sushi.db.execute("INSERT INTO items (name) VALUES (?)", { req.body.name })
    sushi.event.emit("item.created", { name = req.body.name })
    return { status = 201, body = { message = "Created" } }
end)
```

### 7.5 Lua CLI 命令注册

```lua
sushi.cli.command("items:list", "List all items", function(args)
    local rows = sushi.db.query("SELECT * FROM items")
    for _, row in ipairs(rows) do
        print(row.id .. " | " .. row.name)
    end
end)
-- CLI 调用: sushi items:list
```

## 8. 关键依赖

```toml
[workspace.dependencies]
axum = "0.8"
mlua = { version = "0.10", features = ["lua54", "vendored", "async", "send"] }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
anyhow = "1"
thiserror = "2"
tower = "0.5"
tower-http = { version = "0.6", features = ["fs", "cors", "trace"] }
tracing = "0.1"
tracing-subscriber = "0.3"
rusqlite = { version = "0.32", features = ["bundled"] }
jsonwebtoken = "9"
argon2 = "0.5"
chrono = { version = "0.4", features = ["serde"] }
rust-embed = "8"
```
