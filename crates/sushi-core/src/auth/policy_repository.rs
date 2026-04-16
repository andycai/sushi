use crate::auth::authorizer::{CompiledPolicySnapshot, HttpBinding};
use crate::auth::policy::PolicyKey;
use crate::storage::Storage;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPolicyKey {
    pub key: PolicyKey,
    pub name: String,
}

pub struct PolicyRepository {
    storage: Arc<dyn Storage>,
}

impl PolicyRepository {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    pub async fn upsert_policy_key(&self, key: &str, name: &str) -> Result<(), String> {
        let policy_key = PolicyKey::parse(key)?;
        let normalized_name = name.trim();
        if normalized_name.is_empty() {
            return Err("policy key name cannot be empty".to_string());
        }

        self.storage
            .execute(
                r#"
                INSERT INTO policy_keys (key, surface, resource, action, name)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(key) DO UPDATE SET
                    surface = excluded.surface,
                    resource = excluded.resource,
                    action = excluded.action,
                    name = excluded.name,
                    updated_at = datetime('now')
                "#,
                vec![
                    Value::String(policy_key.key),
                    Value::String(policy_key.surface),
                    Value::String(policy_key.resource),
                    Value::String(policy_key.action),
                    Value::String(normalized_name.to_string()),
                ],
            )
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn list_policy_keys(&self) -> Result<Vec<StoredPolicyKey>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT key, name
                FROM policy_keys
                ORDER BY key ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().map(row_to_policy_key).collect()
    }

    pub async fn upsert_plugin_http_binding(
        &self,
        surface: &str,
        method: &str,
        path_pattern: &str,
        policy_key: &str,
        plugin_name: &str,
    ) -> Result<(), String> {
        self.upsert_http_binding(
            surface,
            method,
            path_pattern,
            policy_key,
            "plugin",
            plugin_name,
            false,
        )
        .await
    }

    pub async fn upsert_plugin_cli_binding(
        &self,
        command_name: &str,
        policy_key: &str,
        plugin_name: &str,
    ) -> Result<(), String> {
        self.upsert_cli_binding(
            "cli",
            command_name,
            policy_key,
            "plugin",
            plugin_name,
            false,
        )
        .await
    }

    pub async fn compile_snapshot(&self) -> Result<CompiledPolicySnapshot, String> {
        let role_rows = self
            .storage
            .query(
                r#"
                SELECT r.slug AS role_slug, pk.key AS policy_key
                FROM role_policy_keys rpk
                JOIN roles r ON r.id = rpk.role_id
                JOIN policy_keys pk ON pk.id = rpk.policy_key_id
                ORDER BY r.slug ASC, pk.key ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        let role_grants = role_rows
            .into_iter()
            .map(|row| {
                Ok((
                    required_row_str(&row, "role_slug")?.to_string(),
                    required_row_str(&row, "policy_key")?.to_string(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let http_rows = self
            .storage
            .query(
                r#"
                SELECT pb.surface, pb.method, pb.path_pattern, pk.key AS policy_key
                FROM policy_bindings pb
                JOIN policy_keys pk ON pk.id = pb.policy_key_id
                WHERE pb.target_type = 'http_route'
                ORDER BY pb.id ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        let http_bindings = http_rows
            .into_iter()
            .map(row_to_http_binding)
            .collect::<Result<Vec<_>, String>>()?;

        let cli_rows = self
            .storage
            .query(
                r#"
                SELECT pb.surface, pb.command_name, pk.key AS policy_key
                FROM policy_bindings pb
                JOIN policy_keys pk ON pk.id = pb.policy_key_id
                WHERE pb.target_type = 'cli_command'
                ORDER BY pb.id ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        let command_bindings = cli_rows
            .into_iter()
            .map(|row| {
                Ok((
                    required_row_str(&row, "surface")?.to_string(),
                    required_row_str(&row, "command_name")?.to_string(),
                    required_row_str(&row, "policy_key")?.to_string(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(CompiledPolicySnapshot::new(
            http_bindings,
            command_bindings,
            role_grants,
        ))
    }

    async fn upsert_http_binding(
        &self,
        surface: &str,
        method: &str,
        path_pattern: &str,
        policy_key: &str,
        owner_type: &str,
        owner_id: &str,
        is_system: bool,
    ) -> Result<(), String> {
        let surface = normalize_non_empty(surface, "surface")?.to_ascii_lowercase();
        let method = normalize_non_empty(method, "method")?.to_ascii_uppercase();
        let path_pattern = normalize_non_empty(path_pattern, "path_pattern")?;
        let target_ref = path_pattern.clone();
        let owner_type = normalize_non_empty(owner_type, "owner_type")?.to_ascii_lowercase();
        let owner_id = normalize_non_empty(owner_id, "owner_id")?;
        let parsed = self.ensure_policy_key(policy_key).await?;

        self.storage
            .execute(
                r#"
                INSERT INTO policy_bindings (
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
                    ?1,
                    'http_route',
                    ?2,
                    ?3,
                    ?4,
                    NULL,
                    pk.id,
                    ?5,
                    ?6,
                    ?7
                FROM policy_keys pk
                WHERE pk.key = ?8
                ON CONFLICT DO UPDATE SET
                    updated_at = datetime('now')
                "#,
                vec![
                    Value::String(surface),
                    Value::String(target_ref),
                    Value::String(method),
                    Value::String(path_pattern),
                    Value::String(owner_type),
                    Value::String(owner_id),
                    Value::Bool(is_system),
                    Value::String(parsed.key),
                ],
            )
            .await
            .map_err(|err| err.to_string())
    }

    async fn upsert_cli_binding(
        &self,
        surface: &str,
        command_name: &str,
        policy_key: &str,
        owner_type: &str,
        owner_id: &str,
        is_system: bool,
    ) -> Result<(), String> {
        let surface = normalize_non_empty(surface, "surface")?.to_ascii_lowercase();
        let command_name = normalize_non_empty(command_name, "command_name")?;
        let target_ref = command_name.clone();
        let owner_type = normalize_non_empty(owner_type, "owner_type")?.to_ascii_lowercase();
        let owner_id = normalize_non_empty(owner_id, "owner_id")?;
        let parsed = self.ensure_policy_key(policy_key).await?;

        self.storage
            .execute(
                r#"
                INSERT INTO policy_bindings (
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
                    ?1,
                    'cli_command',
                    ?2,
                    NULL,
                    NULL,
                    ?3,
                    pk.id,
                    ?4,
                    ?5,
                    ?6
                FROM policy_keys pk
                WHERE pk.key = ?7
                ON CONFLICT DO UPDATE SET
                    updated_at = datetime('now')
                "#,
                vec![
                    Value::String(surface),
                    Value::String(target_ref),
                    Value::String(command_name),
                    Value::String(owner_type),
                    Value::String(owner_id),
                    Value::Bool(is_system),
                    Value::String(parsed.key),
                ],
            )
            .await
            .map_err(|err| err.to_string())
    }

    async fn ensure_policy_key(&self, policy_key: &str) -> Result<PolicyKey, String> {
        let parsed = PolicyKey::parse(policy_key)?;
        self.storage
            .execute(
                r#"
                INSERT OR IGNORE INTO policy_keys (key, surface, resource, action, name)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                vec![
                    Value::String(parsed.key.clone()),
                    Value::String(parsed.surface.clone()),
                    Value::String(parsed.resource.clone()),
                    Value::String(parsed.action.clone()),
                    Value::String(parsed.key.clone()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(parsed)
    }
}

fn row_to_policy_key(
    row: std::collections::HashMap<String, Value>,
) -> Result<StoredPolicyKey, String> {
    let key = row
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing policy key".to_string())?;
    let parsed = PolicyKey::parse(key)?;
    let name = row
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing policy key name".to_string())?;
    if name.trim().is_empty() {
        return Err("policy key name cannot be empty".to_string());
    }

    Ok(StoredPolicyKey {
        key: parsed,
        name: name.to_string(),
    })
}

fn row_to_http_binding(
    row: std::collections::HashMap<String, Value>,
) -> Result<HttpBinding, String> {
    Ok(HttpBinding {
        surface: required_row_str(&row, "surface")?.to_string(),
        method: required_row_str(&row, "method")?.to_string(),
        path_pattern: required_row_str(&row, "path_pattern")?.to_string(),
        policy_key: required_row_str(&row, "policy_key")?.to_string(),
    })
}

fn required_row_str<'a>(
    row: &'a std::collections::HashMap<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {key}"))
}

fn normalize_non_empty(value: &str, field: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(format!("{field} cannot be empty"));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::PolicyRepository;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use std::sync::Arc;

    async fn repo_with_schema() -> PolicyRepository {
        let sqlite = Arc::new(
            SqliteStorage::new_in_memory()
                .await
                .expect("sqlite setup should work"),
        );
        sqlite
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .expect("migration 001 should apply");
        sqlite
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .expect("migration 003 should apply");
        sqlite
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .expect("migration 006 should apply");

        let storage: Arc<dyn Storage> = sqlite;
        PolicyRepository::new(storage)
    }

    #[tokio::test]
    async fn upsert_and_load_policy_key_round_trip() {
        let repo = repo_with_schema().await;

        repo.upsert_policy_key("Admin.Users.Read", "Read users")
            .await
            .expect("upsert should work");

        let mut policy_keys = repo
            .list_policy_keys()
            .await
            .expect("loading keys should work");
        policy_keys.retain(|item| item.key.key == "admin.users.read");

        assert_eq!(policy_keys.len(), 1);
        assert_eq!(policy_keys[0].name, "Read users");
        assert_eq!(policy_keys[0].key.surface, "admin");
        assert_eq!(policy_keys[0].key.resource, "users");
        assert_eq!(policy_keys[0].key.action, "read");

        repo.upsert_policy_key("admin.users.read", "Read users in admin")
            .await
            .expect("upsert update should work");
        let policy_keys = repo
            .list_policy_keys()
            .await
            .expect("loading keys should work");
        let updated = policy_keys
            .into_iter()
            .find(|item| item.key.key == "admin.users.read")
            .expect("expected upserted key");

        assert_eq!(updated.name, "Read users in admin");
    }

    #[tokio::test]
    async fn rejects_empty_policy_name() {
        let repo = repo_with_schema().await;

        let err = repo
            .upsert_policy_key("admin.users.read", "   ")
            .await
            .expect_err("blank names should be rejected");

        assert_eq!(err, "policy key name cannot be empty");
    }

    #[tokio::test]
    async fn plugin_binding_upsert_populates_compiled_snapshot() {
        let repo = repo_with_schema().await;
        repo.upsert_plugin_http_binding("api", "GET", "/api/notes/*", "api.notes.read", "notes")
            .await
            .expect("api binding upsert should work");
        repo.upsert_plugin_cli_binding("notes-run", "cli.notes.run", "notes")
            .await
            .expect("cli binding upsert should work");

        let snapshot = repo.compile_snapshot().await.expect("snapshot should load");
        assert!(snapshot
            .http_bindings
            .iter()
            .any(|binding| binding.surface == "api"
                && binding.method == "GET"
                && binding.path_pattern == "/api/notes/*"
                && binding.policy_key == "api.notes.read"));
        assert!(snapshot.has_command_binding("cli", "notes-run"));
    }

    #[tokio::test]
    async fn compile_snapshot_includes_seeded_plugin_list_command_binding() {
        let repo = repo_with_schema().await;
        let snapshot = repo.compile_snapshot().await.expect("snapshot should load");

        assert!(snapshot.has_command_binding("cli", "plugin:list"));
        assert!(snapshot.command_allowed("admin", "cli", "plugin:list"));
    }
}
