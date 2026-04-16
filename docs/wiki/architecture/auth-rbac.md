# Authentication and RBAC

## Authentication

Sushi uses JWT-based authentication with short-lived access tokens and long-lived refresh tokens.
Passwords are hashed with Argon2.

```toml
[jwt]
secret = "your-secret-key-at-least-32-chars"
access_ttl = 3600
refresh_ttl = 604800
```

## Built-in roles

| Role | Intent |
| --- | --- |
| `admin` | Full operational access across built-in surfaces |
| `editor` | Operational read access plus selected write permissions |
| `viewer` | Read-only access to selected surfaces |

## Unified policy model

Runtime authorization now uses unified policy keys instead of direct `permissions` table lookups.

### Policy key format

Policy keys use exactly three dot-separated segments:

`surface.resource.action`

Examples:

- `admin.users.view`
- `admin.users.manage`
- `api.users.read`
- `api.users.manage`
- `cli.kv.execute`

### Core policy tables

```sql
roles (
    id INTEGER PRIMARY KEY,
    slug TEXT UNIQUE,
    name TEXT,
    description TEXT,
    is_system BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

users (
    id INTEGER PRIMARY KEY,
    username TEXT UNIQUE,
    email TEXT UNIQUE,
    password_hash TEXT,
    role TEXT,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

policy_keys (
    id INTEGER PRIMARY KEY,
    key TEXT UNIQUE,
    surface TEXT,
    resource TEXT,
    action TEXT,
    name TEXT,
    description TEXT,
    is_system BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

role_policy_keys (
    role_id INTEGER,
    policy_key_id INTEGER,
    PRIMARY KEY (role_id, policy_key_id)
)

policy_bindings (
    id INTEGER PRIMARY KEY,
    surface TEXT,
    target_type TEXT,
    target_ref TEXT,
    method TEXT,
    path_pattern TEXT,
    command_name TEXT,
    policy_key_id INTEGER,
    owner_type TEXT,
    owner_id TEXT,
    is_system BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

plugin_policy_scopes (
    plugin_name TEXT,
    scope_pattern TEXT,
    PRIMARY KEY (plugin_name, scope_pattern)
)
```

### Enforcement flow

1. Middleware validates the JWT access token.
2. Middleware resolves the caller role from claims.
3. The authorizer matches the request target (HTTP route or CLI command) to a policy binding.
4. Access is granted only if the role has the required policy key.
5. If no binding matches or no grant exists, access is denied (fail closed).

Admin partial routes keep an additional guard that requires the `admin` role.

## Plugin policy model

Plugins declare allowed policy scopes in `plugin.toml` and must attach concrete policy keys during Lua registration.
The loader validates that each concrete key is inside declared scopes.

- manifest scopes: `[policies].scopes = [...]`
- route/page/command registration: `policy = "surface.resource.action"`

## Built-in seed behavior

Migration `006_unified_policy_v2.sql` seeds built-in policy keys and role grants for `admin`, `editor`, and `viewer`.
These seeds are idempotent and are the baseline for end-to-end authorization checks.
