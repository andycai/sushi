use async_trait::async_trait;
use serde::Deserialize;
use thiserror::Error;

use crate::context::SushiContext;

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
    #[serde(default)]
    pub permissions: Permissions,
}

impl Default for PluginManifest {
    fn default() -> Self {
        Self {
            plugin: PluginMeta {
                name: String::new(),
                version: String::new(),
                description: String::new(),
                entry: "init.lua".to_string(),
            },
            permissions: Permissions::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_entry")]
    pub entry: String,
}

fn default_entry() -> String {
    "init.lua".to_string()
}

/// Plugin permission levels.
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
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

#[derive(Debug, Clone, Default, PartialEq)]
pub enum DatabasePermission {
    #[default]
    None,
    ReadOnly,
    Write,
    Admin,
}

// Custom deserialize for DatabasePermission to handle bool and string variants
impl<'de> serde::Deserialize<'de> for DatabasePermission {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct DbPermVisitor;

        impl<'de> Visitor<'de> for DbPermVisitor {
            type Value = DatabasePermission;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("false, true, \"read\", \"write\", or \"admin\"")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(if v {
                    DatabasePermission::ReadOnly
                } else {
                    DatabasePermission::None
                })
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                match v {
                    "read" | "true" => Ok(DatabasePermission::ReadOnly),
                    "write" => Ok(DatabasePermission::Write),
                    "admin" => Ok(DatabasePermission::Admin),
                    other => Err(de::Error::custom(format!("unknown db permission: {other}"))),
                }
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(DatabasePermission::None)
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

    /// Initialise the plugin with access to the global SushiContext.
    async fn init(&self, _ctx: &SushiContext) -> Result<(), PluginError> {
        Ok(())
    }
}

/// A simple function-based plugin that takes a closure for init.
/// Useful for Rust built-in plugins that don't need a full struct.
pub struct FnPlugin {
    name: String,
    version: String,
    init_fn: Box<dyn Fn(&SushiContext) -> Result<(), PluginError> + Send + Sync>,
}

impl FnPlugin {
    pub fn new<F>(name: impl Into<String>, version: impl Into<String>, init_fn: F) -> Self
    where
        F: Fn(&SushiContext) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            version: version.into(),
            init_fn: Box::new(init_fn),
        }
    }
}

#[async_trait]
impl Plugin for FnPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }

    async fn init(&self, ctx: &SushiContext) -> Result<(), PluginError> {
        (self.init_fn)(ctx)
    }
}

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
        assert_eq!(manifest.permissions.database, DatabasePermission::Write);
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::InitFailed("lua error".to_string());
        assert_eq!(err.to_string(), "plugin init failed: lua error");
    }

    #[test]
    fn test_default_manifest() {
        let manifest = PluginManifest::default();
        assert_eq!(manifest.plugin.entry, "init.lua");
        assert_eq!(manifest.permissions.database, DatabasePermission::None);
    }

    #[test]
    fn test_database_permission_bool_true() {
        let toml_str = r#"
[plugin]
name = "test"
version = "0.1.0"

[permissions]
database = true
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.permissions.database, DatabasePermission::ReadOnly);
    }

    #[test]
    fn test_database_permission_false() {
        let toml_str = r#"
[plugin]
name = "test"
version = "0.1.0"

[permissions]
database = false
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.permissions.database, DatabasePermission::None);
    }
}
