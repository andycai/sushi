use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::policy_repository::PolicyRepository;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::logs::tracing_bridge;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::storage::Storage;
use sushi_core::web::template_service::TemplateService;

const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");
const RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/003_rbac.sql");
const MENU_MIGRATION_SQL: &str = include_str!("../../../migrations/004_menu.sql");
const MENUS_RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/005_menus_rbac.sql");
const UNIFIED_POLICY_V2_MIGRATION_SQL: &str =
    include_str!("../../../migrations/006_unified_policy_v2.sql");
const CMS_MIGRATION_SQL: &str = include_str!("../../../migrations/007_cms.sql");
const PLUGIN_GOVERNANCE_MIGRATION_SQL: &str =
    include_str!("../../../migrations/008_plugin_governance_v1.sql");
const PLUGIN_GOVERNANCE_MIGRATION_NAME: &str = "008_plugin_governance_v1";
const PLUGIN_GOVERNANCE_MIGRATION_FINALIZE_SQL: &str = r#"
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
"#;

pub async fn bootstrap(config_path: Option<&Path>) -> Result<SushiContext> {
    let config = match config_path {
        Some(path) if path.exists() => ConfigStore::load(path)
            .await
            .context("failed to load config")?,
        Some(path) => {
            tracing::info!("no config file found at {}, using defaults", path.display());
            ConfigStore::new(SushiConfig::default())
        }
        None => ConfigStore::new(SushiConfig::default()),
    };

    let db_path = {
        let guard = config.get().await;
        guard.database.path.clone()
    };

    if let Some(parent) = Path::new(&db_path).parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    let storage = SqliteStorage::new(&db_path)
        .await
        .context("failed to open database")?;
    storage
        .run_migrations(MIGRATION_SQL)
        .await
        .context("failed to run migrations")?;
    storage
        .run_migrations(KV_MIGRATION_SQL)
        .await
        .context("failed to run kv migrations")?;
    storage
        .run_migrations(RBAC_MIGRATION_SQL)
        .await
        .context("failed to run rbac migrations")?;
    storage
        .run_migrations(MENU_MIGRATION_SQL)
        .await
        .context("failed to run menu migrations")?;
    storage
        .run_migrations(MENUS_RBAC_MIGRATION_SQL)
        .await
        .context("failed to run menus rbac migrations")?;
    storage
        .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
        .await
        .context("failed to run unified policy migrations")?;
    storage
        .run_migrations(CMS_MIGRATION_SQL)
        .await
        .context("failed to run cms migrations")?;
    run_plugin_governance_migration_if_needed(&storage).await?;

    let jwt = {
        let guard = config.get().await;
        JwtService::new(
            &guard.jwt.secret,
            guard.jwt.access_ttl,
            guard.jwt.refresh_ttl,
        )
    };

    let templates_dir = {
        let guard = config.get().await;
        resolve_templates_dir(config_path, &guard.web.templates_dir)?
    };

    let static_dir = {
        let guard = config.get().await;
        resolve_static_dir(config_path, &guard.web.static_dir)?
    };

    let file_browser_root_dir = {
        let guard = config.get().await;
        resolve_file_browser_root_dir(config_path, &guard.file_browser.root_dir)?
    };

    let plugins_dir = {
        let guard = config.get().await;
        PathBuf::from(&guard.plugins.directory)
    };

    config
        .update(|cfg| {
            cfg.web.static_dir = static_dir.to_string_lossy().to_string();
            cfg.file_browser.root_dir = file_browser_root_dir.to_string_lossy().to_string();
        })
        .await;

    tokio::fs::create_dir_all(&templates_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create templates directory {}",
                templates_dir.display()
            )
        })?;

    let mut lua_plugins = Vec::new();
    let mut plugin_template_roots = Vec::new();
    let mut plugin_static_roots = Vec::new();
    if plugins_dir.exists() {
        lua_plugins = LuaPlugin::scan_dir(&plugins_dir)
            .await
            .context("failed to scan plugins directory")?;
        for plugin in &lua_plugins {
            let plugin_path_id = plugin.path_id().to_string();
            let template_root = plugin.web_templates_dir();
            if template_root.is_dir() {
                plugin_template_roots.push((plugin_path_id.clone(), template_root));
            }
            let static_root = plugin.web_static_dir();
            if static_root.is_dir() {
                plugin_static_roots.push((plugin_path_id, static_root));
            }
        }
    }

    let templates =
        TemplateService::new_with_plugin_roots(&templates_dir, plugin_template_roots)
            .with_context(|| format!("failed to init template root {}", templates_dir.display()))?;

    let ctx = SushiContext::new(config, storage, jwt, templates);
    tracing_bridge::register_log_service(ctx.logs.clone());

    for (plugin_name, static_root) in plugin_static_roots {
        ctx.plugins
            .register_plugin_static_root(&plugin_name, static_root)
            .await;
    }

    // Load plugins
    for plugin in lua_plugins {
        let plugin_name = plugin.name().to_string();
        ctx.plugins
            .register_plugin_manifest_with_permissions(
                plugin.manifest(),
                plugin.effective_permissions(),
            )
            .await;

        if let Err(e) = plugin.init(&ctx).await {
            tracing::warn!("failed to init plugin {plugin_name}: {e}");
            ctx.logs
                .warn(&format!("failed to init plugin {plugin_name}: {e}"))
                .await;
            ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
            continue;
        }
        if let Some(lua) = plugin.into_vm() {
            ctx.plugins.register_vm(&plugin_name, lua).await;
            tracing::debug!("registered VM for plugin {plugin_name}");
        }
    }

    hydrate_authorizer_snapshot(&ctx).await?;

    Ok(ctx)
}

fn resolve_templates_dir(config_path: Option<&Path>, templates_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, templates_dir, "template")
}

fn resolve_static_dir(config_path: Option<&Path>, static_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, static_dir, "static")
}

fn resolve_file_browser_root_dir(config_path: Option<&Path>, root_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, root_dir, "file-browser root")
}

fn resolve_dir(config_path: Option<&Path>, dir: &str, kind: &str) -> Result<PathBuf> {
    let base_dir = match config_path.and_then(|path| path.parent()) {
        Some(parent) => parent.to_path_buf(),
        None => env::current_dir().with_context(|| {
            format!("failed to determine current working directory for {kind} resolution")
        })?,
    };

    let candidate = PathBuf::from(dir);
    if candidate.is_absolute() {
        Ok(candidate)
    } else {
        Ok(base_dir.join(candidate))
    }
}

async fn hydrate_authorizer_snapshot(ctx: &SushiContext) -> Result<()> {
    let storage: Arc<dyn Storage> = ctx.db.clone();
    let repository = PolicyRepository::new(storage);
    let snapshot = repository
        .compile_snapshot()
        .await
        .map_err(anyhow::Error::msg)
        .context("failed to compile policy snapshot from database")?;
    ctx.authorizer.replace_snapshot(snapshot).await;
    Ok(())
}

async fn migration_applied(storage: &SqliteStorage, migration_name: &str) -> Result<bool> {
    let migration_name = migration_name.replace('\'', "''");
    let query = format!(
        "SELECT 1 AS found FROM _sushi_migrations WHERE name = '{migration_name}' LIMIT 1"
    );

    let rows = storage
        .query(&query, vec![])
        .await
        .context("failed to query migration history")?;
    Ok(!rows.is_empty())
}

async fn run_plugin_governance_migration_if_needed(storage: &SqliteStorage) -> Result<()> {
    if migration_applied(storage, PLUGIN_GOVERNANCE_MIGRATION_NAME)
        .await
        .context("failed to check plugin governance migration state")?
    {
        return Ok(());
    }

    match storage.run_migrations(PLUGIN_GOVERNANCE_MIGRATION_SQL).await {
        Ok(()) => Ok(()),
        Err(err) => {
            let err_message = err.to_string();
            if is_duplicate_column_error(&err_message) {
                recover_plugin_governance_migration(storage).await
            } else {
                Err(err).context("failed to run plugin governance migrations")
            }
        }
    }
}

fn is_duplicate_column_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("duplicate column name") || lower.contains("already exists")
}

async fn recover_plugin_governance_migration(storage: &SqliteStorage) -> Result<()> {
    let existing_columns = plugin_state_columns(storage).await?;
    let required_columns = [
        (
            "plugin_id",
            "ALTER TABLE plugin_state ADD COLUMN plugin_id TEXT NOT NULL DEFAULT ''",
        ),
        (
            "source_kind",
            "ALTER TABLE plugin_state ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'third_party'",
        ),
        (
            "updated_by",
            "ALTER TABLE plugin_state ADD COLUMN updated_by TEXT NOT NULL DEFAULT ''",
        ),
        (
            "updated_at",
            "ALTER TABLE plugin_state ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now'))",
        ),
        (
            "reason",
            "ALTER TABLE plugin_state ADD COLUMN reason TEXT NOT NULL DEFAULT ''",
        ),
    ];

    for (column, alter_sql) in required_columns {
        if !existing_columns.iter().any(|existing| existing == column) {
            match storage.execute(alter_sql, vec![]).await {
                Ok(()) => {}
                Err(err) if is_duplicate_column_error(&err.to_string()) => {}
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to add missing plugin_state column `{column}`")
                    });
                }
            }
        }
    }

    storage
        .run_migrations(PLUGIN_GOVERNANCE_MIGRATION_FINALIZE_SQL)
        .await
        .context("failed to finalize plugin governance migration")?;

    if !migration_applied(storage, PLUGIN_GOVERNANCE_MIGRATION_NAME)
        .await
        .context("failed to verify plugin governance migration marker after recovery")?
    {
        anyhow::bail!("plugin governance migration recovery completed without migration marker");
    }

    Ok(())
}

async fn plugin_state_columns(storage: &SqliteStorage) -> Result<Vec<String>> {
    let rows = storage
        .query("PRAGMA table_info(plugin_state)", vec![])
        .await
        .context("failed to read plugin_state schema")?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.get("name")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sushi_core::storage::Storage;

    async fn run_base_migrations_to_007(storage: &SqliteStorage) {
        storage.run_migrations(MIGRATION_SQL).await.unwrap();
        storage.run_migrations(KV_MIGRATION_SQL).await.unwrap();
        storage.run_migrations(RBAC_MIGRATION_SQL).await.unwrap();
        storage.run_migrations(MENU_MIGRATION_SQL).await.unwrap();
        storage.run_migrations(MENUS_RBAC_MIGRATION_SQL).await.unwrap();
        storage
            .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
            .await
            .unwrap();
        storage.run_migrations(CMS_MIGRATION_SQL).await.unwrap();
    }

    #[tokio::test]
    async fn plugin_governance_migration_is_skipped_when_already_applied() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        run_base_migrations_to_007(&storage).await;
        storage
            .execute(
                "INSERT OR IGNORE INTO _sushi_migrations (id, name) VALUES (8, '008_plugin_governance_v1')",
                vec![],
            )
            .await
            .unwrap();

        run_plugin_governance_migration_if_needed(&storage)
            .await
            .unwrap();

        let columns = storage.query("PRAGMA table_info(plugin_state)", vec![]).await.unwrap();
        let has_plugin_id = columns.iter().any(|column| {
            column.get("name").and_then(|value| value.as_str()) == Some("plugin_id")
        });

        assert!(
            !has_plugin_id,
            "migration should be skipped when already recorded in _sushi_migrations"
        );
    }

    #[tokio::test]
    async fn plugin_governance_migration_applies_when_marker_missing() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        run_base_migrations_to_007(&storage).await;

        run_plugin_governance_migration_if_needed(&storage)
            .await
            .unwrap();

        assert!(
            migration_applied(&storage, PLUGIN_GOVERNANCE_MIGRATION_NAME)
                .await
                .unwrap()
        );

        let columns = plugin_state_columns(&storage).await.unwrap();
        for required in ["plugin_id", "source_kind", "updated_by", "updated_at", "reason"] {
            assert!(
                columns.iter().any(|column| column == required),
                "expected column `{required}` to exist after applying migration"
            );
        }
    }

    #[tokio::test]
    async fn plugin_governance_migration_recovers_from_partial_apply() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        run_base_migrations_to_007(&storage).await;
        storage
            .execute(
                "ALTER TABLE plugin_state ADD COLUMN plugin_id TEXT NOT NULL DEFAULT ''",
                vec![],
            )
            .await
            .unwrap();

        run_plugin_governance_migration_if_needed(&storage)
            .await
            .unwrap();

        assert!(
            migration_applied(&storage, PLUGIN_GOVERNANCE_MIGRATION_NAME)
                .await
                .unwrap()
        );
        let columns = plugin_state_columns(&storage).await.unwrap();
        for required in ["plugin_id", "source_kind", "updated_by", "updated_at", "reason"] {
            assert!(
                columns.iter().any(|column| column == required),
                "expected column `{required}` to exist after recovery"
            );
        }
    }

    #[test]
    fn duplicate_column_error_detection_is_case_insensitive() {
        assert!(is_duplicate_column_error("duplicate column name: plugin_id"));
        assert!(is_duplicate_column_error("DUPLICATE COLUMN NAME: plugin_id"));
        assert!(!is_duplicate_column_error("no such table: plugin_state"));
    }
}
