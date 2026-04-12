# Admin Menu 重构实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将管理后台硬编码菜单改为可动态配置的二级菜单系统，支持 Lucide 图标和抽屉式二级菜单。

**Architecture:** 数据库存储菜单结构，前端通过 API 获取菜单数据，Alpine.js 管理抽屉展开状态，Lucide SVG 图标动态渲染。

**Tech Stack:** SQLite, Axum, Handlebars templates, Alpine.js, Lucide Icons

---

## 文件变更概览

| 文件 | 变更 |
|-----|------|
| `migrations/004_menu.sql` | 新增迁移，创建 menu_items 表 |
| `crates/sushi-admin/src/routes/menu.rs` | 新增菜单 API 路由 |
| `crates/sushi-admin/src/routes/mod.rs` | 注册菜单路由 |
| `crates/sushi-admin/src/lib.rs` | 导出 menu 模块 |
| `web/templates/base.html` | 重构菜单渲染逻辑 |
| `web/static/admin/css/admin.css` | 添加菜单样式 |
| `web/static/admin/js/menu.js` | 新增菜单交互组件 |

---

## Task 1: 数据库迁移

**Files:**
- Create: `migrations/004_menu.sql`

- [ ] **Step 1: 创建迁移文件**

```sql
CREATE TABLE IF NOT EXISTS menu_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    icon TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    parent_id INTEGER,
    route TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (parent_id) REFERENCES menu_items(id) ON DELETE SET NULL
);

-- 初始化内置一级菜单
INSERT INTO menu_items (id, label, icon, position, parent_id, route) VALUES
(1, 'Dashboard', 'layout-dashboard', 10, NULL, '/admin/'),
(2, 'Users', 'users', 20, NULL, '/admin/users'),
(3, 'Roles', 'shield', 30, NULL, '/admin/roles'),
(4, 'Permissions', 'key', 40, NULL, '/admin/permissions'),
(5, 'Plugins', 'package', 50, NULL, '/admin/plugins'),
(6, 'Config', 'settings', 60, NULL, '/admin/config'),
(7, 'Logs', 'file-text', 70, NULL, '/admin/logs');

-- 初始化内置二级菜单
INSERT INTO menu_items (label, icon, position, parent_id, route) VALUES
('KV Store', 'database', 51, 5, '/admin/kv');
```

- [ ] **Step 2: 运行迁移测试**

Run: `cargo test --package sushi-core -- db_gateway --nocapture`
Expected: 迁移文件可被 SQLite 正确执行

- [ ] **Step 3: Commit**

```bash
git add migrations/004_menu.sql
git commit -m "feat(admin): add menu_items table migration"
```

---

## Task 2: 菜单 API 路由

**Files:**
- Create: `crates/sushi-admin/src/routes/menu.rs`
- Modify: `crates/sushi-admin/src/routes/mod.rs:1` (添加 menu 导出)
- Modify: `crates/sushi-admin/src/lib.rs` (注册路由)

- [ ] **Step 1: 创建菜单 API 路由**

```rust
use axum::{extract::State, routing::get, Json, Router};
use sushi_core::context::SushiContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct MenuItem {
    pub id: i64,
    pub label: String,
    pub icon: Option<String>,
    pub position: i64,
    pub parent_id: Option<i64>,
    pub route: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuResponse {
    pub menu: Vec<MenuItem>,
}

pub async fn menu_api(State(ctx): State<SushiContext>) -> impl IntoResponse {
    let rows = ctx.db
        .query(
            "SELECT id, label, icon, position, parent_id, route
             FROM menu_items
             ORDER BY position ASC, id ASC",
            vec![]
        )
        .await
        .unwrap_or_default();

    let menu: Vec<MenuItem> = rows.into_iter().map(|row| MenuItem {
        id: row.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        label: row.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        icon: row.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string()),
        position: row.get("position").and_then(|v| v.as_i64()).unwrap_or(0),
        parent_id: row.get("parent_id").and_then(|v| v.as_i64()),
        route: row.get("route").and_then(|v| v.as_str()).map(|s| s.to_string()),
    }).collect();

    Json(MenuResponse { menu })
}

pub fn routes() -> Router<SushiContext> {
    Router::new().route("/admin/api/menu", get(menu_api))
}
```

- [ ] **Step 2: 注册路由到 mod.rs**

在 `routes/mod.rs` 添加:
```rust
pub mod menu;
```

- [ ] **Step 3: 注册路由到 admin router**

在 `crates/sushi-admin/src/router.rs` 的 `build_admin_router` 函数中添加:
```rust
.route("/admin/api/menu", get(menu::menu_api))
```

并在 use 语句中添加:
```rust
use crate::routes::menu;
```

- [ ] **Step 4: 构建测试**

Run: `cargo build --package sushi-admin 2>&1`
Expected: 编译成功，无错误

- [ ] **Step 5: 提交**

```bash
git add crates/sushi-admin/src/routes/menu.rs crates/sushi-admin/src/routes/mod.rs
git commit -m "feat(admin): add menu API endpoint"
```

---

## Task 3: 菜单渲染模板

**Files:**
- Modify: `web/templates/base.html`

- [ ] **Step 1: 修改 base.html 添加菜单渲染**

将现有的硬编码菜单:
```html
<nav class="admin-nav">
  {{ nav_link('/admin/', 'Dashboard', 'dashboard') }}
  {{ nav_link('/admin/plugins', 'Plugins', 'plugins') }}
  ...
</nav>
```

替换为动态渲染:

```html
<nav class="admin-nav" x-data="adminMenu()">
  <template x-for="item in menuItems.filter(i => !i.parent_id)" :key="item.id">
    <div class="nav-group">
      <!-- 一级菜单项 -->
      <a
        :href="item.route || '#'"
        class="admin-nav-link"
        :class="{ 'active': isActive(item) }"
        @click="if (hasChildren(item)) { toggleDrawer(item.id); $event.preventDefault(); }"
      >
        <span class="admin-nav-icon" x-html="getIcon(item.icon)"></span>
        <span class="admin-nav-label" x-text="item.label"></span>
        <span
          x-show="hasChildren(item)"
          class="admin-nav-arrow"
          :class="{ 'open': openDrawers[item.id] }"
        >→</span>
      </a>
    </div>
  </template>

  <!-- 抽屉遮罩 -->
  <template x-for="item in menuItems.filter(i => !i.parent_id && openDrawers[i.id])" :key="'drawer-' + item.id">
    <div
      class="ui-drawer-overlay"
      @click="closeDrawer(item.id)"
    ></div>
  </template>

  <!-- 抽屉内容 -->
  <template x-for="item in menuItems.filter(i => !i.parent_id)" :key="'panel-' + item.id">
    <div
      class="ui-drawer-panel"
      x-show="openDrawers[item.id]"
      x-transition:enter="drawer-enter"
      x-transition:leave="drawer-leave"
    >
      <div class="ui-drawer-head">
        <h3 class="ui-drawer-title" x-text="item.label + ' 子菜单'"></h3>
        <button @click="closeDrawer(item.id)" class="ui-btn ui-btn-ghost">关闭</button>
      </div>
      <div class="ui-drawer-body">
        <template x-for="child in getChildren(item.id)" :key="child.id">
          <a
            :href="child.route || '#'"
            class="drawer-nav-item"
            :class="{ 'active': isActive(child) }"
          >
            <span class="admin-nav-icon" x-html="getIcon(child.icon)"></span>
            <span x-text="child.label"></span>
          </a>
        </template>
      </div>
    </div>
  </template>
</nav>
```

- [ ] **Step 2: 添加菜单数据获取**

在 `<body>` 标签添加 `x-data` 和初始化:

```html
<body x-data="adminMenu()" @init.window="initMenu()">
```

- [ ] **Step 3: 提交**

```bash
git add web/templates/base.html
git commit -m "feat(admin): add dynamic menu rendering in base template"
```

---

## Task 4: 菜单交互组件

**Files:**
- Create: `web/static/admin/js/menu.js`

- [ ] **Step 1: 创建菜单 Alpine.js 组件**

```javascript
(() => {
  window.adminMenu = function adminMenu() {
    return {
      menuItems: [],
      openDrawers: {},

      async initMenu() {
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

      hasChildren(item) {
        return this.menuItems.some(i => i.parent_id === item.id);
      },

      getChildren(parentId) {
        return this.menuItems.filter(i => i.parent_id === parentId);
      },

      toggleDrawer(itemId) {
        this.openDrawers[itemId] = !this.openDrawers[itemId];
      },

      closeDrawer(itemId) {
        this.openDrawers[itemId] = false;
      },

      isActive(item) {
        const path = window.location.pathname;
        return item.route === path;
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
        };
        return icons[iconName] || '';
      }
    };
  };
})();
```

- [ ] **Step 2: 在 base.html 中引入**

在 `{% block scripts %}` 前添加:
```html
<script src="{{ static_url_prefix | default(value="/static") }}/admin/js/menu.js"></script>
```

- [ ] **Step 3: 提交**

```bash
git add web/static/admin/js/menu.js
git commit -m "feat(admin): add Alpine.js menu component with Lucide icons"
```

---

## Task 5: 菜单样式

**Files:**
- Modify: `web/static/admin/css/admin.css`

- [ ] **Step 1: 添加菜单相关样式**

在 CSS 文件末尾添加:

```css
/* 菜单样式 */
.nav-group {
  position: relative;
}

.admin-nav-icon {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.admin-nav-icon svg {
  width: 18px;
  height: 18px;
}

.admin-nav-arrow {
  margin-left: auto;
  transition: transform 0.2s ease;
  font-size: 12px;
  opacity: 0.7;
}

.admin-nav-arrow.open {
  transform: rotate(90deg);
}

.admin-nav-label {
  flex: 1;
}

/* 抽屉子菜单项 */
.drawer-nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 12px;
  color: var(--text-primary);
  text-decoration: none;
  font-size: 14px;
  font-weight: 600;
  transition: all 0.2s ease;
  margin-bottom: 4px;
}

.drawer-nav-item:hover {
  background: var(--bg-muted);
  color: var(--brand-strong);
}

.drawer-nav-item.active {
  background: linear-gradient(145deg, rgba(129, 174, 255, 0.2), rgba(103, 149, 255, 0.1));
  color: var(--brand-strong);
  box-shadow: inset 0 0 0 1px rgba(103, 149, 255, 0.2);
}

.drawer-nav-item .admin-nav-icon {
  color: var(--text-secondary);
}

.drawer-nav-item.active .admin-nav-icon,
.drawer-nav-item:hover .admin-nav-icon {
  color: var(--brand-strong);
}

/* 抽屉过渡动画 */
.drawer-enter {
  animation: drawerIn 0.25s ease-out;
}

.drawer-leave {
  animation: drawerOut 0.2s ease-in;
}

@keyframes drawerIn {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes drawerOut {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}
```

- [ ] **Step 2: 提交**

```bash
git add web/static/admin/css/admin.css
git commit -m "feat(admin): add menu drawer styles"
```

---

## Task 6: 集成测试

**Files:**
- Modify: `crates/sushi-admin/tests/admin_web.rs`

- [ ] **Step 1: 添加菜单 API 测试**

```rust
#[tokio::test]
async fn menu_api_returns_menu_items() {
    let app = build_app(None).await;
    let token = admin_bearer_token();

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
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");
    let payload: Value = serde_json::from_slice(&body).expect("invalid json payload");

    let menu = payload.get("menu").and_then(Value::as_array).expect("menu array missing");
    assert!(!menu.is_empty(), "menu should have items");

    // 验证 Dashboard 存在
    let dashboard = menu.iter().find(|m| m.get("label").and_then(Value::as_str) == Some("Dashboard"));
    assert!(dashboard.is_some(), "Dashboard menu item should exist");
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --package sushi-admin menu_api`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add crates/sushi-admin/tests/admin_web.rs
git commit -m "test(admin): add menu API integration test"
```

---

## Task 7: Wiki 文档更新

**Files:**
- Modify: `docs/wiki/architecture/admin-panel.md` (新建)
- Modify: `docs/wiki/README.md`

- [ ] **Step 1: 创建 Admin Panel 文档**

Create: `docs/wiki/architecture/admin-panel.md`

```markdown
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
```

- [ ] **Step 2: 更新 Wiki README**

在 `docs/wiki/README.md` 添加链接到 admin-panel.md

- [ ] **Step 3: 提交**

```bash
git add docs/wiki/architecture/admin-panel.md docs/wiki/README.md
git commit -m "docs(wiki): add admin panel menu documentation"
```

---

## 验证清单

- [ ] 迁移创建成功，menu_items 表存在
- [ ] `/admin/api/menu` 返回正确 JSON 结构
- [ ] base.html 菜单动态渲染
- [ ] 点击菜单项显示抽屉
- [ ] Lucide 图标正确显示
- [ ] 抽屉可同时展开多个
- [ ] 点击遮罩关闭抽屉
- [ ] 测试全部通过

---

## 变更文件汇总

```
migrations/004_menu.sql                                    [新增]
crates/sushi-admin/src/routes/menu.rs                      [新增]
crates/sushi-admin/src/routes/mod.rs                       [修改]
crates/sushi-admin/src/router.rs                          [修改]
web/templates/base.html                                   [修改]
web/static/admin/css/admin.css                            [修改]
web/static/admin/js/menu.js                              [新增]
crates/sushi-admin/tests/admin_web.rs                      [修改]
docs/wiki/architecture/admin-panel.md                      [新增]
docs/wiki/README.md                                       [修改]
```
