# Admin Menu 重构设计方案

> 日期: 2026-04-12
> 状态: Approved

## 目标

将管理后台的硬编码菜单改为可动态配置的二级菜单系统。

## 需求

1. 支持二级菜单（一级菜单下可挂载多个二级菜单项）
2. 新增模块可配置到任意一级菜单下
3. 菜单左侧使用 Lucide SVG 图标
4. 二级菜单使用右侧抽屉组件，支持多个同时展开

## 数据库模型

### menu_items 表

```sql
CREATE TABLE menu_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    icon TEXT,                    -- Lucide 图标名 (如 "settings", "package")
    position INTEGER NOT NULL DEFAULT 0,
    parent_id INTEGER,            -- NULL 表示一级菜单
    route TEXT,                   -- 路由路径 (一级菜单可为空)
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (parent_id) REFERENCES menu_items(id)
);
```

### 字段说明

| 字段 | 类型 | 说明 |
|-----|------|------|
| `id` | INTEGER | 主键 |
| `label` | TEXT | 菜单显示名称 |
| `icon` | TEXT | Lucide 图标名 |
| `position` | INTEGER | 排序位置，数字越小越靠前 |
| `parent_id` | INTEGER | NULL=一级菜单，指向父菜单id=二级菜单 |
| `route` | TEXT | 路由路径，为空则不可直接点击 |

## 内置菜单初始化

系统内置以下菜单，通过迁移或代码初始化：

| 一级菜单 | 图标 | 路由 | 二级菜单 |
|---------|------|------|---------|
| Dashboard | `layout-dashboard` | `/admin/` | - |
| Users | `users` | `/admin/users` | - |
| Roles | `shield` | `/admin/roles` | - |
| Permissions | `key` | `/admin/permissions` | - |
| Plugins | `package` | `/admin/plugins` | KV Store (`/admin/kv`) |
| Config | `settings` | `/admin/config` | Logs (`/admin/logs`) |

## 前端交互

### 菜单渲染结构

```
admin-sidebar
├── admin-brand
└── admin-nav
    ├── [一级菜单项] (有子菜单时带展开箭头)
    │   └── admin-nav-sub-trigger (点击展开抽屉)
    └── [一级菜单项] (无子菜单直接跳转)
```

### 二级菜单抽屉

- 使用现有 `.ui-drawer-overlay` 和 `.ui-drawer-panel` CSS
- 抽屉从右侧滑入
- 可同时展开多个抽屉
- 点击遮罩层关闭

### 图标渲染

使用 Lucide 图标，通过 SVG 内联渲染：

```javascript
// 图标名称映射到 Lucide SVG path
const icons = {
  'layout-dashboard': '<path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z">',
  'users': '<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2">...',
  // ... 更多图标
};
```

### Alpine.js 状态管理

```javascript
function adminMenu() {
  return {
    openDrawers: {},  // { parentId: true/false }

    toggleDrawer(parentId) {
      this.openDrawers[parentId] = !this.openDrawers[parentId];
    },

    closeDrawer(parentId) {
      this.openDrawers[parentId] = false;
    }
  }
}
```

## API 设计

### 获取菜单列表

```
GET /admin/api/menu
Response: { menu: [{ id, label, icon, position, parent_id, route, children: [] }] }
```

### 管理菜单 (可选，后续扩展)

```
POST /admin/api/menu        - 创建菜单项
PUT /admin/api/menu/:id     - 更新菜单项
DELETE /admin/api/menu/:id  - 删除菜单项
```

## 实现计划

1. **数据库迁移** - 创建 `menu_items` 表，初始化内置菜单
2. **后端 API** - 添加 `/admin/api/menu` 端点
3. **模板修改** - 修改 `base.html` 菜单渲染逻辑
4. **CSS 增强** - 添加菜单相关样式
5. **Alpine.js 组件** - 添加菜单交互逻辑
6. **插件集成** - Lua 插件可注册菜单项

## 变更文件

| 文件 | 变更 |
|-----|------|
| `migrations/004_menu.sql` | 新增迁移 |
| `crates/sushi-admin/src/routes/menu.rs` | 新增菜单 API |
| `crates/sushi-admin/src/routes/mod.rs` | 注册菜单路由 |
| `web/templates/base.html` | 重构菜单渲染 |
| `web/static/admin/css/admin.css` | 添加菜单样式 |
| `web/static/admin/js/menu.js` | 新增菜单交互 |

## 图标列表 (常用)

| 图标名 | 用途 |
|-------|------|
| `layout-dashboard` | Dashboard |
| `users` | 用户管理 |
| `shield` | 角色/安全 |
| `key` | 权限 |
| `package` | 插件 |
| `settings` | 配置 |
| `file-text` | 日志/文档 |
| `database` | 数据库 |
| `zap` | 快速操作 |
| `plus` | 添加 |
| `search` | 搜索 |
| `log-out` | 登出 |
