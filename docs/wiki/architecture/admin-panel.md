# Admin Panel

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
