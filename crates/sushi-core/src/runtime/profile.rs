use super::PluginInstanceId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;
const DEFAULT_PROFILE: &str = "default";
const LEGACY_PROFILE: &str = "legacy-default";
const DEFAULT_BUILTINS: [&str; 10] = [
    "host-core",
    "host-cli",
    "policy",
    "identity",
    "api-core",
    "admin-shell",
    "host-admin",
    "governance",
    "rbac-admin",
    "menu-admin",
];

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile '{name}' was not found at {path}")]
    ProfileNotFound { name: String, path: PathBuf },
    #[error("bundle '{name}' was not found at {path}")]
    BundleNotFound { name: String, path: PathBuf },
    #[error("failed to read {kind} {path}: {source}")]
    Read {
        kind: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {kind} {path}: {source}")]
    Parse {
        kind: &'static str,
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("unsupported schema_version {actual} in {kind} {name}; expected 1")]
    UnsupportedSchema {
        kind: &'static str,
        name: String,
        actual: u32,
    },
    #[error("{kind} name '{declared}' does not match requested name '{requested}'")]
    NameMismatch {
        kind: &'static str,
        requested: String,
        declared: String,
    },
    #[error("invalid {kind} name '{name}'")]
    InvalidDocumentName { kind: &'static str, name: String },
    #[error("duplicate runtime entry id '{id}' from {first_origin} and {second_origin}")]
    DuplicateEntryId {
        id: String,
        first_origin: String,
        second_origin: String,
    },
    #[error("profile overlay targets unknown runtime entry '{id}'")]
    UnknownOverlayTarget { id: String },
    #[error("unknown plugin source '{source_ref}' for runtime entry '{id}'")]
    UnknownSource { id: String, source_ref: String },
    #[error("unknown builtin plugin factory '{key}' for runtime entry '{id}'")]
    UnknownBuiltin { id: String, key: String },
    #[error("invalid Lua plugin source '{source_ref}' for runtime entry '{id}': {reason}")]
    InvalidLuaSource {
        id: String,
        source_ref: String,
        reason: String,
    },
    #[error("Lua plugin source '{source_ref}' for runtime entry '{id}' was not found at {path}")]
    LuaSourceNotFound {
        id: String,
        source_ref: String,
        path: PathBuf,
    },
    #[error("required runtime entry '{id}' cannot be disabled")]
    RequiredEntryDisabled { id: String },
    #[error("Lua plugin source '{source_ref}' is mounted by both '{first_id}' and '{second_id}'")]
    DuplicateLuaSource {
        source_ref: String,
        first_id: String,
        second_id: String,
    },
    #[error("invalid runtime entry id '{id}': {reason}")]
    InvalidEntryId { id: String, reason: String },
    #[error("runtime entry '{id}' has invalid {field}: {source}")]
    InvalidValue {
        id: String,
        field: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to inspect legacy plugin directory {path}: {source}")]
    LegacyDiscovery {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize resolved profile: {0}")]
    Dump(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePluginSource {
    Builtin {
        key: String,
        reference: String,
    },
    Lua {
        path_id: String,
        path: PathBuf,
        reference: String,
    },
}

impl RuntimePluginSource {
    pub fn reference(&self) -> &str {
        match self {
            Self::Builtin { reference, .. } | Self::Lua { reference, .. } => reference,
        }
    }

    pub fn resolved_path(&self) -> Option<&Path> {
        match self {
            Self::Builtin { .. } => None,
            Self::Lua { path, .. } => Some(path),
        }
    }

    pub fn lua_path_id(&self) -> Option<&str> {
        match self {
            Self::Builtin { .. } => None,
            Self::Lua { path_id, .. } => Some(path_id),
        }
    }

    pub fn builtin_key(&self) -> Option<&str> {
        match self {
            Self::Builtin { key, .. } => Some(key),
            Self::Lua { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRuntimeEntry {
    pub id: PluginInstanceId,
    pub source: RuntimePluginSource,
    pub enabled: bool,
    pub required: bool,
    pub config: Value,
    pub grants: Value,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRuntimeProfile {
    name: String,
    entries: Vec<ResolvedRuntimeEntry>,
    legacy: bool,
}

impl ResolvedRuntimeProfile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entries(&self) -> &[ResolvedRuntimeEntry] {
        &self.entries
    }

    pub fn is_legacy(&self) -> bool {
        self.legacy
    }

    pub fn has_enabled_builtin(&self, key: &str) -> bool {
        self.entries.iter().any(|entry| {
            entry.enabled
                && matches!(
                    &entry.source,
                    RuntimePluginSource::Builtin { key: entry_key, .. } if entry_key == key
                )
        })
    }

    pub fn dump_json(&self) -> Result<String, ProfileError> {
        let dump = ProfileDump {
            schema_version: SCHEMA_VERSION,
            name: &self.name,
            legacy: self.legacy,
            entries: self
                .entries
                .iter()
                .map(|entry| ProfileEntryDump {
                    id: entry.id.as_str(),
                    source: entry.source.reference(),
                    enabled: entry.enabled,
                    required: entry.required,
                    config: &entry.config,
                    grants: &entry.grants,
                    origin: &entry.origin,
                })
                .collect(),
        };
        Ok(serde_json::to_string_pretty(&dump)?)
    }

    pub fn legacy_empty() -> Self {
        Self {
            name: LEGACY_PROFILE.to_string(),
            entries: Vec::new(),
            legacy: true,
        }
    }
}

#[derive(Debug, Serialize)]
struct ProfileDump<'a> {
    schema_version: u32,
    name: &'a str,
    legacy: bool,
    entries: Vec<ProfileEntryDump<'a>>,
}

#[derive(Debug, Serialize)]
struct ProfileEntryDump<'a> {
    id: &'a str,
    source: &'a str,
    enabled: bool,
    required: bool,
    config: &'a Value,
    grants: &'a Value,
    origin: &'a str,
}

#[derive(Debug, Clone)]
pub struct RuntimeProfileResolver {
    profiles_dir: PathBuf,
    bundles_dir: PathBuf,
    plugins_dir: PathBuf,
    builtins: BTreeSet<String>,
}

impl RuntimeProfileResolver {
    pub fn new(
        profiles_dir: impl Into<PathBuf>,
        bundles_dir: impl Into<PathBuf>,
        plugins_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            profiles_dir: profiles_dir.into(),
            bundles_dir: bundles_dir.into(),
            plugins_dir: plugins_dir.into(),
            builtins: DEFAULT_BUILTINS.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn with_builtins(mut self, builtins: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.builtins = builtins.into_iter().map(Into::into).collect();
        self
    }

    pub fn resolve_configured(
        &self,
        configured_profile: Option<&str>,
    ) -> Result<ResolvedRuntimeProfile, ProfileError> {
        match configured_profile
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            Some(name) => self.resolve(name),
            None if self.profile_path(DEFAULT_PROFILE).is_file() => self.resolve(DEFAULT_PROFILE),
            None => self.resolve_legacy_default(),
        }
    }

    pub fn resolve(&self, profile_name: &str) -> Result<ResolvedRuntimeProfile, ProfileError> {
        validate_document_name("profile", profile_name)?;
        let profile_path = self.profile_path(profile_name);
        if !profile_path.is_file() {
            return Err(ProfileError::ProfileNotFound {
                name: profile_name.to_string(),
                path: profile_path,
            });
        }
        let profile: ProfileDocument = read_document("profile", &profile_path)?;
        validate_document(
            "profile",
            profile_name,
            profile.schema_version,
            &profile.name,
        )?;

        let mut entries = Vec::new();
        let mut indexes = BTreeMap::new();
        let mut origins: BTreeMap<String, String> = BTreeMap::new();
        for bundle_name in &profile.bundles {
            validate_document_name("bundle", bundle_name)?;
            let bundle_path = self.bundle_path(bundle_name);
            if !bundle_path.is_file() {
                return Err(ProfileError::BundleNotFound {
                    name: bundle_name.clone(),
                    path: bundle_path,
                });
            }
            let bundle: BundleDocument = read_document("bundle", &bundle_path)?;
            validate_document("bundle", bundle_name, bundle.schema_version, &bundle.name)?;
            let origin = format!("bundle:{bundle_name}");
            for raw_entry in bundle.entries {
                let entry = self.resolve_entry(raw_entry, &origin)?;
                let id = entry.id.as_str().to_string();
                if let Some(first_origin) = origins.get(&id) {
                    return Err(ProfileError::DuplicateEntryId {
                        id,
                        first_origin: first_origin.clone(),
                        second_origin: origin.clone(),
                    });
                }
                origins.insert(id.clone(), origin.clone());
                indexes.insert(id, entries.len());
                entries.push(entry);
            }
        }

        let overlay_origin = format!("profile:{}", profile.name);
        let mut overlay_ids = BTreeSet::new();
        for raw_overlay in profile.overlays {
            let id = PluginInstanceId::new(raw_overlay.id.clone())
                .map_err(|reason| ProfileError::InvalidEntryId {
                    id: raw_overlay.id.clone(),
                    reason,
                })?
                .as_str()
                .to_string();
            if !overlay_ids.insert(id.clone()) {
                return Err(ProfileError::DuplicateEntryId {
                    id,
                    first_origin: overlay_origin.clone(),
                    second_origin: overlay_origin.clone(),
                });
            }
            let Some(index) = indexes.get(&id).copied() else {
                return Err(ProfileError::UnknownOverlayTarget { id });
            };
            let overlay = self.resolve_entry(raw_overlay, &overlay_origin)?;
            entries[index] = overlay;
            origins.insert(id, overlay_origin.clone());
        }

        validate_unique_lua_sources(&entries)?;

        Ok(ResolvedRuntimeProfile {
            name: profile.name,
            entries,
            legacy: false,
        })
    }

    fn resolve_legacy_default(&self) -> Result<ResolvedRuntimeProfile, ProfileError> {
        let mut entries = Vec::new();
        for (id, key) in [
            ("host.core", "host-core"),
            ("host.cli", "host-cli"),
            ("policy.core", "policy"),
            ("identity.core", "identity"),
            ("api.core", "api-core"),
            ("admin.shell", "admin-shell"),
            ("host.admin", "host-admin"),
            ("governance.admin", "governance"),
            ("rbac.admin", "rbac-admin"),
            ("menu.admin", "menu-admin"),
        ] {
            entries.push(self.resolve_entry(
                EntryDocument {
                    id: id.to_string(),
                    source: format!("builtin:{key}"),
                    enabled: true,
                    required: true,
                    config: toml::Value::Table(Default::default()),
                    grants: toml::Value::Table(Default::default()),
                },
                "legacy-default",
            )?);
        }
        let mut path_ids = Vec::new();
        for tier in ["official", "third_party"] {
            let tier_dir = self.plugins_dir.join(tier);
            if !tier_dir.exists() {
                continue;
            }
            let entries =
                fs::read_dir(&tier_dir).map_err(|source| ProfileError::LegacyDiscovery {
                    path: tier_dir.clone(),
                    source,
                })?;
            for entry in entries {
                let entry = entry.map_err(|source| ProfileError::LegacyDiscovery {
                    path: tier_dir.clone(),
                    source,
                })?;
                let path = entry.path();
                if path.is_dir() && path.join("plugin.toml").is_file() {
                    path_ids.push(format!("{tier}/{}", entry.file_name().to_string_lossy()));
                }
            }
        }
        path_ids.sort();

        for path_id in path_ids {
            let id = format!("legacy.{}", path_id.replace(['/', '_'], "."));
            let grants = if path_id.starts_with("official/") {
                toml::Value::Table(toml::Table::from_iter([(
                    "database".to_string(),
                    toml::Value::String("admin".to_string()),
                )]))
            } else {
                toml::Value::Table(Default::default())
            };
            entries.push(self.resolve_entry(
                EntryDocument {
                    id,
                    source: format!("lua:{path_id}"),
                    enabled: true,
                    required: false,
                    config: toml::Value::Table(Default::default()),
                    grants,
                },
                "legacy-discovery",
            )?);
        }
        Ok(ResolvedRuntimeProfile {
            name: LEGACY_PROFILE.to_string(),
            entries,
            legacy: true,
        })
    }

    fn resolve_entry(
        &self,
        raw: EntryDocument,
        origin: &str,
    ) -> Result<ResolvedRuntimeEntry, ProfileError> {
        let id = PluginInstanceId::new(raw.id.clone()).map_err(|reason| {
            ProfileError::InvalidEntryId {
                id: raw.id.clone(),
                reason,
            }
        })?;
        let source = self.resolve_source(id.as_str(), &raw.source)?;
        if raw.required && !raw.enabled {
            return Err(ProfileError::RequiredEntryDisabled {
                id: id.as_str().to_string(),
            });
        }
        let config =
            serde_json::to_value(raw.config).map_err(|source| ProfileError::InvalidValue {
                id: id.as_str().to_string(),
                field: "config",
                source,
            })?;
        let grants =
            serde_json::to_value(raw.grants).map_err(|source| ProfileError::InvalidValue {
                id: id.as_str().to_string(),
                field: "grants",
                source,
            })?;
        Ok(ResolvedRuntimeEntry {
            id,
            source,
            enabled: raw.enabled,
            required: raw.required,
            config,
            grants,
            origin: origin.to_string(),
        })
    }

    fn resolve_source(
        &self,
        entry_id: &str,
        source: &str,
    ) -> Result<RuntimePluginSource, ProfileError> {
        if let Some(key) = source.strip_prefix("builtin:") {
            if key.is_empty() || !self.builtins.contains(key) {
                return Err(ProfileError::UnknownBuiltin {
                    id: entry_id.to_string(),
                    key: key.to_string(),
                });
            }
            return Ok(RuntimePluginSource::Builtin {
                key: key.to_string(),
                reference: source.to_string(),
            });
        }
        let Some(path_id) = source.strip_prefix("lua:") else {
            return Err(ProfileError::UnknownSource {
                id: entry_id.to_string(),
                source_ref: source.to_string(),
            });
        };
        validate_lua_path_id(entry_id, source, path_id)?;
        let path = self.plugins_dir.join(path_id);
        if !path.join("plugin.toml").is_file() {
            return Err(ProfileError::LuaSourceNotFound {
                id: entry_id.to_string(),
                source_ref: source.to_string(),
                path,
            });
        }
        Ok(RuntimePluginSource::Lua {
            path_id: path_id.to_string(),
            path,
            reference: source.to_string(),
        })
    }

    fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(format!("{name}.toml"))
    }

    fn bundle_path(&self, name: &str) -> PathBuf {
        self.bundles_dir.join(format!("{name}.toml"))
    }
}

fn validate_unique_lua_sources(entries: &[ResolvedRuntimeEntry]) -> Result<(), ProfileError> {
    let mut owners = BTreeMap::new();
    for entry in entries {
        let RuntimePluginSource::Lua { path_id, .. } = &entry.source else {
            continue;
        };
        if let Some(first_id) = owners.insert(path_id.clone(), entry.id.as_str().to_string()) {
            return Err(ProfileError::DuplicateLuaSource {
                source_ref: format!("lua:{path_id}"),
                first_id,
                second_id: entry.id.as_str().to_string(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ProfileDocument {
    schema_version: u32,
    name: String,
    #[serde(default)]
    bundles: Vec<String>,
    #[serde(default)]
    overlays: Vec<EntryDocument>,
}

#[derive(Debug, Deserialize)]
struct BundleDocument {
    schema_version: u32,
    name: String,
    #[serde(default)]
    entries: Vec<EntryDocument>,
}

#[derive(Debug, Deserialize)]
struct EntryDocument {
    id: String,
    source: String,
    enabled: bool,
    required: bool,
    #[serde(default = "empty_toml_table")]
    config: toml::Value,
    #[serde(default = "empty_toml_table")]
    grants: toml::Value,
}

fn empty_toml_table() -> toml::Value {
    toml::Value::Table(Default::default())
}

fn read_document<T: for<'de> Deserialize<'de>>(
    kind: &'static str,
    path: &Path,
) -> Result<T, ProfileError> {
    let content = fs::read_to_string(path).map_err(|source| ProfileError::Read {
        kind,
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&content).map_err(|source| ProfileError::Parse {
        kind,
        path: path.to_path_buf(),
        source,
    })
}

fn validate_document(
    kind: &'static str,
    requested: &str,
    schema_version: u32,
    declared: &str,
) -> Result<(), ProfileError> {
    if schema_version != SCHEMA_VERSION {
        return Err(ProfileError::UnsupportedSchema {
            kind,
            name: requested.to_string(),
            actual: schema_version,
        });
    }
    validate_document_name(kind, declared)?;
    if requested != declared {
        return Err(ProfileError::NameMismatch {
            kind,
            requested: requested.to_string(),
            declared: declared.to_string(),
        });
    }
    Ok(())
}

fn validate_document_name(kind: &'static str, name: &str) -> Result<(), ProfileError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ProfileError::InvalidDocumentName {
            kind,
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_lua_path_id(entry_id: &str, source: &str, path_id: &str) -> Result<(), ProfileError> {
    let path = Path::new(path_id);
    let components = path.components().collect::<Vec<_>>();
    let valid_tier = matches!(components.first(), Some(Component::Normal(tier)) if *tier == "official" || *tier == "third_party");
    let valid_shape = components.len() == 2
        && components
            .iter()
            .all(|component| matches!(component, Component::Normal(value) if !value.is_empty()));
    if path_id.trim() != path_id || !valid_tier || !valid_shape {
        return Err(ProfileError::InvalidLuaSource {
            id: entry_id.to_string(),
            source_ref: source.to_string(),
            reason: "expected a relative official/<name> or third_party/<name> path".to_string(),
        });
    }
    Ok(())
}
