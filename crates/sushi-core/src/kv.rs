use std::sync::Arc;

use crate::storage::sqlite::SqliteStorage;
use crate::storage::{Storage, StorageError};

#[derive(Clone)]
pub struct KvStore {
    storage: Arc<SqliteStorage>,
}

impl KvStore {
    pub fn new(storage: Arc<SqliteStorage>) -> Self {
        Self { storage }
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let rows = Storage::query(&*self.storage, "SELECT value FROM kv_store WHERE key = ?1", vec![serde_json::Value::String(key.to_string())]).await?;
        Ok(rows.into_iter().next().and_then(|r| r.get("value").and_then(|v| v.as_str().map(String::from))))
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        Storage::execute(&*self.storage, "INSERT INTO kv_store (key, value, updated_at) VALUES (?1, ?2, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')", vec![serde_json::Value::String(key.to_string()), serde_json::Value::String(value.to_string())]).await
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        Storage::execute(&*self.storage, "DELETE FROM kv_store WHERE key = ?1", vec![serde_json::Value::String(key.to_string())]).await
    }

    pub async fn list(&self) -> Result<Vec<(String, String)>, StorageError> {
        let rows = Storage::query(&*self.storage, "SELECT key, value FROM kv_store ORDER BY key", vec![]).await?;
        rows.into_iter().map(|r| {
            let key = r.get("key").and_then(|v| v.as_str()).map(String::from).unwrap_or_default();
            let value = r.get("value").and_then(|v| v.as_str()).map(String::from).unwrap_or_default();
            Ok((key, value))
        }).collect()
    }
}
