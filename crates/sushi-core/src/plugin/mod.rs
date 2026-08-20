pub mod manager;
mod repository;
pub mod state_repository;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

use crate::context::PluginContext;

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
    pub schema_version: u32,
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
        declared.clamp_to(&self.host_ceiling())
    }

    pub fn host_ceiling(self) -> Permissions {
        match self {
            Self::Official => Permissions {
                routes: true,
                commands: true,
                admin: true,
                database: DatabasePermission::Admin,
            },
            Self::ThirdParty => Permissions {
                routes: true,
                commands: true,
                admin: true,
                database: DatabasePermission::Write,
            },
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
            schema_version: PluginManifest::CURRENT_SCHEMA_VERSION,
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
    #[serde(default)]
    schema_version: u32,
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
}

impl PluginManifest {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    fn from_raw(raw: PluginManifestRaw) -> Self {
        Self {
            schema_version: raw.schema_version,
            plugin: PluginMeta {
                name: raw.plugin.name,
                version: raw.plugin.version,
                description: raw.plugin.description,
                entry: raw.plugin.entry,
            },
            permissions: raw.permissions,
            policies: raw.policies,
            admin: raw.admin,
        }
    }

    pub fn validate_schema(&self) -> Result<(), String> {
        match self.schema_version {
            0 => Err(format!(
                "plugin '{}' is missing required schema_version {}; add `schema_version = {}`",
                self.plugin.name,
                Self::CURRENT_SCHEMA_VERSION,
                Self::CURRENT_SCHEMA_VERSION
            )),
            Self::CURRENT_SCHEMA_VERSION => Ok(()),
            version if version > Self::CURRENT_SCHEMA_VERSION => Err(format!(
                "plugin '{}' uses unsupported manifest schema_version {}; maximum supported version is {}",
                self.plugin.name,
                version,
                Self::CURRENT_SCHEMA_VERSION
            )),
            version => Err(format!(
                "plugin '{}' uses invalid manifest schema_version {}",
                self.plugin.name, version
            )),
        }
    }
}

impl<'de> serde::Deserialize<'de> for PluginManifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = PluginManifestRaw::deserialize(deserializer)?;
        Ok(Self::from_raw(raw))
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

impl Permissions {
    pub fn clamp_to(&self, ceiling: &Permissions) -> Permissions {
        Permissions {
            routes: self.routes && ceiling.routes,
            commands: self.commands && ceiling.commands,
            admin: self.admin && ceiling.admin,
            database: self.database.clone().min(ceiling.database.clone()),
        }
    }

    pub fn clamp_to_grants(&self, grants: &Value) -> Permissions {
        if grants.get("approved").and_then(Value::as_bool) != Some(true) {
            return Permissions::default();
        }
        let mut result = self.clone();
        if let Some(value) = grants.get("routes").and_then(Value::as_bool) {
            result.routes &= value;
        }
        if let Some(value) = grants.get("commands").and_then(Value::as_bool) {
            result.commands &= value;
        }
        if let Some(value) = grants.get("admin").and_then(Value::as_bool) {
            result.admin &= value;
        }
        if let Some(value) = grants.get("database") {
            if let Some(grant) = parse_database_grant(value) {
                result.database = result.database.min(grant);
            }
        }
        result
    }
}

impl DatabasePermission {
    fn rank(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::ReadOnly => 1,
            Self::Write => 2,
            Self::Admin => 3,
        }
    }

    fn min(self, other: Self) -> Self {
        if self.rank() <= other.rank() {
            self
        } else {
            other
        }
    }
}

fn parse_database_grant(value: &Value) -> Option<DatabasePermission> {
    match value {
        Value::Bool(false) => Some(DatabasePermission::None),
        Value::Bool(true) => Some(DatabasePermission::ReadOnly),
        Value::String(value) => match value.as_str() {
            "none" | "false" => Some(DatabasePermission::None),
            "read" | "readonly" | "true" => Some(DatabasePermission::ReadOnly),
            "write" => Some(DatabasePermission::Write),
            "admin" => Some(DatabasePermission::Admin),
            _ => None,
        },
        _ => None,
    }
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

    /// Activate the plugin with a capability-scoped context.
    async fn activate(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
}

/// A simple function-based plugin that takes a closure for init.
/// Useful for Rust built-in plugins that don't need a full struct.
pub struct FnPlugin {
    name: String,
    version: String,
    init_fn: Box<dyn Fn(&PluginContext) -> Result<(), PluginError> + Send + Sync>,
}

impl FnPlugin {
    pub fn new<F>(name: impl Into<String>, version: impl Into<String>, init_fn: F) -> Self
    where
        F: Fn(&PluginContext) -> Result<(), PluginError> + Send + Sync + 'static,
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

    async fn activate(&self, ctx: &PluginContext) -> Result<(), PluginError> {
        (self.init_fn)(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_permissions_do_not_escalate_declared_capabilities() {
        let declared = Permissions {
            routes: true,
            commands: false,
            admin: false,
            database: DatabasePermission::ReadOnly,
        };

        assert_eq!(
            PluginKind::Official.effective_permissions(&declared),
            declared
        );
    }

    #[test]
    fn third_party_permissions_are_capped_at_write_database_access() {
        let declared = Permissions {
            routes: true,
            commands: true,
            admin: true,
            database: DatabasePermission::Admin,
        };

        let effective = PluginKind::ThirdParty.effective_permissions(&declared);
        assert_eq!(effective.database, DatabasePermission::Write);
        assert!(effective.routes && effective.commands && effective.admin);
    }

    #[test]
    fn profile_grants_can_only_reduce_requested_permissions() {
        let requested = Permissions {
            routes: true,
            commands: true,
            admin: true,
            database: DatabasePermission::Admin,
        };

        let effective = requested.clamp_to_grants(&serde_json::json!({
            "approved": true,
            "routes": false,
            "admin": false,
            "database": "read"
        }));

        assert!(!effective.routes);
        assert!(effective.commands);
        assert!(!effective.admin);
        assert_eq!(effective.database, DatabasePermission::ReadOnly);
    }

    #[test]
    fn profile_grants_require_explicit_administrator_approval() {
        let requested = Permissions {
            routes: true,
            commands: true,
            admin: true,
            database: DatabasePermission::Admin,
        };

        assert_eq!(
            requested.clamp_to_grants(&serde_json::json!({ "database": "admin" })),
            Permissions::default()
        );
    }

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
        assert_eq!(manifest.schema_version, 0);
        assert!(manifest.validate_schema().is_err());
    }

    #[test]
    fn manifest_schema_rejects_future_versions() {
        let toml_str = r#"
schema_version = 2

[plugin]
name = "future_plugin"
version = "0.1.0"
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let error = manifest
            .validate_schema()
            .expect_err("future manifest schemas must fail closed");
        assert!(error.contains("unsupported manifest schema_version 2"));
    }

    #[test]
    fn manifest_schema_accepts_current_version() {
        let toml_str = r#"
schema_version = 1

[plugin]
name = "current_plugin"
version = "0.1.0"
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(
            manifest.schema_version,
            PluginManifest::CURRENT_SCHEMA_VERSION
        );
        manifest
            .validate_schema()
            .expect("current schema is supported");
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

    #[test]
    fn test_parse_plugin_manifest_admin_asset_bundles() {
        let toml_str = r#"
[plugin]
name = "admin_assets"
version = "0.1.0"
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
    fn manifest_unknown_metadata_is_ignored() {
        let toml_str = r#"
[plugin]
name = "official_plugin"
version = "0.1.0"
host_trust = "official"
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "official_plugin");
    }

    #[test]
    fn manifest_has_no_trust_tier_field() {
        let toml_str = r#"
[plugin]
name = "host_selected_tier"
version = "0.1.0"
"#;

        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "host_selected_tier");
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
        assert_eq!(official, declared);

        let third_party = PluginKind::ThirdParty.effective_permissions(&declared);
        assert_eq!(third_party, declared);
    }

    #[test]
    fn product_specific_manifest_config_is_ignored() {
        let toml_str = r#"
[plugin]
name = "file_browser_plugin"
version = "0.1.0"

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
        assert_eq!(manifest.plugin.name, "file_browser_plugin");
    }
}
