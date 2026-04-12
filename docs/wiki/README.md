# Sushi Wiki

Sushi 通用应用平台的知识库文档。

## 文档索引

### Lua API 参考
Lua 插件可调用的 Rust 接口文档。

- [Lua API 总览](lua-api/README.md)
- [sushi.log](lua-api/sushi.log.md) - 日志接口
- [sushi.api](lua-api/sushi.api.md) - HTTP 路由接口
- [sushi.cli](lua-api/sushi.cli.md) - CLI 命令接口
- [sushi.admin](lua-api/sushi.admin.md) - Admin 页面接口
- [sushi.web](lua-api/sushi.web.md) - Web 渲染接口
- [sushi.db](lua-api/sushi.db.md) - 数据库接口
- [sushi.json](lua-api/sushi.json.md) - JSON 编解码接口
- [sushi.config](lua-api/sushi.config.md) - 配置接口
- [sushi.event](lua-api/sushi.event.md) - 事件总线接口
- [sushi.auth](lua-api/sushi.auth.md) - 认证接口

### 架构文档
- [架构总览](architecture/README.md)
- [插件系统](architecture/plugin-system.md)
- [认证与 RBAC](architecture/auth-rbac.md)
- [数据库层](architecture/database.md)
- [Admin 面板](architecture/admin-panel.md)

### 开发指南
- [插件开发指南](guides/plugin-development.md)
- [配置指南](guides/configuration.md)

---

## 文档更新约定

新增或修改 Lua API 时，必须同步更新对应文档：

| 变更类型 | 需更新文档 |
|---------|-----------|
| 新增 binding | 新增 `lua-api/sushi.*.md` |
| 修改现有 binding | 更新对应 `lua-api/sushi.*.md` |
| 新增 Rust 模块 | 在 `architecture/` 添加文档 |
| 新增功能指南 | 在 `guides/` 添加文档 |
| 修改权限逻辑 | 更新 `architecture/plugin-system.md` |
