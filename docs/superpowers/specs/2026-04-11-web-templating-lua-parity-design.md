# Sushi Web 模板引擎与 Lua 对等能力设计

> 日期: 2026-04-11  
> 状态: Proposed（已评审通过，待实施）  
> 目标: 完成 Web 模板/静态资源统一改造，并实现 Lua 与 Rust 在 Admin Web 开发与数据库访问上的能力对等

## 1. 背景与目标

当前项目存在三类结构性问题：

1. Admin 模板分散在 `crates/sushi-admin/templates/`，静态资源（脚本/样式/图片）未形成统一资源目录。
2. Lua 插件页面常以内嵌 HTML 字符串方式实现（如 `plugins/kv-store/init.lua`），可维护性差，无法复用统一布局。
3. Lua 数据能力以特定封装（如 `sushi.kv.*`）为主，缺少与 Rust 对齐的通用数据库接口，限制插件独立性。

本设计分两期推进：

- **Phase 1（本轮优先）**：模板引擎上线 + 目录迁移 + 静态资源统一 + 内置 Admin 页面重构。
- **Phase 2（能力对齐）**：Lua 完整页面开发能力 + Lua 通用数据库接口 + 权限分级控制。

## 2. 方案选择结论

### 2.1 模板引擎选择

采用 **MiniJinja**（Jinja2 风格）作为统一模板引擎。

选择依据：

- 满足运行时模板加载（支持插件侧动态模板）。
- 语法符合团队偏好（Jinja2 风格）。
- 能为 Rust 与 Lua 提供统一 `render(template, context)` 渲染能力。

### 2.2 架构路径选择

采用 **统一运行时模板引擎方案（推荐方案 A）**：

- Rust 内置页面与 Lua 插件页面共享同一模板渲染底座。
- 避免“双模板栈并存”导致的长期维护成本。
- 从架构上保证 Rust/Lua 对等，而不是 Lua 作为二等扩展层。

### 2.3 数据权限策略

采用 **数据库分级权限**：`read` / `write` / `admin`。

- `read`: 只允许查询类 SQL。
- `write`: 允许 DML（增删改查）。
- `admin`: 允许 DDL/迁移级操作。

权限校验由 Rust 网关统一执行，Lua 不可绕过。

## 3. 目标目录结构

### 3.1 Web 资源统一目录

```text
sushi/
├── web/
│   ├── templates/
│   │   ├── base.html
│   │   ├── admin/
│   │   │   ├── login.html
│   │   │   ├── dashboard.html
│   │   │   ├── users.html
│   │   │   ├── plugins.html
│   │   │   ├── config.html
│   │   │   ├── logs.html
│   │   │   └── kv.html
│   │   └── plugins/
│   │       └── <plugin_name>/...
│   └── static/
│       ├── admin/
│       │   ├── css/
│       │   ├── js/
│       │   └── images/
│       └── plugins/
│           └── <plugin_name>/...
```

### 3.2 路由映射约定

- 模板读取根：`web/templates`
- 静态映射：`/static/*path -> web/static/*path`
- URL 保持兼容：`/admin/*`、`/admin-login` 不变
- 前端依赖使用本地静态资源，不使用 CDN（例如 Alpine/Tailwind 脚本由 `web/static/js/` 提供）

## 4. 核心组件设计

### 4.1 TemplateService

**职责**：

- 加载 `web/templates/**`
- 提供模板渲染入口
- 注入全局上下文（如 `static_base`, `app_name`, `current_user`）

**Rust 接口（概念）**：

```rust
render(template: &str, context: serde_json::Value) -> Result<String, TemplateError>
```

### 4.2 StaticAssetService

**职责**：

- 挂载 `/static/*` 到 `web/static/`
- 统一缓存头与响应策略

### 4.3 LuaWebBridge

**目标**：Lua 具备完整页面开发能力。

**新增 Lua API（Phase 2）**：

- `sushi.web.render(template, ctx)`：渲染模板字符串输出
- `sushi.web.page(path, template, opts)`：声明页面并绑定模板
- `sushi.web.json(status, table)`：统一 JSON 响应
- `sushi.web.static(prefix, dir)`：声明插件静态资源（受目录沙箱约束）

### 4.4 DbGateway

**职责**：

- 提供统一 SQL 执行接口给 Rust/Lua
- 统一参数绑定与错误模型
- 执行 SQL 权限分级校验

**新增 Lua API（Phase 2）**：

- `sushi.db.query(sql, params?) -> rows`
- `sushi.db.execute(sql, params?) -> { affected_rows, last_insert_id }`

## 5. Phase 1 范围（模板与静态资源重构）

### 5.1 范围清单

1. 全部 Admin 模板迁移至 `web/templates/admin/`。
2. 引入 `base.html`，子页面模板继承统一布局。
3. 所有页面脚本/样式外置至 `web/static/admin/`，清理内联脚本。
4. Admin 路由改为调用 `TemplateService` 渲染，而非 `include_str!`。
5. 新增 `/static/*` 静态映射。
6. 模板中统一改为引用本地静态依赖（`/static/...`），去除所有 CDN 链接。

### 5.2 非目标

- 本阶段不实现 Lua 新 API（仅为 Phase 2 准备底座）。
- 本阶段不替换已有 Lua 插件业务逻辑（仅保证兼容）。

### 5.3 验收标准

- 内置 Admin 所有页面可正常访问。
- 页面无内联大段脚本，脚本均从 `web/static` 加载。
- `web/templates` 成为唯一运行时模板来源。
- URL 对外保持兼容，不引入破坏性路由变更。
- 页面依赖不再请求外部 CDN（离线可访问本地依赖）。

## 6. Phase 2 范围（Lua 对等能力）

### 6.1 页面能力对齐

- Lua 插件可声明页面并渲染模板，不再内嵌 HTML 字符串。
- 插件模板统一落在 `web/templates/plugins/<plugin>/...`（不从插件私有目录直接读取），统一通过 TemplateService 读取。

### 6.2 DB 能力对齐

- Lua 插件使用通用 `sushi.db` 接口直接操作数据库。
- `read/write/admin` 强制执行，错误结构化返回。

### 6.3 迁移示例（kv-store）

将 `plugins/kv-store/init.lua` 从内嵌 HTML 改为：

- 声明模板路径 `plugins/kv-store/kv.html`
- 通过 `sushi.web.page(...)` 绑定页面
- 数据读写默认通过 `sushi.db`；`sushi.kv` 仅保留兼容一个小版本并标记 deprecated。

## 7. 错误处理与安全设计

### 7.1 错误处理

- 模板缺失/渲染失败 -> 统一 500 页面 + 结构化日志。
- 静态资源不存在 -> 标准 404。
- Lua 渲染异常 -> 安全错误响应，避免 panic 泄漏。

### 7.2 安全控制

- 插件静态资源目录仅允许挂载在插件边界内，禁止任意文件系统暴露。
- 数据库访问一律经 DbGateway 权限校验。
- SQL 参数化绑定为默认路径，禁止将“字符串拼接 SQL”作为推荐用法。

## 8. 测试策略

### 8.1 单元测试

- TemplateService：模板继承、上下文注入、错误路径。
- DbGateway：`read/write/admin` SQL 分类与拒绝逻辑。

### 8.2 集成测试

- `/admin/*` 页面渲染回归。
- `/static/*` 可访问性。
- Lua 页面模板渲染链路。
- Lua DB 权限矩阵（read/write/admin）。

### 8.3 回归测试

- 登录后 admin 页面访问行为不回退。
- 原 API 路径兼容。

## 9. 迁移与兼容策略

- Phase 1 保持对外路由兼容。
- 旧插件内嵌 HTML 继续可运行，但标记 deprecated。
- Phase 2 提供插件迁移指南（内嵌 HTML -> 模板 + `sushi.web`）。

## 10. 风险与缓解

1. **风险：模板/静态路径散落导致维护困难**  
   缓解：引入集中配置 `WebConfig { templates_dir, static_dir, static_url_prefix }`。

2. **风险：Lua SQL 使用不当带来安全问题**  
   缓解：Rust 层强制权限分级 + SQL 类型校验 + 审计日志。

3. **风险：插件模板命名冲突**  
   缓解：命名空间约定 `admin/*`、`plugins/<name>/*`，禁止覆盖核心模板。

## 11. 实施顺序（高层）

1. 引入 MiniJinja 与 TemplateService（不改业务行为）。
2. 完成 Phase 1 模板与静态资源迁移。
3. 验证 Phase 1 回归。
4. 实现 LuaWebBridge + DbGateway 权限分级。
5. 迁移示例插件（kv-store）并验证 Phase 2。

---

该设计文档为后续 implementation plan 的输入基线。
