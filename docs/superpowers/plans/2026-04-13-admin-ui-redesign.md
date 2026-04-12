# Admin UI 重构实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重构管理后台 UI，实现菜单管理、收缩侧边栏、退出登录和 HTMX 局部刷新。

**Architecture:** 数据库存储菜单，前端 Alpine.js 管理菜单状态，HTMX 处理内容区域局部刷新，后端返回 HTML 片段。

**Tech Stack:** SQLite, Axum, Handlebars, Alpine.js, HTMX, Lucide Icons

---

## 文件变更概览

| 文件 | 变更 |
|-----|------|
| `migrations/004_menu.sql` | 添加 is_hidden 字段 |
| `crates/sushi-admin/src/routes/menu.rs` | 扩展 CRUD API |
| `crates/sushi-admin/src/routes/mod.rs` | 注册菜单管理路由 |
| `crates/sushi-admin/src/router.rs` | 添加路由 |
| `web/templates/admin/menus.html` | 新增菜单管理页面 |
| `web/templates/admin/partials/menu_rows.html` | 新增菜单行片段 |
| `web/templates/admin/partials/menu_form.html` | 新增菜单表单 |
| `web/templates/base.html` | 修改布局和菜单渲染 |
| `web/static/admin/js/menu.js` | 修改为多展开组件 |
| `web/static/admin/css/admin.css` | 添加相关样式 |
| `crates/sushi-admin/tests/admin_web.rs` | 添加菜单 API 测试 |

---

## Task 1: 数据库扩展 - 添加 is_hidden 字段

**Files:**
- Modify: `migrations/004_menu.sql`

- [ ] **Step 1: 添加 is_hidden 字段**

```sql
CREATE TABLE IF NOT EXISTS menu_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    icon TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    parent_id INTEGER,
    route TEXT,
    is_hidden INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (parent_id) REFERENCES menu_items(id) ON DELETE SET NULL
);
```

- [ ] **Step 2: 提交**

```bash
git add migrations/004_menu.sql
git commit -m "feat(admin): add is_hidden field to menu_items table"
```

---

## Task 2: 菜单 API 扩展 - CRUD 操作

**Files:**
- Modify: `crates/sushi-admin/src/routes/menu.rs`

- [ ] **Step 1: 扩展菜单 API**

```rust
use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use sushi_core::context::SushiContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateMenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub position: Option<i64>,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMenuItem {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub position: Option<i64>,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct MenuItem {
    pub id: i64,
    pub label: String,
    pub icon: Option<String>,
    pub position: i64,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
    pub is_hidden: bool,
}

pub async fn create_menu_item(
    State(ctx): State<SushiContext>,
    Json(payload): Json<CreateMenuItem>,
) -> impl IntoResponse {
    let position = payload.position.unwrap_or(0);
    let is_hidden = payload.is_hidden.unwrap_or(false) as i64;

    ctx.db.execute(
        "INSERT INTO menu_items (label, icon, position, parent_id, route, is_hidden)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            serde_json::Value::String(payload.label),
            payload.icon.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            serde_json::Value::Number(position.into()),
            payload.parent_id.map(|id| serde_json::Value::Number(id.into())).unwrap_or(serde_json::Value::Null),
            payload.route.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            serde_json::Value::Number(is_hidden.into()),
        ],
    ).await.unwrap();

    Json(serde_json::json!({ "success": true }))
}

pub async fn update_menu_item(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateMenuItem>,
) -> impl IntoResponse {
    let mut updates = Vec::new();
    let mut params: Vec<serde_json::Value> = Vec::new();

    if let Some(label) = payload.label {
        updates.push("label = ?".to_string());
        params.push(serde_json::Value::String(label));
    }
    if let Some(icon) = payload.icon {
        updates.push("icon = ?".to_string());
        params.push(serde_json::Value::String(icon));
    }
    if let Some(position) = payload.position {
        updates.push("position = ?".to_string());
        params.push(serde_json::Value::Number(position.into()));
    }
    if let Some(parent_id) = payload.parent_id {
        updates.push("parent_id = ?".to_string());
        params.push(serde_json::Value::Number(parent_id.into()));
    }
    if let Some(route) = payload.route {
        updates.push("route = ?".to_string());
        params.push(serde_json::Value::String(route));
    }
    if let Some(is_hidden) = payload.is_hidden {
        updates.push("is_hidden = ?".to_string());
        params.push(serde_json::Value::Number((is_hidden as i64).into()));
    }

    if !updates.is_empty() {
        params.push(serde_json::Value::Number(id.into()));
        let sql = format!("UPDATE menu_items SET {} WHERE id = ?", updates.join(", "));
        ctx.db.execute(&sql, params).await.unwrap();
    }

    Json(serde_json::json!({ "success": true }))
}

pub async fn delete_menu_item(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    ctx.db.execute(
        "DELETE FROM menu_items WHERE id = ?1",
        vec![serde_json::Value::Number(id.into())],
    ).await.unwrap();
    Json(serde_json::json!({ "success": true }))
}

pub fn routes() -> Router<SushiContext> {
    Router::new()
        .route("/admin/api/menu", get(super::menu_api))
        .route("/admin/api/menu", post(create_menu_item))
        .route("/admin/api/menu/:id", put(update_menu_item))
        .route("/admin/api/menu/:id", delete(delete_menu_item))
}
```

- [ ] **Step 2: 更新 mod.rs**

```rust
pub mod menu;
```

- [ ] **Step 3: 更新 router.rs**

添加路由:
```rust
use crate::routes::menu;

.route("/admin/menus", get(menus_page))
.route("/admin/partials/menu/rows", get(menu_rows_partial))
.route("/admin/partials/menu/form", get(menu_form_partial))
```

- [ ] **Step 4: 构建测试**

Run: `cargo build --package sushi-admin 2>&1`

- [ ] **Step 5: 提交**

```bash
git add crates/sushi-admin/src/routes/menu.rs
git commit -m "feat(admin): add menu CRUD API endpoints"
```

---

## Task 3: 前端菜单组件重构 - 多展开支持

**Files:**
- Modify: `web/static/admin/js/menu.js`

- [ ] **Step 1: 修改菜单组件支持多展开**

```javascript
(() => {
  window.adminMenu = function adminMenu() {
    return {
      menuItems: [],
      expandedMenus: {},

      async init() {
        await this.loadMenu();
      },

      async loadMenu() {
        try {
          const resp = await fetch('/admin/api/menu');
          if (resp.ok) {
            const data = await resp.json();
            this.menuItems = data.menu || [];
          }
        } catch (e) {
          console.error('Failed to load menu:', e);
        }
      },

      topMenuItems() {
        return this.menuItems.filter(i => !i.parent_id && !i.is_hidden);
      },

      hasChildren(item) {
        return this.menuItems.some(i => i.parent_id === item.id && !i.is_hidden);
      },

      getChildren(parentId) {
        return this.menuItems.filter(i => i.parent_id === parentId && !i.is_hidden);
      },

      isExpanded(itemId) {
        return !!this.expandedMenus[itemId];
      },

      toggleExpand(itemId) {
        this.expandedMenus[itemId] = !this.expandedMenus[itemId];
        this.expandedMenus = { ...this.expandedMenus };
      },

      isActive(item) {
        const path = window.location.pathname;
        return item.route === path;
      },

      handleMenuClick(item) {
        if (this.hasChildren(item)) {
          this.toggleExpand(item.id);
        }
      },

      getIcon(iconName) {
        if (!iconName) return '';
        const icons = {
          'layout-dashboard': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="7" height="9" x="3" y="3" rx="1"/><rect width="7" height="5" x="14" y="3" rx="1"/><rect width="7" height="9" x="14" y="12" rx="1"/><rect width="7" height="5" x="3" y="16" rx="1"/></svg>',
          'users': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>',
          'shield': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10"/></svg>',
          'key': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="7.5" cy="15.5" r="5.5"/><path d="m21 2-9.6 9.6"/><path d="m15.5 7.5 3 3L22 7l-3-3"/></svg>',
          'package': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7.5 4.27 9 5.15"/><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/></svg>',
          'settings': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>',
          'file-text': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/><line x1="16" x2="8" y1="13" y2="13"/><line x1="16" x2="8" y1="17" y2="17"/><line x1="10" x2="8" y1="9" y2="9"/></svg>',
          'database': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/></svg>',
          'log-out': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" x2="9" y1="12" y2="12"/></svg>',
        };
        return icons[iconName] || '';
      },

      async logout() {
        try {
          await fetch('/api/auth/logout', { method: 'POST' });
          window.location.href = '/admin-login';
        } catch (e) {
          console.error('Logout failed:', e);
        }
      }
    };
  };
})();
```

- [ ] **Step 2: 提交**

```bash
git add web/static/admin/js/menu.js
git commit -m "feat(admin): refactor menu component for multi-expand support"
```

---

## Task 4: 模板重构 - HTMX 和布局

**Files:**
- Modify: `web/templates/base.html`

- [ ] **Step 1: 修改 base.html 布局和菜单渲染**

```html
<body x-data="adminMenu()" class="{% block body_class %}{% endblock %}">
  <div class="admin-shell">
    {% block nav %}
    {% set active_section = active_section or "" %}
    <aside class="admin-sidebar">
      <div class="admin-brand">
        <span class="admin-brand-logo">S</span>
        <span class="admin-brand-title">
          <strong>Sushi Admin</strong>
          <span>Control Surface</span>
        </span>
      </div>
      <nav class="admin-nav">
        <template x-for="item in topMenuItems()" :key="item.id">
          <div class="nav-group">
            <!-- 一级菜单项 -->
            <div class="nav-item-wrapper">
              <a
                :href="item.route || '#'"
                class="admin-nav-link"
                :class="{ 'active': isActive(item), 'has-children': hasChildren(item) }"
                @click.prevent="handleMenuClick(item)"
                hx-get="item.route"
                hx-target="#admin-content"
                hx-swap="innerHTML"
                hx-push-url="true"
              >
                <span class="admin-nav-icon" x-html="getIcon(item.icon)"></span>
                <span class="admin-nav-label" x-text="item.label"></span>
                <span
                  x-show="hasChildren(item)"
                  class="admin-nav-arrow"
                  :class="{ 'expanded': isExpanded(item.id) }"
                >›</span>
              </a>
              <!-- 子菜单 -->
              <div
                x-show="isExpanded(item.id)"
                class="nav-children"
                x-transition
              >
                <template x-for="child in getChildren(item.id)" :key="child.id">
                  <a
                    :href="child.route"
                    class="nav-child-link"
                    :class="{ 'active': isActive(child) }"
                    hx-get="child.route"
                    hx-target="#admin-content"
                    hx-swap="innerHTML"
                    hx-push-url="true"
                    x-text="child.label"
                  ></a>
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
    </aside>
    {% endblock %}

    <main id="admin-content" class="admin-main">
      {% block content %}{% endblock %}
    </main>
  </div>
  <script src="{{ static_prefix }}/admin/js/menu.js"></script>
  {% block scripts %}{% endblock %}
</body>
```

- [ ] **Step 2: 提交**

```bash
git add web/templates/base.html
git commit -m "feat(admin): refactor base template for HTMX partial updates"
```

---

## Task 5: 菜单样式 - 收缩组件样式

**Files:**
- Modify: `web/static/admin/css/admin.css`

- [ ] **Step 1: 添加收缩菜单样式**

```css
/* 导航组件 */
.nav-item-wrapper {
  position: relative;
}

.nav-children {
  padding-left: 24px;
  overflow: hidden;
  transition: max-height 0.2s ease;
}

.nav-child-link {
  display: block;
  padding: 8px 12px;
  border-radius: 8px;
  color: #d3e0ff;
  text-decoration: none;
  font-size: 13px;
  font-weight: 500;
  transition: all 0.15s ease;
  margin-bottom: 2px;
}

.nav-child-link:hover {
  background: rgba(158, 193, 255, 0.14);
  color: #f2f6ff;
}

.nav-child-link.active {
  background: rgba(129, 174, 255, 0.26);
  color: #ffffff;
}

.admin-nav-link.has-children .admin-nav-arrow {
  font-size: 16px;
  transition: transform 0.2s ease;
}

.admin-nav-link.has-children .admin-nav-arrow.expanded {
  transform: rotate(90deg);
}

/* 导航底部 */
.nav-footer {
  margin-top: auto;
  padding-top: 16px;
  border-top: 1px solid rgba(190, 209, 255, 0.15);
}

.logout-link {
  color: #d3e0ff !important;
}

.logout-link:hover {
  background: rgba(180, 100, 100, 0.2) !important;
  color: #ffb3b3 !important;
}

/* 内容区域 */
#admin-content {
  flex: 1;
  min-width: 0;
  padding: 24px 28px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  overflow-y: auto;
}
```

- [ ] **Step 2: 提交**

```bash
git add web/static/admin/css/admin.css
git commit -m "feat(admin): add collapsible menu styles"
```

---

## Task 6: 菜单管理页面

**Files:**
- Create: `web/templates/admin/menus.html`
- Create: `web/templates/admin/partials/menu_rows.html`
- Create: `web/templates/admin/partials/menu_form.html`
- Modify: `crates/sushi-admin/src/routes/menu.rs`

- [ ] **Step 1: 创建菜单管理页面**

```html
{% extends "base.html" %}
{% set active_section = "menus" %}

{% block title %}Menu Management — Sushi Admin{% endblock %}
{% block main_attrs %}x-data="menuManagement()"{% endblock %}

{% block content %}
  <div class="ui-page-header">
    <div>
      <h1 class="ui-title">Menu Management</h1>
      <p class="ui-subtitle">Manage admin navigation menus and items.</p>
    </div>
    <div class="ui-toolbar">
      <button type="button" class="ui-btn ui-btn-primary" @click="openForm()">
        + Add Menu Item
      </button>
    </div>
  </div>

  <section class="ui-card">
    <div class="ui-card-body">
      <div class="menu-tree">
        <template x-for="item in topItems()" :key="item.id">
          <div class="menu-item">
            <div class="menu-item-row">
              <span class="menu-icon" x-html="getIcon(item.icon)"></span>
              <span class="menu-label" x-text="item.label"></span>
              <span class="menu-route" x-text="item.route || '-'"></span>
              <span class="menu-actions">
                <button @click="openForm(item)" class="ui-action-link">Edit</button>
                <button @click="deleteItem(item.id)" class="ui-action-link danger">Delete</button>
              </span>
            </div>
            <template x-for="child in getChildren(item.id)" :key="child.id">
              <div class="menu-item menu-item-child">
                <div class="menu-item-row">
                  <span class="menu-icon" x-html="getIcon(child.icon)"></span>
                  <span class="menu-label" x-text="child.label"></span>
                  <span class="menu-route" x-text="child.route || '-'"></span>
                  <span class="menu-actions">
                    <button @click="openForm(child)" class="ui-action-link">Edit</button>
                    <button @click="deleteItem(child.id)" class="ui-action-link danger">Delete</button>
                  </span>
                </div>
              </div>
            </template>
          </div>
        </template>
      </div>
    </div>
  </section>

  <!-- Modal Form -->
  <div x-show="showForm" class="ui-modal-overlay" @click.self="closeForm()">
    <div class="ui-modal-card">
      <div class="ui-modal-head">
        <h3 class="ui-modal-title" x-text="editingItem ? 'Edit Menu Item' : 'Add Menu Item'"></h3>
      </div>
      <div class="ui-modal-body">
        <form @submit.prevent="saveItem()">
          <div class="form-group">
            <label class="ui-label">Label</label>
            <input type="text" x-model="form.label" class="ui-input" required>
          </div>
          <div class="form-group">
            <label class="ui-label">Icon</label>
            <select x-model="form.icon" class="ui-select">
              <option value="">No icon</option>
              <template x-for="[name, svg] in iconOptions" :key="name">
                <option :value="name" x-text="name"></option>
              </template>
            </select>
          </div>
          <div class="form-group">
            <label class="ui-label">Route</label>
            <input type="text" x-model="form.route" class="ui-input">
          </div>
          <div class="form-group">
            <label class="ui-label">Parent Menu</label>
            <select x-model="form.parent_id" class="ui-select">
              <option value="">Top Level</option>
              <template x-for="item in topItems()" :key="item.id">
                <option :value="item.id" x-text="item.label"></option>
              </template>
            </select>
          </div>
          <div class="form-group">
            <label class="ui-label">
              <input type="checkbox" x-model="form.is_hidden"> Hidden
            </label>
          </div>
          <div class="ui-modal-actions">
            <button type="button" @click="closeForm()" class="ui-btn ui-btn-secondary">Cancel</button>
            <button type="submit" class="ui-btn ui-btn-primary">Save</button>
          </div>
        </form>
      </div>
    </div>
  </div>
{% endblock %}

{% block scripts %}
<script src="{{ static_prefix }}/admin/js/menu.js"></script>
<script src="{{ static_prefix }}/admin/js/menus.js"></script>
{% endblock %}
```

- [ ] **Step 2: 创建菜单管理组件**

```javascript
// web/static/admin/js/menus.js
(() => {
  window.menuManagement = function menuManagement() {
    return {
      ...window.adminMenu(),

      showForm: false,
      editingItem: null,
      form: {
        label: '',
        icon: '',
        route: '',
        parent_id: '',
        is_hidden: false,
      },

      iconOptions: [
        ['layout-dashboard', '...'],
        ['users', '...'],
        ['shield', '...'],
        ['key', '...'],
        ['package', '...'],
        ['settings', '...'],
        ['file-text', '...'],
        ['database', '...'],
      ],

      topItems() {
        return this.menuItems.filter(i => !i.parent_id);
      },

      openForm(item = null) {
        this.editingItem = item;
        if (item) {
          this.form = { ...item, parent_id: item.parent_id || '' };
        } else {
          this.form = { label: '', icon: '', route: '', parent_id: '', is_hidden: false };
        }
        this.showForm = true;
      },

      closeForm() {
        this.showForm = false;
        this.editingItem = null;
      },

      async saveItem() {
        const method = this.editingItem ? 'PUT' : 'POST';
        const url = this.editingItem
          ? `/admin/api/menu/${this.editingItem.id}`
          : '/admin/api/menu';

        await fetch(url, {
          method,
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(this.form),
        });

        this.closeForm();
        await this.loadMenu();
      },

      async deleteItem(id) {
        if (!confirm('Delete this menu item?')) return;
        await fetch(`/admin/api/menu/${id}`, { method: 'DELETE' });
        await this.loadMenu();
      },
    };
  };
})();
```

- [ ] **Step 3: 提交**

```bash
git add web/templates/admin/menus.html web/static/admin/js/menus.js
git commit -m "feat(admin): add menu management page"
```

---

## Task 7: 集成测试

**Files:**
- Modify: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: 添加菜单 API 测试**

```rust
#[tokio::test]
async fn menu_crud_operations() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

    // Create
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
                .method("POST")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"label":"Test Menu","icon":"settings","route":"/admin/test"}"#))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);

    // List
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/menu")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --package sushi-admin`

- [ ] **Step 3: 提交**

```bash
git add crates/sushi-admin/tests/admin_web.rs
git commit -m "test(admin): add menu CRUD API tests"
```

---

## Task 8: Wiki 文档更新

**Files:**
- Modify: `docs/wiki/architecture/admin-panel.md`

- [ ] **Step 1: 更新文档**

```markdown
## 菜单管理

管理员可以在 `/admin/menus` 管理菜单项：

### API

| 方法 | 路径 | 功能 |
|-----|------|------|
| GET | `/admin/api/menu` | 获取菜单列表 |
| POST | `/admin/api/menu` | 创建菜单项 |
| PUT | `/admin/api/menu/:id` | 更新菜单项 |
| DELETE | `/admin/api/menu/:id` | 删除菜单项 |

### 字段说明

| 字段 | 类型 | 说明 |
|-----|------|------|
| label | TEXT | 菜单显示名称 |
| icon | TEXT | Lucide 图标名 |
| position | INTEGER | 排序位置 |
| parent_id | INTEGER | NULL=一级菜单 |
| route | TEXT | 路由路径 |
| is_hidden | INTEGER | 0=显示, 1=隐藏 |
```

- [ ] **Step 2: 提交**

```bash
git add docs/wiki/architecture/admin-panel.md
git commit -m "docs(wiki): update admin panel menu documentation"
```

---

## 验证清单

- [ ] 数据库迁移成功
- [ ] 菜单 API CRUD 正常
- [ ] 左侧菜单多展开正常
- [ ] 点击菜单局部刷新正常
- [ ] 退出登录按钮正常
- [ ] 菜单管理页面正常
- [ ] 测试全部通过
