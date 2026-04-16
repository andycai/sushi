CREATE TABLE IF NOT EXISTS policy_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    surface TEXT NOT NULL,
    resource TEXT NOT NULL,
    action TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS role_policy_keys (
    role_id INTEGER NOT NULL,
    policy_key_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (role_id, policy_key_id),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY (policy_key_id) REFERENCES policy_keys(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS policy_bindings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    surface TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_ref TEXT NOT NULL,
    method TEXT,
    path_pattern TEXT,
    command_name TEXT,
    policy_key_id INTEGER NOT NULL,
    owner_type TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (policy_key_id) REFERENCES policy_keys(id) ON DELETE CASCADE,
    CHECK (
        (target_type = 'http_route'
            AND method IS NOT NULL
            AND path_pattern IS NOT NULL
            AND command_name IS NULL)
        OR (target_type = 'cli_command'
            AND command_name IS NOT NULL
            AND method IS NULL
            AND path_pattern IS NULL)
        OR (target_type NOT IN ('http_route', 'cli_command'))
    )
);

CREATE TABLE IF NOT EXISTS plugin_policy_scopes (
    plugin_name TEXT NOT NULL,
    scope_pattern TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_name, scope_pattern)
);

CREATE INDEX IF NOT EXISTS idx_role_policy_keys_policy_key_id
    ON role_policy_keys(policy_key_id);
CREATE INDEX IF NOT EXISTS idx_policy_bindings_policy_key_id
    ON policy_bindings(policy_key_id);
CREATE INDEX IF NOT EXISTS idx_policy_bindings_surface_target
    ON policy_bindings(surface, target_type, target_ref);
CREATE UNIQUE INDEX IF NOT EXISTS idx_policy_bindings_unique_tuple
    ON policy_bindings(
        surface,
        target_type,
        target_ref,
        COALESCE(method, ''),
        COALESCE(path_pattern, ''),
        COALESCE(command_name, ''),
        policy_key_id,
        owner_type,
        owner_id
    );

INSERT OR IGNORE INTO policy_keys (key, surface, resource, action, name, description, is_system) VALUES
    ('admin.dashboard.view', 'admin', 'dashboard', 'view', 'View Admin Dashboard', 'Access the admin dashboard.', 1),
    ('admin.users.view', 'admin', 'users', 'view', 'View Admin Users', 'Read users from admin surfaces.', 1),
    ('admin.users.manage', 'admin', 'users', 'manage', 'Manage Admin Users', 'Create or delete users from admin surfaces.', 1),
    ('admin.roles.view', 'admin', 'roles', 'view', 'View Admin Roles', 'Read role assignments from admin surfaces.', 1),
    ('admin.roles.manage', 'admin', 'roles', 'manage', 'Manage Admin Roles', 'Edit role assignments from admin surfaces.', 1),
    ('admin.permissions.view', 'admin', 'permissions', 'view', 'View Admin Permissions', 'Read permission catalog entries from admin surfaces.', 1),
    ('admin.permissions.manage', 'admin', 'permissions', 'manage', 'Manage Admin Permissions', 'Create, edit, or delete permissions from admin surfaces.', 1),
    ('admin.plugins.view', 'admin', 'plugins', 'view', 'View Admin Plugins', 'Inspect plugin metadata from admin surfaces.', 1),
    ('admin.kv.manage', 'admin', 'kv', 'manage', 'Manage Admin KV', 'Manage key-value entries from admin surfaces.', 1),
    ('admin.config.view', 'admin', 'config', 'view', 'View Admin Config', 'Inspect runtime config from admin surfaces.', 1),
    ('admin.logs.view', 'admin', 'logs', 'view', 'View Admin Logs', 'Read runtime logs from admin surfaces.', 1),
    ('admin.menus.view', 'admin', 'menus', 'view', 'View Admin Menus', 'Read admin navigation menu entries.', 1),
    ('admin.menus.manage', 'admin', 'menus', 'manage', 'Manage Admin Menus', 'Create, update, and delete admin navigation menu entries.', 1),
    ('api.users.read', 'api', 'users', 'read', 'Read API Users', 'List users through API routes.', 1),
    ('api.users.manage', 'api', 'users', 'manage', 'Manage API Users', 'Create or delete users through API routes.', 1),
    ('cli.plugins.read', 'cli', 'plugins', 'read', 'Read Plugin List', 'List discovered plugins from the CLI.', 1);

INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
SELECT r.id, pk.id
FROM roles r
JOIN role_permissions rp ON rp.role_id = r.id
JOIN permissions p ON p.id = rp.permission_id
JOIN policy_keys pk ON pk.key = ('admin.' || p.slug);

INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
SELECT roles.id, policy_keys.id
FROM roles
JOIN policy_keys ON 1 = 1
WHERE roles.slug = 'admin';

INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
SELECT roles.id, policy_keys.id
FROM roles
JOIN policy_keys ON policy_keys.key IN (
    'admin.dashboard.view',
    'admin.users.view',
    'admin.users.manage',
    'admin.roles.view',
    'admin.permissions.view',
    'admin.plugins.view',
    'admin.kv.manage',
    'admin.logs.view',
    'admin.menus.view',
    'api.users.read',
    'api.users.manage',
    'cli.plugins.read'
)
WHERE roles.slug = 'editor';

INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
SELECT roles.id, policy_keys.id
FROM roles
JOIN policy_keys ON policy_keys.key IN (
    'admin.dashboard.view',
    'admin.logs.view'
)
WHERE roles.slug = 'viewer';

WITH seeded_bindings (
    surface,
    target_type,
    target_ref,
    method,
    path_pattern,
    command_name,
    policy_key,
    owner_type,
    owner_id,
    is_system
) AS (
    VALUES
    ('admin', 'http_route', '/admin/', 'GET', '/admin/', NULL, 'admin.dashboard.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/logs', 'GET', '/admin/logs', NULL, 'admin.logs.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/logs', 'GET', '/admin/api/logs', NULL, 'admin.logs.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/config', 'GET', '/admin/config', NULL, 'admin.config.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/config', 'GET', '/admin/api/config', NULL, 'admin.config.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/plugins', 'GET', '/admin/plugins', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/plugins/{plugin}', 'GET', '/admin/plugins/{plugin}', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/plugins/table', 'GET', '/admin/partials/plugins/table', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/plugins', 'GET', '/admin/api/plugins', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/plugins/{plugin}/pages', 'GET', '/admin/api/plugins/{plugin}/pages', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/workspace/assets', 'GET', '/admin/api/workspace/assets', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/menus', 'GET', '/admin/menus', NULL, 'admin.menus.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/menus/table', 'GET', '/admin/partials/menus/table', NULL, 'admin.menus.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/menu', 'GET', '/admin/api/menu', NULL, 'admin.menus.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/users', 'GET', '/admin/users', NULL, 'admin.users.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/users/table', 'GET', '/admin/partials/users/table', NULL, 'admin.users.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/roles', 'GET', '/admin/roles', NULL, 'admin.roles.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/roles/table', 'GET', '/admin/partials/roles/table', NULL, 'admin.roles.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/permissions', 'GET', '/admin/permissions', NULL, 'admin.permissions.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/permissions/table', 'GET', '/admin/partials/permissions/table', NULL, 'admin.permissions.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/roles/{id}/permissions/form', 'GET', '/admin/partials/roles/{id}/permissions/form', NULL, 'admin.roles.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/users/create', 'POST', '/admin/partials/users/create', NULL, 'admin.users.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/users/{id}', 'DELETE', '/admin/partials/users/{id}', NULL, 'admin.users.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/roles/create', 'POST', '/admin/partials/roles/create', NULL, 'admin.roles.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/roles/{id}/update', 'POST', '/admin/partials/roles/{id}/update', NULL, 'admin.roles.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/roles/{id}/permissions', 'POST', '/admin/partials/roles/{id}/permissions', NULL, 'admin.roles.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/roles/{id}', 'DELETE', '/admin/partials/roles/{id}', NULL, 'admin.roles.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/permissions/create', 'POST', '/admin/partials/permissions/create', NULL, 'admin.permissions.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/permissions/{id}/update', 'POST', '/admin/partials/permissions/{id}/update', NULL, 'admin.permissions.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/permissions/{id}', 'DELETE', '/admin/partials/permissions/{id}', NULL, 'admin.permissions.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/menus/create', 'POST', '/admin/partials/menus/create', NULL, 'admin.menus.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/menus/{id}/update', 'POST', '/admin/partials/menus/{id}/update', NULL, 'admin.menus.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/partials/menus/{id}', 'DELETE', '/admin/partials/menus/{id}', NULL, 'admin.menus.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/menu', 'POST', '/admin/api/menu', NULL, 'admin.menus.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/menu/{id}', 'PUT', '/admin/api/menu/{id}', NULL, 'admin.menus.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/api/menu/{id}', 'DELETE', '/admin/api/menu/{id}', NULL, 'admin.menus.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/dashboard', 'GET', '/admin/workspace/dashboard', NULL, 'admin.dashboard.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/users', 'GET', '/admin/workspace/users', NULL, 'admin.users.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/roles', 'GET', '/admin/workspace/roles', NULL, 'admin.roles.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/permissions', 'GET', '/admin/workspace/permissions', NULL, 'admin.permissions.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/plugins', 'GET', '/admin/workspace/plugins', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/plugins/{plugin}', 'GET', '/admin/workspace/plugins/{plugin}', NULL, 'admin.plugins.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/kv', 'GET', '/admin/workspace/kv', NULL, 'admin.kv.manage', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/config', 'GET', '/admin/workspace/config', NULL, 'admin.config.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/logs', 'GET', '/admin/workspace/logs', NULL, 'admin.logs.view', 'system', 'builtin', 1),
    ('admin', 'http_route', '/admin/workspace/menus', 'GET', '/admin/workspace/menus', NULL, 'admin.menus.view', 'system', 'builtin', 1),
    ('api', 'http_route', '/api/users', 'GET', '/api/users', NULL, 'api.users.read', 'system', 'builtin', 1),
    ('api', 'http_route', '/api/users', 'POST', '/api/users', NULL, 'api.users.manage', 'system', 'builtin', 1),
    ('api', 'http_route', '/api/users/{id}', 'DELETE', '/api/users/{id}', NULL, 'api.users.manage', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:list', NULL, NULL, 'plugin:list', 'cli.plugins.read', 'system', 'builtin', 1)
)
INSERT OR IGNORE INTO policy_bindings (
    surface,
    target_type,
    target_ref,
    method,
    path_pattern,
    command_name,
    policy_key_id,
    owner_type,
    owner_id,
    is_system
)
SELECT
    seeded_bindings.surface,
    seeded_bindings.target_type,
    seeded_bindings.target_ref,
    seeded_bindings.method,
    seeded_bindings.path_pattern,
    seeded_bindings.command_name,
    pk.id,
    seeded_bindings.owner_type,
    seeded_bindings.owner_id,
    seeded_bindings.is_system
FROM seeded_bindings
JOIN policy_keys pk ON pk.key = seeded_bindings.policy_key;

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (6, '006_unified_policy_v2');
