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
    #[serde(default)]
    pub web: WebConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_body_size_limit")]
    pub body_size_limit: usize, // in bytes
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            body_size_limit: default_body_size_limit(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    3000
}
fn default_body_size_limit() -> usize {
    1024 * 64 // 64KB default
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

fn default_db_path() -> String {
    "data/sushi.db".to_string()
}

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

fn default_jwt_secret() -> String {
    // Generate a random secret if not configured
    use std::sync::OnceLock;
    static GENERATED_SECRET: OnceLock<String> = OnceLock::new();

    let secret = GENERATED_SECRET.get_or_init(|| {
        tracing::warn!(
            "JWT secret not configured - using generated secret. \
             Set jwt.secret in config.toml for production use."
        );
        generate_random_secret()
    });
    secret.clone()
}

fn generate_random_secret() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Generate a cryptographically random secret
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    hasher.write_u64(std::process::id() as u64);

    // Create a 32-character hex string (16 bytes of entropy)
    format!("{:032x}", hasher.finish())
}
fn default_access_ttl() -> i64 {
    3600
}
fn default_refresh_ttl() -> i64 {
    604800
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default = "default_plugins_dir")]
    pub directory: String,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            directory: default_plugins_dir(),
        }
    }
}

fn default_plugins_dir() -> String {
    "plugins".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,
    #[serde(default = "default_static_dir")]
    pub static_dir: String,
    #[serde(default = "default_static_url_prefix")]
    pub static_url_prefix: String,
}

fn default_templates_dir() -> String {
    "web/templates".to_string()
}

fn default_static_dir() -> String {
    "web/static".to_string()
}

fn default_static_url_prefix() -> String {
    "/static".to_string()
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            templates_dir: default_templates_dir(),
            static_dir: default_static_dir(),
            static_url_prefix: default_static_url_prefix(),
        }
    }
}

impl Default for SushiConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            jwt: JwtConfig::default(),
            plugins: PluginsConfig::default(),
            web: WebConfig::default(),
        }
    }
}

/// Thread-safe config store.
#[derive(Clone)]
pub struct ConfigStore {
    inner: Arc<RwLock<SushiConfig>>,
}

impl ConfigStore {
    pub fn new(config: SushiConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn load(path: &Path) -> anyhow::Result<Self> {
        // Check if config file exists
        if !path.exists() {
            tracing::warn!(
                "Config file not found at {}, using defaults",
                path.display()
            );
            return Ok(Self::new(SushiConfig::default()));
        }

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| anyhow::anyhow!("failed to read config {}: {e}", path.display()))?;

        let config: SushiConfig =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("failed to parse config: {e}"))?;

        Ok(Self::new(config))
    }

    pub async fn get(&self) -> tokio::sync::RwLockReadGuard<'_, SushiConfig> {
        self.inner.read().await
    }

    pub async fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut SushiConfig),
    {
        let mut guard = self.inner.write().await;
        f(&mut guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
[server]
host = "0.0.0.0"
port = 8080

[database]
path = "data/sushi.db"

[jwt]
secret = "test-secret-key-at-least-32-chars"
access_ttl = 7200
refresh_ttl = 1209600

[plugins]
directory = "plugins"

[web]
templates_dir = "custom/templates"
static_dir = "static/www"
static_url_prefix = "/assets"
"#;
        let config: SushiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.path, "data/sushi.db");
        assert_eq!(config.jwt.access_ttl, 7200);
        assert_eq!(config.jwt.refresh_ttl, 1209600);
        assert_eq!(config.plugins.directory, "plugins");
        assert_eq!(config.web.templates_dir, "custom/templates");
        assert_eq!(config.web.static_dir, "static/www");
        assert_eq!(config.web.static_url_prefix, "/assets");
    }

    #[test]
    fn test_config_defaults() {
        let config = SushiConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.path, "data/sushi.db");
        assert_eq!(config.jwt.access_ttl, 3600);
        assert_eq!(config.jwt.refresh_ttl, 604800);
        assert_eq!(config.plugins.directory, "plugins");
        assert_eq!(config.web.templates_dir, "web/templates");
        assert_eq!(config.web.static_dir, "web/static");
        assert_eq!(config.web.static_url_prefix, "/static");
    }

    #[test]
    fn test_empty_toml_uses_defaults() {
        let config: SushiConfig = toml::from_str("").unwrap();
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.plugins.directory, "plugins");
        assert_eq!(config.web.templates_dir, "web/templates");
        assert_eq!(config.web.static_dir, "web/static");
        assert_eq!(config.web.static_url_prefix, "/static");
    }

    #[tokio::test]
    async fn test_config_store_get() {
        let store = ConfigStore::new(SushiConfig::default());
        let config = store.get().await;
        assert_eq!(config.server.port, 3000);
    }

    #[tokio::test]
    async fn test_config_store_update() {
        let store = ConfigStore::new(SushiConfig::default());
        store.update(|c| c.server.port = 8080).await;
        let config = store.get().await;
        assert_eq!(config.server.port, 8080);
    }
}
