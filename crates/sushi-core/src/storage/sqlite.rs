use super::{Row, Storage, StorageConn, StorageError};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SqliteStorage {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteStorage {
    pub async fn new(path: &str) -> Result<Self, StorageError> {
        let path = path.to_string();
        let conn = tokio::task::spawn_blocking(move || {
            rusqlite::Connection::open(&path)
                .map_err(|e| StorageError::ConnectionError(e.to_string()))
        })
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))??;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn new_in_memory() -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn run_migrations(&self, sql: &str) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute_batch(&sql)
                .map_err(|e| StorageError::QueryError(e.to_string()))
        })
        .await
        .map_err(|e| StorageError::QueryError(e.to_string()))?
    }

    pub async fn transaction<T, F>(&self, operation: F) -> Result<T, StorageError>
    where
        T: Send + 'static,
        F: FnOnce(&mut StorageConn<'_>) -> Result<T, StorageError> + Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let mut conn = conn.blocking_lock();
            let transaction = conn
                .transaction()
                .map_err(|e| StorageError::TransactionError(e.to_string()))?;
            let result = {
                let mut transaction_conn = StorageConn { conn: &transaction };
                operation(&mut transaction_conn)?
            };
            transaction
                .commit()
                .map_err(|e| StorageError::TransactionError(e.to_string()))?;
            Ok(result)
        })
        .await
        .map_err(|e| StorageError::TransactionError(e.to_string()))?
    }

    pub async fn apply_plugin_migration(
        &self,
        plugin_id: &str,
        migration_id: &str,
        checksum: &str,
        sql: &str,
    ) -> Result<(), StorageError> {
        let plugin_id = plugin_id.to_string();
        let migration_id = migration_id.to_string();
        let checksum = checksum.to_string();
        let sql = sql.to_string();
        self.transaction(move |conn| {
            conn.execute_batch(&sql)?;
            conn.execute(
                "INSERT INTO plugin_migrations (plugin_id, migration_id, checksum) VALUES (?, ?, ?)",
                vec![plugin_id.into(), migration_id.into(), checksum.into()],
            )
        })
        .await
    }

    pub async fn record_plugin_migration(
        &self,
        plugin_id: &str,
        migration_id: &str,
        checksum: &str,
    ) -> Result<(), StorageError> {
        let plugin_id = plugin_id.to_string();
        let migration_id = migration_id.to_string();
        let checksum = checksum.to_string();
        self.transaction(move |conn| {
            conn.execute(
                "INSERT INTO plugin_migrations (plugin_id, migration_id, checksum) VALUES (?, ?, ?)",
                vec![plugin_id.into(), migration_id.into(), checksum.into()],
            )
        })
        .await
    }
}

#[async_trait::async_trait]
impl Storage for SqliteStorage {
    async fn execute(&self, query: &str, params: Vec<Value>) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let params: Vec<rusqlite::types::Value> =
                params.into_iter().map(super::json_to_sqlite).collect();
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = params
                .iter()
                .map(|p| p as &dyn rusqlite::types::ToSql)
                .collect();
            conn.execute(&query, params_ref.as_slice())
                .map_err(|e| StorageError::QueryError(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::QueryError(e.to_string()))?
    }

    async fn query(&self, query: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let params: Vec<rusqlite::types::Value> =
                params.into_iter().map(super::json_to_sqlite).collect();
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = params
                .iter()
                .map(|p| p as &dyn rusqlite::types::ToSql)
                .collect();
            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| StorageError::QueryError(e.to_string()))?;
            let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let mut rows = stmt
                .query(params_ref.as_slice())
                .map_err(|e| StorageError::QueryError(e.to_string()))?;
            let mut result = Vec::new();
            while let Ok(Some(row)) = rows.next() {
                let mut map = std::collections::HashMap::new();
                for (i, col) in columns.iter().enumerate() {
                    let val: rusqlite::types::Value = row
                        .get(i)
                        .map_err(|e: rusqlite::Error| StorageError::QueryError(e.to_string()))?;
                    map.insert(col.clone(), super::sqlite_to_json(&val));
                }
                result.push(map);
            }
            Ok(result)
        })
        .await
        .map_err(|e| StorageError::QueryError(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_execute_and_query() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage
            .execute(
                "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
                vec![],
            )
            .await
            .unwrap();

        storage
            .execute(
                "INSERT INTO test_items (name) VALUES (?1)",
                vec![Value::String("hello".to_string())],
            )
            .await
            .unwrap();

        let rows = storage
            .query("SELECT * FROM test_items", vec![])
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().as_str().unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_sqlite_multiple_rows() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage
            .execute("CREATE TABLE nums (val INTEGER)", vec![])
            .await
            .unwrap();
        for i in 1..=5 {
            storage
                .execute(
                    "INSERT INTO nums (val) VALUES (?1)",
                    vec![Value::Number(i.into())],
                )
                .await
                .unwrap();
        }
        let rows = storage
            .query("SELECT * FROM nums ORDER BY val", vec![])
            .await
            .unwrap();
        assert_eq!(rows.len(), 5);
    }

    #[tokio::test]
    async fn test_sqlite_null_handling() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage
            .execute(
                "CREATE TABLE nullable (id INTEGER PRIMARY KEY, val TEXT)",
                vec![],
            )
            .await
            .unwrap();
        storage
            .execute("INSERT INTO nullable (val) VALUES (NULL)", vec![])
            .await
            .unwrap();
        let rows = storage
            .query("SELECT * FROM nullable", vec![])
            .await
            .unwrap();
        assert!(rows[0].get("val").unwrap().is_null());
    }

    #[tokio::test]
    async fn test_run_migrations() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage
            .run_migrations(
                "CREATE TABLE test_mig (id INTEGER PRIMARY KEY);
             INSERT INTO test_mig DEFAULT VALUES;",
            )
            .await
            .unwrap();
        let rows = storage
            .query("SELECT COUNT(*) as cnt FROM test_mig", vec![])
            .await
            .unwrap();
        assert_eq!(rows[0].get("cnt").unwrap().as_i64().unwrap(), 1);
    }
}
