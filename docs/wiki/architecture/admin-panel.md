# Admin Panel

## Workspace 导航模型（HTMX + Tabs）

当前后台采用“左侧菜单 + 右侧 Workspace”的局部加载模型：

- 左侧菜单点击叶子节点时，不再整页刷新，而是通过 HTMX 请求 `GET /admin/workspace/:module` 局部加载右侧内容。
- 右侧支持多 Tab 工作区，同一路径去重（重复点击只激活已有 Tab）。
- Dashboard（`/admin/`）固定存在且不可关闭；其他模块 Tab 可关闭。
- 激活 Tab 时同步浏览器 URL（`history.pushState`），支持前进/后退恢复。
- Tab 状态持久化到 `localStorage`（key: `admin.workspace.tabs.v1`），刷新后恢复。
- HTMX 不可用时自动降级为整页跳转，保证功能可用性。

### Workspace 模块映射

| 模块 | 路由 |
|-----|------|
| dashboard | /admin/ |
| users | /admin/users |
| roles | /admin/roles |
| permissions | /admin/permissions |
| plugins | /admin/plugins |
| kv | /admin/kv |
| config | /admin/config |
| logs | /admin/logs |
| menus | /admin/menus |

### Workspace Partial Endpoint

| 方法 | 路径 | 功能 |
|-----|------|------|
| GET | /admin/workspace/:module | 返回右侧内容片段（不含 sidebar/base） |

### RBAC 映射

`/admin/workspace/:module` 复用现有读权限模型：

- `dashboard -> dashboard.view`
- `users -> users.view`
- `roles -> roles.view`
- `permissions -> permissions.view`
- `plugins -> plugins.view`
- `kv -> kv.manage`
- `config -> config.view`
- `logs -> logs.view`
- `menus -> menus.view`

## 菜单系统

Admin 面板使用动态菜单系统，菜单数据存储在数据库 `menu_items` 表中。

### 数据库结构

| 字段 | 类型 | 说明 |
|-----|------|------|
| id | INTEGER | 主键 |
| label | TEXT | 菜单显示名称 |
| icon | TEXT | Lucide 图标名 |
| position | INTEGER | 排序位置 |
| parent_id | INTEGER | NULL=一级菜单，指向父ID=二级菜单 |
| route | TEXT | 路由路径 |
| is_hidden | INTEGER | 0=显示, 1=隐藏 |

### API

| 方法 | 路径 | 功能 |
|-----|------|------|
| GET | /admin/api/menu | 获取菜单列表 |
| POST | /admin/api/menu | 创建菜单项 |
| PUT | /admin/api/menu/:id | 更新菜单项 |
| DELETE | /admin/api/menu/:id | 删除菜单项 |

### 菜单管理页面

路由: `/admin/menus`

功能:
- 列表显示所有菜单项（树形结构）
- 添加/编辑/删除菜单项
- 显示/隐藏切换

### 图标

使用 Lucide Icons SVG 内联渲染。常用图标:

| 图标名 | 用途 |
|-------|------|
| layout-dashboard | Dashboard |
| users | 用户管理 |
| shield | 角色 |
| key | 权限 |
| package | 插件 |
| settings | 配置 |
| file-text | 日志 |
| database | 数据库 |
