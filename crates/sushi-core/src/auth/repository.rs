use crate::auth::model::{User, UserRole};
use crate::storage::sqlite::SqliteStorage;
use crate::storage::Storage;
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct UserRepository<'a> {
    storage: &'a SqliteStorage,
}

impl<'a> UserRepository<'a> {
    pub fn new(storage: &'a SqliteStorage) -> Self {
        Self { storage }
    }

    pub async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: UserRole,
    ) -> Result<User, String> {
        let role_str = role.to_string();
        self.storage.execute(
            "INSERT INTO users (username, email, password_hash, role) VALUES (?1, ?2, ?3, ?4)",
            vec![
                Value::String(username.to_string()),
                Value::String(email.to_string()),
                Value::String(password_hash.to_string()),
                Value::String(role_str),
            ],
        ).await.map_err(|e| e.to_string())?;

        self.find_by_username(username)
            .await?
            .ok_or_else(|| "user not found after insert".to_string())
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>, String> {
        let rows = self.storage.query(
            "SELECT * FROM users WHERE username = ?1",
            vec![Value::String(username.to_string())],
        ).await.map_err(|e| e.to_string())?;

        match rows.into_iter().next() {
            Some(row) => Ok(Some(row_to_user(row)?)),
            None => Ok(None),
        }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<User>, String> {
        let rows = self.storage.query(
            "SELECT * FROM users WHERE id = ?1",
            vec![Value::Number(id.into())],
        ).await.map_err(|e| e.to_string())?;

        match rows.into_iter().next() {
            Some(row) => Ok(Some(row_to_user(row)?)),
            None => Ok(None),
        }
    }

    pub async fn list_users(&self) -> Result<Vec<User>, String> {
        let rows = self.storage.query("SELECT * FROM users ORDER BY id", vec![])
            .await.map_err(|e| e.to_string())?;
        rows.into_iter().map(row_to_user).collect()
    }

    pub async fn delete_user(&self, id: i64) -> Result<(), String> {
        self.storage.execute(
            "DELETE FROM users WHERE id = ?1",
            vec![Value::Number(id.into())],
        ).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn row_to_user(row: std::collections::HashMap<String, Value>) -> Result<User, String> {
    let role_str = row.get("role").and_then(|v| v.as_str()).unwrap_or("viewer");
    let role = match role_str {
        "admin" => UserRole::Admin,
        "editor" => UserRole::Editor,
        _ => UserRole::Viewer,
    };
    
    // Parse SQLite datetime format: YYYY-MM-DD HH:MM:SS
    let parse_sqlite_datetime = |s: &str| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
            .unwrap_or_default()
    };
    
    Ok(User {
        id: row.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        username: row.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        email: row.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        password_hash: row.get("password_hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        role,
        created_at: row.get("created_at").and_then(|v| v.as_str())
            .map(parse_sqlite_datetime)
            .unwrap_or_default(),
        updated_at: row.get("updated_at").and_then(|v| v.as_str())
            .map(parse_sqlite_datetime)
            .unwrap_or_default(),
    })
}
