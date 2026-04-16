pub mod manager;

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::BTreeMap;
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
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub permissions: Permissions,
    pub policies: PluginPoliciesConfig,
    pub admin: Option<PluginAdminConfig>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    Official,
    ThirdParty,
}

impl PluginKind {
    pub fn tier_name(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::ThirdParty => "third_party",
        }
    }

    pub fn effective_permissions(self, declared: &Permissions) -> Permissions {
        match self {
            Self::Official => Permissions {
                routes: true,
                commands: true,
                admin: true,
                database: DatabasePermission::Admin,
            },
            Self::ThirdParty => declared.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct PluginAdminConfig {
    #[serde(default)]
    pub assets: Option<PluginAdminAssetsConfig>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct PluginPoliciesConfig {
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct PluginAdminAssetsConfig {
    #[serde(default)]
    pub bundles: BTreeMap<String, PluginAssetBundle>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct PluginAssetBundle {
    #[serde(default)]
    pub js: Vec<String>,
    #[serde(default)]
    pub css: Vec<String>,
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
            policies: PluginPoliciesConfig::default(),
            admin: None,
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

#[derive(Debug, Deserialize)]
struct PluginManifestRaw {
    plugin: PluginMetaRaw,
    #[serde(default)]
    permissions: Permissions,
    #[serde(default)]
    policies: PluginPoliciesConfig,
    #[serde(default)]
    admin: Option<PluginAdminConfig>,
}

#[derive(Debug, Deserialize)]
struct PluginMetaRaw {
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_entry")]
    entry: String,
    kind: PluginKind,
}

impl PluginManifest {
    fn from_raw(raw: PluginManifestRaw) -> (Self, PluginKind) {
        (
            Self {
                plugin: PluginMeta {
                    name: raw.plugin.name,
                    version: raw.plugin.version,
                    description: raw.plugin.description,
                    entry: raw.plugin.entry,
                },
                permissions: raw.permissions,
                policies: raw.policies,
                admin: raw.admin,
            },
            raw.plugin.kind,
        )
    }

    pub fn parse_with_kind(input: &str) -> Result<(Self, PluginKind), toml::de::Error> {
        let raw: PluginManifestRaw = toml::from_str(input)?;
        Ok(Self::from_raw(raw))
    }
}

impl<'de> serde::Deserialize<'de> for PluginManifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = PluginManifestRaw::deserialize(deserializer)?;
        Ok(Self::from_raw(raw).0)
    }
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
kind = "official"
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
        assert!(manifest.policies.scopes.is_empty());
        assert!(manifest.admin.is_none());
    }

    #[test]
    fn test_database_permission_bool_true() {
        let toml_str = r#"
[plugin]
name = "test"
version = "0.1.0"
kind = "third_party"

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
kind = "third_party"

[permissions]
database = false
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.permissions.database, DatabasePermission::None);
    }

    #[test]
    fn test_parse_plugin_manifest_admin_asset_bundles() {
        let toml_str = r#"
[plugin]
name = "admin_assets"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["pages/workspace.js", "vendor/charts.js"]
css = ["pages/workspace.css"]
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "admin_assets");
        assert!(manifest.permissions.admin);
        let admin = manifest.admin.expect("expected admin config");
        let assets = admin.assets.expect("expected admin assets config");
        let workspace = assets
            .bundles
            .get("workspace")
            .expect("expected workspace bundle");
        assert_eq!(
            workspace.js,
            vec![
                "pages/workspace.js".to_string(),
                "vendor/charts.js".to_string()
            ]
        );
        assert_eq!(workspace.css, vec!["pages/workspace.css".to_string()]);
    }

    #[test]
    fn parse_plugin_policy_scopes_from_manifest() {
        let toml_str = r#"
[plugin]
name = "policy_scopes"
version = "0.1.0"
kind = "third_party"

[policies]
scopes = ["admin.users", "api.reports"]
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(
            manifest.policies.scopes,
            vec!["admin.users".to_string(), "api.reports".to_string()]
        );
    }

    #[test]
    fn test_parse_plugin_manifest_kind() {
        let toml_str = r#"
[plugin]
name = "official_plugin"
version = "0.1.0"
kind = "official"
"#;

        let (manifest, kind) = PluginManifest::parse_with_kind(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "official_plugin");
        assert_eq!(kind, PluginKind::Official);
    }

    #[test]
    fn test_parse_plugin_manifest_requires_kind() {
        let toml_str = r#"
[plugin]
name = "missing_kind"
version = "0.1.0"
"#;

        let result = PluginManifest::parse_with_kind(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_kind_effective_permissions() {
        let declared = Permissions {
            routes: false,
            commands: false,
            admin: false,
            database: DatabasePermission::ReadOnly,
        };

        let official = PluginKind::Official.effective_permissions(&declared);
        assert_eq!(
            official,
            Permissions {
                routes: true,
                commands: true,
                admin: true,
                database: DatabasePermission::Admin,
            }
        );

        let third_party = PluginKind::ThirdParty.effective_permissions(&declared);
        assert_eq!(third_party, declared);
    }
}
