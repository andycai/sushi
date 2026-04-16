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
    FOREIGN KEY (policy_key_id) REFERENCES policy_keys(id) ON DELETE CASCADE
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

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (6, '006_unified_policy_v2');
