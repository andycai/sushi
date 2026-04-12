# 认证与 RBAC

## 认证方案

- **JWT** - 无状态 access token + refresh token
- **密码哈希** - Argon2

## JWT 配置

```toml
[jwt]
secret = "your-secret-key-at-least-32-chars"
access_ttl = 3600       # 1 hour
refresh_ttl = 604800     # 7 days
```

## 用户角色

| 角色 | 说明 |
|-----|------|
| `admin` | 完全访问权限 |
| `editor` | 内容管理权限 |
| `viewer` | 只读权限 |

## RBAC 权限模型

### 数据模型

```sql
roles (
    id INTEGER PRIMARY KEY,
    slug TEXT UNIQUE,        -- admin, editor, viewer
    name TEXT,
    description TEXT,
    is_system BOOLEAN,        -- 系统角色不可删除
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

permissions (
    id INTEGER PRIMARY KEY,
    slug TEXT UNIQUE,        -- users.create, items.delete
    name TEXT,
    module TEXT,             -- users, items, settings
    description TEXT,
    is_system BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

role_permissions (
    role_id INTEGER,
    permission_id INTEGER,
    PRIMARY KEY (role_id, permission_id)
)

users (
    id INTEGER PRIMARY KEY,
    username TEXT UNIQUE,
    email TEXT UNIQUE,
    password_hash TEXT,
    role TEXT,               -- 关联 roles.slug
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)
```

### 内置系统权限

系统角色 `admin`, `editor`, `viewer` 不可删除。

系统权限按模块分组：
- `users.*` - 用户管理
- `roles.*` - 角色管理
- `permissions.*` - 权限管理
- `plugins.*` - 插件管理
- `config.*` - 配置管理

## API 认证端点

| 方法 | 路径 | 说明 |
|-----|------|------|
| POST | `/api/auth/login` | 登录，返回 access/refresh token |
| POST | `/api/auth/refresh` | 刷新 token |
| POST | `/api/auth/logout` | 登出 |
| GET | `/api/auth/me` | 获取当前用户信息 |

## 在 Lua 插件中使用认证

```lua
sushi.api.route("GET", "/api/protected", function(req)
    local token = req.headers["authorization"]
    if not token then
        return { status = 401, body = { error = "Unauthorized" } }
    end

    local claims = sushi.auth.verify_token(token)
    if not claims then
        return { status = 403, body = { error = "Invalid token" } }
    end

    return { status = 200, body = { user = claims } }
end)
```

## 相关文档

- [sushi.auth API](../lua-api/sushi.auth.md)
