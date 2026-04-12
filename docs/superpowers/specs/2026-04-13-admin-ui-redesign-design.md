# Admin UI 重构设计方案

> 日期: 2026-04-13
> 状态: Approved

## 目标

重构管理后台 UI，改善菜单交互体验，实现局部刷新，并增加菜单管理功能。

## 需求

1. 后台增加菜单管理页面（完整 CRUD）
2. 左侧菜单改为可多展开的收缩组件
3. 增加退出登录按钮
4. 点击菜单局部刷新内容区域（HTMX）

## 布局设计

```
┌──────────────────────────────────────────────────────────┐
│  Sushi Admin          │  Sushi Runtime Administration    │
│  Control Surface      │  Enterprise Console   [User]    │
├────────────┬──────────┴─────────────────────────────────┤
│            │                                         │
│ Dashboard  │                                         │
│  ↳ Overview│        内容区域                         │
│  ↳ Stats  │        (HTMX 局部刷新)                   │
│            │                                         │
│ Users      │                                         │
│  ↳ List    │                                         │
│  ↳ Create  │                                         │
│            │                                         │
│ ...        │                                         │
│            │                                         │
│ ────────── │                                         │
│ [Logout]   │                                         │
└────────────┴─────────────────────────────────────────┘
```

## 交互设计

### 菜单收缩

- 点击一级菜单：展开/收起子菜单（不跳转）
- 点击子菜单：HTMX 请求加载内容区域（局部刷新）
- 无子菜单的一级菜单：直接 HTMX 加载内容
- 可同时展开多个子菜单，彼此独立

### HTMX 局部刷新

- 左菜单栏保留不刷新
- 内容区域通过 HTMX 局部更新
- 后端返回 HTML 片段

## 数据库变更

`menu_items` 表扩展字段：

| 字段 | 类型 | 说明 |
|-----|------|------|
| `id` | INTEGER | 主键 |
| `label` | TEXT | 菜单显示名称 |
| `icon` | TEXT | Lucide 图标名 |
| `position` | INTEGER | 排序位置 |
| `parent_id` | INTEGER | NULL=一级菜单 |
| `route` | TEXT | 路由路径 |
| `is_hidden` | INTEGER | 是否隐藏（0=显示，1=隐藏） |

## API 设计

| 方法 | 路径 | 功能 |
|-----|------|------|
| GET | `/admin/api/menu` | 获取菜单列表 |
| POST | `/admin/api/menu` | 创建菜单项 |
| PUT | `/admin/api/menu/:id` | 更新菜单项 |
| DELETE | `/admin/api/menu/:id` | 删除菜单项 |
| POST | `/admin/api/menu/reorder` | 批量更新排序 |

## 菜单管理页面

**路由:** `/admin/menus`

**功能:**
- 列表显示所有菜单项（树形结构）
- 拖拽排序
- 添加/编辑/删除菜单项
- 图标选择器（Lucide 图标选择）
- 显示/隐藏切换

## 前端模板

### 菜单渲染

```html
<nav class="admin-nav">
  <template x-for="item in topMenuItems" :key="item.id">
    <div class="nav-group">
      <!-- 一级菜单项 -->
      <div class="nav-item-wrapper">
        <a
          :href="item.route || '#'"
          class="admin-nav-link"
          :class="{ 'active': isActive(item), 'has-children': hasChildren(item) }"
          @click.prevent="handleMenuClick(item)"
          hx-get="/admin/partials/..."
          hx-target="#admin-content"
          hx-swap="innerHTML"
        >
          <span class="admin-nav-icon" x-html="getIcon(item.icon)"></span>
          <span class="admin-nav-label" x-text="item.label"></span>
          <span v-if="hasChildren(item)" class="admin-nav-arrow" :class="{ 'expanded': isExpanded(item.id) }">›</span>
        </a>
        <!-- 子菜单 -->
        <div x-show="isExpanded(item.id)" class="nav-children">
          <template x-for="child in getChildren(item.id)" :key="child.id">
            <a
              :href="child.route"
              class="nav-child-link"
              :class="{ 'active': isActive(child) }"
              hx-get="/admin/partials/..."
              hx-target="#admin-content"
              hx-swap="innerHTML"
            >
              <span x-text="child.label"></span>
            </a>
          </template>
        </div>
      </div>
    </div>
  </template>

  <!-- 退出登录 -->
  <div class="nav-footer">
    <a href="#" @click.prevent="logout()" class="admin-nav-link logout-link">
      <span class="admin-nav-icon" x-html="getIcon('log-out')"></span>
      <span class="admin-nav-label">Logout</span>
    </a>
  </div>
</nav>
```

### 内容区域

```html
<main id="admin-content" class="admin-main">
  <!-- HTMX 加载的内容 -->
</main>
```

## 实现计划

### Phase 1: 数据库和 API 扩展
1. 添加 `is_hidden` 字段到 `menu_items` 表
2. 扩展菜单 API 支持 CRUD 操作

### Phase 2: 前端菜单重构
1. 修改 `menu.js` 支持多展开状态
2. 添加 HTMX 属性到菜单链接
3. 修改 `base.html` 布局结构

### Phase 3: 菜单管理页面
1. 创建菜单管理页面 `/admin/menus`
2. 添加图标选择器组件
3. 添加拖拽排序功能

### Phase 4: 退出登录
1. 添加退出登录按钮到菜单底部
2. 实现 logout 函数调用 `/api/auth/logout`

## 变更文件

| 文件 | 变更 |
|-----|------|
| `migrations/004_menu.sql` | 添加 is_hidden 字段 |
| `crates/sushi-admin/src/routes/menu.rs` | 扩展 CRUD API |
| `web/templates/admin/menus.html` | 新增菜单管理页面 |
| `web/templates/admin/partials/menu_form.html` | 新增菜单表单 |
| `web/templates/base.html` | 修改布局和菜单渲染 |
| `web/static/admin/js/menu.js` | 修改为多展开组件 |
| `web/static/admin/css/admin.css` | 添加相关样式 |

## 图标列表

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
| log-out | 退出登录 |
| menu | 菜单（移动端） |
| plus | 添加 |
| edit-2 | 编辑 |
| trash-2 | 删除 |
| eye | 显示 |
| eye-off | 隐藏 |
| grip-vertical | 拖拽 |
