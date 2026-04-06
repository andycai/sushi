pub mod sqlite;

use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("query error: {0}")]
    QueryError(String),

    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("transaction error: {0}")]
    TransactionError(String),
}

/// A single row returned from a query, keyed by column name.
pub type Row = HashMap<String, Value>;

/// Async storage trait.
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn execute(&self, query: &str, params: Vec<Value>) -> Result<(), StorageError>;
    async fn query(&self, query: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError>;
}

/// Synchronous connection handle used inside transactions.
pub struct StorageConn<'a> {
    conn: &'a mut rusqlite::Connection,
}

impl<'a> StorageConn<'a> {
    pub fn execute(&mut self, sql: &str, params: Vec<Value>) -> Result<(), StorageError> {
        let params: Vec<rusqlite::types::Value> = params.into_iter().map(json_to_sqlite).collect();
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
        self.conn.execute(sql, params_ref.as_slice())
            .map_err(|e| StorageError::QueryError(e.to_string()))?;
        Ok(())
    }

    pub fn query(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError> {
        let params: Vec<rusqlite::types::Value> = params.into_iter().map(json_to_sqlite).collect();
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = self.conn.prepare(sql)
            .map_err(|e| StorageError::QueryError(e.to_string()))?;
        let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt.query(params_ref.as_slice())
            .map_err(|e| StorageError::QueryError(e.to_string()))?;
        let mut result = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            let mut map = HashMap::new();
            for (i, col) in columns.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i)
                    .map_err(|e: rusqlite::Error| StorageError::QueryError(e.to_string()))?;
                map.insert(col.clone(), sqlite_to_json(&val));
            }
            result.push(map);
        }
        Ok(result)
    }
}

pub fn json_to_sqlite(v: Value) -> rusqlite::types::Value {
    match v {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(b as i64),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                rusqlite::types::Value::Null
            }
        }
        Value::String(s) => rusqlite::types::Value::Text(s),
        _ => rusqlite::types::Value::Null,
    }
}

pub fn sqlite_to_json(v: &rusqlite::types::Value) -> Value {
    match v {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::json!(i),
        rusqlite::types::Value::Real(f) => serde_json::json!(f),
        rusqlite::types::Value::Text(s) => Value::String(s.clone()),
        rusqlite::types::Value::Blob(_) => Value::Null,
    }
}
