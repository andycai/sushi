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

### API

- `GET /admin/api/menu` - 获取菜单列表

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
