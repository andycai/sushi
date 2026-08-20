use crate::runtime::PluginId;
use crate::storage::sqlite::SqliteStorage;
use crate::storage::{Row, Storage};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct StoredPluginState {
    pub plugin_id: PluginId,
    pub name: String,
    pub source_kind: String,
    pub enabled: bool,
    pub loaded: bool,
    pub version: String,
    pub updated_by: Option<String>,
    pub updated_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct StoredPluginStateEvent {
    pub plugin_id: PluginId,
    pub source_kind: String,
    pub changed_by: String,
    pub previous_enabled: Option<bool>,
    pub next_enabled: Option<bool>,
    pub reason: String,
}

pub struct PluginStateRepository {
    storage: Arc<dyn Storage>,
    sqlite: Option<Arc<SqliteStorage>>,
}

impl PluginStateRepository {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            sqlite: None,
        }
    }

    pub fn new_sqlite(storage: Arc<SqliteStorage>) -> Self {
        let storage_trait: Arc<dyn Storage> = storage.clone();
        Self {
            storage: storage_trait,
            sqlite: Some(storage),
        }
    }

    pub async fn upsert_discovered_plugin(
        &self,
        plugin_id: &str,
        name: &str,
        source_kind: &str,
        version: &str,
    ) -> Result<StoredPluginState, String> {
        self.upsert_profile_plugin(plugin_id, name, source_kind, version, true, false)
            .await
    }

    pub async fn upsert_profile_plugin(
        &self,
        plugin_id: &str,
        name: &str,
        source_kind: &str,
        version: &str,
        default_enabled: bool,
        required: bool,
    ) -> Result<StoredPluginState, String> {
        self.storage
            .execute(
                r#"
                INSERT INTO plugin_state (plugin_id, name, source_kind, enabled, loaded, version, updated_at)
                VALUES (?1, ?2, ?3, ?5, 0, ?4, datetime('now'))
                ON CONFLICT(name) DO UPDATE SET
                    plugin_id = excluded.plugin_id,
                    source_kind = excluded.source_kind,
                    version = excluded.version,
                    enabled = CASE
                        WHEN ?6 = 1 THEN 1
                        WHEN ?5 = 0 THEN 0
                        ELSE plugin_state.enabled
                    END,
                    updated_at = datetime('now')
                "#,
                vec![
                    Value::String(plugin_id.to_string()),
                    Value::String(name.to_string()),
                    Value::String(source_kind.to_string()),
                    Value::String(version.to_string()),
                    Value::Bool(default_enabled),
                    Value::Bool(required),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.get_by_name(name)
            .await?
            .ok_or_else(|| format!("plugin state row missing after upsert: {name}"))
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<StoredPluginState>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT plugin_id, name, source_kind, enabled, loaded, version, updated_by, updated_at, reason
                FROM plugin_state
                WHERE name = ?1
                "#,
                vec![Value::String(name.to_string())],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().next().map(row_to_state).transpose()
    }

    pub async fn set_loaded(&self, name: &str, loaded: bool) -> Result<(), String> {
        if self.get_by_name(name).await?.is_none() {
            return Err(format!("plugin not found: {name}"));
        }

        self.storage
            .execute(
                r#"
                UPDATE plugin_state
                SET loaded = ?2,
                    loaded_at = CASE WHEN ?2 = 1 THEN datetime('now') ELSE loaded_at END,
                    updated_at = datetime('now')
                WHERE name = ?1
                "#,
                vec![Value::String(name.to_string()), Value::Bool(loaded)],
            )
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn set_enabled(
        &self,
        name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<StoredPluginState, String> {
        let before = self
            .get_by_name(name)
            .await?
            .ok_or_else(|| format!("plugin not found: {name}"))?;

        let actor_value = actor.unwrap_or("").trim().to_string();
        let reason_value = reason.unwrap_or("").trim().to_string();

        if let Some(sqlite) = &self.sqlite {
            let name = name.to_string();
            let transaction_name = name.clone();
            let next_enabled = enabled;
            let previous_enabled = before.enabled;
            sqlite
                .transaction(move |connection| {
                    connection.execute(
                        r#"
                        UPDATE plugin_state
                        SET enabled = ?2,
                            updated_by = ?3,
                            reason = ?4,
                            updated_at = datetime('now')
                        WHERE name = ?1
                        "#,
                        vec![
                            Value::String(transaction_name.clone()),
                            Value::Bool(next_enabled),
                            Value::String(actor_value.clone()),
                            Value::String(reason_value.clone()),
                        ],
                    )?;
                    connection.execute(
                        r#"
                        INSERT INTO plugin_state_events (
                            plugin_id,
                            source_kind,
                            changed_by,
                            previous_enabled,
                            next_enabled,
                            reason
                        )
                        SELECT plugin_id, source_kind, ?2, ?3, ?4, ?5
                        FROM plugin_state
                        WHERE name = ?1
                        "#,
                        vec![
                            Value::String(transaction_name),
                            Value::String(actor_value),
                            Value::Bool(previous_enabled),
                            Value::Bool(next_enabled),
                            Value::String(reason_value),
                        ],
                    )
                })
                .await
                .map_err(|error| {
                    format!("failed to update plugin state and event for {name}: {error}")
                })?;

            return self
                .get_by_name(&name)
                .await?
                .ok_or_else(|| format!("plugin state row missing after update: {name}"));
        }

        self.storage
            .execute(
                r#"
                UPDATE plugin_state
                SET enabled = ?2,
                    updated_by = ?3,
                    reason = ?4,
                    updated_at = datetime('now')
                WHERE name = ?1
                "#,
                vec![
                    Value::String(name.to_string()),
                    Value::Bool(enabled),
                    Value::String(actor_value.clone()),
                    Value::String(reason_value.clone()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        let after = self
            .get_by_name(name)
            .await?
            .ok_or_else(|| format!("plugin state row missing after update: {name}"))?;

        self.storage
            .execute(
                r#"
                INSERT INTO plugin_state_events (
                    plugin_id,
                    source_kind,
                    changed_by,
                    previous_enabled,
                    next_enabled,
                    reason
                )
                SELECT plugin_id, source_kind, ?2, ?3, ?4, ?5
                FROM plugin_state
                WHERE name = ?1
                "#,
                vec![
                    Value::String(name.to_string()),
                    Value::String(actor_value),
                    Value::Bool(before.enabled),
                    Value::Bool(after.enabled),
                    Value::String(reason_value),
                ],
            )
            .await
            .map_err(|err| format!("failed to insert plugin_state_events row for {name}: {err}"))?;

        Ok(after)
    }

    pub async fn get_latest_event_by_plugin_id(
        &self,
        plugin_id: &str,
    ) -> Result<Option<StoredPluginStateEvent>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT plugin_id, source_kind, changed_by, previous_enabled, next_enabled, reason
                FROM plugin_state_events
                WHERE plugin_id = ?1
                ORDER BY changed_at DESC, id DESC
                LIMIT 1
                "#,
                vec![Value::String(plugin_id.to_string())],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().next().map(row_to_event).transpose()
    }
}

fn row_to_state(row: Row) -> Result<StoredPluginState, String> {
    Ok(StoredPluginState {
        plugin_id: required_plugin_id(&row, "plugin_state", "plugin_id")?,
        name: required_string(&row, "plugin_state", "name")?,
        source_kind: string_or_default(&row, "source_kind", "third_party"),
        enabled: bool_or_default(&row, "enabled", true),
        loaded: bool_or_default(&row, "loaded", false),
        version: string_or_default(&row, "version", ""),
        updated_by: optional_non_empty_string(&row, "updated_by"),
        updated_at: string_or_default(&row, "updated_at", ""),
        reason: string_or_default(&row, "reason", ""),
    })
}

fn row_to_event(row: Row) -> Result<StoredPluginStateEvent, String> {
    Ok(StoredPluginStateEvent {
        plugin_id: required_plugin_id(&row, "plugin_state_events", "plugin_id")?,
        source_kind: string_or_default(&row, "source_kind", "third_party"),
        changed_by: string_or_default(&row, "changed_by", ""),
        previous_enabled: optional_bool(&row, "previous_enabled"),
        next_enabled: optional_bool(&row, "next_enabled"),
        reason: string_or_default(&row, "reason", ""),
    })
}

fn required_string(row: &Row, scope: &str, key: &str) -> Result<String, String> {
    row.get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing or invalid {scope}.{key}"))
}

fn required_plugin_id(row: &Row, scope: &str, key: &str) -> Result<PluginId, String> {
    let value = required_string(row, scope, key)?;
    PluginId::new(value).map_err(|reason| format!("invalid {scope}.{key}: {reason}"))
}

fn string_or_default(row: &Row, key: &str, default: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn optional_non_empty_string(row: &Row, key: &str) -> Option<String> {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn bool_or_default(row: &Row, key: &str, default: bool) -> bool {
    row.get(key)
        .and_then(|value| {
            if let Some(v) = value.as_bool() {
                return Some(v);
            }
            value.as_i64().map(|v| v != 0)
        })
        .unwrap_or(default)
}

fn optional_bool(row: &Row, key: &str) -> Option<bool> {
    row.get(key).and_then(|value| {
        if value.is_null() {
            return None;
        }
        if let Some(v) = value.as_bool() {
            return Some(v);
        }
        value.as_i64().map(|v| v != 0)
    })
}

#[cfg(test)]
mod tests {
    use super::PluginStateRepository;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use std::sync::Arc;

    #[tokio::test]
    async fn plugin_state_upsert_and_toggle_round_trip() {
        let sqlite = Arc::new(SqliteStorage::new_in_memory().await.unwrap());
        sqlite
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();

        sqlite
            .run_migrations(
                r#"
                CREATE TABLE IF NOT EXISTS roles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    slug TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS policy_keys (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL UNIQUE,
                    surface TEXT NOT NULL,
                    resource TEXT NOT NULL,
                    action TEXT NOT NULL,
                    name TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    is_system INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS role_policy_keys (
                    role_id INTEGER NOT NULL,
                    policy_key_id INTEGER NOT NULL,
                    UNIQUE(role_id, policy_key_id)
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
                    is_system INTEGER NOT NULL DEFAULT 0
                );

                INSERT OR IGNORE INTO roles (slug) VALUES ('admin');
                "#,
            )
            .await
            .unwrap();

        sqlite
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();

        let storage: Arc<dyn Storage> = sqlite;
        let repo = PluginStateRepository::new(storage);

        let created = repo
            .upsert_discovered_plugin("official/kv-store", "kv-store", "official", "1.0.0")
            .await
            .unwrap();
        assert!(created.enabled);

        let disabled = repo
            .set_enabled("kv-store", false, Some("admin"), Some("incident response"))
            .await
            .unwrap();
        assert!(!disabled.enabled);

        let latest_event = repo
            .get_latest_event_by_plugin_id("official/kv-store")
            .await
            .unwrap()
            .expect("expected audit event");
        assert_eq!(latest_event.plugin_id.as_str(), "official/kv-store");
        assert_eq!(latest_event.source_kind, "official");
        assert_eq!(latest_event.changed_by, "admin");
        assert_eq!(latest_event.previous_enabled, Some(true));
        assert_eq!(latest_event.next_enabled, Some(false));
        assert_eq!(latest_event.reason, "incident response");
    }

    #[tokio::test]
    async fn set_loaded_returns_error_for_missing_plugin() {
        let sqlite = Arc::new(SqliteStorage::new_in_memory().await.unwrap());
        sqlite
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        sqlite
            .run_migrations(
                r#"
                CREATE TABLE IF NOT EXISTS roles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    slug TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS policy_keys (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL UNIQUE,
                    surface TEXT NOT NULL,
                    resource TEXT NOT NULL,
                    action TEXT NOT NULL,
                    name TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    is_system INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS role_policy_keys (
                    role_id INTEGER NOT NULL,
                    policy_key_id INTEGER NOT NULL,
                    UNIQUE(role_id, policy_key_id)
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
                    is_system INTEGER NOT NULL DEFAULT 0
                );

                INSERT OR IGNORE INTO roles (slug) VALUES ('admin');
                "#,
            )
            .await
            .unwrap();
        sqlite
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();

        let storage: Arc<dyn Storage> = sqlite;
        let repo = PluginStateRepository::new(storage);

        let err = repo.set_loaded("missing-plugin", true).await.unwrap_err();
        assert_eq!(err, "plugin not found: missing-plugin");
    }
}
