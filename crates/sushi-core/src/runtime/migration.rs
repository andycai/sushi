use super::{PluginId, ResolvedRuntimeEntry, RuntimePluginSource};
use crate::plugin::DatabasePermission;
use crate::storage::sqlite::SqliteStorage;
use crate::storage::Storage;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

const CATALOG_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS plugin_migrations (
    plugin_id TEXT NOT NULL,
    migration_id TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_id, migration_id)
);
"#;

const MIGRATION_001_INIT: &str = include_str!("../../../../migrations/001_init.sql");
const MIGRATION_003_RBAC: &str = include_str!("../../../../migrations/003_rbac.sql");
const MIGRATION_004_MENU: &str = include_str!("../../../../migrations/004_menu.sql");
const MIGRATION_005_MENUS_RBAC: &str = include_str!("../../../../migrations/005_menus_rbac.sql");
const MIGRATION_006_UNIFIED_POLICY: &str =
    include_str!("../../../../migrations/006_unified_policy_v2.sql");
const MIGRATION_008_PLUGIN_GOVERNANCE: &str =
    include_str!("../../../../migrations/008_plugin_governance_v1.sql");
const MIGRATION_009_MENU_CONTRIBUTIONS: &str = r#"
CREATE TABLE IF NOT EXISTS runtime_menu_items (
    contribution_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    menu_item_id INTEGER NOT NULL UNIQUE,
    FOREIGN KEY (menu_item_id) REFERENCES menu_items(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_runtime_menu_items_owner_id
    ON runtime_menu_items(owner_id);
"#;
const MIGRATION_008_FINALIZE: &str = r#"
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMigration {
    plugin_id: PluginId,
    migration_id: String,
    checksum: String,
    sql: String,
    legacy_name: Option<String>,
    legacy_table: Option<String>,
    legacy_recovery: Option<LegacyRecovery>,
    order: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyRecovery {
    PluginGovernanceV1,
}

impl PluginMigration {
    pub fn new(
        plugin_id: impl Into<String>,
        migration_id: impl Into<String>,
        sql: impl Into<String>,
    ) -> Result<Self, MigrationError> {
        let plugin_id =
            PluginId::new(plugin_id).map_err(|reason| MigrationError::InvalidDescriptor {
                field: "plugin_id",
                reason,
            })?;
        let migration_id = validate_identifier("migration_id", migration_id.into())?;
        let order = migration_order(&migration_id)?;
        let sql = sql.into();
        if sql.trim().is_empty() {
            return Err(MigrationError::InvalidDescriptor {
                field: "sql",
                reason: "migration SQL must not be empty".to_string(),
            });
        }
        let checksum = checksum(&sql);
        Ok(Self {
            plugin_id,
            migration_id,
            checksum,
            sql,
            legacy_name: None,
            legacy_table: None,
            legacy_recovery: None,
            order,
        })
    }

    pub fn with_legacy_name(
        mut self,
        legacy_name: impl Into<String>,
    ) -> Result<Self, MigrationError> {
        self.legacy_name = Some(validate_identifier("legacy_name", legacy_name.into())?);
        Ok(self)
    }

    pub fn with_legacy_table(
        mut self,
        legacy_table: impl Into<String>,
    ) -> Result<Self, MigrationError> {
        self.legacy_table = Some(validate_identifier("legacy_table", legacy_table.into())?);
        Ok(self)
    }

    pub fn plugin_id(&self) -> &str {
        self.plugin_id.as_str()
    }

    pub fn migration_id(&self) -> &str {
        &self.migration_id
    }

    pub fn checksum(&self) -> &str {
        &self.checksum
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn legacy_name(&self) -> Option<&str> {
        self.legacy_name.as_deref()
    }

    fn with_legacy_recovery(mut self, recovery: LegacyRecovery) -> Self {
        self.legacy_recovery = Some(recovery);
        self
    }
}

pub fn historical_host_core_migrations() -> Result<Vec<PluginMigration>, MigrationError> {
    Ok(vec![
        historical_migration(
            "builtin/host-core",
            "001_init",
            MIGRATION_001_INIT,
            Some("001_init"),
        )?,
        historical_migration(
            "builtin/host-core",
            "008_plugin_governance_v1",
            MIGRATION_008_PLUGIN_GOVERNANCE,
            Some("008_plugin_governance_v1"),
        )?
        .with_legacy_recovery(LegacyRecovery::PluginGovernanceV1),
    ])
}

pub fn historical_policy_migrations() -> Result<Vec<PluginMigration>, MigrationError> {
    Ok(vec![
        historical_migration(
            "builtin/policy",
            "003_rbac",
            MIGRATION_003_RBAC,
            Some("003_rbac"),
        )?,
        historical_migration(
            "builtin/policy",
            "006_unified_policy_v2",
            MIGRATION_006_UNIFIED_POLICY,
            Some("006_unified_policy_v2"),
        )?,
    ])
}

pub fn historical_menu_admin_migrations() -> Result<Vec<PluginMigration>, MigrationError> {
    Ok(vec![
        historical_migration("builtin/menu-admin", "004_menu", MIGRATION_004_MENU, None)?
            .with_legacy_table("menu_items")?,
        historical_migration(
            "builtin/menu-admin",
            "005_menus_rbac",
            MIGRATION_005_MENUS_RBAC,
            Some("005_menus_rbac"),
        )?,
        historical_migration(
            "builtin/menu-admin",
            "009_menu_contributions",
            MIGRATION_009_MENU_CONTRIBUTIONS,
            None,
        )?,
    ])
}

pub fn load_lua_migrations(
    entry: &ResolvedRuntimeEntry,
    database_permission: &DatabasePermission,
) -> Result<Vec<PluginMigration>, MigrationError> {
    let RuntimePluginSource::Lua { path_id, path, .. } = &entry.source else {
        return Ok(Vec::new());
    };
    let migrations_dir = path.join("migrations");
    if !migrations_dir.is_dir() {
        return Ok(Vec::new());
    }
    let paths = std::fs::read_dir(&migrations_dir)
        .map_err(|error| MigrationError::ReadDirectory {
            path: migrations_dir.clone(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| MigrationError::ReadDirectory {
            path: migrations_dir.clone(),
            message: error.to_string(),
        })?;
    let mut paths = paths
        .into_iter()
        .filter(|path| {
            path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("sql")
        })
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    if !path_id.starts_with("official/") {
        return Err(MigrationError::UntrustedSource {
            plugin_id: path_id.clone(),
        });
    }
    if !matches!(
        database_permission,
        DatabasePermission::Write | DatabasePermission::Admin
    ) || !database_grant_allows_write(&entry.grants)
    {
        return Err(MigrationError::PermissionDenied {
            plugin_id: path_id.clone(),
        });
    }

    paths
        .into_iter()
        .map(|path| load_lua_migration(path_id, &path))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    Applied,
    AlreadyApplied,
    Bridged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReportEntry {
    pub plugin_id: String,
    pub migration_id: String,
    pub status: MigrationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationReport {
    pub entries: Vec<MigrationReportEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationVerificationStatus {
    Applied,
    Pending,
    LegacyBridge,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationVerificationEntry {
    pub plugin_id: String,
    pub migration_id: String,
    pub status: MigrationVerificationStatus,
}

pub struct MigrationRunner<'a> {
    storage: &'a SqliteStorage,
}

impl<'a> MigrationRunner<'a> {
    pub fn new(storage: &'a SqliteStorage) -> Self {
        Self { storage }
    }

    pub async fn apply(
        &self,
        migrations: &[PluginMigration],
    ) -> Result<MigrationReport, MigrationError> {
        self.ensure_catalog().await?;
        let mut migrations = migrations.to_vec();
        migrations.sort_by(|left, right| {
            (&left.order, &left.plugin_id, &left.migration_id).cmp(&(
                &right.order,
                &right.plugin_id,
                &right.migration_id,
            ))
        });
        validate_unique_migrations(&migrations)?;

        let mut report = MigrationReport::default();
        for migration in migrations {
            let status = self.apply_one(&migration).await?;
            report.entries.push(MigrationReportEntry {
                plugin_id: migration.plugin_id.to_string(),
                migration_id: migration.migration_id,
                status,
            });
        }
        Ok(report)
    }

    pub async fn verify(
        &self,
        migrations: &[PluginMigration],
    ) -> Result<Vec<MigrationVerificationEntry>, MigrationError> {
        let mut migrations = migrations.to_vec();
        migrations.sort_by(|left, right| {
            (&left.order, &left.plugin_id, &left.migration_id).cmp(&(
                &right.order,
                &right.plugin_id,
                &right.migration_id,
            ))
        });
        validate_unique_migrations(&migrations)?;
        let catalog_exists = self.table_exists("plugin_migrations").await?;
        let mut entries = Vec::with_capacity(migrations.len());
        for migration in migrations {
            let status = if catalog_exists {
                match self.applied_checksum(&migration).await? {
                    Some(applied_checksum) if applied_checksum == migration.checksum => {
                        MigrationVerificationStatus::Applied
                    }
                    Some(applied_checksum) => {
                        return Err(MigrationError::ChecksumMismatch {
                            plugin_id: migration.plugin_id.to_string(),
                            migration_id: migration.migration_id.clone(),
                            expected: applied_checksum,
                            actual: migration.checksum.clone(),
                        })
                    }
                    None => self.pending_verification_status(&migration).await?,
                }
            } else {
                self.pending_verification_status(&migration).await?
            };
            entries.push(MigrationVerificationEntry {
                plugin_id: migration.plugin_id.to_string(),
                migration_id: migration.migration_id,
                status,
            });
        }
        Ok(entries)
    }

    async fn pending_verification_status(
        &self,
        migration: &PluginMigration,
    ) -> Result<MigrationVerificationStatus, MigrationError> {
        if let Some(legacy_name) = migration.legacy_name() {
            if self.legacy_migration_applied(legacy_name).await? {
                return Ok(MigrationVerificationStatus::LegacyBridge);
            }
        }
        if let Some(legacy_table) = &migration.legacy_table {
            if self.table_exists(legacy_table).await? {
                return Ok(MigrationVerificationStatus::LegacyBridge);
            }
        }
        if migration.legacy_recovery == Some(LegacyRecovery::PluginGovernanceV1)
            && self.plugin_governance_partially_applied().await?
        {
            return Ok(MigrationVerificationStatus::RecoveryRequired);
        }
        Ok(MigrationVerificationStatus::Pending)
    }

    async fn ensure_catalog(&self) -> Result<(), MigrationError> {
        self.storage
            .run_migrations(CATALOG_SQL)
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "ensure migration catalog",
                message: error.to_string(),
            })
    }

    async fn apply_one(
        &self,
        migration: &PluginMigration,
    ) -> Result<MigrationStatus, MigrationError> {
        if let Some(applied_checksum) = self.applied_checksum(migration).await? {
            if applied_checksum == migration.checksum {
                return Ok(MigrationStatus::AlreadyApplied);
            }
            return Err(MigrationError::ChecksumMismatch {
                plugin_id: migration.plugin_id.to_string(),
                migration_id: migration.migration_id.clone(),
                expected: applied_checksum,
                actual: migration.checksum.clone(),
            });
        }

        if let Some(legacy_name) = migration.legacy_name() {
            if self.legacy_migration_applied(legacy_name).await? {
                self.record_bridge(migration).await?;
                return Ok(MigrationStatus::Bridged);
            }
        }
        if let Some(legacy_table) = &migration.legacy_table {
            if self.table_exists(legacy_table).await? {
                self.record_bridge(migration).await?;
                return Ok(MigrationStatus::Bridged);
            }
        }
        if migration.legacy_recovery == Some(LegacyRecovery::PluginGovernanceV1)
            && self.plugin_governance_partially_applied().await?
        {
            let recovery_sql = self.plugin_governance_recovery_sql().await?;
            self.storage
                .apply_plugin_migration(
                    migration.plugin_id(),
                    migration.migration_id(),
                    migration.checksum(),
                    &recovery_sql,
                )
                .await
                .map_err(|error| MigrationError::Storage {
                    operation: "recover plugin governance migration",
                    message: error.to_string(),
                })?;
            return Ok(MigrationStatus::Bridged);
        }

        self.storage
            .apply_plugin_migration(
                migration.plugin_id(),
                migration.migration_id(),
                migration.checksum(),
                migration.sql(),
            )
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "apply plugin migration",
                message: error.to_string(),
            })?;
        Ok(MigrationStatus::Applied)
    }

    async fn record_bridge(&self, migration: &PluginMigration) -> Result<(), MigrationError> {
        self.storage
            .record_plugin_migration(
                migration.plugin_id(),
                migration.migration_id(),
                migration.checksum(),
            )
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "bridge legacy migration",
                message: error.to_string(),
            })
    }

    async fn applied_checksum(
        &self,
        migration: &PluginMigration,
    ) -> Result<Option<String>, MigrationError> {
        let rows = self
            .storage
            .query(
                "SELECT checksum FROM plugin_migrations WHERE plugin_id = ? AND migration_id = ?",
                vec![
                    Value::String(migration.plugin_id.to_string()),
                    Value::String(migration.migration_id.clone()),
                ],
            )
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "read migration catalog",
                message: error.to_string(),
            })?;
        Ok(rows.first().and_then(|row| {
            row.get("checksum")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }))
    }

    async fn legacy_migration_applied(&self, legacy_name: &str) -> Result<bool, MigrationError> {
        if !self.table_exists("_sushi_migrations").await? {
            return Ok(false);
        }
        let rows = self
            .storage
            .query(
                "SELECT 1 AS found FROM _sushi_migrations WHERE name = ? LIMIT 1",
                vec![Value::String(legacy_name.to_string())],
            )
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "read legacy migration history",
                message: error.to_string(),
            })?;
        Ok(!rows.is_empty())
    }

    async fn table_exists(&self, table: &str) -> Result<bool, MigrationError> {
        let rows = self
            .storage
            .query(
                "SELECT 1 AS found FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
                vec![Value::String(table.to_string())],
            )
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "inspect migration bridge table",
                message: error.to_string(),
            })?;
        Ok(!rows.is_empty())
    }

    async fn plugin_governance_partially_applied(&self) -> Result<bool, MigrationError> {
        let columns = self.plugin_state_columns().await?;
        Ok([
            "plugin_id",
            "source_kind",
            "updated_by",
            "updated_at",
            "reason",
        ]
        .iter()
        .any(|column| columns.contains(*column)))
    }

    async fn plugin_governance_recovery_sql(&self) -> Result<String, MigrationError> {
        let columns = self.plugin_state_columns().await?;
        let mut sql = String::new();
        for (column, alter_sql) in [
            (
                "plugin_id",
                "ALTER TABLE plugin_state ADD COLUMN plugin_id TEXT NOT NULL DEFAULT '';",
            ),
            (
                "source_kind",
                "ALTER TABLE plugin_state ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'third_party';",
            ),
            (
                "updated_by",
                "ALTER TABLE plugin_state ADD COLUMN updated_by TEXT NOT NULL DEFAULT '';",
            ),
            (
                "updated_at",
                "ALTER TABLE plugin_state ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now'));",
            ),
            (
                "reason",
                "ALTER TABLE plugin_state ADD COLUMN reason TEXT NOT NULL DEFAULT '';",
            ),
        ] {
            if !columns.contains(column) {
                sql.push_str(alter_sql);
                sql.push('\n');
            }
        }
        sql.push_str(MIGRATION_008_FINALIZE);
        Ok(sql)
    }

    async fn plugin_state_columns(&self) -> Result<BTreeSet<String>, MigrationError> {
        if !self.table_exists("plugin_state").await? {
            return Ok(BTreeSet::new());
        }
        let rows = self
            .storage
            .query("PRAGMA table_info(plugin_state)", vec![])
            .await
            .map_err(|error| MigrationError::Storage {
                operation: "inspect plugin governance schema",
                message: error.to_string(),
            })?;
        Ok(rows
            .iter()
            .filter_map(|row| row.get("name").and_then(Value::as_str))
            .map(ToString::to_string)
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MigrationError {
    #[error("invalid migration descriptor field {field}: {reason}")]
    InvalidDescriptor { field: &'static str, reason: String },
    #[error("duplicate migration {plugin_id}:{migration_id}")]
    DuplicateMigration {
        plugin_id: String,
        migration_id: String,
    },
    #[error(
        "migration checksum mismatch for {plugin_id}:{migration_id}: expected {expected}, got {actual}"
    )]
    ChecksumMismatch {
        plugin_id: String,
        migration_id: String,
        expected: String,
        actual: String,
    },
    #[error("untrusted plugin source cannot declare migrations: {plugin_id}")]
    UntrustedSource { plugin_id: String },
    #[error("plugin migration requires an explicit database write grant: {plugin_id}")]
    PermissionDenied { plugin_id: String },
    #[error("failed to read migration directory {path}: {message}")]
    ReadDirectory { path: PathBuf, message: String },
    #[error("failed to read migration file {path}: {message}")]
    ReadFile { path: PathBuf, message: String },
    #[error("failed to {operation}: {message}")]
    Storage {
        operation: &'static str,
        message: String,
    },
}

fn validate_identifier(field: &'static str, value: String) -> Result<String, MigrationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MigrationError::InvalidDescriptor {
            field,
            reason: "value must not be empty".to_string(),
        });
    }
    if trimmed != value || value.chars().any(char::is_control) {
        return Err(MigrationError::InvalidDescriptor {
            field,
            reason: "value must not contain surrounding whitespace or control characters"
                .to_string(),
        });
    }
    Ok(value)
}

fn migration_order(migration_id: &str) -> Result<u64, MigrationError> {
    let prefix = migration_id
        .split_once('_')
        .map(|(prefix, _)| prefix)
        .unwrap_or(migration_id);
    prefix
        .parse::<u64>()
        .map_err(|_| MigrationError::InvalidDescriptor {
            field: "migration_id",
            reason: "migration ID must begin with a numeric order followed by '_'".to_string(),
        })
}

fn validate_unique_migrations(migrations: &[PluginMigration]) -> Result<(), MigrationError> {
    let mut keys = BTreeSet::new();
    for migration in migrations {
        let key = (migration.plugin_id.clone(), migration.migration_id.clone());
        if !keys.insert(key) {
            return Err(MigrationError::DuplicateMigration {
                plugin_id: migration.plugin_id.to_string(),
                migration_id: migration.migration_id.clone(),
            });
        }
    }
    Ok(())
}

fn checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn historical_migration(
    plugin_id: &str,
    migration_id: &str,
    sql: &str,
    legacy_name: Option<&str>,
) -> Result<PluginMigration, MigrationError> {
    let migration = PluginMigration::new(plugin_id, migration_id, sql)?;
    match legacy_name {
        Some(name) => migration.with_legacy_name(name),
        None => Ok(migration),
    }
}

fn database_grant_allows_write(grants: &Value) -> bool {
    grants.get("approved").and_then(Value::as_bool) == Some(true)
        && matches!(
            grants.get("database").and_then(Value::as_str),
            Some("write" | "admin")
        )
}

fn load_lua_migration(plugin_id: &str, path: &Path) -> Result<PluginMigration, MigrationError> {
    let migration_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| MigrationError::InvalidDescriptor {
            field: "migration_id",
            reason: format!("migration file has invalid UTF-8 name: {}", path.display()),
        })?;
    let sql = std::fs::read_to_string(path).map_err(|error| MigrationError::ReadFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let migration = PluginMigration::new(plugin_id, migration_id, sql)?;
    if matches!(
        (plugin_id, migration_id),
        ("official/kv-store", "002_kv_store") | ("official/cms", "007_cms")
    ) {
        migration.with_legacy_name(migration_id)
    } else {
        Ok(migration)
    }
}
