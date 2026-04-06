# Sushi Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a modular application platform with Rust runtime and Lua plugin system, delivering admin UI, API server, and CLI as a single binary.

**Architecture:** Plugin-First — all business capabilities (routes, commands, pages, auth) are exposed through a unified `Plugin` trait and `SushiContext`. Rust built-in features and Lua plugins share the same registration interfaces. Single Axum server serves both API and admin, with clap for CLI subcommands.

**Tech Stack:** Rust Axum 0.8, mlua 0.10 (Lua 5.4, vendored, async, send), clap 4, Alpine.js, TailwindCSS, SQLite (rusqlite), JWT (jsonwebtoken), Argon2, rust-embed

**Spec:** `docs/superpowers/specs/2026-04-06-sushi-platform-design.md`

---

## File Structure

```
sushi/
├── Cargo.toml
├── config.toml
├── crates/
│   ├── sushi-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── plugin.rs          # Plugin trait, PluginError, PluginManifest
│   │       ├── context.rs         # SushiContext
│   │       ├── config.rs          # ConfigStore (TOML config)
│   │       ├── registry/
│   │       │   ├── mod.rs         # ApiRegistry, AdminRegistry, CliRegistry
│   │       │   └── event.rs       # EventBus
│   │       ├── storage/
│   │       │   ├── mod.rs         # Storage trait
│   │       │   └── sqlite.rs      # SqliteStorage
│   │       ├── auth/
│   │       │   ├── mod.rs
│   │       │   ├── model.rs       # User, UserRole
│   │       │   ├── password.rs    # Argon2 hashing
│   │       │   ├── jwt.rs         # JWT creation/verification
│   │       │   ├── repository.rs  # User CRUD
│   │       │   └── middleware.rs   # Axum auth extractor
│   │       └── lua/
│   │           ├── mod.rs
│   │           ├── vm.rs          # Lua VM creation, sandbox
│   │           ├── loader.rs      # Plugin directory scanner, LuaPlugin
│   │           └── bindings.rs    # sushi.* API bindings
│   ├── sushi-api/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs
│   │       └── routes/
│   │           ├── mod.rs
│   │           └── auth.rs
│   ├── sushi-admin/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs
│   │       └── routes/
│   │           ├── mod.rs
│   │           ├── dashboard.rs
│   │           ├── plugins.rs
│   │           ├── users.rs
│   │           ├── config.rs
│   │           └── logs.rs
│   ├── sushi-cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   └── sushi/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
├── plugins/
│   └── _example/
│       ├── plugin.toml
│       └── init.lua
├── ui/
│   ├── package.json
│   ├── tailwind.config.js
│   └── src/
│       ├── index.html
│       ├── app.js
│       └── styles.css
└── migrations/
    └── 001_init.sql
```

---

## Phase 1: Workspace & Core Types

### Task 1: Workspace Skeleton

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/sushi-core/Cargo.toml`
- Create: `crates/sushi-core/src/lib.rs`
- Create: `crates/sushi-api/Cargo.toml`
- Create: `crates/sushi-api/src/lib.rs`
- Create: `crates/sushi-admin/Cargo.toml`
- Create: `crates/sushi-admin/src/lib.rs`
- Create: `crates/sushi-cli/Cargo.toml`
- Create: `crates/sushi-cli/src/lib.rs`
- Create: `crates/sushi/Cargo.toml`
- Create: `crates/sushi/src/main.rs`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/sushi-core",
    "crates/sushi-api",
    "crates/sushi-admin",
    "crates/sushi-cli",
    "crates/sushi",
]

[workspace.dependencies]
axum = "0.8"
mlua = { version = "0.10", features = ["lua54", "vendored", "async", "send"] }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
anyhow = "1"
thiserror = "2"
tower = "0.5"
tower-http = { version = "0.6", features = ["fs", "cors", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
rusqlite = { version = "0.32", features = ["bundled"] }
jsonwebtoken = "9"
argon2 = "0.5"
chrono = { version = "0.4", features = ["serde"] }
rust-embed = "8"
async-trait = "0.1"
```

- [ ] **Step 2: Create sushi-core/Cargo.toml**

```toml
[package]
name = "sushi-core"
version = "0.1.0"
edition = "2021"

[dependencies]
mlua = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
rusqlite = { workspace = true }
jsonwebtoken = { workspace = true }
argon2 = { workspace = true }
chrono = { workspace = true }
async-trait = { workspace = true }
```

```rust
// crates/sushi-core/src/lib.rs
pub mod plugin;
pub mod config;
pub mod registry;
pub mod storage;
pub mod auth;
pub mod lua;
pub mod context;
```

- [ ] **Step 3: Create sushi-api/Cargo.toml**

```toml
[package]
name = "sushi-api"
version = "0.1.0"
edition = "2021"

[dependencies]
sushi-core = { path = "../sushi-core" }
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
```

```rust
// crates/sushi-api/src/lib.rs
pub mod router;
pub mod routes;
```

- [ ] **Step 4: Create sushi-admin/Cargo.toml**

```toml
[package]
name = "sushi-admin"
version = "0.1.0"
edition = "2021"

[dependencies]
sushi-core = { path = "../sushi-core" }
sushi-api = { path = "../sushi-api" }
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tower-http = { workspace = true }
rust-embed = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
mime_guess = "2"
```

```rust
// crates/sushi-admin/src/lib.rs
pub mod router;
pub mod routes;
```

- [ ] **Step 5: Create sushi-cli/Cargo.toml**

```toml
[package]
name = "sushi-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
sushi-core = { path = "../sushi-core" }
clap = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/sushi-cli/src/lib.rs
pub mod commands;
```

- [ ] **Step 6: Create sushi binary crate**

```toml
# crates/sushi/Cargo.toml
[package]
name = "sushi"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "sushi"
path = "src/main.rs"

[dependencies]
sushi-core = { path = "../sushi-core" }
sushi-api = { path = "../sushi-api" }
sushi-admin = { path = "../sushi-admin" }
sushi-cli = { path = "../sushi-cli" }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

```rust
// crates/sushi/src/main.rs
fn main() {
    println!("sushi placeholder");
}
```

- [ ] **Step 7: Verify workspace compiles**

Run: `cargo build --workspace`
Expected: SUCCESS (all crates compile with placeholder lib.rs/main.rs)

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/
git commit -m "feat: initialize Cargo workspace with all crate skeletons"
```

---

## Phase 2: Core Plugin Trait & Config

### Task 2: Plugin Trait & Error Types

**Files:**
- Create: `crates/sushi-core/src/plugin.rs`

- [ ] **Step 1: Write the test for Plugin trait and PluginManifest**

```rust
// crates/sushi-core/src/plugin.rs — add at bottom in #[cfg(test)] module

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plugin_manifest() {
        let toml_str = r#"
[plugin]
name = "test_plugin"
version = "0.1.0"
description = "A test plugin"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = false
database = "write"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "test_plugin");
        assert_eq!(manifest.plugin.version, "0.1.0");
        assert_eq!(manifest.plugin.entry, "init.lua");
        assert!(manifest.permissions.routes);
        assert!(manifest.permissions.commands);
        assert!(!manifest.permissions.admin);
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::InitFailed("lua error".to_string());
        assert_eq!(err.to_string(), "plugin init failed: lua error");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core -- plugin::tests`
Expected: FAIL — `plugin.rs` does not exist yet

- [ ] **Step 3: Write the implementation**

```rust
// crates/sushi-core/src/plugin.rs
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;

/// Error type for plugin operations.
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("plugin not found: {0}")]
    NotFound(String),

    #[error("plugin init failed: {0}")]
    InitFailed(String),

    #[error("manifest parse error: {0}")]
    ManifestError(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("lua error: {0}")]
    LuaError(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Plugin manifest parsed from plugin.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub permissions: Permissions,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default = "default_entry")]
    pub entry: String,
}

fn default_entry() -> String {
    "init.lua".to_string()
}

/// Plugin permission levels.
#[derive(Debug, Clone, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub routes: bool,
    #[serde(default)]
    pub commands: bool,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub database: DatabasePermission,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatabasePermission {
    #[default]
    None,
    #[serde(rename = "true")]
    ReadOnly,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "admin")]
    Admin,
}

// serde treats bare `true` as ReadOnly for backward compat
impl<'de> serde::Deserialize<'de> for DatabasePermission {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{self, Visitor};
        struct DbPermVisitor;
        impl<'de> Visitor<'de> for DbPermVisitor {
            type Value = DatabasePermission;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("false, true, \"read\", \"write\", or \"admin\"")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(if v { DatabasePermission::ReadOnly } else { DatabasePermission::None })
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                match v {
                    "read" | "true" => Ok(DatabasePermission::ReadOnly),
                    "write" => Ok(DatabasePermission::Write),
                    "admin" => Ok(DatabasePermission::Admin),
                    other => Err(de::Error::custom(format!("unknown db permission: {other}"))),
                }
            }
        }
        deserializer.deserialize_any(DbPermVisitor)
    }
}

/// The core Plugin trait. Both Rust plugins and Lua plugins implement this.
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    async fn init(&self, ctx: &crate::context::SushiContext) -> Result<(), PluginError>;
}

/// A simple Rust-based plugin from a closure (for built-in functionality).
pub struct FnPlugin {
    name: String,
    version: String,
    init_fn: Box<dyn Fn(&crate::context::SushiContext) -> Result<(), PluginError> + Send + Sync>,
}

impl FnPlugin {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        init_fn: Box<dyn Fn(&crate::context::SushiContext) -> Result<(), PluginError> + Send + Sync>,
    ) -> Self {
        Self { name: name.into(), version: version.into(), init_fn }
    }
}

#[async_trait]
impl Plugin for FnPlugin {
    fn name(&self) -> &str { &self.name }
    fn version(&self) -> &str { &self.version }
    async fn init(&self, ctx: &crate::context::SushiContext) -> Result<(), PluginError> {
        (self.init_fn)(ctx)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core -- plugin::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/plugin.rs
git commit -m "feat(core): add Plugin trait, PluginManifest, error types"
```

### Task 3: ConfigStore

**Files:**
- Create: `crates/sushi-core/src/config.rs`

- [ ] **Step 1: Write the test**

```rust
// at bottom of crates/sushi-core/src/config.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
[server]
host = "127.0.0.1"
port = 3000

[database]
path = "data/sushi.db"

[jwt]
secret = "test-secret-key-at-least-32-chars"
access_ttl = 3600
refresh_ttl = 604800

[plugins]
directory = "plugins"
"#;
        let config: SushiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.path, "data/sushi.db");
        assert_eq!(config.jwt.access_ttl, 3600);
    }

    #[test]
    fn test_config_defaults() {
        let toml_str = "";
        let config: SushiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.plugins.directory, "plugins");
    }

    #[test]
    fn test_config_store_get_set() {
        let config = SushiConfig::default();
        let store = ConfigStore::new(config);
        assert_eq!(store.get_server_port(), 3000);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core -- config::tests`
Expected: FAIL — file does not exist

- [ ] **Step 3: Write the implementation**

```rust
// crates/sushi-core/src/config.rs
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SushiConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub jwt: JwtConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { host: default_host(), port: default_port() }
    }
}

fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 3000 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self { path: default_db_path() }
    }
}

fn default_db_path() -> String { "data/sushi.db".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    #[serde(default = "default_jwt_secret")]
    pub secret: String,
    #[serde(default = "default_access_ttl")]
    pub access_ttl: i64,
    #[serde(default = "default_refresh_ttl")]
    pub refresh_ttl: i64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: default_jwt_secret(),
            access_ttl: default_access_ttl(),
            refresh_ttl: default_refresh_ttl(),
        }
    }
}

fn default_jwt_secret() -> String { "change-me-in-production-at-least-32-chars".to_string() }
fn default_access_ttl() -> i64 { 3600 }
fn default_refresh_ttl() -> i64 { 604800 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default = "default_plugins_dir")]
    pub directory: String,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self { directory: default_plugins_dir() }
    }
}

fn default_plugins_dir() -> String { "plugins".to_string() }

impl Default for SushiConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            jwt: JwtConfig::default(),
            plugins: PluginsConfig::default(),
        }
    }
}

/// Thread-safe config store.
pub struct ConfigStore {
    inner: Arc<RwLock<SushiConfig>>,
}

impl ConfigStore {
    pub fn new(config: SushiConfig) -> Self {
        Self { inner: Arc::new(RwLock::new(config)) }
    }

    pub async fn load(path: &Path) -> anyhow::Result<Self> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| anyhow::anyhow!("failed to read config {}: {e}", path.display()))?;
        let config: SushiConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse config: {e}"))?;
        Ok(Self::new(config))
    }

    pub async fn get(&self) -> tokio::sync::RwLockReadGuard<'_, SushiConfig> {
        self.inner.read().await
    }

    pub async fn update<F>(&self, f: F)
    where F: FnOnce(&mut SushiConfig) {
        let mut guard = self.inner.write().await;
        f(&mut guard);
    }

    pub fn get_server_port(&self) -> u16 {
        // synchronous peek for initialization — use .get() in async contexts
        self.inner.try_read().map(|g| g.server.port).unwrap_or(3000)
    }

    pub fn get_plugins_dir(&self) -> String {
        self.inner.try_read().map(|g| g.plugins.directory.clone()).unwrap_or_else(|_| "plugins".to_string())
    }
}

impl Clone for ConfigStore {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core -- config::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/config.rs
git commit -m "feat(core): add SushiConfig and ConfigStore"
```

---

## Phase 3: Storage, EventBus & Registries

### Task 4: SQLite Storage Layer

**Files:**
- Create: `crates/sushi-core/src/storage/mod.rs`
- Create: `crates/sushi-core/src/storage/sqlite.rs`
- Create: `migrations/001_init.sql`

- [ ] **Step 1: Write the test**

```rust
// at bottom of crates/sushi-core/src/storage/sqlite.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_execute_and_query() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage.execute(
            "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            vec![],
        ).await.unwrap();

        storage.execute(
            "INSERT INTO test_items (name) VALUES (?1)",
            vec![Value::String("hello".to_string())],
        ).await.unwrap();

        let rows = storage.query("SELECT * FROM test_items", vec![]).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().as_str().unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_sqlite_transaction() {
        let storage = SqliteStorage::new_in_memory().await.unwrap();
        storage.execute(
            "CREATE TABLE test_tx (id INTEGER PRIMARY KEY, val INTEGER)",
            vec![],
        ).await.unwrap();

        let result = storage.transaction(|conn| {
            conn.execute("INSERT INTO test_tx (val) VALUES (42)", vec![])?;
            conn.execute("INSERT INTO test_tx (val) VALUES (84)", vec![])?;
            Ok(2)
        }).await.unwrap();

        assert_eq!(result, 2);
        let rows = storage.query("SELECT COUNT(*) as cnt FROM test_tx", vec![]).await.unwrap();
        assert_eq!(rows[0].get("cnt").unwrap().as_i64().unwrap(), 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core -- storage::sqlite::tests`
Expected: FAIL

- [ ] **Step 3: Write storage/mod.rs**

```rust
// crates/sushi-core/src/storage/mod.rs
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

/// Async storage trait — SQLite implementation wraps blocking calls in spawn_blocking.
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn execute(&self, query: &str, params: Vec<Value>) -> Result<(), StorageError>;
    async fn query(&self, query: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError>;
    async fn transaction<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&mut StorageConn) -> Result<T, StorageError> + Send + 'static,
        T: Send + 'static;
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
        let rows = stmt.query(params_ref.as_slice())
            .map_err(|e| StorageError::QueryError(e.to_string()))?;
        let mut result = Vec::new();
        for row_res in rows {
            let row = row_res.map_err(|e| StorageError::QueryError(e.to_string()))?;
            let mut map = HashMap::new();
            for (i, col) in columns.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i)
                    .map_err(|e| StorageError::QueryError(e.to_string()))?;
                map.insert(col.clone(), sqlite_to_json(&val));
            }
            result.push(map);
        }
        Ok(result)
    }
}

fn json_to_sqlite(v: Value) -> rusqlite::types::Value {
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

fn sqlite_to_json(v: &rusqlite::types::Value) -> Value {
    match v {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::json!(i),
        rusqlite::types::Value::Real(f) => serde_json::json!(f),
        rusqlite::types::Value::Text(s) => Value::String(s.clone()),
        rusqlite::types::Value::Blob(b) => {
            use serde_json::to_value;
            to_value(b).unwrap_or(Value::Null)
        }
    }
}
```

- [ ] **Step 4: Write storage/sqlite.rs**

```rust
// crates/sushi-core/src/storage/sqlite.rs
use super::{Row, Storage, StorageConn, StorageError};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SqliteStorage {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteStorage {
    pub async fn new(path: &str) -> Result<Self, StorageError> {
        let conn = tokio::task::spawn_blocking(|| {
            rusqlite::Connection::open(path)
                .map_err(|e| StorageError::ConnectionError(e.to_string()))
        }).await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))??;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub async fn new_in_memory() -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub async fn run_migrations(&self, sql: &str) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute_batch(&sql)
                .map_err(|e| StorageError::QueryError(e.to_string()))
        }).await
            .map_err(|e| StorageError::QueryError(e.to_string()))?
    }
}

#[async_trait::async_trait]
impl Storage for SqliteStorage {
    async fn execute(&self, query: &str, params: Vec<Value>) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let params: Vec<rusqlite::types::Value> = params.into_iter().map(super::json_to_sqlite).collect();
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
            conn.execute(&query, params_ref.as_slice())
                .map_err(|e| StorageError::QueryError(e.to_string()))?;
            Ok(())
        }).await
            .map_err(|e| StorageError::QueryError(e.to_string()))?
    }

    async fn query(&self, query: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let params: Vec<rusqlite::types::Value> = params.into_iter().map(super::json_to_sqlite).collect();
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
            let mut stmt = conn.prepare(&query)
                .map_err(|e| StorageError::QueryError(e.to_string()))?;
            let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let rows = stmt.query(params_ref.as_slice())
                .map_err(|e| StorageError::QueryError(e.to_string()))?;
            let mut result = Vec::new();
            for row_res in rows {
                let row = row_res.map_err(|e| StorageError::QueryError(e.to_string()))?;
                let mut map = std::collections::HashMap::new();
                for (i, col) in columns.iter().enumerate() {
                    let val: rusqlite::types::Value = row.get(i)
                        .map_err(|e| StorageError::QueryError(e.to_string()))?;
                    map.insert(col.clone(), super::sqlite_to_json(&val));
                }
                result.push(map);
            }
            Ok(result)
        }).await
            .map_err(|e| StorageError::QueryError(e.to_string()))?
    }

    async fn transaction<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&mut StorageConn) -> Result<T, StorageError> + Send + 'static,
        T: Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()
                .map_err(|e| StorageError::TransactionError(e.to_string()))?;
            let mut storage_conn = StorageConn { conn: unsafe { &mut *(&tx as *const rusqlite::Transaction as *mut rusqlite::Connection) } };
            let result = f(&mut storage_conn)?;
            tx.commit().map_err(|e| StorageError::TransactionError(e.to_string()))?;
            Ok(result)
        }).await
            .map_err(|e| StorageError::QueryError(e.to_string()))?
    }
}
```

- [ ] **Step 5: Write migration file**

```sql
-- migrations/001_init.sql

CREATE TABLE IF NOT EXISTS _sushi_migrations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'viewer',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS plugin_state (
    name TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    loaded INTEGER NOT NULL DEFAULT 0,
    version TEXT,
    loaded_at TEXT
);

INSERT INTO _sushi_migrations (id, name) VALUES (1, '001_init');
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p sushi-core -- storage::sqlite::tests`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/sushi-core/src/storage/ migrations/
git commit -m "feat(core): add SQLite storage layer with migrations"
```

### Task 5: EventBus

**Files:**
- Create: `crates/sushi-core/src/registry/mod.rs`
- Create: `crates/sushi-core/src/registry/event.rs`

- [ ] **Step 1: Write the test**

```rust
// at bottom of crates/sushi-core/src/registry/event.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_event_subscribe_and_emit() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = counter.clone();
        bus.on("test.event", move |data| {
            let c = c.clone();
            Box::pin(async move {
                if let Some(v) = data.get("value").and_then(|v| v.as_i64()) {
                    c.fetch_add(v as usize, Ordering::SeqCst);
                }
            })
        }).await;

        let mut data = serde_json::Map::new();
        data.insert("value".to_string(), serde_json::json!(42));
        bus.emit("test.event", &serde_json::Value::Object(data)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 42);
    }

    #[tokio::test]
    async fn test_event_multiple_subscribers() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        bus.on("multi", move |_| {
            let c = c1.clone();
            Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); })
        }).await;

        let c2 = counter.clone();
        bus.on("multi", move |_| {
            let c = c2.clone();
            Box::pin(async move { c.fetch_add(10, Ordering::SeqCst); })
        }).await;

        bus.emit("multi", &serde_json::Value::Null).await;
        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sushi-core -- registry::event::tests`
Expected: FAIL

- [ ] **Step 3: Write the implementation**

```rust
// crates/sushi-core/src/registry/event.rs
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

type EventHandler = Box<dyn Fn(&Value) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<String, Vec<EventHandler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn on<F, Fut>(&self, event: &str, handler: F)
    where
        F: Fn(&Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let wrapped: EventHandler = Box::new(move |data| Box::pin(handler(data)));
        let mut subs = self.subscribers.write().await;
        subs.entry(event.to_string()).or_default().push(wrapped);
    }

    pub async fn emit(&self, event: &str, data: &Value) {
        let subs = self.subscribers.read().await;
        if let Some(handlers) = subs.get(event) {
            for handler in handlers {
                handler(data).await;
            }
        }
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self { subscribers: Arc::clone(&self.subscribers) }
    }
}

impl Default for EventBus {
    fn default() -> Self { Self::new() }
}
```

```rust
// crates/sushi-core/src/registry/mod.rs
pub mod event;

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Registered API route.
pub struct RouteEntry {
    pub method: String,
    pub path: String,
    pub handler: Arc<dyn Fn(serde_json::Value) -> Box<dyn std::future::Future<Output = Value> + Send + Sync> + Send + Sync>,
}

/// Registered admin page.
pub struct AdminPageEntry {
    pub path: String,
    pub title: String,
    pub renderer: Arc<dyn Fn() -> String + Send + Sync>,
}

/// Registered admin widget.
pub struct AdminWidgetEntry {
    pub name: String,
    pub renderer: Arc<dyn Fn() -> String + Send + Sync>,
}

/// Registered CLI command.
pub struct CliCommandEntry {
    pub name: String,
    pub description: String,
    pub handler: Arc<dyn Fn(Vec<String>) + Send + Sync>,
}

/// API route registry.
#[derive(Default)]
pub struct ApiRegistry {
    pub routes: Arc<Mutex<Vec<RouteEntry>>>,
}

impl ApiRegistry {
    pub fn new() -> Self { Self::default() }

    pub async fn route<F, Fut>(&self, method: &str, path: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Value> + Send + Sync + 'static,
    {
        let handler = Arc::new(move |req| Box::new(handler(req)) as _);
        self.routes.lock().await.push(RouteEntry {
            method: method.to_string(),
            path: path.to_string(),
            handler,
        });
    }

    pub async fn get_routes(&self) -> Vec<(String, String)> {
        let routes = self.routes.lock().await;
        routes.iter().map(|r| (r.method.clone(), r.path.clone())).collect()
    }
}

impl Clone for ApiRegistry {
    fn clone(&self) -> Self {
        Self { routes: Arc::clone(&self.routes) }
    }
}

/// Admin page/widget registry.
#[derive(Default)]
pub struct AdminRegistry {
    pub pages: Arc<Mutex<Vec<AdminPageEntry>>>,
    pub widgets: Arc<Mutex<Vec<AdminWidgetEntry>>>,
}

impl AdminRegistry {
    pub fn new() -> Self { Self::default() }

    pub async fn page<F>(&self, path: &str, title: &str, renderer: F)
    where F: Fn() -> String + Send + Sync + 'static {
        self.pages.lock().await.push(AdminPageEntry {
            path: path.to_string(),
            title: title.to_string(),
            renderer: Arc::new(renderer),
        });
    }

    pub async fn widget<F>(&self, name: &str, renderer: F)
    where F: Fn() -> String + Send + Sync + 'static {
        self.widgets.lock().await.push(AdminWidgetEntry {
            name: name.to_string(),
            renderer: Arc::new(renderer),
        });
    }
}

impl Clone for AdminRegistry {
    fn clone(&self) -> Self {
        Self {
            pages: Arc::clone(&self.pages),
            widgets: Arc::clone(&self.widgets),
        }
    }
}

/// CLI command registry.
#[derive(Default)]
pub struct CliRegistry {
    pub commands: Arc<Mutex<Vec<CliCommandEntry>>>,
}

impl CliRegistry {
    pub fn new() -> Self { Self::default() }

    pub async fn command<F>(&self, name: &str, description: &str, handler: F)
    where F: Fn(Vec<String>) + Send + Sync + 'static {
        self.commands.lock().await.push(CliCommandEntry {
            name: name.to_string(),
            description: description.to_string(),
            handler: Arc::new(handler),
        });
    }

    pub async fn get_commands(&self) -> Vec<(String, String)> {
        let cmds = self.commands.lock().await;
        cmds.iter().map(|c| (c.name.clone(), c.description.clone())).collect()
    }
}

impl Clone for CliRegistry {
    fn clone(&self) -> Self {
        Self { commands: Arc::clone(&self.commands) }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sushi-core -- registry`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-core/src/registry/
git commit -m "feat(core): add EventBus, ApiRegistry, AdminRegistry, CliRegistry"
```

---

## Phase 4: Auth System

### Task 6: Auth — User Model, Password Hashing, JWT

**Files:**
- Create: `crates/sushi-core/src/auth/mod.rs`
- Create: `crates/sushi-core/src/auth/model.rs`
- Create: `crates/sushi-core/src/auth/password.rs`
- Create: `crates/sushi-core/src/auth/jwt.rs`
- Create: `crates/sushi-core/src/auth/repository.rs`
- Create: `crates/sushi-core/src/auth/middleware.rs`

- [ ] **Step 1: Write auth/mod.rs**

```rust
// crates/sushi-core/src/auth/mod.rs
pub mod model;
pub mod password;
pub mod jwt;
pub mod repository;
pub mod middleware;
```

- [ ] **Step 2: Write auth/model.rs**

```rust
// crates/sushi-core/src/auth/model.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Editor,
    Viewer,
}

impl Default for UserRole {
    fn default() -> Self { Self::Viewer }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Editor => write!(f, "editor"),
            UserRole::Viewer => write!(f, "viewer"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
}
```

- [ ] **Step 3: Write auth/password.rs with test**

```rust
// crates/sushi-core/src/auth/password.rs
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("hash error: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| format!("parse hash error: {e}"))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let hash = hash_password("secret123").unwrap();
        assert!(verify_password("secret123", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }
}
```

- [ ] **Step 4: Write auth/jwt.rs with test**

```rust
// crates/sushi-core/src/auth/jwt.rs
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // user id
    pub username: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String, // "access" or "refresh"
}

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl: i64,
    refresh_ttl: i64,
}

impl JwtService {
    pub fn new(secret: &str, access_ttl: i64, refresh_ttl: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_ttl,
            refresh_ttl,
        }
    }

    pub fn create_access_token(&self, user_id: i64, username: &str, role: &str) -> Result<String, String> {
        self.create_token(user_id, username, role, self.access_ttl, "access")
    }

    pub fn create_refresh_token(&self, user_id: i64, username: &str, role: &str) -> Result<String, String> {
        self.create_token(user_id, username, role, self.refresh_ttl, "refresh")
    }

    fn create_token(&self, user_id: i64, username: &str, role: &str, ttl: i64, token_type: &str) -> Result<String, String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            role: role.to_string(),
            exp: (now + Duration::seconds(ttl)).timestamp(),
            iat: now.timestamp(),
            token_type: token_type.to_string(),
        };
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| format!("token encode error: {e}"))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, String> {
        let data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| format!("token decode error: {e}"))?;
        Ok(data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_verify_token() {
        let svc = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        let token = svc.create_access_token(1, "admin", "admin").unwrap();
        let claims = svc.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "1");
        assert_eq!(claims.username, "admin");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.token_type, "access");
    }
}
```

- [ ] **Step 5: Write auth/repository.rs**

```rust
// crates/sushi-core/src/auth/repository.rs
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

        let rows = self.storage.query(
            "SELECT * FROM users WHERE username = ?1",
            vec![Value::String(username.to_string())],
        ).await.map_err(|e| e.to_string())?;

        row_to_user(rows.into_iter().next().ok_or("user not found after insert")?)
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
    Ok(User {
        id: row.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        username: row.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        email: row.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        password_hash: row.get("password_hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        role,
        created_at: row.get("created_at").and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(&format!("{s}Z")).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_default(),
        updated_at: row.get("updated_at").and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(&format!("{s}Z")).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_default(),
    })
}
```

- [ ] **Step 6: Write auth/middleware.rs**

```rust
// crates/sushi-core/src/auth/middleware.rs
use crate::auth::jwt::JwtService;
use crate::auth::model::User;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub struct AuthUser(pub User);

#[derive(Clone)]
pub struct AuthState {
    pub jwt_service: Arc<JwtService>,
}

pub async fn require_auth(
    axum::extract::State(state): axum::extract::State<AuthState>,
    mut req: Request,
    next: Next,
) -> Response {
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return (StatusCode::UNAUTHORIZED, "{\"error\":\"Missing authorization header\"}").into_response(),
    };

    match state.jwt_service.verify_token(token) {
        Ok(claims) => {
            let user = User {
                id: claims.sub.parse().unwrap_or(0),
                username: claims.username.clone(),
                email: String::new(),
                password_hash: String::new(),
                role: match claims.role.as_str() {
                    "admin" => crate::auth::model::UserRole::Admin,
                    "editor" => crate::auth::model::UserRole::Editor,
                    _ => crate::auth::model::UserRole::Viewer,
                },
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            req.extensions_mut().insert(AuthUser(user));
            next.run(req).await
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "{\"error\":\"Invalid token\"}").into_response(),
    }
}
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p sushi-core -- auth`
Expected: PASS (password tests + jwt tests)

- [ ] **Step 8: Commit**

```bash
git add crates/sushi-core/src/auth/
git commit -m "feat(core): add auth system — User model, Argon2, JWT, repository, middleware"
```

---

## Phase 5: SushiContext & Lua Integration

### Task 7: SushiContext

**Files:**
- Create: `crates/sushi-core/src/context.rs`

- [ ] **Step 1: Write the implementation**

```rust
// crates/sushi-core/src/context.rs
use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::config::ConfigStore;
use crate::registry::event::EventBus;
use crate::registry::{ApiRegistry, AdminRegistry, CliRegistry};
use crate::storage::sqlite::SqliteStorage;
use std::sync::Arc;

/// The central context passed to all plugins during init.
/// Provides access to all registries and services.
#[derive(Clone)]
pub struct SushiContext {
    pub api: ApiRegistry,
    pub admin: AdminRegistry,
    pub cli: CliRegistry,
    pub config: ConfigStore,
    pub db: Arc<SqliteStorage>,
    pub event: EventBus,
    pub jwt: Arc<JwtService>,
}

impl SushiContext {
    pub fn new(
        config: ConfigStore,
        db: SqliteStorage,
        jwt: JwtService,
    ) -> Self {
        Self {
            api: ApiRegistry::new(),
            admin: AdminRegistry::new(),
            cli: CliRegistry::new(),
            config,
            db: Arc::new(db),
            event: EventBus::new(),
            jwt: Arc::new(jwt),
        }
    }

    pub fn auth_state(&self) -> AuthState {
        AuthState {
            jwt_service: Arc::clone(&self.jwt),
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/sushi-core/src/context.rs
git commit -m "feat(core): add SushiContext"
```

### Task 8: Lua VM, Sandbox & Bindings

**Files:**
- Create: `crates/sushi-core/src/lua/mod.rs`
- Create: `crates/sushi-core/src/lua/vm.rs`
- Create: `crates/sushi-core/src/lua/bindings.rs`
- Create: `crates/sushi-core/src/lua/loader.rs`

- [ ] **Step 1: Write lua/mod.rs**

```rust
// crates/sushi-core/src/lua/mod.rs
pub mod vm;
pub mod bindings;
pub mod loader;
```

- [ ] **Step 2: Write lua/vm.rs — sandboxed Lua VM creation**

```rust
// crates/sushi-core/src/lua/vm.rs
use mlua::Lua;

/// Create a new sandboxed Lua 5.4 VM.
/// Dangerous globals (os.execute, io, etc.) are removed.
pub fn create_sandboxed_vm() -> Result<Lua, mlua::Error> {
    let lua = Lua::new();

    // Remove dangerous globals
    let globals = lua.globals();

    // Nullify os.execute, io library
    let os_table: mlua::Table = globals.get("os")?;
    os_table.set("execute", mlua::Value::Nil)?;
    os_table.set("exit", mlua::Value::Nil)?;
    os_table.set("getenv", mlua::Value::Nil)?;
    os_table.set("remove", mlua::Value::Nil)?;
    os_table.set("rename", mlua::Value::Nil)?;
    os_table.set("tmpname", mlua::Value::Nil)?;

    // Remove io library entirely
    globals.set("io", mlua::Value::Nil)?;

    // Remove package loading (prevent loading native C modules)
    globals.set("package", mlua::Value::Nil)?;
    globals.set("require", mlua::Value::Nil)?;
    globals.set("dofile", mlua::Value::Nil)?;
    globals.set("loadfile", mlua::Value::Nil)?;

    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_blocks_os_execute() {
        let lua = create_sandboxed_vm().unwrap();
        let result = lua.load("os.execute('echo test')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_blocks_io() {
        let lua = create_sandboxed_vm().unwrap();
        let result = lua.load("io.open('test.txt')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_allows_basic_lua() {
        let lua = create_sandboxed_vm().unwrap();
        let result: i32 = lua.load("1 + 2").eval().unwrap();
        assert_eq!(result, 3);
    }
}
```

- [ ] **Step 3: Write lua/bindings.rs — inject sushi.* namespace**

```rust
// crates/sushi-core/src/lua/bindings.rs
use crate::context::SushiContext;
use crate::plugin::Permissions;
use mlua::Lua;

/// Inject the `sushi` global table into the Lua VM.
/// Only namespaces permitted by the plugin's permissions are injected.
pub fn inject_sushi_api(
    lua: &Lua,
    ctx: &SushiContext,
    permissions: &Permissions,
) -> Result<(), mlua::Error> {
    let sushi = lua.create_table()?;

    // sushi.log — always available
    let log_table = lua.create_table()?;
    let log_table_clone = log_table.clone();
    log_table.set("info", lua.create_function(|_, msg: String| {
        tracing::info!("[lua] {msg}");
        Ok(())
    })?)?;
    log_table.set("warn", lua.create_function(|_, msg: String| {
        tracing::warn!("[lua] {msg}");
        Ok(())
    })?)?;
    log_table.set("error", lua.create_function(|_, msg: String| {
        tracing::error!("[lua] {msg}");
        Ok(())
    })?)?;
    sushi.set("log", log_table)?;

    // sushi.api — if routes permitted
    if permissions.routes {
        let api_table = lua.create_table()?;
        // NOTE: actual route registration collects functions into a queue
        // which is then processed by the LuaPlugin loader after init.
        // We store a "pending routes" table that the loader reads.
        let pending = lua.create_table()?;
        sushi.set("__pending_routes", pending.clone())?;

        api_table.set("route", lua.create_function(move |lua, (method, path): (String, String)| {
            let pending: mlua::Table = lua.globals().get::<mlua::Table>("sushi")?.get("__pending_routes")?;
            let entry = lua.create_table()?;
            entry.set("method", method)?;
            entry.set("path", path)?;
            let len = pending.raw_len();
            pending.set(len + 1, entry)?;
            Ok(())
        })?)?;
        sushi.set("api", api_table)?;
    }

    // sushi.cli — if commands permitted
    if permissions.commands {
        let cli_table = lua.create_table()?;
        let pending = lua.create_table()?;
        sushi.set("__pending_commands", pending.clone())?;

        cli_table.set("command", lua.create_function(move |lua, (name, desc): (String, String)| {
            let pending: mlua::Table = lua.globals().get::<mlua::Table>("sushi")?.get("__pending_commands")?;
            let entry = lua.create_table()?;
            entry.set("name", name)?;
            entry.set("description", desc)?;
            let len = pending.raw_len();
            pending.set(len + 1, entry)?;
            Ok(())
        })?)?;
        sushi.set("cli", cli_table)?;
    }

    // sushi.admin — if admin permitted
    if permissions.admin {
        let admin_table = lua.create_table()?;
        let pending = lua.create_table()?;
        sushi.set("__pending_pages", pending.clone())?;

        admin_table.set("page", lua.create_function(move |lua, (path, title): (String, String)| {
            let pending: mlua::Table = lua.globals().get::<mlua::Table>("sushi")?.get("__pending_pages")?;
            let entry = lua.create_table()?;
            entry.set("path", path)?;
            entry.set("title", title)?;
            let len = pending.raw_len();
            pending.set(len + 1, entry)?;
            Ok(())
        })?)?;
        sushi.set("admin", admin_table)?;
    }

    // sushi.config — always available (read-only for now)
    {
        let config_table = lua.create_table()?;
        config_table.set("get", lua.create_function(|_, _key: String| {
            // TODO: implement config get via async bridge
            Ok(mlua::Value::Nil)
        })?)?;
        sushi.set("config", config_table)?;
    }

    // sushi.event — always available
    {
        let event_table = lua.create_table()?;
        event_table.set("on", lua.create_function(|_, (_event, _callback): (String, mlua::Function)| {
            // TODO: wire up event subscription
            Ok(())
        })?)?;
        event_table.set("emit", lua.create_function(|_, (_event, _data): (String, mlua::Value)| {
            // TODO: wire up event emission
            Ok(())
        })?)?;
        sushi.set("event", event_table)?;
    }

    // sushi.auth — always available
    {
        let auth_table = lua.create_table()?;
        auth_table.set("verify_token", lua.create_function(|_, _token: String| {
            // TODO: implement via async bridge
            Ok(mlua::Value::Nil)
        })?)?;
        sushi.set("auth", auth_table)?;
    }

    lua.globals().set("sushi", sushi)?;
    Ok(())
}
```

- [ ] **Step 4: Write lua/loader.rs — directory scanner & LuaPlugin**

```rust
// crates/sushi-core/src/lua/loader.rs
use crate::context::SushiContext;
use crate::lua::bindings::inject_sushi_api;
use crate::lua::vm::create_sandboxed_vm;
use crate::plugin::{Plugin, PluginError, PluginManifest};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tracing;

/// A Lua-based plugin loaded from the filesystem.
pub struct LuaPlugin {
    manifest: PluginManifest,
    lua: mlua::Lua,
    plugin_dir: PathBuf,
}

impl LuaPlugin {
    /// Scan a directory for plugins. Returns one LuaPlugin per subdirectory with a plugin.toml.
    pub async fn scan_dir(dir: &Path) -> Result<Vec<Self>, PluginError> {
        let mut plugins = Vec::new();
        let mut entries = tokio::fs::read_dir(dir).await
            .map_err(|e| PluginError::IoError(e))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| PluginError::IoError(e))?
        {
            let path = entry.path();
            if !path.is_dir() { continue; }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() { continue; }

            let manifest_content = tokio::fs::read_to_string(&manifest_path).await
                .map_err(|e| PluginError::ManifestError(format!("read {}: {e}", manifest_path.display())))?;
            let manifest: PluginManifest = toml::from_str(&manifest_content)
                .map_err(|e| PluginError::ManifestError(format!("parse {}: {e}", manifest_path.display())))?;

            let lua = create_sandboxed_vm()
                .map_err(|e| PluginError::LuaError(format!("create VM for {}: {e}", manifest.plugin.name)))?;

            plugins.push(Self {
                manifest,
                lua,
                plugin_dir: path,
            });
        }

        Ok(plugins)
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

#[async_trait]
impl Plugin for LuaPlugin {
    fn name(&self) -> &str { &self.manifest.plugin.name }
    fn version(&self) -> &str { &self.manifest.plugin.version }

    async fn init(&self, ctx: &SushiContext) -> Result<(), PluginError> {
        // Inject sushi.* API
        inject_sushi_api(&self.lua, ctx, &self.manifest.permissions)
            .map_err(|e| PluginError::LuaError(format!("inject API: {e}")))?;

        // Load and execute the entry script
        let entry_path = self.plugin_dir.join(&self.manifest.plugin.entry);
        let code = tokio::fs::read_to_string(&entry_path).await
            .map_err(|e| PluginError::LuaError(format!("read {}: {e}", entry_path.display())))?;

        self.lua.load(&code).exec()
            .map_err(|e| PluginError::InitFailed(format!("{}: {e}", self.manifest.plugin.name)))?;

        // Call sushi.init() if defined
        let globals = self.lua.globals();
        let sushi: mlua::Table = globals.get("sushi")
            .map_err(|e| PluginError::LuaError(format!("no sushi global: {e}")))?;

        if let Ok(init_fn) = sushi.get::<mlua::Function>("init") {
            init_fn.call::<()>(())
                .map_err(|e| PluginError::InitFailed(format!("{}.init(): {e}", self.manifest.plugin.name)))?;
        }

        tracing::info!("plugin loaded: {} v{}", self.manifest.plugin.name, self.manifest.plugin.version);
        Ok(())
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p sushi-core -- lua::vm::tests`
Expected: PASS (sandbox tests)

- [ ] **Step 6: Commit**

```bash
git add crates/sushi-core/src/lua/ crates/sushi-core/src/context.rs
git commit -m "feat(core): add SushiContext, Lua VM sandbox, bindings, and plugin loader"
```

---

## Phase 6: API Server (sushi-api)

### Task 9: API Router & Auth Routes

**Files:**
- Create: `crates/sushi-api/src/routes/mod.rs`
- Create: `crates/sushi-api/src/routes/auth.rs`
- Create: `crates/sushi-api/src/router.rs`

- [ ] **Step 1: Write routes/mod.rs**

```rust
// crates/sushi-api/src/routes/mod.rs
pub mod auth;
```

- [ ] **Step 2: Write routes/auth.rs**

```rust
// crates/sushi-api/src/routes/auth.rs
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::model::{AuthError, LoginRequest, TokenResponse};
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::storage::sqlite::SqliteStorage;

#[derive(Clone)]
pub struct AuthRouteState {
    pub storage: Arc<SqliteStorage>,
    pub jwt: Arc<JwtService>,
}

pub fn auth_routes(state: AuthRouteState) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/me", get(me))
        .with_state(state)
}

async fn login(
    State(state): State<AuthRouteState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let repo = UserRepository::new(&state.storage);
    match repo.find_by_username(&req.username).await {
        Ok(Some(user)) => {
            if password::verify_password(&req.password, &user.password_hash).unwrap_or(false) {
                match state.jwt.create_access_token(user.id, &user.username, &user.role.to_string())
                    .and_then(|at| {
                        let rt = state.jwt.create_refresh_token(user.id, &user.username, &user.role.to_string())?;
                        Ok(TokenResponse { access_token: at, refresh_token: rt, token_type: "Bearer".to_string() })
                    }) {
                    Ok(tokens) => (StatusCode::OK, Json(json!(tokens))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
                }
            } else {
                (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Invalid credentials" }))).into_response()
            }
        }
        Ok(None) => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Invalid credentials" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh(
    State(state): State<AuthRouteState>,
    Json(req): Json<RefreshRequest>,
) -> impl IntoResponse {
    match state.jwt.verify_token(&req.refresh_token) {
        Ok(claims) if claims.token_type == "refresh" => {
            match state.jwt.create_access_token(
                claims.sub.parse().unwrap_or(0),
                &claims.username,
                &claims.role,
            ) {
                Ok(access_token) => (StatusCode::OK, Json(json!({
                    "access_token": access_token,
                    "token_type": "Bearer"
                }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
            }
        }
        _ => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Invalid refresh token" }))).into_response(),
    }
}

async fn me(
    axum::extract::Extension(user): axum::extract::Extension<sushi_core::auth::middleware::AuthUser>,
) -> impl IntoResponse {
    Json(json!({
        "id": user.0.id,
        "username": user.0.username,
        "role": user.0.role.to_string(),
    }))
}
```

- [ ] **Step 3: Write router.rs — main API router builder**

```rust
// crates/sushi-api/src/router.rs
use axum::{Router, middleware};
use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::middleware::{AuthState, require_auth};
use sushi_core::context::SushiContext;
use sushi_core::registry::RouteEntry;
use std::sync::Arc;

use crate::routes::auth;

pub fn build_api_router(ctx: &SushiContext) -> Router {
    let auth_state = ctx.auth_state();
    let auth_route_state = auth::AuthRouteState {
        storage: Arc::clone(&ctx.db),
        jwt: Arc::clone(&ctx.jwt),
    };

    let mut api = Router::new()
        .nest("/auth", auth::auth_routes(auth_route_state));

    // Register plugin routes (sync read from registry)
    // NOTE: plugin routes are registered as catch-all handlers
    // because Axum routes must be known at router build time.
    // We use a fallback handler that dispatches to Lua.
    api
}

pub fn build_app(ctx: &SushiContext) -> Router {
    let auth_state = ctx.auth_state();

    Router::new()
        .nest("/api", build_api_router(ctx))
        .layer(axum::middleware::from_fn_with_state(auth_state, require_auth))
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p sushi-api`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add crates/sushi-api/src/
git commit -m "feat(api): add API router with auth routes (login, refresh, me)"
```

---

## Phase 7: Admin Server & Frontend

### Task 10: Admin Router & Routes

**Files:**
- Create: `crates/sushi-admin/src/routes/mod.rs`
- Create: `crates/sushi-admin/src/routes/dashboard.rs`
- Create: `crates/sushi-admin/src/routes/plugins.rs`
- Create: `crates/sushi-admin/src/routes/users.rs`
- Create: `crates/sushi-admin/src/routes/config.rs`
- Create: `crates/sushi-admin/src/routes/logs.rs`
- Create: `crates/sushi-admin/src/router.rs`

- [ ] **Step 1: Write routes/mod.rs**

```rust
// crates/sushi-admin/src/routes/mod.rs
pub mod dashboard;
pub mod plugins;
pub mod users;
pub mod config;
pub mod logs;
```

- [ ] **Step 2: Write routes/dashboard.rs**

```rust
// crates/sushi-admin/src/routes/dashboard.rs
use axum::response::Html;

pub async fn dashboard_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Sushi Admin</title>
<script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
<link href="/static/styles.css" rel="stylesheet"></head>
<body class="bg-gray-100 min-h-screen" x-data="adminApp()">
<div class="flex h-screen">
  <!-- Sidebar -->
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 hover:bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 hover:bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 hover:bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 hover:bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
    </div>
  </nav>
  <!-- Main -->
  <main class="flex-1 p-6 overflow-auto">
    <h1 class="text-2xl font-bold mb-6">Dashboard</h1>
    <div class="grid grid-cols-3 gap-4">
      <div class="bg-white p-4 rounded shadow">
        <h3 class="text-gray-500">Plugins Loaded</h3>
        <p class="text-3xl font-bold" x-text="$store.stats.plugins">0</p>
      </div>
      <div class="bg-white p-4 rounded shadow">
        <h3 class="text-gray-500">Total Users</h3>
        <p class="text-3xl font-bold" x-text="$store.stats.users">0</p>
      </div>
      <div class="bg-white p-4 rounded shadow">
        <h3 class="text-gray-500">Uptime</h3>
        <p class="text-3xl font-bold" x-text="$store.stats.uptime">-</p>
      </div>
    </div>
  </main>
</div>
<script>
function adminApp() { return {}; }
</script>
</body></html>"#)
}
```

- [ ] **Step 3: Write routes/plugins.rs**

```rust
// crates/sushi-admin/src/routes/plugins.rs
use axum::{extract::State, response::Html, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use sushi_core::context::SushiContext;

pub async fn plugins_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Plugins — Sushi Admin</title>
<script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
<link href="/static/styles.css" rel="stylesheet"></head>
<body class="bg-gray-100 min-h-screen">
<div class="flex h-screen">
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 hover:bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 hover:bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 hover:bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
    </div>
  </nav>
  <main class="flex-1 p-6 overflow-auto" x-data="pluginsPage()">
    <h1 class="text-2xl font-bold mb-6">Plugins</h1>
    <div class="bg-white rounded shadow">
      <table class="w-full">
        <thead><tr class="bg-gray-50 border-b">
          <th class="px-4 py-2 text-left">Name</th>
          <th class="px-4 py-2 text-left">Version</th>
          <th class="px-4 py-2 text-left">Description</th>
          <th class="px-4 py-2">Status</th>
        </tr></thead>
        <tbody>
          <template x-for="p in plugins" :key="p.name">
            <tr class="border-b hover:bg-gray-50">
              <td class="px-4 py-2" x-text="p.name"></td>
              <td class="px-4 py-2" x-text="p.version"></td>
              <td class="px-4 py-2" x-text="p.description"></td>
              <td class="px-4 py-2 text-center">
                <span class="px-2 py-1 rounded text-sm" :class="p.loaded ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'" x-text="p.loaded ? 'Active' : 'Inactive'"></span>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
  </main>
</div>
<script>
function pluginsPage() {
  return {
    plugins: [],
    async init() {
      const resp = await fetch('/admin/api/plugins');
      this.plugins = await resp.json();
    }
  };
}
</script>
</body></html>"#)
}
```

- [ ] **Step 4: Write routes/users.rs, config.rs, logs.rs (skeletons)**

```rust
// crates/sushi-admin/src/routes/users.rs
use axum::response::Html;

pub async fn users_page() -> Html<&'static str> {
    Html("<html><body><h1>Users Management — TODO</h1></body></html>")
}
```

```rust
// crates/sushi-admin/src/routes/config.rs
use axum::response::Html;

pub async fn config_page() -> Html<&'static str> {
    Html("<html><body><h1>Config — TODO</h1></body></html>")
}
```

```rust
// crates/sushi-admin/src/routes/logs.rs
use axum::response::Html;

pub async fn logs_page() -> Html<&'static str> {
    Html("<html><body><h1>Logs — TODO</h1></body></html>")
}
```

- [ ] **Step 5: Write router.rs**

```rust
// crates/sushi-admin/src/router.rs
use axum::{routing::get, Router};
use sushi_core::context::SushiContext;
use std::sync::Arc;

use crate::routes::{dashboard, plugins, users, config, logs};

pub fn build_admin_router(ctx: &SushiContext) -> Router {
    Router::new()
        .route("/", get(dashboard::dashboard_page))
        .route("/plugins", get(plugins::plugins_page))
        .route("/users", get(users::users_page))
        .route("/config", get(config::config_page))
        .route("/logs", get(logs::logs_page))
        .route("/api/plugins", get(list_plugins_api))
}

async fn list_plugins_api() -> axum::Json<serde_json::Value> {
    // TODO: read from plugin registry
    axum::Json(serde_json::json!([]))
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build -p sushi-admin`
Expected: SUCCESS

- [ ] **Step 7: Commit**

```bash
git add crates/sushi-admin/src/
git commit -m "feat(admin): add admin router with dashboard, plugins, users, config, logs pages"
```

### Task 11: Frontend Setup (Alpine.js + TailwindCSS)

**Files:**
- Create: `ui/package.json`
- Create: `ui/tailwind.config.js`
- Create: `ui/src/styles.css`
- Create: `ui/src/index.html`
- Create: `ui/src/app.js`

- [ ] **Step 1: Write ui/package.json**

```json
{
  "name": "sushi-admin-ui",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "build:css": "npx tailwindcss -i src/styles.css -o dist/styles.css --minify",
    "build": "npm run build:css",
    "dev:css": "npx tailwindcss -i src/styles.css -o dist/styles.css --watch"
  },
  "devDependencies": {
    "tailwindcss": "^3.4.0"
  }
}
```

- [ ] **Step 2: Write ui/tailwind.config.js**

```javascript
/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.html", "./src/**/*.js"],
  theme: { extend: {} },
  plugins: [],
}
```

- [ ] **Step 3: Write ui/src/styles.css**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

- [ ] **Step 4: Commit**

```bash
git add ui/
git commit -m "feat(ui): add Alpine.js + TailwindCSS frontend scaffold"
```

---

## Phase 8: CLI & Binary Entry Point

### Task 12: CLI Commands

**Files:**
- Create: `crates/sushi-cli/src/commands/mod.rs`
- Create: `crates/sushi-cli/src/commands/serve.rs`
- Create: `crates/sushi-cli/src/commands/run.rs`
- Create: `crates/sushi-cli/src/commands/plugin.rs`
- Create: `crates/sushi-cli/src/commands/config_cmd.rs`

- [ ] **Step 1: Write commands/mod.rs**

```rust
// crates/sushi-cli/src/commands/mod.rs
pub mod serve;
pub mod run;
pub mod plugin;
pub mod config_cmd;
```

- [ ] **Step 2: Write commands/serve.rs**

```rust
// crates/sushi-cli/src/commands/serve.rs
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct ServeArgs {
    /// Host to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

    /// Only start API server
    #[arg(long)]
    pub api_only: bool,

    /// Only start Admin server
    #[arg(long)]
    pub admin_only: bool,

    /// Config file path
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Dev mode: serve admin UI from local ui/src/ directory
    #[arg(long)]
    pub admin_dev: bool,
}
```

- [ ] **Step 3: Write commands/run.rs**

```rust
// crates/sushi-cli/src/commands/run.rs
use clap::Args;

#[derive(Args)]
pub struct RunArgs {
    /// Plugin name to run
    pub plugin_name: String,

    /// Arguments to pass to the plugin
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}
```

- [ ] **Step 4: Write commands/plugin.rs**

```rust
// crates/sushi-cli/src/commands/plugin.rs
use clap::Args;

#[derive(Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

#[derive(clap::Subcommand)]
pub enum PluginCommand {
    /// List all discovered plugins
    List,
}
```

- [ ] **Step 5: Write commands/config_cmd.rs**

```rust
// crates/sushi-cli/src/commands/config_cmd.rs
use clap::Args;

#[derive(Args)]
pub struct ConfigCmdArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(clap::Subcommand)]
pub enum ConfigCommand {
    /// Get a config value
    Get { key: String },
    /// Set a config value
    Set { key: String, value: String },
}
```

- [ ] **Step 6: Update lib.rs with clap derive**

```rust
// crates/sushi-cli/src/lib.rs
pub mod commands;

use clap::{Parser, Subcommand};
use commands::{serve::ServeArgs, run::RunArgs, plugin::PluginArgs, config_cmd::ConfigCmdArgs};

#[derive(Parser)]
#[command(name = "sushi", version, about = "Sushi — A modular application platform")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the API and/or Admin server
    Serve(ServeArgs),
    /// Run a single Lua plugin
    Run(RunArgs),
    /// Manage plugins
    Plugin(PluginArgs),
    /// Get or set configuration
    Config(ConfigCmdArgs),
}
```

- [ ] **Step 7: Commit**

```bash
git add crates/sushi-cli/src/
git commit -m "feat(cli): add clap command structure — serve, run, plugin, config"
```

### Task 13: Binary Entry Point (main.rs)

**Files:**
- Modify: `crates/sushi/src/main.rs`

- [ ] **Step 1: Write the full main.rs**

```rust
// crates/sushi/src/main.rs
use anyhow::Result;
use clap::Parser;
use sushi_cli::{Cli, Commands};
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::auth::jwt::JwtService;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use std::path::Path;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => {
            // Load config
            let config = if Path::new(&args.config).exists() {
                ConfigStore::load(&args.config).await?
            } else {
                ConfigStore::new(SushiConfig::default())
            };

            // Init storage
            let config_ref = config.get().await;
            let storage = SqliteStorage::new(&config_ref.database.path).await?;
            drop(config_ref);

            // Run migrations
            let migration_sql = include_str!("../../../../migrations/001_init.sql");
            storage.run_migrations(migration_sql).await?;

            // Init JWT
            let config_ref = config.get().await;
            let jwt = JwtService::new(
                &config_ref.jwt.secret,
                config_ref.jwt.access_ttl,
                config_ref.jwt.refresh_ttl,
            );
            drop(config_ref);

            // Build context
            let ctx = SushiContext::new(config.clone(), storage, jwt);

            // Load plugins
            let plugins_dir = config.get_plugins_dir();
            if Path::new(&plugins_dir).exists() {
                let lua_plugins = LuaPlugin::scan_dir(Path::new(&plugins_dir)).await?;
                for plugin in &lua_plugins {
                    if let Err(e) = plugin.init(&ctx).await {
                        tracing::error!("failed to load plugin {}: {e}", plugin.name());
                    }
                }
                tracing::info!("loaded {} plugins", lua_plugins.len());
            }

            // Build Axum app
            let host = args.host.clone();
            let port = if args.port != 3000 { args.port } else { config.get_server_port() };

            let mut app = axum::Router::new();

            if !args.admin_only {
                app = app.merge(sushi_api::router::build_api_router(&ctx));
            }
            if !args.api_only {
                app = app.merge(sushi_admin::router::build_admin_router(&ctx).nest("/admin", axum::Router::new()));
            }

            let addr = format!("{host}:{port}");
            tracing::info!("sushi serving on {addr}");
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
        }
        Commands::Run(args) => {
            println!("running plugin: {}", args.plugin_name);
            // TODO: init minimal context, find plugin, run it
        }
        Commands::Plugin(args) => {
            match args.command {
                sushi_cli::commands::plugin::PluginCommand::List => {
                    println!("Scanning plugins directory...");
                    // TODO: scan and list
                }
            }
        }
        Commands::Config(args) => {
            match args.command {
                sushi_cli::commands::config_cmd::ConfigCommand::Get { key } => {
                    println!("config get: {key}");
                    // TODO: implement
                }
                sushi_cli::commands::config_cmd::ConfigCommand::Set { key, value } => {
                    println!("config set: {key} = {value}");
                    // TODO: implement
                }
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Verify the full workspace compiles**

Run: `cargo build --workspace`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add crates/sushi/src/main.rs
git commit -m "feat: wire up binary entry point with serve, run, plugin, config commands"
```

---

## Phase 9: Example Plugin & Integration

### Task 14: Example Plugin

**Files:**
- Create: `plugins/_example/plugin.toml`
- Create: `plugins/_example/init.lua`

- [ ] **Step 1: Write plugin.toml**

```toml
[plugin]
name = "example"
version = "0.1.0"
description = "An example Lua plugin for Sushi"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true
database = true
```

- [ ] **Step 2: Write init.lua**

```lua
-- Example Sushi Plugin
-- Demonstrates route, command, and admin page registration

function sushi.init()
    -- Register a hello API route
    sushi.api.route("GET", "/api/hello")
    sushi.log.info("example plugin: registered GET /api/hello")

    -- Register a CLI command
    sushi.cli.command("hello", "Say hello from the example plugin")
    sushi.log.info("example plugin: registered 'hello' CLI command")

    -- Register an admin page
    sushi.admin.page("/admin/example", "Example Plugin")
    sushi.log.info("example plugin: registered admin page /admin/example")
end
```

- [ ] **Step 3: Commit**

```bash
git add plugins/
git commit -m "feat: add example Lua plugin"
```

### Task 15: Default config.toml

**Files:**
- Create: `config.toml`

- [ ] **Step 1: Write config.toml**

```toml
[server]
host = "127.0.0.1"
port = 3000

[database]
path = "data/sushi.db"

[jwt]
secret = "change-me-in-production-at-least-32-chars"
access_ttl = 3600
refresh_ttl = 604800

[plugins]
directory = "plugins"
```

- [ ] **Step 2: Final build check**

Run: `cargo build --workspace`
Expected: SUCCESS

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add config.toml
git commit -m "feat: add default config.toml"
```

---

## Task Summary

| Task | Component | What it delivers |
|------|-----------|-----------------|
| 1 | Workspace | All crate skeletons, compiles |
| 2 | Core | Plugin trait, PluginManifest, error types |
| 3 | Core | SushiConfig, ConfigStore |
| 4 | Core | SQLite storage layer, migrations |
| 5 | Core | EventBus, ApiRegistry, AdminRegistry, CliRegistry |
| 6 | Core | Auth — User model, Argon2, JWT, repository, middleware |
| 7 | Core | SushiContext |
| 8 | Core | Lua VM sandbox, sushi.* bindings, plugin loader |
| 9 | API | Axum router, auth routes (login/refresh/me) |
| 10 | Admin | Admin pages (dashboard, plugins, users, config, logs) |
| 11 | Frontend | Alpine.js + TailwindCSS scaffold |
| 12 | CLI | clap commands (serve, run, plugin, config) |
| 13 | Binary | main.rs wiring everything together |
| 14 | Plugins | Example Lua plugin |
| 15 | Config | Default config.toml + final build check |
