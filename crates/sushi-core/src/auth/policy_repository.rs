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
                SELECT key, surface, resource, action, name
                FROM policy_keys
                ORDER BY key ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().map(row_to_policy_key).collect()
    }
}

fn row_to_policy_key(row: std::collections::HashMap<String, Value>) -> Result<StoredPolicyKey, String> {
    let key = row
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing policy key".to_string())?;
    let parsed = PolicyKey::parse(key)?;
    let name = row
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    Ok(StoredPolicyKey { key: parsed, name })
}

#[cfg(test)]
mod tests {
    use super::PolicyRepository;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use std::sync::Arc;

    #[tokio::test]
    async fn upsert_and_load_policy_key_round_trip() {
        let sqlite = Arc::new(SqliteStorage::new_in_memory().await.expect("sqlite setup should work"));
        sqlite
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .expect("migration 001 should apply");
        sqlite
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .expect("migration 003 should apply");
        sqlite
            .run_migrations(include_str!("../../../../migrations/006_unified_policy_v2.sql"))
            .await
            .expect("migration 006 should apply");

        let storage: Arc<dyn Storage> = sqlite;
        let repo = PolicyRepository::new(storage);

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
}
