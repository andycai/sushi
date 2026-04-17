pub mod manager;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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
    pub file_browser: Option<PluginFileBrowserConfig>,
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

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct PluginFileBrowserConfig {
    #[serde(default = "default_file_browser_route_prefix")]
    pub route_prefix: String,
    #[serde(default = "default_true")]
    pub hide_dotfiles: bool,
    #[serde(default = "default_true")]
    pub deny_symlink: bool,
    #[serde(default)]
    pub text_extensions: Vec<String>,
    #[serde(default)]
    pub roots: Vec<PluginFileBrowserRoot>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct PluginFileBrowserRoot {
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub path: String,
    #[serde(default)]
    pub capabilities: PluginFileBrowserCapabilities,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct PluginFileBrowserCapabilities {
    #[serde(default)]
    pub can_list: bool,
    #[serde(default)]
    pub can_view_text: bool,
    #[serde(default)]
    pub can_edit_text: bool,
    #[serde(default)]
    pub can_create_text: bool,
    #[serde(default)]
    pub can_create_dir: bool,
    #[serde(default)]
    pub can_rename: bool,
    #[serde(default)]
    pub can_delete: bool,
    #[serde(default)]
    pub can_upload: bool,
    #[serde(default)]
    pub can_download: bool,
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
            file_browser: None,
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
    #[serde(default)]
    file_browser: Option<PluginFileBrowserConfig>,
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
                file_browser: raw.file_browser,
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

fn default_file_browser_route_prefix() -> String {
    "/app/files".to_string()
}

fn default_true() -> bool {
    true
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
        assert!(manifest.file_browser.is_none());
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

    #[test]
    fn parse_file_browser_config_from_manifest() {
        let toml_str = r#"
[plugin]
name = "file_browser_plugin"
version = "0.1.0"
kind = "official"

[file_browser]
route_prefix = "/app/files"
hide_dotfiles = true
deny_symlink = true
text_extensions = ["txt", "md", "json"]

[[file_browser.roots]]
id = "workspace"
title = "Workspace"
path = "/tmp"

[file_browser.roots.capabilities]
can_list = true
can_view_text = true
can_edit_text = true
can_create_text = true
can_create_dir = true
can_rename = true
can_delete = true
can_upload = true
can_download = false

[[file_browser.roots]]
id = "logs"
title = "Logs"
path = "/var/log"

[file_browser.roots.capabilities]
can_list = true
can_view_text = true
can_download = true
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let cfg = manifest.file_browser.expect("expected file_browser config");
        assert_eq!(cfg.route_prefix, "/app/files");
        assert!(cfg.hide_dotfiles);
        assert!(cfg.deny_symlink);
        assert_eq!(
            cfg.text_extensions,
            vec!["txt".to_string(), "md".to_string(), "json".to_string()]
        );
        assert_eq!(cfg.roots.len(), 2);
        assert_eq!(cfg.roots[0].id, "workspace");
        assert_eq!(cfg.roots[0].title, "Workspace");
        assert_eq!(cfg.roots[0].path, "/tmp");
        assert!(cfg.roots[0].capabilities.can_list);
        assert!(cfg.roots[0].capabilities.can_edit_text);
        assert!(cfg.roots[0].capabilities.can_upload);
        assert!(!cfg.roots[0].capabilities.can_download);
        assert_eq!(cfg.roots[1].id, "logs");
        assert_eq!(cfg.roots[1].title, "Logs");
        assert_eq!(cfg.roots[1].path, "/var/log");
        assert!(cfg.roots[1].capabilities.can_list);
        assert!(cfg.roots[1].capabilities.can_view_text);
        assert!(!cfg.roots[1].capabilities.can_edit_text);
        assert!(cfg.roots[1].capabilities.can_download);
    }
}
