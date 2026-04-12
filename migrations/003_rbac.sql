CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    module TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id INTEGER NOT NULL,
    permission_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY (permission_id) REFERENCES permissions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_role_permissions_role_id ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission_id ON role_permissions(permission_id);

INSERT OR IGNORE INTO roles (slug, name, description, is_system) VALUES
    ('admin', 'Administrator', 'Full platform access for operational administrators.', 1),
    ('editor', 'Editor', 'Operational role for maintaining runtime content and settings.', 1),
    ('viewer', 'Viewer', 'Read-only role for dashboards and observability surfaces.', 1);

INSERT OR IGNORE INTO permissions (slug, name, module, description, is_system) VALUES
    ('dashboard.view', 'View Dashboard', 'dashboard', 'Access operational dashboard metrics and summaries.', 1),
    ('users.view', 'View Users', 'users', 'Read user identity and role records.', 1),
    ('users.manage', 'Manage Users', 'users', 'Create or delete users from the admin surface.', 1),
    ('roles.view', 'View Roles', 'roles', 'Read role definitions and assignments.', 1),
    ('roles.manage', 'Manage Roles', 'roles', 'Edit role metadata and role-to-permission mappings.', 1),
    ('permissions.view', 'View Permissions', 'permissions', 'Read permission catalog entries.', 1),
    ('permissions.manage', 'Manage Permissions', 'permissions', 'Create, edit, or delete permission entries.', 1),
    ('plugins.view', 'View Plugins', 'plugins', 'Inspect plugin manifests and runtime load state.', 1),
    ('plugins.manage', 'Manage Plugins', 'plugins', 'Modify plugin activation and lifecycle controls.', 1),
    ('config.view', 'View Config', 'config', 'Inspect sanitized runtime configuration values.', 1),
    ('logs.view', 'View Logs', 'logs', 'Read operational event and service logs.', 1),
    ('kv.manage', 'Manage KV Store', 'kv', 'Create, edit, and delete KV store entries.', 1);

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM roles
JOIN permissions ON 1 = 1
WHERE roles.slug = 'admin';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM roles
JOIN permissions ON permissions.slug IN (
    'dashboard.view',
    'users.view',
    'users.manage',
    'roles.view',
    'permissions.view',
    'plugins.view',
    'logs.view',
    'kv.manage'
)
WHERE roles.slug = 'editor';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM roles
JOIN permissions ON permissions.slug IN (
    'dashboard.view',
    'logs.view'
)
WHERE roles.slug = 'viewer';

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (3, '003_rbac');
