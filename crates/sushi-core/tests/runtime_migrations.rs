use sushi_core::plugin::DatabasePermission;
use sushi_core::runtime::{
    historical_host_core_migrations, historical_menu_admin_migrations,
    historical_policy_migrations, load_lua_migrations, PluginInstanceId, ResolvedRuntimeEntry,
    RuntimePluginSource,
};
use sushi_core::runtime::{MigrationError, MigrationRunner, MigrationStatus, PluginMigration};
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::storage::Storage;
use tempfile::TempDir;

fn migration(sql: &str) -> PluginMigration {
    PluginMigration::new("official/notes", "001_create_notes", sql)
        .expect("migration descriptor is valid")
}

#[test]
fn migration_descriptor_rejects_invalid_plugin_identity() {
    let error = PluginMigration::new(" ", "001_invalid", "SELECT 1").unwrap_err();
    assert!(error.to_string().contains("plugin_id"));
}

fn all_historical_builtin_migrations(
    include_menu_admin: bool,
) -> Result<Vec<PluginMigration>, MigrationError> {
    let mut migrations = historical_host_core_migrations()?;
    migrations.extend(historical_policy_migrations()?);
    if include_menu_admin {
        migrations.extend(historical_menu_admin_migrations()?);
    }
    Ok(migrations)
}

#[tokio::test]
async fn migration_is_applied_once_and_recorded_with_checksum() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    let runner = MigrationRunner::new(&storage);
    let descriptor =
        migration("CREATE TABLE notes (id INTEGER PRIMARY KEY); INSERT INTO notes DEFAULT VALUES;");

    let first = runner.apply(&[descriptor.clone()]).await.unwrap();
    assert_eq!(first.entries[0].status, MigrationStatus::Applied);

    let second = runner.apply(&[descriptor.clone()]).await.unwrap();
    assert_eq!(second.entries[0].status, MigrationStatus::AlreadyApplied);

    let rows = storage
        .query("SELECT COUNT(*) AS count FROM notes", vec![])
        .await
        .unwrap();
    assert_eq!(rows[0]["count"].as_i64(), Some(1));
    let records = storage
        .query(
            "SELECT checksum FROM plugin_migrations WHERE plugin_id = ? AND migration_id = ?",
            vec!["official/notes".into(), "001_create_notes".into()],
        )
        .await
        .unwrap();
    assert_eq!(records[0]["checksum"].as_str(), Some(descriptor.checksum()));
}

#[tokio::test]
async fn checksum_mismatch_fails_closed_without_reapplying() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    let runner = MigrationRunner::new(&storage);
    runner
        .apply(&[migration("CREATE TABLE notes (id INTEGER PRIMARY KEY);")])
        .await
        .unwrap();

    let changed = migration("CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT);");
    let error = runner
        .apply(&[changed.clone()])
        .await
        .expect_err("changed historical migration must fail closed");

    assert!(matches!(
        error,
        MigrationError::ChecksumMismatch {
            plugin_id,
            migration_id,
            ..
        } if plugin_id == "official/notes" && migration_id == "001_create_notes"
    ));
}

#[tokio::test]
async fn failed_migration_rolls_back_sql_and_catalog_record() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    let runner = MigrationRunner::new(&storage);
    let descriptor = migration(
        "CREATE TABLE transient_notes (id INTEGER PRIMARY KEY); INSERT INTO missing_table DEFAULT VALUES;",
    );

    runner
        .apply(&[descriptor])
        .await
        .expect_err("invalid SQL must roll back the migration transaction");

    let tables = storage
        .query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'transient_notes'",
            vec![],
        )
        .await
        .unwrap();
    assert!(tables.is_empty());
    let records = storage
        .query("SELECT * FROM plugin_migrations", vec![])
        .await
        .unwrap();
    assert!(records.is_empty());
}

#[tokio::test]
async fn legacy_history_is_bridged_without_executing_sql_again() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    storage
        .run_migrations(
            "CREATE TABLE _sushi_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);\
             INSERT INTO _sushi_migrations (id, name) VALUES (9, '009_legacy_notes');",
        )
        .await
        .unwrap();
    let runner = MigrationRunner::new(&storage);
    let descriptor = migration("THIS SQL MUST NOT EXECUTE")
        .with_legacy_name("009_legacy_notes")
        .expect("legacy migration name is valid");

    let report = runner.apply(&[descriptor]).await.unwrap();
    assert_eq!(report.entries[0].status, MigrationStatus::Bridged);

    let records = storage
        .query(
            "SELECT COUNT(*) AS count FROM plugin_migrations WHERE plugin_id = 'official/notes'",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(records[0]["count"].as_i64(), Some(1));
}

#[tokio::test]
async fn new_lua_migration_does_not_bridge_same_named_unrelated_legacy_record() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    storage
        .run_migrations(
            "CREATE TABLE _sushi_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);\
             INSERT INTO _sushi_migrations (id, name) VALUES (10, '010_create_notes');",
        )
        .await
        .unwrap();
    let descriptor = PluginMigration::new(
        "official/notes",
        "010_create_notes",
        "CREATE TABLE notes (id INTEGER PRIMARY KEY);",
    )
    .unwrap();

    let report = MigrationRunner::new(&storage)
        .apply(&[descriptor])
        .await
        .unwrap();
    assert_eq!(report.entries[0].status, MigrationStatus::Applied);
    let tables = storage
        .query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(tables.len(), 1);
}

#[tokio::test]
async fn complete_historical_database_bridges_all_catalog_entries() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    for sql in [
        include_str!("../../../migrations/001_init.sql"),
        include_str!("../../../migrations/002_kv_store.sql"),
        include_str!("../../../migrations/003_rbac.sql"),
        include_str!("../../../migrations/004_menu.sql"),
        include_str!("../../../migrations/005_menus_rbac.sql"),
        include_str!("../../../migrations/006_unified_policy_v2.sql"),
        include_str!("../../../migrations/007_cms.sql"),
        include_str!("../../../migrations/008_plugin_governance_v1.sql"),
    ] {
        storage.run_migrations(sql).await.unwrap();
    }

    let mut migrations = all_historical_builtin_migrations(true).unwrap();
    migrations.push(
        PluginMigration::new(
            "official/kv-store",
            "002_kv_store",
            include_str!("../../../plugins/official/kv-store/migrations/002_kv_store.sql"),
        )
        .unwrap()
        .with_legacy_name("002_kv_store")
        .unwrap(),
    );
    migrations.push(
        PluginMigration::new(
            "official/cms",
            "007_cms",
            include_str!("../../../plugins/official/cms/migrations/007_cms.sql"),
        )
        .unwrap()
        .with_legacy_name("007_cms")
        .unwrap(),
    );

    let report = MigrationRunner::new(&storage)
        .apply(&migrations)
        .await
        .unwrap();
    assert_eq!(report.entries.len(), 9);
    assert!(report.entries[..8]
        .iter()
        .all(|entry| entry.status == MigrationStatus::Bridged));
    assert_eq!(report.entries[8].status, MigrationStatus::Applied);
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| entry.migration_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "001_init",
            "002_kv_store",
            "003_rbac",
            "004_menu",
            "005_menus_rbac",
            "006_unified_policy_v2",
            "007_cms",
            "008_plugin_governance_v1",
            "009_menu_contributions",
        ]
    );

    let records = storage
        .query("SELECT COUNT(*) AS count FROM plugin_migrations", vec![])
        .await
        .unwrap();
    assert_eq!(records[0]["count"].as_i64(), Some(9));
}

#[tokio::test]
async fn historical_menu_table_without_legacy_marker_is_bridged() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    storage
        .run_migrations(
            "CREATE TABLE _sushi_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);\
             CREATE TABLE menu_items (id INTEGER PRIMARY KEY, label TEXT NOT NULL);\
             INSERT INTO menu_items (id, label) VALUES (1, 'Existing Menu');",
        )
        .await
        .unwrap();
    let migration = all_historical_builtin_migrations(true)
        .unwrap()
        .into_iter()
        .find(|migration| migration.migration_id() == "004_menu")
        .unwrap();

    let report = MigrationRunner::new(&storage)
        .apply(&[migration])
        .await
        .unwrap();
    assert_eq!(report.entries[0].status, MigrationStatus::Bridged);

    let rows = storage
        .query("SELECT COUNT(*) AS count FROM menu_items", vec![])
        .await
        .unwrap();
    assert_eq!(rows[0]["count"].as_i64(), Some(1));
}

#[tokio::test]
async fn partially_applied_governance_migration_is_recovered_atomically() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    let base = all_historical_builtin_migrations(false).unwrap();
    MigrationRunner::new(&storage)
        .apply(
            &base
                .iter()
                .filter(|migration| migration.migration_id() != "008_plugin_governance_v1")
                .cloned()
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();
    storage
        .execute(
            "ALTER TABLE plugin_state ADD COLUMN plugin_id TEXT NOT NULL DEFAULT ''",
            vec![],
        )
        .await
        .unwrap();
    let migration = base
        .into_iter()
        .find(|migration| migration.migration_id() == "008_plugin_governance_v1")
        .unwrap();

    let report = MigrationRunner::new(&storage)
        .apply(&[migration])
        .await
        .unwrap();
    assert_eq!(report.entries[0].status, MigrationStatus::Bridged);

    let columns = storage
        .query("PRAGMA table_info(plugin_state)", vec![])
        .await
        .unwrap();
    for required in [
        "plugin_id",
        "source_kind",
        "updated_by",
        "updated_at",
        "reason",
    ] {
        assert!(columns.iter().any(|column| {
            column.get("name").and_then(|value| value.as_str()) == Some(required)
        }));
    }
}

#[test]
fn lua_migrations_require_official_source_and_explicit_write_grant() {
    let temp = TempDir::new().unwrap();
    let official = temp.path().join("official/notes");
    std::fs::create_dir_all(official.join("migrations")).unwrap();
    std::fs::write(
        official.join("migrations/010_create_notes.sql"),
        "CREATE TABLE notes (id INTEGER PRIMARY KEY);",
    )
    .unwrap();
    let entry = ResolvedRuntimeEntry {
        id: PluginInstanceId::new("notes.default").unwrap(),
        source: RuntimePluginSource::Lua {
            path_id: "official/notes".to_string(),
            path: official.clone(),
            reference: "lua:official/notes".to_string(),
        },
        enabled: true,
        required: false,
        config: serde_json::json!({}),
        grants: serde_json::json!({}),
        origin: "test".to_string(),
    };

    let error = load_lua_migrations(&entry, &DatabasePermission::Admin)
        .expect_err("missing profile database grant must fail closed");
    assert_eq!(
        error,
        MigrationError::PermissionDenied {
            plugin_id: "official/notes".to_string()
        }
    );

    let mut granted = entry.clone();
    granted.grants = serde_json::json!({ "approved": true, "database": "write" });
    let migrations = load_lua_migrations(&granted, &DatabasePermission::Admin).unwrap();
    assert_eq!(migrations.len(), 1);
    assert_eq!(migrations[0].migration_id(), "010_create_notes");

    let mut third_party = granted;
    third_party.source = RuntimePluginSource::Lua {
        path_id: "third_party/notes".to_string(),
        path: official,
        reference: "lua:third_party/notes".to_string(),
    };
    let error = load_lua_migrations(&third_party, &DatabasePermission::Admin)
        .expect_err("third-party source cannot declare migrations");
    assert_eq!(
        error,
        MigrationError::UntrustedSource {
            plugin_id: "third_party/notes".to_string()
        }
    );
}
