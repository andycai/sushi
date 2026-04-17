CREATE TABLE IF NOT EXISTS cms_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS cms_pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    markdown_body TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS cms_posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    excerpt TEXT,
    markdown_body TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published')),
    category_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT,
    FOREIGN KEY (category_id) REFERENCES cms_categories(id)
);

CREATE INDEX IF NOT EXISTS idx_cms_pages_status_deleted ON cms_pages(status, deleted_at);
CREATE INDEX IF NOT EXISTS idx_cms_posts_status_deleted ON cms_posts(status, deleted_at);
CREATE INDEX IF NOT EXISTS idx_cms_posts_category_deleted ON cms_posts(category_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_cms_categories_slug_deleted ON cms_categories(slug, deleted_at);

INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (7, '007_cms');
