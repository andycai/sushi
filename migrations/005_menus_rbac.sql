INSERT OR IGNORE INTO permissions (slug, name, module, description, is_system) VALUES
    ('menus.view', 'View Menus', 'menus', 'Read admin navigation menu catalog entries.', 1),
    ('menus.manage', 'Manage Menus', 'menus', 'Create, edit, and delete admin navigation menu entries.', 1);

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM roles
JOIN permissions ON permissions.slug IN (
    'menus.view',
    'menus.manage'
)
WHERE roles.slug = 'admin';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM roles
JOIN permissions ON permissions.slug IN (
    'menus.view'
)
WHERE roles.slug = 'editor';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM roles
JOIN permissions ON permissions.slug IN (
    'menus.view'
)
WHERE roles.slug = 'viewer';

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (5, '005_menus_rbac');
