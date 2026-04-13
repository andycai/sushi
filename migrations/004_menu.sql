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

-- 初始化内置一级菜单 (使用 INSERT OR IGNORE 避免重复插入)
INSERT OR IGNORE INTO menu_items (id, label, icon, position, parent_id, route) VALUES
(1, 'Dashboard', 'layout-dashboard', 10, NULL, '/admin/'),
(5, 'Plugins', 'package', 50, NULL, '/admin/plugins'),
(8, 'System', 'settings', 60, NULL, '/admin/system'),
(2, 'Users', 'users', 20, 8, '/admin/users'),
(3, 'Roles', 'shield', 30, 8, '/admin/roles'),
(4, 'Permissions', 'key', 40, 8, '/admin/permissions'),
(6, 'Config', 'settings', 60, 8, '/admin/config'),
(7, 'Logs', 'file-text', 70, 8, '/admin/logs');

-- 初始化内置一级菜单（菜单管理）
INSERT INTO menu_items (label, icon, position, parent_id, route)
SELECT 'Menus', 'settings', 61, 8, '/admin/menus'
WHERE NOT EXISTS (
    SELECT 1 FROM menu_items
    WHERE route = '/admin/menus'
);

-- 初始化内置二级菜单（KV Store）
INSERT INTO menu_items (label, icon, position, parent_id, route)
SELECT 'KV Store', 'database', 51, 5, '/admin/kv'
WHERE NOT EXISTS (
    SELECT 1 FROM menu_items
    WHERE parent_id = 5 AND route = '/admin/kv'
);

-- 将历史平铺的系统菜单迁移到 System 分组，仅回填顶级项，避免覆盖自定义层级。
UPDATE menu_items
SET parent_id = (
    SELECT id FROM menu_items
    WHERE route = '/admin/system'
    ORDER BY id
    LIMIT 1
)
WHERE parent_id IS NULL
  AND route IN (
      '/admin/users',
      '/admin/roles',
      '/admin/permissions',
      '/admin/config',
      '/admin/menus',
      '/admin/logs'
  );

-- 兼容历史重复数据：仅保留最早一条菜单记录
DELETE FROM menu_items
WHERE route = '/admin/kv'
  AND id <> (
      SELECT MIN(id) FROM menu_items
      WHERE route = '/admin/kv'
  );

DELETE FROM menu_items
WHERE route = '/admin/menus'
  AND id <> (
      SELECT MIN(id) FROM menu_items
      WHERE route = '/admin/menus'
  );
