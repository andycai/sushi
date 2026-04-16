# Admin Panel

## Workspace navigation model

The admin UI uses a left menu plus right workspace partials.

- Leaf menu clicks load right-side content through HTMX (`GET /admin/workspace/:module`).
- Tabs deduplicate by path (clicking the same item re-activates the existing tab).
- Dashboard (`/admin/`) is fixed and not closable.
- Active tabs sync browser history (`history.pushState`) and restore with back/forward.
- Tab state persists in `localStorage` (`admin.workspace.tabs.v1`).
- If HTMX is unavailable, links fall back to full-page navigation.

### Workspace module routes

| Module | Route |
| --- | --- |
| dashboard | `/admin/` |
| users | `/admin/users` |
| roles | `/admin/roles` |
| permissions | `/admin/permissions` |
| plugins | `/admin/plugins` |
| kv | `/admin/kv` |
| config | `/admin/config` |
| logs | `/admin/logs` |
| menus | `/admin/menus` |

### Workspace partial endpoint

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/admin/workspace/:module` | Returns workspace content fragment only |

## Unified policy enforcement

Admin authorization is enforced by the unified authorizer with policy keys in `surface.resource.action` format.
Built-in admin bindings use `surface = admin` and match by HTTP method + path pattern.

### Built-in policy keys used by admin routes

- `admin.dashboard.view`
- `admin.users.view`
- `admin.users.manage`
- `admin.roles.view`
- `admin.roles.manage`
- `admin.permissions.view`
- `admin.permissions.manage`
- `admin.plugins.view`
- `admin.kv.manage`
- `admin.config.view`
- `admin.logs.view`
- `admin.menus.view`
- `admin.menus.manage`

### Enforcement notes

- Admin middleware validates JWT access tokens and resolves role from claims.
- `admin` role keeps full access to built-in admin routes.
- Non-admin roles are checked through authorizer bindings and role grants.
- `/admin/partials/*` routes keep an explicit admin-only guard.

## Menu system

The admin menu is dynamic and stored in `menu_items`.

### Table columns

| Column | Type | Meaning |
| --- | --- | --- |
| id | INTEGER | Primary key |
| label | TEXT | Display label |
| icon | TEXT | Lucide icon name |
| position | INTEGER | Sort order |
| parent_id | INTEGER | `NULL` for top-level, otherwise parent row id |
| route | TEXT | Route path |
| is_hidden | INTEGER | `0` visible, `1` hidden |

### Menu API

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/admin/api/menu` | List menu entries |
| POST | `/admin/api/menu` | Create menu entry |
| PUT | `/admin/api/menu/:id` | Update menu entry |
| DELETE | `/admin/api/menu/:id` | Delete menu entry |

### Menu management page

Route: `/admin/menus`

- Render menu tree.
- Support add/edit/delete.
- Support visible/hidden toggles.
