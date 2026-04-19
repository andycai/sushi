ALTER TABLE plugin_state ADD COLUMN plugin_id TEXT NOT NULL DEFAULT '';
ALTER TABLE plugin_state ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'third_party';
ALTER TABLE plugin_state ADD COLUMN updated_by TEXT NOT NULL DEFAULT '';
ALTER TABLE plugin_state ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now'));
ALTER TABLE plugin_state ADD COLUMN reason TEXT NOT NULL DEFAULT '';

UPDATE plugin_state
SET plugin_id = name
WHERE plugin_id IS NULL OR TRIM(plugin_id) = '';

CREATE UNIQUE INDEX IF NOT EXISTS idx_plugin_state_plugin_id ON plugin_state(plugin_id);

CREATE TABLE IF NOT EXISTS plugin_state_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plugin_id TEXT NOT NULL,
    source_kind TEXT NOT NULL DEFAULT 'third_party',
    changed_by TEXT NOT NULL DEFAULT '',
    previous_enabled INTEGER,
    next_enabled INTEGER,
    reason TEXT NOT NULL DEFAULT '',
    changed_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (plugin_id) REFERENCES plugin_state(plugin_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_plugin_state_events_plugin_changed_at
    ON plugin_state_events(plugin_id, changed_at DESC);

INSERT OR IGNORE INTO policy_keys (key, surface, resource, action, name, description, is_system) VALUES
    ('admin.plugins.manage', 'admin', 'plugins', 'manage', 'Manage Admin Plugins', 'Enable and disable plugins from admin surfaces.', 1),
    ('cli.plugins.manage', 'cli', 'plugins', 'manage', 'Manage CLI Plugins', 'Enable and disable plugins from CLI surfaces.', 1);

INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
SELECT r.id, pk.id
FROM roles r
JOIN policy_keys pk ON pk.key IN ('admin.plugins.manage', 'cli.plugins.manage')
WHERE r.slug = 'admin';

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
    ('admin', 'http_route', '/admin/api/plugins/{plugin}/state', 'PATCH', '/admin/api/plugins/{plugin}/state', NULL, 'admin.plugins.manage', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:status', NULL, NULL, 'plugin:status', 'cli.plugins.read', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:enable', NULL, NULL, 'plugin:enable', 'cli.plugins.manage', 'system', 'builtin', 1),
    ('cli', 'cli_command', 'plugin:disable', NULL, NULL, 'plugin:disable', 'cli.plugins.manage', 'system', 'builtin', 1)
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

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (8, '008_plugin_governance_v1');
