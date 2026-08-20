use crate::auth::policy_repository::{replace_plugin_policy_bindings, PluginPolicyBinding};
use crate::context::{PluginContext, SushiContext};
use crate::fs::FileBrowserFsService;
use crate::lua::bindings::{inject_plugin_api, inject_sushi_fs};
use crate::lua::module_loader::install_plugin_require;
use crate::lua::vm::create_sandboxed_vm;
use crate::plugin::manager::PageResolvedAssets;
use crate::plugin::{
    Permissions, Plugin, PluginError, PluginFileBrowserConfig, PluginKind, PluginManifest,
};
use crate::runtime::{
    AdminPageSpec, CliCommandSpec, EventSubscriptionSpec, HttpRouteSpec, HttpSurface,
    LuaRuntimeInstance, MenuContributionSpec, PluginHandle, PluginInstanceId, PluginLifecycleState,
    StagedRegistrar, StaticRootSpec, TemplateRootSpec,
};
use async_trait::async_trait;
use mlua::LuaSerdeExt;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

#[path = "adapters/admin.rs"]
mod admin_adapter;
#[path = "adapters/api.rs"]
mod api_adapter;
#[path = "adapters/cli.rs"]
mod cli_adapter;
#[path = "adapters/db.rs"]
mod db_adapter;
#[path = "adapters/event.rs"]
mod event_adapter;
#[path = "adapters/fs.rs"]
mod fs_adapter;
#[path = "adapters/menu.rs"]
mod menu_adapter;
#[path = "adapters/web.rs"]
mod web_adapter;

/// A Lua-based plugin loaded from the filesystem.
pub struct LuaPlugin {
    manifest: PluginManifest,
    kind: PluginKind,
    effective_permissions: Permissions,
    approved: bool,
    lua: Option<mlua::Lua>,
    plugin_dir: PathBuf,
    plugin_path_id: String,
    instance_id: PluginInstanceId,
    config: serde_json::Value,
}

#[derive(Clone)]
struct LuaPluginSource {
    manifest: PluginManifest,
    kind: PluginKind,
    effective_permissions: Permissions,
    approved: bool,
    plugin_dir: PathBuf,
    plugin_path_id: String,
    instance_id: PluginInstanceId,
    config: serde_json::Value,
}

#[derive(Clone, Default)]
pub struct RuntimeHost {
    lua_sources: Arc<RwLock<HashMap<String, (LuaPluginSource, bool)>>>,
    statuses: Arc<RwLock<HashMap<String, RuntimePluginStatus>>>,
    handles: Arc<RwLock<HashMap<String, PluginHandle>>>,
    lifecycle_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePluginStatus {
    pub state: PluginLifecycleState,
    pub last_error: Option<String>,
}

impl RuntimeHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_lua_source(&self, plugin: &LuaPlugin, required: bool) {
        self.register_lua_source_for_instance(plugin, plugin.instance_id.clone(), required)
            .await;
    }

    pub async fn register_lua_source_for_instance(
        &self,
        plugin: &LuaPlugin,
        instance_id: PluginInstanceId,
        required: bool,
    ) {
        self.register_lua_source_for_instance_with_config(
            plugin,
            instance_id,
            required,
            serde_json::Value::Object(serde_json::Map::new()),
        )
        .await;
    }

    pub async fn register_lua_source_for_instance_with_config(
        &self,
        plugin: &LuaPlugin,
        instance_id: PluginInstanceId,
        required: bool,
        config: serde_json::Value,
    ) {
        let mut source = plugin.source();
        source.instance_id = instance_id;
        source.config = config;
        self.lua_sources
            .write()
            .await
            .insert(plugin.name().to_string(), (source, required));
        self.set_status(plugin.name(), PluginLifecycleState::Discovered, None)
            .await;
    }

    pub async fn begin_migration(&self, plugin_name: &str) -> Result<(), PluginError> {
        self.ensure_known_plugin(plugin_name).await?;
        self.set_status(plugin_name, PluginLifecycleState::Migrating, None)
            .await;
        Ok(())
    }

    pub async fn complete_migration(&self, plugin_name: &str) -> Result<(), PluginError> {
        self.ensure_known_plugin(plugin_name).await?;
        self.set_status(plugin_name, PluginLifecycleState::Resolved, None)
            .await;
        Ok(())
    }

    pub async fn mark_inactive(&self, plugin_name: &str) -> Result<(), PluginError> {
        self.ensure_known_plugin(plugin_name).await?;
        self.set_status(plugin_name, PluginLifecycleState::Inactive, None)
            .await;
        Ok(())
    }

    pub async fn record_failure(
        &self,
        plugin_name: &str,
        error: impl Into<String>,
    ) -> Result<(), PluginError> {
        self.ensure_known_plugin(plugin_name).await?;
        self.set_status(
            plugin_name,
            PluginLifecycleState::Failed,
            Some(error.into()),
        )
        .await;
        Ok(())
    }

    pub async fn status(&self, plugin_name: &str) -> Option<RuntimePluginStatus> {
        self.statuses.read().await.get(plugin_name).cloned()
    }

    pub async fn handle(&self, plugin_name: &str) -> Option<PluginHandle> {
        self.handles.read().await.get(plugin_name).cloned()
    }

    pub async fn is_required(&self, plugin_name: &str) -> bool {
        self.lua_sources
            .read()
            .await
            .get(plugin_name)
            .map(|(_, required)| *required)
            .unwrap_or(false)
    }

    pub(crate) async fn acquire_lifecycle_lock(&self, plugin_name: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.lifecycle_locks.write().await;
            locks
                .entry(plugin_name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        lock.lock_owned().await
    }

    pub async fn activate(&self, ctx: &SushiContext, plugin_name: &str) -> Result<(), PluginError> {
        let _runtime_guard = self.acquire_lifecycle_lock(plugin_name).await;
        self.activate_locked(ctx, plugin_name).await
    }

    pub(crate) async fn activate_locked(
        &self,
        ctx: &SushiContext,
        plugin_name: &str,
    ) -> Result<(), PluginError> {
        self.set_status(plugin_name, PluginLifecycleState::Activating, None)
            .await;
        let source = self
            .lua_sources
            .read()
            .await
            .get(plugin_name)
            .map(|(source, _)| source.clone())
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;
        if !source.approved {
            let error = PluginError::PermissionDenied(format!(
                "plugin '{plugin_name}' is not approved by its runtime profile; set grants.approved = true and restart"
            ));
            self.set_status(
                plugin_name,
                PluginLifecycleState::Failed,
                Some(error.to_string()),
            )
            .await;
            return Err(error);
        }
        let plugin = LuaPlugin::from_source(source.clone())?;
        let plugin_context = ctx.plugin_context_for(
            source.instance_id.clone(),
            source.config.clone(),
            &source.effective_permissions,
        );
        match plugin.activate(&plugin_context).await {
            Ok(()) => {
                if let Some(runtime) = ctx.plugins.lua_runtime(plugin_name).await {
                    let snapshot = ctx.plugins.capability_snapshot().await;
                    let registrations = snapshot.registration_ids_for_owner(&source.instance_id);
                    let tasks = ctx.tasks.registrations_for_owner(&source.instance_id).await;
                    self.handles.write().await.insert(
                        plugin_name.to_string(),
                        PluginHandle::new(
                            source.instance_id.clone(),
                            runtime,
                            PluginLifecycleState::Active,
                            registrations,
                            tasks,
                            plugin_context.cancellation(),
                        ),
                    );
                }
                self.set_status(plugin_name, PluginLifecycleState::Active, None)
                    .await;
                Ok(())
            }
            Err(error) => {
                plugin_context.cancellation().cancel();
                ctx.plugins.mark_plugin_loaded(plugin_name, false).await;
                self.set_status(
                    plugin_name,
                    PluginLifecycleState::Failed,
                    Some(error.to_string()),
                )
                .await;
                Err(error)
            }
        }
    }

    pub async fn deactivate(
        &self,
        ctx: &SushiContext,
        plugin_name: &str,
    ) -> Result<(), PluginError> {
        let _runtime_guard = self.acquire_lifecycle_lock(plugin_name).await;
        self.deactivate_locked(ctx, plugin_name).await
    }

    pub async fn reload(&self, ctx: &SushiContext, plugin_name: &str) -> Result<(), PluginError> {
        let _runtime_guard = self.acquire_lifecycle_lock(plugin_name).await;
        let previous_handle = self.handles.read().await.get(plugin_name).cloned();
        self.set_status(plugin_name, PluginLifecycleState::Activating, None)
            .await;
        let source = self
            .lua_sources
            .read()
            .await
            .get(plugin_name)
            .map(|(source, _)| source.clone())
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;
        if !source.approved {
            let error = PluginError::PermissionDenied(format!(
                "plugin '{plugin_name}' is not approved by its runtime profile; set grants.approved = true and restart"
            ));
            self.set_status(
                plugin_name,
                PluginLifecycleState::Failed,
                Some(error.to_string()),
            )
            .await;
            return Err(error);
        }
        let plugin = LuaPlugin::from_source(source.clone())?;
        let plugin_context = ctx.plugin_context_for(
            source.instance_id.clone(),
            source.config.clone(),
            &source.effective_permissions,
        );
        match plugin.activate(&plugin_context).await {
            Ok(()) => {
                if let Some(runtime) = ctx.plugins.lua_runtime(plugin_name).await {
                    let snapshot = ctx.plugins.capability_snapshot().await;
                    let registrations = snapshot.registration_ids_for_owner(&source.instance_id);
                    let previous_task_ids = previous_handle
                        .as_ref()
                        .map(|handle| {
                            handle
                                .tasks
                                .iter()
                                .map(|registration| registration.id)
                                .collect::<HashSet<_>>()
                        })
                        .unwrap_or_default();
                    let tasks = ctx
                        .tasks
                        .registrations_for_owner(&source.instance_id)
                        .await
                        .into_iter()
                        .filter(|registration| !previous_task_ids.contains(&registration.id))
                        .collect();
                    self.handles.write().await.insert(
                        plugin_name.to_string(),
                        PluginHandle::new(
                            source.instance_id.clone(),
                            runtime,
                            PluginLifecycleState::Active,
                            registrations,
                            tasks,
                            plugin_context.cancellation(),
                        ),
                    );
                    if let Some(previous) = previous_handle {
                        previous.cancellation.cancel();
                        ctx.tasks
                            .cancel_registrations(
                                &previous.tasks,
                                std::time::Duration::from_secs(5),
                            )
                            .await;
                    }
                }
                self.set_status(plugin_name, PluginLifecycleState::Active, None)
                    .await;
                Ok(())
            }
            Err(error) => {
                plugin_context.cancellation().cancel();
                self.set_status(
                    plugin_name,
                    PluginLifecycleState::Active,
                    Some(error.to_string()),
                )
                .await;
                Err(error)
            }
        }
    }

    pub(crate) async fn deactivate_locked(
        &self,
        ctx: &SushiContext,
        plugin_name: &str,
    ) -> Result<(), PluginError> {
        self.set_status(plugin_name, PluginLifecycleState::Deactivating, None)
            .await;
        let existing_handle = { self.handles.read().await.get(plugin_name).cloned() };
        if let Some(handle) = existing_handle {
            handle.cancellation.cancel();
            self.handles.write().await.insert(
                plugin_name.to_string(),
                handle.with_state(PluginLifecycleState::Deactivating),
            );
        }
        let source_identity = self
            .lua_sources
            .read()
            .await
            .get(plugin_name)
            .map(|(source, _)| (source.instance_id.clone(), source.plugin_path_id.clone()));
        let (owner, plugin_id) = if let Some(identity) = source_identity {
            identity
        } else {
            let plugin = ctx
                .plugins
                .list_plugins()
                .await
                .into_iter()
                .find(|plugin| plugin.name == plugin_name)
                .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;
            let plugin_id = plugin.plugin_id;
            let owner = PluginInstanceId::new(format!("lua:{plugin_id}"))
                .map_err(PluginError::InitFailed)?;
            (owner, plugin_id)
        };
        ctx.remove_owner_effects(&owner).await;
        let path_owner =
            PluginInstanceId::new(format!("lua:{plugin_id}")).map_err(PluginError::InitFailed)?;
        if path_owner != owner {
            ctx.remove_owner_effects(&path_owner).await;
        }
        ctx.remove_owner_effects(&PluginInstanceId::legacy(plugin_name))
            .await;
        let policy_result = replace_plugin_policy_bindings(&ctx.db, plugin_name, &[]).await;
        ctx.plugins.unregister_vm(plugin_name).await;
        self.handles.write().await.remove(plugin_name);
        match policy_result {
            Ok(()) => {
                self.set_status(plugin_name, PluginLifecycleState::Inactive, None)
                    .await;
                Ok(())
            }
            Err(error) => {
                let error = PluginError::InitFailed(format!(
                    "failed to remove policy bindings for plugin {plugin_name}: {error}"
                ));
                self.set_status(
                    plugin_name,
                    PluginLifecycleState::Inactive,
                    Some(error.to_string()),
                )
                .await;
                Err(error)
            }
        }
    }

    async fn set_status(
        &self,
        plugin_name: &str,
        state: PluginLifecycleState,
        last_error: Option<String>,
    ) {
        self.statuses.write().await.insert(
            plugin_name.to_string(),
            RuntimePluginStatus { state, last_error },
        );
    }

    async fn ensure_known_plugin(&self, plugin_name: &str) -> Result<(), PluginError> {
        if self.lua_sources.read().await.contains_key(plugin_name) {
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_name.to_string()))
        }
    }
}

impl LuaPlugin {
    fn source(&self) -> LuaPluginSource {
        LuaPluginSource {
            manifest: self.manifest.clone(),
            kind: self.kind,
            effective_permissions: self.effective_permissions.clone(),
            approved: self.approved,
            plugin_dir: self.plugin_dir.clone(),
            plugin_path_id: self.plugin_path_id.clone(),
            instance_id: self.instance_id.clone(),
            config: self.config.clone(),
        }
    }

    fn from_source(source: LuaPluginSource) -> Result<Self, PluginError> {
        let lua = create_sandboxed_vm().map_err(|error| {
            PluginError::LuaError(format!(
                "create VM for {}: {error}",
                source.manifest.plugin.name
            ))
        })?;
        Ok(Self {
            manifest: source.manifest,
            kind: source.kind,
            effective_permissions: source.effective_permissions,
            approved: source.approved,
            lua: Some(lua),
            plugin_dir: source.plugin_dir,
            plugin_path_id: source.plugin_path_id,
            instance_id: source.instance_id,
            config: source.config,
        })
    }
}

impl LuaPlugin {
    pub async fn load_dir(plugin_dir: &Path, plugin_path_id: &str) -> Result<Self, PluginError> {
        let mut components = Path::new(plugin_path_id).components();
        let tier = components
            .next()
            .and_then(|component| match component {
                std::path::Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .ok_or_else(|| {
                PluginError::ManifestError(format!(
                    "invalid plugin path id '{plugin_path_id}': expected <tier>/<name>"
                ))
            })?;
        let name = components
            .next()
            .and_then(|component| match component {
                std::path::Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .ok_or_else(|| {
                PluginError::ManifestError(format!(
                    "invalid plugin path id '{plugin_path_id}': expected <tier>/<name>"
                ))
            })?;
        if components.next().is_some() || name.is_empty() {
            return Err(PluginError::ManifestError(format!(
                "invalid plugin path id '{plugin_path_id}': expected <tier>/<name>"
            )));
        }
        let expected_kind = match tier {
            "official" => PluginKind::Official,
            "third_party" => PluginKind::ThirdParty,
            _ => {
                return Err(PluginError::ManifestError(format!(
                    "invalid plugin tier '{tier}' in path id '{plugin_path_id}'"
                )))
            }
        };
        let manifest_path = plugin_dir.join("plugin.toml");
        let manifest_content =
            tokio::fs::read_to_string(&manifest_path)
                .await
                .map_err(|error| {
                    PluginError::ManifestError(format!("read {}: {error}", manifest_path.display()))
                })?;
        let manifest: PluginManifest = toml::from_str(&manifest_content).map_err(|error| {
            PluginError::ManifestError(format!("parse {}: {error}", manifest_path.display()))
        })?;
        manifest.validate_schema().map_err(|message| {
            PluginError::ManifestError(format!("validate {}: {message}", manifest_path.display()))
        })?;
        let effective_permissions = expected_kind.effective_permissions(&manifest.permissions);
        let lua = create_sandboxed_vm().map_err(|error| {
            PluginError::LuaError(format!("create VM for {}: {error}", manifest.plugin.name))
        })?;
        let instance_id = PluginInstanceId::new(format!("lua:{plugin_path_id}"))
            .map_err(PluginError::ManifestError)?;
        Ok(Self {
            manifest,
            kind: expected_kind,
            effective_permissions,
            approved: true,
            lua: Some(lua),
            plugin_dir: plugin_dir.to_path_buf(),
            plugin_path_id: plugin_path_id.to_string(),
            instance_id,
            config: serde_json::Value::Object(serde_json::Map::new()),
        })
    }

    /// Scan a directory for plugins in tiered official/third_party layout.
    pub async fn scan_dir(dir: &Path) -> Result<Vec<Self>, PluginError> {
        let mut plugins = Vec::new();
        let mut legacy_dirs = Vec::new();
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(PluginError::IoError)?;

        while let Some(entry) = entries.next_entry().await.map_err(PluginError::IoError)? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let category_name = entry.file_name().to_string_lossy().to_string();
            if category_name != "official" && category_name != "third_party" {
                if path.join("plugin.toml").is_file() {
                    legacy_dirs.push(path.display().to_string());
                }
                continue;
            }

            let expected_kind = if category_name == "official" {
                PluginKind::Official
            } else {
                PluginKind::ThirdParty
            };

            let mut plugin_entries = tokio::fs::read_dir(&path)
                .await
                .map_err(PluginError::IoError)?;
            while let Some(plugin_entry) = plugin_entries
                .next_entry()
                .await
                .map_err(PluginError::IoError)?
            {
                let plugin_path = plugin_entry.path();
                if !plugin_path.is_dir() {
                    continue;
                }

                let manifest_path = plugin_path.join("plugin.toml");
                if !manifest_path.exists() {
                    continue;
                }

                let manifest_content =
                    tokio::fs::read_to_string(&manifest_path)
                        .await
                        .map_err(|e| {
                            PluginError::ManifestError(format!(
                                "read {}: {e}",
                                manifest_path.display()
                            ))
                        })?;
                let manifest: PluginManifest = toml::from_str(&manifest_content).map_err(|e| {
                    PluginError::ManifestError(format!("parse {}: {e}", manifest_path.display()))
                })?;
                manifest.validate_schema().map_err(|message| {
                    PluginError::ManifestError(format!(
                        "validate {}: {message}",
                        manifest_path.display()
                    ))
                })?;

                let effective_permissions =
                    expected_kind.effective_permissions(&manifest.permissions);
                let plugin_dir_name = plugin_entry.file_name().to_string_lossy().to_string();
                let plugin_path_id = format!("{}/{}", expected_kind.tier_name(), plugin_dir_name);

                let lua = create_sandboxed_vm().map_err(|e| {
                    PluginError::LuaError(format!("create VM for {}: {e}", manifest.plugin.name))
                })?;

                plugins.push(Self {
                    manifest,
                    kind: expected_kind,
                    effective_permissions,
                    approved: true,
                    lua: Some(lua),
                    plugin_dir: plugin_path,
                    plugin_path_id,
                    instance_id: PluginInstanceId::new(format!(
                        "lua:{}/{}",
                        expected_kind.tier_name(),
                        plugin_dir_name
                    ))
                    .map_err(PluginError::ManifestError)?,
                    config: serde_json::Value::Object(serde_json::Map::new()),
                });
            }
        }

        if !legacy_dirs.is_empty() {
            return Err(PluginError::ManifestError(format!(
                "legacy flat plugin directories are not supported; move these into plugins/official or plugins/third_party: {}",
                legacy_dirs.join(", ")
            )));
        }

        plugins.sort_by(|left, right| left.plugin_path_id.cmp(&right.plugin_path_id));
        Ok(plugins)
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn kind(&self) -> PluginKind {
        self.kind
    }

    pub fn path_id(&self) -> &str {
        &self.plugin_path_id
    }

    pub fn effective_permissions(&self) -> &Permissions {
        &self.effective_permissions
    }

    pub fn is_approved(&self) -> bool {
        self.approved
    }

    pub fn apply_profile_grants(&mut self, grants: &serde_json::Value) {
        self.approved = grants.get("approved").and_then(serde_json::Value::as_bool) == Some(true);
        self.effective_permissions = self.effective_permissions.clamp_to_grants(grants);
    }

    pub fn web_templates_dir(&self) -> PathBuf {
        self.plugin_dir.join("web").join("templates")
    }

    pub fn web_static_dir(&self) -> PathBuf {
        self.plugin_dir.join("web").join("static")
    }

    /// Take the Lua VM out of the plugin after init.
    /// This transfers ownership to the caller (typically PluginManager).
    pub fn into_vm(self) -> Option<mlua::Lua> {
        self.lua
    }
}

fn validate_optional_file_browser_config(
    config: Option<&PluginFileBrowserConfig>,
) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };

    validate_route_prefix(config)?;
    validate_text_extensions(config)?;
    validate_roots(config)?;
    Ok(())
}

fn resolve_file_browser_config(
    entry_config: &serde_json::Value,
) -> Result<Option<PluginFileBrowserConfig>, serde_json::Error> {
    match entry_config.get("file_browser") {
        Some(value) if !value.is_null() => serde_json::from_value(value.clone()).map(Some),
        Some(_) => Ok(None),
        None => Ok(None),
    }
}

fn validate_route_prefix(config: &PluginFileBrowserConfig) -> Result<(), String> {
    let route_prefix = config.route_prefix.as_str();
    if route_prefix.is_empty() {
        return Err("route_prefix must be non-empty".to_string());
    }
    if route_prefix.trim() != route_prefix {
        return Err("route_prefix cannot contain leading/trailing whitespace".to_string());
    }
    if !route_prefix.starts_with('/') {
        return Err(format!("route_prefix '{route_prefix}' must start with '/'"));
    }
    Ok(())
}

fn validate_text_extensions(config: &PluginFileBrowserConfig) -> Result<(), String> {
    for ext in &config.text_extensions {
        let trimmed = ext.trim();
        if trimmed.is_empty() {
            return Err("text_extensions cannot contain empty values".to_string());
        }
        let normalized = trimmed.trim_start_matches('.');
        if normalized.is_empty()
            || !normalized
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "text_extensions entry '{ext}' is invalid; expected extension token"
            ));
        }
    }
    Ok(())
}

fn validate_roots(config: &PluginFileBrowserConfig) -> Result<(), String> {
    let mut seen_ids = HashSet::new();

    for root in &config.roots {
        let id = root.id.as_str();
        if id.is_empty() {
            return Err("root id must be non-empty".to_string());
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(format!(
                "root id '{id}' is invalid; expected pattern [a-z0-9-_]+"
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(format!("duplicate root id '{id}'"));
        }

        if root.path.trim() != root.path {
            return Err(format!(
                "root path '{}' for id '{id}' cannot contain leading/trailing whitespace",
                root.path
            ));
        }
        let path = Path::new(root.path.as_str());
        if path.as_os_str().is_empty() {
            return Err(format!("root path for id '{id}' must be non-empty"));
        }
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(format!(
                        "root path '{}' for id '{id}' cannot contain '..'",
                        root.path
                    ));
                }
                std::path::Component::Prefix(_) => {
                    return Err(format!(
                        "root path '{}' for id '{id}' has invalid prefix",
                        root.path
                    ));
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn normalize_static_url_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/static".to_string();
    }

    let mut prefix = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    if prefix.len() > 1 {
        prefix = prefix.trim_end_matches('/').to_string();
    }

    if prefix == "/" {
        return "/static".to_string();
    }

    prefix
}

fn validate_resolvable_relative_path(path: &str, field: &str) -> Result<(), PluginError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path: empty value"
        )));
    }
    if Path::new(trimmed).is_absolute() {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path '{trimmed}': absolute paths are not allowed"
        )));
    }
    if trimmed.contains("..") {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path '{trimmed}': parent directory segments are not allowed"
        )));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || trimmed.starts_with("//") {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path '{trimmed}': URL values are not allowed"
        )));
    }
    Ok(())
}

fn push_resolved_assets(
    plugin_path_id: &str,
    static_url_prefix: &str,
    static_root: &Path,
    source_paths: &[String],
    field: &str,
    target: &mut Vec<String>,
    seen: &mut HashSet<String>,
) -> Result<(), PluginError> {
    for path in source_paths {
        let normalized_path = path.trim().to_string();
        validate_resolvable_relative_path(&normalized_path, field)?;
        if !seen.insert(normalized_path.clone()) {
            continue;
        }

        let file_path = static_root.join(&normalized_path);
        if !file_path.is_file() {
            return Err(PluginError::InitFailed(format!(
                "missing plugin asset file '{}'",
                file_path.display()
            )));
        }

        target.push(format!(
            "{static_url_prefix}/plugins/{plugin_path_id}/{}",
            normalized_path
        ));
    }

    Ok(())
}

fn resolve_page_assets(
    plugin_path_id: &str,
    manifest: &PluginManifest,
    bundle_names: &[String],
    page_js: &[String],
    page_css: &[String],
    static_root: &Path,
    static_url_prefix: &str,
) -> Result<PageResolvedAssets, PluginError> {
    let mut resolved = PageResolvedAssets::default();
    let mut seen_js = HashSet::new();
    let mut seen_css = HashSet::new();

    for bundle_name in bundle_names {
        let bundle = manifest
            .admin
            .as_ref()
            .and_then(|admin| admin.assets.as_ref())
            .and_then(|assets| assets.bundles.get(bundle_name))
            .ok_or_else(|| {
                PluginError::InitFailed(format!("unknown page asset bundle: {bundle_name}"))
            })?;

        push_resolved_assets(
            plugin_path_id,
            static_url_prefix,
            static_root,
            &bundle.js,
            "js",
            &mut resolved.js,
            &mut seen_js,
        )?;
        push_resolved_assets(
            plugin_path_id,
            static_url_prefix,
            static_root,
            &bundle.css,
            "css",
            &mut resolved.css,
            &mut seen_css,
        )?;
    }

    push_resolved_assets(
        plugin_path_id,
        static_url_prefix,
        static_root,
        page_js,
        "js",
        &mut resolved.js,
        &mut seen_js,
    )?;
    push_resolved_assets(
        plugin_path_id,
        static_url_prefix,
        static_root,
        page_css,
        "css",
        &mut resolved.css,
        &mut seen_css,
    )?;

    Ok(resolved)
}

fn policy_matches_scope(policy_key: &str, scope: &str) -> bool {
    if let Some(prefix) = scope.strip_suffix('*') {
        policy_key.starts_with(prefix)
    } else {
        policy_key == scope
    }
}

fn validate_policy_scope(
    plugin_name: &str,
    target: &str,
    policy_key: &str,
    scopes: &[String],
) -> Result<(), PluginError> {
    if scopes
        .iter()
        .any(|scope| policy_matches_scope(policy_key, scope))
    {
        return Ok(());
    }

    let scope_list = if scopes.is_empty() {
        "<none>".to_string()
    } else {
        scopes.join(", ")
    };

    Err(PluginError::InitFailed(format!(
        "plugin '{plugin_name}' declared policy '{policy_key}' for {target}, but it is outside manifest policy scopes: [{scope_list}]"
    )))
}

fn stage_menu_contribution(
    staged: &mut StagedRegistrar,
    plugin_name: &str,
    allowed_policy_scopes: &[String],
    contribution: crate::lua::contract::schema::menu::MenuContributionContract,
) -> Result<(), PluginError> {
    if contribution.id.trim().is_empty() || contribution.label.trim().is_empty() {
        return Err(PluginError::InitFailed(
            "menu contribution requires non-empty id and label".to_string(),
        ));
    }
    if let Some(route) = contribution.route.as_deref() {
        if !route.starts_with('/') {
            return Err(PluginError::InitFailed(format!(
                "menu contribution '{}' route must start with '/'",
                contribution.id
            )));
        }
    }
    if let Some(policy_key) = contribution.policy.as_deref() {
        validate_policy_scope(
            plugin_name,
            &format!("menu contribution {}", contribution.id),
            policy_key,
            allowed_policy_scopes,
        )?;
    }

    staged.register_menu(
        MenuContributionSpec::new(contribution.id, contribution.label, contribution.position)
            .with_icon(contribution.icon)
            .with_parent(contribution.parent_id)
            .with_route(contribution.route)
            .with_policy(contribution.policy),
    );
    Ok(())
}

fn stage_api_route_binding(
    staged: &mut StagedRegistrar,
    policy_bindings: &mut Vec<PluginPolicyBinding>,
    runtime: &Arc<LuaRuntimeInstance>,
    plugin_name: &str,
    allowed_policy_scopes: &[String],
    method: &str,
    path: &str,
    handler_key: &str,
    policy_key: Option<&str>,
    is_public: bool,
) -> Result<(), PluginError> {
    if method.trim().is_empty() || path.trim().is_empty() || handler_key.trim().is_empty() {
        return Err(PluginError::InitFailed(format!(
            "route registration requires non-empty method, path, and handler_key"
        )));
    }
    let surface = HttpSurface::from_path(path);

    if is_public && policy_key.is_some() {
        return Err(PluginError::InitFailed(format!(
            "route {method} {path} cannot declare both policy and public=true"
        )));
    }

    if let Some(policy_key_value) = policy_key {
        validate_policy_scope(
            plugin_name,
            &format!("route {method} {path}"),
            policy_key_value,
            allowed_policy_scopes,
        )?;
        policy_bindings.push(PluginPolicyBinding::Http {
            surface: surface.as_str().to_string(),
            method: method.to_uppercase(),
            path_pattern: path.to_string(),
            policy_key: policy_key_value.to_string(),
        });
    }

    staged.register_http(
        HttpRouteSpec::new(method, path, plugin_name, handler_key)
            .with_surface(surface)
            .with_policy(policy_key.map(ToOwned::to_owned))
            .with_public(is_public)
            .with_lua_runtime(Arc::clone(runtime)),
    );
    Ok(())
}

fn stage_admin_page_binding(
    staged: &mut StagedRegistrar,
    policy_bindings: &mut Vec<PluginPolicyBinding>,
    runtime: &Arc<LuaRuntimeInstance>,
    plugin_name: &str,
    allowed_policy_scopes: &[String],
    path: &str,
    title: &str,
    handler_key: &str,
    assets: PageResolvedAssets,
    policy_key: Option<&str>,
) -> Result<(), PluginError> {
    if path.trim().is_empty() || title.trim().is_empty() || handler_key.trim().is_empty() {
        return Err(PluginError::InitFailed(
            "admin page registration requires non-empty path, title, and handler_key".to_string(),
        ));
    }
    if let Some(policy_key_value) = policy_key {
        validate_policy_scope(
            plugin_name,
            &format!("page {path}"),
            policy_key_value,
            allowed_policy_scopes,
        )?;
        policy_bindings.push(PluginPolicyBinding::Http {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: path.to_string(),
            policy_key: policy_key_value.to_string(),
        });
    }

    staged.register_admin(
        AdminPageSpec::new(path, title, plugin_name, handler_key)
            .with_policy(policy_key.map(ToOwned::to_owned))
            .with_assets(assets.js, assets.css)
            .with_lua_runtime(Arc::clone(runtime)),
    );
    Ok(())
}

fn stage_cli_command_binding(
    staged: &mut StagedRegistrar,
    policy_bindings: &mut Vec<PluginPolicyBinding>,
    runtime: &Arc<LuaRuntimeInstance>,
    plugin_name: &str,
    allowed_policy_scopes: &[String],
    name: &str,
    description: &str,
    handler_key: &str,
    policy_key: Option<&str>,
) -> Result<(), PluginError> {
    if name.trim().is_empty() || description.trim().is_empty() || handler_key.trim().is_empty() {
        return Err(PluginError::InitFailed(
            "cli command registration requires non-empty name, description, and handler_key"
                .to_string(),
        ));
    }
    if let Some(policy_key_value) = policy_key {
        validate_policy_scope(
            plugin_name,
            &format!("command {name}"),
            policy_key_value,
            allowed_policy_scopes,
        )?;
        policy_bindings.push(PluginPolicyBinding::Cli {
            command_name: name.to_string(),
            policy_key: policy_key_value.to_string(),
        });
    }
    staged.register_cli(
        CliCommandSpec::new(name, description, plugin_name, handler_key)
            .with_policy(policy_key.map(ToOwned::to_owned))
            .with_lua_runtime(Arc::clone(runtime)),
    );
    Ok(())
}

fn stage_event_subscription(
    staged: &mut StagedRegistrar,
    runtime: &Arc<LuaRuntimeInstance>,
    event: &str,
    handler_key: &str,
) -> Result<(), PluginError> {
    if event.trim().is_empty() || handler_key.trim().is_empty() {
        return Err(PluginError::InitFailed(
            "event subscription requires non-empty event and handler_key".to_string(),
        ));
    }

    let callback_runtime = Arc::clone(runtime);
    let callback_handler_key = handler_key.to_string();
    staged.register_event_subscription(
        EventSubscriptionSpec::new(event, move |data| {
            let runtime = Arc::clone(&callback_runtime);
            let handler_key = callback_handler_key.clone();
            async move {
                let lua = runtime.lua();
                let handlers = match lua.globals().get::<mlua::Value>("app") {
                    Ok(mlua::Value::Table(app)) => app.get::<mlua::Table>("__handlers"),
                    _ => lua
                        .globals()
                        .get::<mlua::Table>("sushi")
                        .and_then(|sushi| sushi.get::<mlua::Table>("__handlers")),
                };
                let result = async {
                    let handlers = handlers?;
                    let callback = handlers.get::<mlua::Function>(&*handler_key)?;
                    let data = lua.to_value(&data)?;
                    callback.call_async::<()>(data).await
                }
                .await;
                if let Err(error) = result {
                    tracing::error!(
                        plugin = runtime.plugin_name(),
                        event_handler = handler_key,
                        error = %error,
                        "lua event handler failed"
                    );
                }
            }
        })
        .with_lua_runtime(Arc::clone(runtime)),
    );
    Ok(())
}

#[async_trait]
impl Plugin for LuaPlugin {
    fn name(&self) -> &str {
        &self.manifest.plugin.name
    }
    fn version(&self) -> &str {
        &self.manifest.plugin.version
    }

    async fn activate(&self, ctx: &PluginContext) -> Result<(), PluginError> {
        // Take the Lua VM out of self (init should only be called once)
        let lua = self.lua.as_ref().ok_or_else(|| {
            PluginError::InitFailed(format!(
                "{}: already initialized",
                self.manifest.plugin.name
            ))
        })?;

        let file_browser_root_dir = {
            let cfg = ctx.config().get().await;
            cfg.file_browser.root_dir.clone()
        };

        let file_browser_config = resolve_file_browser_config(ctx.config_value()).map_err(|e| {
            PluginError::InitFailed(format!(
                "{}: invalid profile file_browser config: {e}",
                self.manifest.plugin.name
            ))
        })?;
        validate_optional_file_browser_config(file_browser_config.as_ref()).map_err(|e| {
            PluginError::InitFailed(format!(
                "{}: file_browser config invalid: {e}",
                self.manifest.plugin.name
            ))
        })?;

        let file_browser_fs = file_browser_config
            .as_ref()
            .map(|manifest| {
                FileBrowserFsService::from_manifest_with_root_base(
                    manifest,
                    Path::new(&file_browser_root_dir),
                )
            })
            .transpose()
            .map_err(|e| {
                PluginError::InitFailed(format!(
                    "{}: failed to build file_browser fs service: {e}",
                    self.manifest.plugin.name
                ))
            })?;

        // Inject sushi.* API into the Lua VM
        inject_plugin_api(lua, ctx, &self.effective_permissions)
            .await
            .map_err(|e| PluginError::LuaError(format!("inject API: {e}")))?;
        if let Some(service) = file_browser_fs {
            inject_sushi_fs(lua, Arc::new(service))
                .map_err(|e| PluginError::LuaError(format!("inject sushi.fs API: {e}")))?;
        }

        install_plugin_require(lua, &self.plugin_dir)
            .map_err(|e| PluginError::LuaError(format!("install plugin module loader: {e}")))?;

        // Load and execute the entry script
        let entry_path = self.plugin_dir.join(&self.manifest.plugin.entry);

        // Check file size limit (1MB max)
        const MAX_PLUGIN_SIZE: u64 = 1024 * 1024; // 1MB
        let metadata = tokio::fs::metadata(&entry_path)
            .await
            .map_err(|e| PluginError::LuaError(format!("stat {}: {e}", entry_path.display())))?;

        if metadata.len() > MAX_PLUGIN_SIZE {
            return Err(PluginError::LuaError(format!(
                "plugin {} code too large: {} bytes (max: {} bytes)",
                self.manifest.plugin.name,
                metadata.len(),
                MAX_PLUGIN_SIZE
            )));
        }

        let code = tokio::fs::read_to_string(&entry_path)
            .await
            .map_err(|e| PluginError::LuaError(format!("read {}: {e}", entry_path.display())))?;

        lua.load(&code)
            .exec()
            .map_err(|e| PluginError::InitFailed(format!("{}: {e}", self.manifest.plugin.name)))?;

        // Call app.init() if defined (sushi.init() kept for backward compat via alias)
        let app: mlua::Table = lua
            .globals()
            .get("app")
            .map_err(|e| PluginError::LuaError(format!("no app global: {e}")))?;

        if let Ok(init_fn) = app.get::<mlua::Function>("init") {
            init_fn.call::<()>(()).map_err(|e| {
                PluginError::InitFailed(format!("{}.init(): {e}", self.manifest.plugin.name))
            })?;
        }

        let plugin_name = &self.manifest.plugin.name;
        if let Ok(diagnostics) = app.get::<mlua::Table>("__deprecation_diagnostics") {
            let mut legacy_apis = diagnostics
                .sequence_values::<String>()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    PluginError::InitFailed(format!(
                        "plugin '{plugin_name}' produced invalid deprecation diagnostics: {error}"
                    ))
                })?;
            legacy_apis.sort();
            legacy_apis.dedup();
            for api in legacy_apis {
                let message = format!(
                    "plugin '{plugin_name}' uses deprecated Lua registration API '{api}'; migrate to sushi.capability.register"
                );
                tracing::warn!(plugin = plugin_name, deprecated_api = api, "{message}");
                ctx.logs().warn(&message).await;
            }
        }
        let allowed_policy_scopes = &self.manifest.policies.scopes;
        let owner = self.instance_id.clone();
        let runtime = Arc::new(LuaRuntimeInstance::new(plugin_name, lua.clone()));
        let mut staged = ctx.plugin_manager().stage_lua_activation(owner);
        let mut policy_bindings = Vec::new();
        let static_prefix = {
            let cfg = ctx.config().get().await;
            normalize_static_url_prefix(&cfg.web.static_url_prefix)
        };
        let plugin_static_root = self.web_static_dir();
        let plugin_template_root = self.web_templates_dir();
        if plugin_template_root.is_dir() {
            staged.register_template_root(
                TemplateRootSpec::new(&self.plugin_path_id, plugin_template_root)
                    .map_err(PluginError::InitFailed)?,
            );
        }
        if plugin_static_root.is_dir() {
            staged.register_static_root(
                StaticRootSpec::new(&self.plugin_path_id, plugin_static_root.clone())
                    .map_err(PluginError::InitFailed)?,
            );
        }

        if let Ok(raw_registry) = app.get::<mlua::Table>("__contract_registry") {
            let admin_pages = admin_adapter::snapshot_from_lua(lua, raw_registry.clone())?;
            let cli_commands = cli_adapter::snapshot_from_lua(lua, raw_registry.clone())?;
            let event_entries = event_adapter::snapshot_from_lua(raw_registry.clone())?;
            let menu_contributions = menu_adapter::snapshot_from_lua(raw_registry.clone())?;
            let _ = db_adapter::snapshot_from_lua(raw_registry.clone())?;
            let _ = fs_adapter::snapshot_from_lua(raw_registry.clone())?;
            let web_pages = web_adapter::snapshot_from_lua(lua, raw_registry.clone())?;
            let snapshot = api_adapter::snapshot_from_lua(lua, raw_registry)?;

            if !self.effective_permissions.routes && !snapshot.api_routes.is_empty() {
                return Err(PluginError::InitFailed(format!(
                    "plugin '{}' contract registry includes api entries but routes permission is disabled",
                    plugin_name
                )));
            }
            if !self.effective_permissions.admin
                && (!admin_pages.is_empty()
                    || !web_pages.is_empty()
                    || !menu_contributions.is_empty())
            {
                return Err(PluginError::InitFailed(format!(
                    "plugin '{}' contract registry includes web page, admin page, or menu entries but admin permission is disabled",
                    plugin_name
                )));
            }
            if !self.effective_permissions.commands && !cli_commands.is_empty() {
                return Err(PluginError::InitFailed(format!(
                    "plugin '{}' contract registry includes cli entries but commands permission is disabled",
                    plugin_name
                )));
            }

            for route in snapshot.api_routes {
                stage_api_route_binding(
                    &mut staged,
                    &mut policy_bindings,
                    &runtime,
                    plugin_name,
                    allowed_policy_scopes,
                    &route.method,
                    &route.path,
                    &route.handler_key,
                    route.policy.as_deref(),
                    route.public,
                )?;
            }

            for page in web_pages {
                let assets = resolve_page_assets(
                    &self.plugin_path_id,
                    &self.manifest,
                    &page.bundle_names,
                    &page.page_js,
                    &page.page_css,
                    &plugin_static_root,
                    &static_prefix,
                )?;
                stage_admin_page_binding(
                    &mut staged,
                    &mut policy_bindings,
                    &runtime,
                    plugin_name,
                    allowed_policy_scopes,
                    &page.path,
                    &page.title,
                    &page.handler_key,
                    assets,
                    page.policy.as_deref(),
                )?;
            }

            for page in admin_pages {
                let assets = resolve_page_assets(
                    &self.plugin_path_id,
                    &self.manifest,
                    &page.bundles,
                    &page.js,
                    &page.css,
                    &plugin_static_root,
                    &static_prefix,
                )?;
                stage_admin_page_binding(
                    &mut staged,
                    &mut policy_bindings,
                    &runtime,
                    plugin_name,
                    allowed_policy_scopes,
                    &page.path,
                    &page.title,
                    &page.handler_key,
                    assets,
                    page.policy.as_deref(),
                )?;
            }

            for command in cli_commands {
                stage_cli_command_binding(
                    &mut staged,
                    &mut policy_bindings,
                    &runtime,
                    plugin_name,
                    allowed_policy_scopes,
                    &command.name,
                    &command.description,
                    &command.handler_key,
                    command.policy.as_deref(),
                )?;
            }

            for contribution in menu_contributions {
                stage_menu_contribution(
                    &mut staged,
                    plugin_name,
                    allowed_policy_scopes,
                    contribution,
                )?;
            }

            for entry in event_entries {
                if entry.kind == "subscribe" {
                    let handler_key = entry.handler_key.ok_or_else(|| {
                        PluginError::InitFailed(format!(
                            "event subscription '{}' requires handler_key",
                            entry.event
                        ))
                    })?;
                    stage_event_subscription(&mut staged, &runtime, &entry.event, &handler_key)?;
                }
            }
        }

        drop(app);
        let pending = ctx
            .plugin_manager()
            .prepare_owner_activation(staged)
            .await
            .map_err(|err| PluginError::InitFailed(err.to_string()))?;
        replace_plugin_policy_bindings(ctx.storage(), plugin_name, &policy_bindings)
            .await
            .map_err(|err| {
                PluginError::InitFailed(format!(
                    "failed to persist policy bindings for plugin {plugin_name}: {err}"
                ))
            })?;
        ctx.plugin_manager()
            .publish_lua_activation(plugin_name, pending, runtime)
            .await;
        ctx.start_registered_tasks().await;

        tracing::info!(
            "plugin loaded: {} v{}",
            plugin_name,
            self.manifest.plugin.version
        );
        Ok(())
    }
}

#[cfg(test)]
impl LuaPlugin {
    async fn activate_for_test(&self, ctx: &SushiContext) -> Result<(), PluginError> {
        let plugin_context = ctx.plugin_context_for(
            self.instance_id.clone(),
            self.config.clone(),
            &self.effective_permissions,
        );
        self.activate(&plugin_context).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::JwtService;
    use crate::config::ConfigStore;
    use crate::plugin::PluginManifest;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use crate::web::template_service::TemplateService;
    use serde_json::Value;
    use std::ops::Deref;
    use tempfile::TempDir;

    struct TestContext {
        ctx: SushiContext,
        _templates_dir: TempDir,
    }

    impl Deref for TestContext {
        type Target = SushiContext;

        fn deref(&self) -> &Self::Target {
            &self.ctx
        }
    }

    async fn test_context() -> TestContext {
        let config = ConfigStore::new(crate::config::SushiConfig::default());
        let db = SqliteStorage::new_in_memory().await.unwrap();
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);

        let templates_dir = tempfile::tempdir().unwrap();
        let templates = TemplateService::new(templates_dir.path()).unwrap();

        TestContext {
            ctx: SushiContext::new(config, db, jwt, templates),
            _templates_dir: templates_dir,
        }
    }

    #[tokio::test]
    async fn runtime_host_exposes_discovery_migration_and_failure_states() {
        let temp = TempDir::new().unwrap();
        let plugin_dir = create_plugin_dir(temp.path(), "official", "lifecycle_probe");
        let plugin = LuaPlugin::load_dir(&plugin_dir, "official/lifecycle_probe")
            .await
            .unwrap();
        let host = RuntimeHost::new();

        host.register_lua_source(&plugin, false).await;
        assert_eq!(
            host.status("lifecycle_probe").await.unwrap().state,
            PluginLifecycleState::Discovered
        );

        host.begin_migration("lifecycle_probe").await.unwrap();
        assert_eq!(
            host.status("lifecycle_probe").await.unwrap().state,
            PluginLifecycleState::Migrating
        );

        host.complete_migration("lifecycle_probe").await.unwrap();
        assert_eq!(
            host.status("lifecycle_probe").await.unwrap().state,
            PluginLifecycleState::Resolved
        );

        host.record_failure("lifecycle_probe", "migration checksum mismatch")
            .await
            .unwrap();
        let status = host.status("lifecycle_probe").await.unwrap();
        assert_eq!(status.state, PluginLifecycleState::Failed);
        assert_eq!(
            status.last_error.as_deref(),
            Some("migration checksum mismatch")
        );
    }

    fn resolve_page_assets_for_test(
        plugin_path_id: &str,
        manifest: &PluginManifest,
        bundle_names: &[String],
        page_js: &[String],
        page_css: &[String],
        static_root: &Path,
    ) -> Result<PageResolvedAssets, PluginError> {
        resolve_page_assets(
            plugin_path_id,
            manifest,
            bundle_names,
            page_js,
            page_css,
            static_root,
            "/static",
        )
    }

    fn create_plugin_dir(parent: &Path, category: &str, name: &str) -> PathBuf {
        let dir = parent.join(category).join(name);
        std::fs::create_dir_all(&dir).unwrap();

        let manifest_content = format!(
            r#"
schema_version = 1

[plugin]
name = "{name}"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#,
        );
        std::fs::write(dir.join("plugin.toml"), manifest_content).unwrap();

        let init_lua = r#"
sushi.log.info("hello from plugin")
sushi.api.route("GET", "/api/test", function()
    return "ok"
end)
"#;
        std::fs::write(dir.join("init.lua"), init_lua).unwrap();

        dir
    }

    fn assert_contains_method_path_route(source: &str, method: &str, path: &str) {
        let method_pattern = format!("method = \"{method}\"");
        let path_pattern = format!("path = \"{path}\"");
        let mut search_start = 0;

        while let Some(relative_idx) = source[search_start..].find(&method_pattern) {
            let method_idx = search_start + relative_idx;
            let window_end = (method_idx + 220).min(source.len());
            if source[method_idx..window_end].contains(&path_pattern) {
                return;
            }
            search_start = method_idx + method_pattern.len();
        }

        panic!("missing combined method/path route for {method} {path}");
    }

    #[test]
    fn kv_store_plugin_no_longer_embeds_html() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/init.lua");
        let plugin_source = std::fs::read_to_string(&plugin_path).unwrap();

        assert!(!plugin_source.contains("<!DOCTYPE html>"));
        assert!(!plugin_source.contains("<html"));
        assert!(!plugin_source.contains("<div class=\\\"ui-flash"));
        assert!(!plugin_source.contains("app.admin.page"));
        assert!(plugin_source.contains("require(\"bootstrap.register\")"));
        assert!(plugin_source.contains("function app.init()"));

        let template_path = repo_root.join("plugins/official/kv-store/web/templates/kv.html");
        assert!(template_path.exists());
        let template_source = std::fs::read_to_string(&template_path).unwrap();
        assert!(template_source.contains("{% extends \"base.html\" %}"));
        assert!(!template_source.contains("http://"));
        assert!(!template_source.contains("https://"));

        let flash_template_path =
            repo_root.join("plugins/official/kv-store/web/templates/partials/flash.html");
        assert!(flash_template_path.exists());
        let flash_template_source = std::fs::read_to_string(&flash_template_path).unwrap();
        assert!(flash_template_source.contains("data-ui-flash"));
        assert!(flash_template_source.contains("class=\"alert {{ tone }}"));

        let static_path = repo_root.join("plugins/official/kv-store/web/static/kv.js");
        assert!(static_path.exists());
        let static_source = std::fs::read_to_string(&static_path).unwrap();
        assert!(static_source.contains("kvPage"));
        assert!(!static_source.contains("http://"));
        assert!(!static_source.contains("https://"));
    }

    #[test]
    fn kv_store_plugin_declares_admin_asset_bundles() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/plugin.toml");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("schema_version = 1"));
        assert!(!source.contains("kind ="));
        assert!(source.contains("[admin.assets.bundles.workspace]"));
        assert!(source.contains("js = [\"kv.js\"]"));
    }

    #[test]
    fn kv_store_registration_uses_page_assets_option() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("assets = {"));
        assert!(source.contains("bundles = { \"workspace\" }"));
    }

    #[test]
    fn kv_store_plugin_is_split_into_module_files() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(repo_root
            .join("plugins/official/kv-store/lua/utils/json.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/utils/form.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/utils/html.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/infra/db.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/domain/store.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/interfaces/api.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/interfaces/admin.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/interfaces/cli.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/bootstrap/register.lua")
            .is_file());
    }

    #[test]
    fn kv_bootstrap_uses_contract_registration() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("app.capability.register"));
        assert!(!source.contains("app.api.route("));
        assert!(source.contains("definition.surface = \"api\""));
        assert_contains_method_path_route(&source, "GET", "/api/kv");
        assert_contains_method_path_route(&source, "GET", "/api/kv/*");
        assert_contains_method_path_route(&source, "POST", "/api/kv");
        assert_contains_method_path_route(&source, "PUT", "/api/kv/*");
        assert_contains_method_path_route(&source, "DELETE", "/api/kv/*");
        assert!(source.contains("handler = deps.api.dispatch"));
        assert!(source.contains("handler = deps.api.delete_dispatch"));
        assert!(source.contains("policy = \"api.kv.read\""));
        assert!(source.contains("policy = \"api.kv.write\""));
        assert!(source.contains("policy = \"api.kv.delete\""));
        assert_contains_method_path_route(&source, "GET", "/admin/partials/kv/table");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/kv/upsert");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/kv/delete");
        assert!(source.contains("handler = deps.admin.table_partial"));
        assert!(source.contains("handler = deps.admin.upsert_partial"));
        assert!(source.contains("handler = deps.admin.delete_partial"));
        assert!(source.contains("policy = \"admin.kv.manage\""));
    }

    #[test]
    fn file_browser_plugin_is_split_into_module_files() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(repo_root
            .join("plugins/official/file-browser/plugin.toml")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/init.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/bootstrap/register.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/interfaces/web.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/domain/browser.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/utils/form.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/utils/path.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/file_browser.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/fragments/list.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/fragments/editor.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/fragments/flash.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/static/file_browser.js")
            .is_file());
    }

    #[test]
    fn file_browser_bootstrap_uses_contract_registration() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path =
            repo_root.join("plugins/official/file-browser/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("app.capability.register"));
        assert!(!source.contains("sushi.api.route("));
        assert!(source.contains("definition.surface = \"api\""));
        assert!(source.contains("definition.public = true"));
        assert!(source.contains("local prefix = tostring(config.route_prefix or \"/app/files\")"));
        assert!(source.contains("local function route(suffix)"));
        assert!(source.contains("route(\"/list/*\")"));
        assert!(source.contains("route(\"/open/*\")"));
        assert!(source.contains("route(\"/save/*\")"));
        assert!(source.contains("route(\"/create-text\")"));
        assert!(source.contains("route(\"/create-dir\")"));
        assert!(source.contains("route(\"/rename\")"));
        assert!(source.contains("route(\"/delete\")"));
        assert!(source.contains("route(\"/upload/*\")"));
        assert!(source.contains("route(\"/download/*\")"));
    }

    #[test]
    fn cms_plugin_files_exist_and_are_modular() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(repo_root.join("plugins/official/cms/plugin.toml").is_file());
        assert!(repo_root
            .join("plugins/official/cms/web/templates/cms.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/web/templates/fragments/rows.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/web/static/cms.js")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/lua/interfaces/api.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/lua/interfaces/admin.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/lua/interfaces/cli.lua")
            .is_file());
    }

    #[test]
    fn cms_bootstrap_uses_contract_registration() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/cms/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("app.capability.register"));
        assert!(!source.contains("app.api.route("));
        assert!(!source.contains("app.cli.command("));
        assert!(!source.contains("app.web.page("));

        assert!(source.contains("definition.surface = \"api\""));
        assert!(source.contains("definition.surface = \"web\""));
        assert!(source.contains("definition.surface = \"cli\""));
        assert!(source.contains("definition.kind = \"page\""));

        assert_contains_method_path_route(&source, "GET", "/api/cms/pages");
        assert_contains_method_path_route(&source, "POST", "/api/cms/pages");
        assert_contains_method_path_route(&source, "PUT", "/api/cms/pages/*");
        assert_contains_method_path_route(&source, "DELETE", "/api/cms/pages/*");
        assert_contains_method_path_route(&source, "GET", "/api/cms/posts");
        assert_contains_method_path_route(&source, "POST", "/api/cms/posts");
        assert_contains_method_path_route(&source, "PUT", "/api/cms/posts/*");
        assert_contains_method_path_route(&source, "DELETE", "/api/cms/posts/*");
        assert_contains_method_path_route(&source, "GET", "/api/cms/categories");
        assert_contains_method_path_route(&source, "POST", "/api/cms/categories");
        assert_contains_method_path_route(&source, "PUT", "/api/cms/categories/*");
        assert_contains_method_path_route(&source, "DELETE", "/api/cms/categories/*");
        assert_contains_method_path_route(&source, "GET", "/app/cms");
        assert_contains_method_path_route(&source, "GET", "/app/pages");
        assert_contains_method_path_route(&source, "GET", "/app/pages/*");
        assert_contains_method_path_route(&source, "GET", "/app/posts");
        assert_contains_method_path_route(&source, "GET", "/app/partials/cms/posts");
        assert_contains_method_path_route(&source, "GET", "/app/posts/*");
        assert_contains_method_path_route(&source, "GET", "/app/categories/*");
        assert_contains_method_path_route(&source, "GET", "/admin/preview/cms/pages/*");
        assert_contains_method_path_route(&source, "GET", "/admin/preview/cms/posts/*");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/pages/table");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/pages/upsert");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/pages/delete");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/posts/table");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/posts/upsert");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/posts/delete");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/categories/table");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/categories/upsert");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/categories/delete");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/overview");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/library/*");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/editor/*");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/editor/save");
        assert_contains_method_path_route(&source, "POST", "/admin/partials/cms/status/transition");
        assert_contains_method_path_route(&source, "GET", "/admin/partials/cms/commands");
        assert!(source.contains("path = \"/admin/cms\""));
        assert!(source.contains("template = \"plugins/official/cms/cms.html\""));
        assert!(source.contains("name = \"cms\""));
        assert!(source.contains("description = \"CMS CRUD command\""));
    }

    #[test]
    fn cms_plugin_declares_workspace_css_bundle() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let source =
            std::fs::read_to_string(repo_root.join("plugins/official/cms/plugin.toml")).unwrap();

        assert!(source.contains("[admin.assets.bundles.workspace]"));
        assert!(source.contains("js = [\"cms.js\"]"));
        assert!(source.contains("css = [\"cms.css\"]"));
    }

    #[test]
    fn cms_utils_contract_is_stable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let slug =
            std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/slug.lua")).unwrap();
        let validate =
            std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/validate.lua"))
                .unwrap();
        let markdown =
            std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/markdown.lua"))
                .unwrap();

        assert!(slug.contains("function M.normalize"));
        assert!(validate.contains("function M.validate_status"));
        assert!(markdown.contains("function M.to_html"));
    }

    #[tokio::test]
    async fn test_scan_dir_finds_plugins() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "official", "my_plugin");

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name(), "my_plugin");
        assert_eq!(plugins[0].version(), "0.1.0");
        assert_eq!(plugins[0].path_id(), "official/my_plugin");
        assert_eq!(plugins[0].kind(), PluginKind::Official);
    }

    #[tokio::test]
    async fn test_scan_dir_skips_dirs_without_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("official").join("no_manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("init.lua"), "print('hello')").unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_dir_tiered_discovery_success() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "official", "kv_store");
        create_plugin_dir(tmp.path(), "third_party", "notes");

        let mut plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        plugins.sort_by(|left, right| left.path_id().cmp(right.path_id()));

        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].path_id(), "official/kv_store");
        assert_eq!(plugins[0].kind(), PluginKind::Official);
        assert_eq!(
            plugins[0].effective_permissions().database,
            crate::plugin::DatabasePermission::None
        );
        assert_eq!(plugins[1].path_id(), "third_party/notes");
        assert_eq!(plugins[1].kind(), PluginKind::ThirdParty);
        assert_eq!(
            plugins[1].effective_permissions().database,
            crate::plugin::DatabasePermission::None
        );
    }

    #[tokio::test]
    async fn test_scan_dir_rejects_missing_manifest_schema_version() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("third_party").join("legacy_schema");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "legacy_schema"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();
        std::fs::write(plugin_dir.join("init.lua"), "").unwrap();

        let error = match LuaPlugin::scan_dir(tmp.path()).await {
            Ok(_) => panic!("missing manifest schema version must be rejected"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("missing required schema_version 1"));
        assert!(message.contains("schema_version = 1"));
    }

    #[tokio::test]
    async fn test_scan_dir_rejects_legacy_flat_plugin_directory() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "official", "modern");

        let legacy = tmp.path().join("legacy_flat");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "legacy_flat"
version = "0.1.0"
"#,
        )
        .unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("legacy flat plugin directories"));
        assert!(err.contains("legacy_flat"));
    }

    #[test]
    fn profile_file_browser_config_rejects_invalid_route_prefix() {
        let config = resolve_file_browser_config(&serde_json::json!({
            "file_browser": { "route_prefix": "admin/files" }
        }))
        .unwrap();

        let error = validate_optional_file_browser_config(config.as_ref()).unwrap_err();
        assert!(error.contains("route_prefix"));
    }

    #[test]
    fn profile_file_browser_config_accepts_relative_root_paths() {
        let config = resolve_file_browser_config(&serde_json::json!({
            "file_browser": {
                "route_prefix": "/app/files",
                "roots": [{ "id": "docs", "path": "docs" }]
            }
        }))
        .unwrap();

        validate_optional_file_browser_config(config.as_ref()).unwrap();
    }

    #[test]
    fn profile_file_browser_config_rejects_whitespace_values() {
        for (config, expected) in [
            (
                serde_json::json!({ "file_browser": { "route_prefix": " /app/files" } }),
                "route_prefix",
            ),
            (
                serde_json::json!({
                    "file_browser": {
                        "route_prefix": "/app/files",
                        "roots": [{ "id": "docs ", "path": "docs" }]
                    }
                }),
                "root id",
            ),
            (
                serde_json::json!({
                    "file_browser": {
                        "route_prefix": "/app/files",
                        "roots": [{ "id": "docs", "path": " docs" }]
                    }
                }),
                "root path",
            ),
        ] {
            let config = resolve_file_browser_config(&config).unwrap();
            let error = validate_optional_file_browser_config(config.as_ref()).unwrap_err();
            assert!(error.contains(expected), "unexpected error: {error}");
        }
    }

    #[tokio::test]
    async fn test_scan_dir_uses_host_path_tier_for_trust() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("official").join("mismatch");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "mismatch"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();
        std::fs::write(plugin_dir.join("init.lua"), "sushi.log.info('hi')").unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        let plugins = result.expect("host path tier determines trust");
        assert_eq!(plugins[0].kind(), PluginKind::Official);
    }

    #[tokio::test]
    async fn test_lua_plugin_init_executes_entry_script() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "third_party", "test_plugin");

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;

        // init() should succeed without error
        plugins[0].activate_for_test(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_lua_plugin_init_calls_sushi_init() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("init_fn_plugin");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "init_fn_plugin"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.log.info("init called!")
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;

        plugins[0].activate_for_test(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn disabled_plugin_is_not_invoked_after_scan_registration() {
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();

        let manifest = PluginManifest {
            schema_version: PluginManifest::CURRENT_SCHEMA_VERSION,
            plugin: crate::plugin::PluginMeta {
                name: "notes".to_string(),
                version: "0.1.0".to_string(),
                description: "notes".to_string(),
                entry: "init.lua".to_string(),
            },
            permissions: crate::plugin::Permissions::default(),
            policies: crate::plugin::PluginPoliciesConfig::default(),
            admin: None,
        };

        ctx.plugins
            .register_plugin_manifest_with_permissions_and_identity(
                &manifest,
                &crate::plugin::Permissions::default(),
                "third_party/notes",
                crate::plugin::PluginKind::ThirdParty,
            )
            .await;

        ctx.set_plugin_enabled("notes", false, Some("admin"), Some("seed"))
            .await
            .unwrap();

        assert_eq!(
            ctx.plugins
                .list_plugins()
                .await
                .into_iter()
                .find(|p| p.name == "notes")
                .unwrap()
                .enabled,
            false
        );
    }

    #[tokio::test]
    async fn plugin_load_fails_when_declared_policy_outside_scope() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_mismatch");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "policy_mismatch"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true

[policies]
scopes = ["api.plugin.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/mismatch", function()
        return "ok"
    end, { policy = "admin.users.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        let err = plugins[0].activate_for_test(&ctx).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("policy_mismatch"));
        assert!(msg.contains("admin.users.read"));
        assert!(msg.contains("api.plugin.*"));
    }

    #[tokio::test]
    async fn loader_reads_contract_registry_for_api_routes() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("contract_case");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "contract_case"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true

[policies]
scopes = ["api.notes.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    local h = function()
        return "ok"
    end
    sushi.capability.register({
        surface = "api",
        method = "GET",
        path = "/api/notes",
        handler = h,
        policy = "api.notes.read"
    })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();

        plugins[0]
            .activate_for_test(&ctx)
            .await
            .expect("plugin initializes");

        assert_eq!(
            ctx.plugins
                .api_route_policy("GET", "/api/notes")
                .await
                .as_deref(),
            Some("api.notes.read")
        );
    }

    #[tokio::test]
    async fn loader_reads_contract_registry_for_menu_contributions() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("contract_menu");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "contract_menu"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[policies]
scopes = ["admin.notes.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.capability.register({
        surface = "menu",
        id = "notes.menu",
        label = "Notes",
        icon = "notebook",
        position = 80,
        parent_id = "host-admin.plugins",
        route = "/admin/notes",
        policy = "admin.notes.view"
    })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();

        plugins[0]
            .activate_for_test(&ctx)
            .await
            .expect("plugin initializes");

        let snapshot = ctx.plugins.capability_snapshot().await;
        let contribution = snapshot
            .menu_contributions()
            .iter()
            .find(|registration| registration.value.id == "notes.menu")
            .expect("Lua menu contribution should be registered");
        assert_eq!(contribution.value.label, "Notes");
        assert_eq!(contribution.value.icon.as_deref(), Some("notebook"));
        assert_eq!(contribution.value.position, 80);
        assert_eq!(
            contribution.value.parent_id.as_deref(),
            Some("host-admin.plugins")
        );
        assert_eq!(contribution.value.route.as_deref(), Some("/admin/notes"));
        assert_eq!(
            contribution.value.policy_key.as_deref(),
            Some("admin.notes.view")
        );
    }

    #[tokio::test]
    async fn loader_resolves_contract_registry_web_page_assets() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("contract_web_assets");
        std::fs::create_dir_all(dir.join("web/static")).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "contract_web_assets"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["notes.js"]
css = ["notes.css"]
"#,
        )
        .unwrap();

        std::fs::write(dir.join("web/static/notes.js"), "console.log('notes');").unwrap();
        std::fs::write(
            dir.join("web/static/notes.css"),
            ".notes-panel { color: #0f172a; }",
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.capability.register({
        surface = "web",
        kind = "page",
        path = "/admin/notes",
        title = "Notes",
        handler = function()
            return "notes"
        end,
        assets = {
            bundles = { "workspace" }
        }
    })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();

        plugins[0]
            .activate_for_test(&ctx)
            .await
            .expect("plugin initializes");

        let assets = ctx
            .plugins
            .admin_page_assets("/admin/notes")
            .await
            .expect("missing admin assets for /admin/notes");

        assert_eq!(
            assets.js,
            vec!["/static/plugins/third_party/contract_web_assets/notes.js"]
        );
        assert_eq!(
            assets.css,
            vec!["/static/plugins/third_party/contract_web_assets/notes.css"]
        );
    }

    #[tokio::test]
    async fn plugin_load_persists_policy_metadata_for_registrations() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_capture");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "policy_capture"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true

[policies]
scopes = ["api.notes.*", "cli.notes.run", "admin.notes.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function()
        return "api"
    end, { policy = "api.notes.read" })

    sushi.api.route("GET", "/admin/partials/notes/table", function()
        return "admin partial"
    end, { policy = "admin.notes.read" })

    sushi.cli.command("notes-run", "Run notes command", function()
        return "cli"
    end, { policy = "cli.notes.run" })

    sushi.admin.page("/admin/notes", "Notes", function()
        return "admin"
    end, { policy = "admin.notes.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        plugins[0].activate_for_test(&ctx).await.unwrap();

        assert_eq!(
            ctx.plugins
                .api_route_policy("GET", "/api/notes")
                .await
                .as_deref(),
            Some("api.notes.read")
        );
        assert_eq!(
            ctx.plugins
                .api_route_policy("GET", "/admin/partials/notes/table")
                .await
                .as_deref(),
            Some("admin.notes.read")
        );
        assert_eq!(
            ctx.plugins.cli_command_policy("notes-run").await.as_deref(),
            Some("cli.notes.run")
        );
        assert_eq!(
            ctx.plugins
                .admin_page_policy("/admin/notes")
                .await
                .as_deref(),
            Some("admin.notes.read")
        );

        let key_rows = ctx
            .db
            .query(
                r#"
                SELECT key
                FROM policy_keys
                WHERE key IN ('api.notes.read', 'cli.notes.run', 'admin.notes.read')
                ORDER BY key ASC
                "#,
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(key_rows.len(), 3);

        let binding_rows = ctx
            .db
            .query(
                r#"
                SELECT
                    pb.surface,
                    pb.target_type,
                    pb.target_ref,
                    pb.method,
                    pb.path_pattern,
                    pb.command_name,
                    pk.key AS policy_key,
                    pb.owner_type,
                    pb.owner_id
                FROM policy_bindings pb
                JOIN policy_keys pk ON pk.id = pb.policy_key_id
                WHERE pb.owner_type = 'plugin'
                  AND pb.owner_id = 'policy_capture'
                ORDER BY pb.surface ASC, pb.target_type ASC, pb.target_ref ASC
                "#,
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(binding_rows.len(), 4);

        let api_route_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("surface")
                    .and_then(Value::as_str)
                    .map(|surface| surface == "api")
                    .unwrap_or(false)
                    && row
                        .get("path_pattern")
                        .and_then(Value::as_str)
                        .map(|path| path == "/api/notes")
                        .unwrap_or(false)
            })
            .expect("missing api policy binding");
        assert_eq!(
            api_route_binding
                .get("target_type")
                .and_then(Value::as_str)
                .unwrap(),
            "http_route"
        );
        assert_eq!(
            api_route_binding
                .get("path_pattern")
                .and_then(Value::as_str)
                .unwrap(),
            "/api/notes"
        );
        assert_eq!(
            api_route_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "api.notes.read"
        );

        let admin_page_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("surface")
                    .and_then(Value::as_str)
                    .map(|surface| surface == "admin")
                    .unwrap_or(false)
                    && row
                        .get("path_pattern")
                        .and_then(Value::as_str)
                        .map(|path| path == "/admin/notes")
                        .unwrap_or(false)
            })
            .expect("missing admin page policy binding");
        assert_eq!(
            admin_page_binding
                .get("target_type")
                .and_then(Value::as_str)
                .unwrap(),
            "http_route"
        );
        assert_eq!(
            admin_page_binding
                .get("path_pattern")
                .and_then(Value::as_str)
                .unwrap(),
            "/admin/notes"
        );
        assert_eq!(
            admin_page_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "admin.notes.read"
        );

        let admin_route_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("surface")
                    .and_then(Value::as_str)
                    .map(|surface| surface == "admin")
                    .unwrap_or(false)
                    && row
                        .get("path_pattern")
                        .and_then(Value::as_str)
                        .map(|path| path == "/admin/partials/notes/table")
                        .unwrap_or(false)
            })
            .expect("missing admin route policy binding");
        assert_eq!(
            admin_route_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "admin.notes.read"
        );
        assert!(
            !binding_rows.iter().any(|row| {
                row.get("surface")
                    .and_then(Value::as_str)
                    .map(|surface| surface == "api")
                    .unwrap_or(false)
                    && row
                        .get("path_pattern")
                        .and_then(Value::as_str)
                        .map(|path| path == "/admin/partials/notes/table")
                        .unwrap_or(false)
            }),
            "admin-prefixed routes must not persist api-surface bindings"
        );

        let cli_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("target_type")
                    .and_then(Value::as_str)
                    .map(|target_type| target_type == "cli_command")
                    .unwrap_or(false)
            })
            .expect("missing cli policy binding");
        assert_eq!(
            cli_binding
                .get("command_name")
                .and_then(Value::as_str)
                .unwrap(),
            "notes-run"
        );
        assert_eq!(
            cli_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "cli.notes.run"
        );
    }

    #[tokio::test]
    async fn plugin_load_without_policy_clears_stale_plugin_bindings() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_capture");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "policy_capture"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true

[policies]
scopes = ["api.notes.*", "cli.notes.run", "admin.notes.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function()
        return "api"
    end, { policy = "api.notes.read" })

    sushi.cli.command("notes-run", "Run notes command", function()
        return "cli"
    end, { policy = "cli.notes.run" })

    sushi.admin.page("/admin/notes", "Notes", function()
        return "admin"
    end, { policy = "admin.notes.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        plugins[0].activate_for_test(&ctx).await.unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function()
        return "api"
    end)

    sushi.cli.command("notes-run", "Run notes command", function()
        return "cli"
    end)

    sushi.admin.page("/admin/notes", "Notes", function()
        return "admin"
    end)
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        plugins[0].activate_for_test(&ctx).await.unwrap();

        assert_eq!(
            ctx.plugins.api_route_policy("GET", "/api/notes").await,
            None
        );
        assert_eq!(ctx.plugins.cli_command_policy("notes-run").await, None);
        assert_eq!(ctx.plugins.admin_page_policy("/admin/notes").await, None);

        let rows = ctx
            .db
            .query(
                r#"
                SELECT COUNT(*) AS count
                FROM policy_bindings pb
                WHERE pb.owner_type = 'plugin'
                  AND pb.owner_id = 'policy_capture'
                  AND (
                      pb.target_ref = '/api/notes'
                      OR pb.target_ref = '/admin/notes'
                      OR pb.target_ref = 'notes-run'
                  )
                "#,
                vec![],
            )
            .await
            .unwrap();

        let count = rows
            .first()
            .and_then(|row| row.get("count"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        assert_eq!(count, 0, "stale plugin policy bindings should be removed");
    }

    #[tokio::test]
    async fn test_lua_plugin_init_bad_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("official").join("bad_plugin");
        std::fs::create_dir_all(&dir).unwrap();

        // Invalid TOML
        std::fs::write(dir.join("plugin.toml"), "this is not valid toml [[[[").unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn page_assets_resolve_bundle_then_page_assets() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("asset_plugin");
        std::fs::create_dir_all(plugin_dir.join("web/static/pages")).unwrap();
        std::fs::write(plugin_dir.join("web/static/kv.js"), "console.log('kv')").unwrap();
        std::fs::write(
            plugin_dir.join("web/static/pages/extra.js"),
            "console.log('extra')",
        )
        .unwrap();

        let manifest: PluginManifest = toml::from_str(
            r#"
schema_version = 1

[plugin]
name = "asset_plugin"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["kv.js"]
"#,
        )
        .unwrap();

        let resolved = resolve_page_assets_for_test(
            "third_party/asset_plugin",
            &manifest,
            &["workspace".to_string()],
            &["pages/extra.js".to_string()],
            &[],
            &plugin_dir.join("web/static"),
        )
        .unwrap();

        assert_eq!(
            resolved.js,
            vec![
                "/static/plugins/third_party/asset_plugin/kv.js".to_string(),
                "/static/plugins/third_party/asset_plugin/pages/extra.js".to_string()
            ]
        );
        assert!(resolved.css.is_empty());
    }

    #[tokio::test]
    async fn page_assets_fail_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("asset_plugin");
        std::fs::create_dir_all(plugin_dir.join("web/static")).unwrap();

        let manifest: PluginManifest = toml::from_str(
            r#"
schema_version = 1

[plugin]
name = "asset_plugin"
version = "0.1.0"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["missing.js"]
"#,
        )
        .unwrap();

        let err = resolve_page_assets_for_test(
            "third_party/asset_plugin",
            &manifest,
            &["workspace".to_string()],
            &[],
            &[],
            &plugin_dir.join("web/static"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("missing.js"));
    }

    #[tokio::test]
    async fn failed_init_leaks_no_capabilities_policy_bindings_or_vm() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("failed_activation");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "failed_activation"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true

[policies]
scopes = ["api.failed.read"]
"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("web/templates")).unwrap();
        std::fs::create_dir_all(dir.join("web/static")).unwrap();
        std::fs::write(dir.join("web/templates/page.html"), "never visible").unwrap();
        std::fs::write(dir.join("web/static/app.js"), "never visible").unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.task.spawn("never-started", function()
        sushi.config.get("task_started")
    end)
    sushi.event.on("activation.failed", function() end)
    sushi.api.route("GET", "/api/failed", function()
        return "never published"
    end, { policy = "api.failed.read" })
    sushi.api.route("GET", "/api/rejected", function()
        return "also never published"
    end, { policy = "api.rejected.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();

        let err = plugins[0].activate_for_test(&ctx).await.unwrap_err();
        assert!(err.to_string().contains("api.rejected.read"));
        assert!(ctx
            .plugins
            .capability_snapshot()
            .await
            .http_routes()
            .is_empty());
        let snapshot = ctx.plugins.capability_snapshot().await;
        assert!(snapshot.template_roots().is_empty());
        assert!(snapshot.static_roots().is_empty());
        assert!(snapshot.event_subscriptions().is_empty());
        assert!(ctx
            .plugins
            .call_api_handler("GET", "/api/failed", None)
            .await
            .is_none());
        assert!(!ctx.plugins.has_vm("failed_activation").await);
        assert_eq!(ctx.tasks.active_count(&plugins[0].instance_id).await, 0);

        let policy_rows = ctx
            .db
            .query(
                "SELECT key FROM policy_keys WHERE key = 'api.failed.read'",
                vec![],
            )
            .await
            .unwrap();
        assert!(policy_rows.is_empty());
    }

    #[tokio::test]
    async fn lua_event_subscriptions_publish_only_after_successful_activation() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("event_listener");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "event_listener"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = false
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.event.on("notes.changed", function(data)
        sushi.config.last_event = data.value
    end)
end
"#,
        )
        .unwrap();

        let plugin = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let ctx = test_context().await;
        plugin.activate_for_test(&ctx).await.unwrap();
        assert_eq!(
            ctx.plugins
                .capability_snapshot()
                .await
                .event_subscriptions()
                .len(),
            1
        );

        ctx.event
            .emit("notes.changed", &serde_json::json!({"value": "received"}))
            .await;
        let runtime = ctx
            .plugins
            .capability_snapshot()
            .await
            .event_subscriptions()[0]
            .value
            .lua_runtime
            .clone()
            .unwrap();
        let sushi: mlua::Table = runtime.lua().globals().get("sushi").unwrap();
        let config: mlua::Table = sushi.get("config").unwrap();
        assert_eq!(config.get::<String>("last_event").unwrap(), "received");
    }

    #[tokio::test]
    async fn runtime_host_disables_and_reenables_optional_lua_plugin_without_restart() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("runtime_toggle");
        std::fs::create_dir_all(dir.join("web/templates")).unwrap();
        std::fs::create_dir_all(dir.join("web/static")).unwrap();
        std::fs::write(dir.join("web/templates/page.html"), "toggle template").unwrap();
        std::fs::write(dir.join("web/static/app.js"), "toggle static").unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "runtime_toggle"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.task.interval("runtime-loop", 5, function()
        task_ticks = (task_ticks or 0) + 1
    end)
    sushi.api.route("GET", "/api/runtime-toggle", function()
        return "active"
    end)
    sushi.event.on("runtime.toggle", function() end)
end
"#,
        )
        .unwrap();

        let plugin = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();
        ctx.plugins
            .register_profile_plugin_manifest(
                plugin.manifest(),
                plugin.effective_permissions(),
                plugin.path_id(),
                plugin.kind(),
                true,
                false,
            )
            .await;
        ctx.runtime_host.register_lua_source(&plugin, false).await;
        ctx.runtime_host
            .activate(&ctx, "runtime_toggle")
            .await
            .unwrap();

        let handle = ctx
            .runtime_host
            .handle("runtime_toggle")
            .await
            .expect("active plugin should expose a lifecycle handle");
        assert_eq!(handle.state, PluginLifecycleState::Active);
        assert!(!handle.registrations.is_empty());
        assert_eq!(handle.tasks.len(), 1);
        assert!(!handle.cancellation.is_cancelled());
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        let runtime = ctx.plugins.lua_runtime("runtime_toggle").await.unwrap();
        assert!(runtime.lua().globals().get::<u64>("task_ticks").unwrap() > 0);

        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/runtime-toggle", None)
                .await
                .unwrap()
                .unwrap(),
            "active"
        );
        let initial_runtime_id = ctx.plugins.capability_snapshot().await.http_routes()[0]
            .value
            .lua_runtime
            .as_ref()
            .unwrap()
            .id();

        let unchanged = ctx
            .set_plugin_enabled(
                "runtime_toggle",
                true,
                Some("admin"),
                Some("already enabled"),
            )
            .await
            .unwrap();
        assert!(unchanged.loaded);
        assert_eq!(
            ctx.plugins.capability_snapshot().await.http_routes()[0]
                .value
                .lua_runtime
                .as_ref()
                .unwrap()
                .id(),
            initial_runtime_id,
            "idempotent enable must not reload the Lua generation"
        );

        let disabled = ctx
            .set_plugin_enabled("runtime_toggle", false, Some("admin"), Some("maintenance"))
            .await
            .unwrap();
        assert!(!disabled.enabled);
        assert!(!disabled.loaded);
        assert!(!ctx.plugins.has_vm("runtime_toggle").await);
        assert!(ctx.runtime_host.handle("runtime_toggle").await.is_none());
        assert_eq!(ctx.tasks.active_count(&handle.owner).await, 0);
        let snapshot = ctx.plugins.capability_snapshot().await;
        assert!(snapshot.http_routes().is_empty());
        assert!(snapshot.template_roots().is_empty());
        assert!(snapshot.static_roots().is_empty());
        assert!(snapshot.event_subscriptions().is_empty());

        let enabled = ctx
            .set_plugin_enabled(
                "runtime_toggle",
                true,
                Some("admin"),
                Some("maintenance complete"),
            )
            .await
            .unwrap();
        assert!(enabled.enabled);
        assert!(enabled.loaded);
        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/runtime-toggle", None)
                .await
                .unwrap()
                .unwrap(),
            "active"
        );
    }

    #[tokio::test]
    async fn required_plugin_toggle_returns_stable_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("official").join("required_toggle");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "required_toggle"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();
        std::fs::write(dir.join("init.lua"), "").unwrap();
        let plugin = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();
        ctx.plugins
            .register_profile_plugin_manifest(
                plugin.manifest(),
                plugin.effective_permissions(),
                plugin.path_id(),
                plugin.kind(),
                true,
                true,
            )
            .await;
        ctx.runtime_host.register_lua_source(&plugin, true).await;

        let error = ctx
            .set_plugin_enabled("required_toggle", false, Some("admin"), Some("test"))
            .await
            .unwrap_err();
        assert!(error.starts_with("required_plugin_toggle_forbidden:"));
    }

    #[tokio::test]
    async fn failed_enable_keeps_enabled_intent_and_loaded_false() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("broken_enable");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "broken_enable"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();
        std::fs::write(dir.join("init.lua"), "error('activation failed')").unwrap();
        let plugin = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();
        ctx.plugins
            .register_plugin_manifest_with_permissions_and_identity(
                plugin.manifest(),
                plugin.effective_permissions(),
                plugin.path_id(),
                plugin.kind(),
            )
            .await;
        ctx.runtime_host.register_lua_source(&plugin, false).await;
        ctx.set_plugin_enabled("broken_enable", false, Some("seed"), Some("disabled"))
            .await
            .unwrap();

        let error = ctx
            .set_plugin_enabled("broken_enable", true, Some("admin"), Some("retry"))
            .await
            .unwrap_err();
        assert!(error.contains("activation failed"));
        let state = ctx
            .plugins
            .list_plugins()
            .await
            .into_iter()
            .find(|plugin| plugin.name == "broken_enable")
            .unwrap();
        assert!(state.enabled);
        assert!(!state.loaded);
    }

    #[tokio::test]
    async fn failed_reload_preserves_previous_snapshot_and_runtime() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("reload_safe");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "reload_safe"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.api.route("GET", "/api/reload-safe", function()
    return "old"
end)
"#,
        )
        .unwrap();
        let plugin = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();
        ctx.plugins
            .register_plugin_manifest_with_permissions_and_identity(
                plugin.manifest(),
                plugin.effective_permissions(),
                plugin.path_id(),
                plugin.kind(),
            )
            .await;
        ctx.runtime_host.register_lua_source(&plugin, false).await;
        ctx.runtime_host
            .activate(&ctx, "reload_safe")
            .await
            .unwrap();
        let before = ctx.plugins.capability_snapshot().await;

        std::fs::write(dir.join("init.lua"), "error('reload rejected')").unwrap();
        let error = ctx
            .runtime_host
            .reload(&ctx, "reload_safe")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("reload rejected"));
        assert_eq!(
            ctx.plugins.capability_snapshot().await.as_ref(),
            before.as_ref()
        );
        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/reload-safe", None)
                .await
                .unwrap()
                .unwrap(),
            "old"
        );
        let status = ctx.runtime_host.status("reload_safe").await.unwrap();
        assert_eq!(status.state, PluginLifecycleState::Active);
        assert!(status
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("reload rejected"));
    }

    #[tokio::test]
    async fn successful_reload_replaces_previous_generation_tasks() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("reload_tasks");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "reload_tasks"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
function sushi.init()
    sushi.task.interval("runtime-loop", 1000, function() end)
end
"#,
        )
        .unwrap();
        let plugin = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();
        ctx.plugins
            .register_plugin_manifest_with_permissions_and_identity(
                plugin.manifest(),
                plugin.effective_permissions(),
                plugin.path_id(),
                plugin.kind(),
            )
            .await;
        ctx.runtime_host.register_lua_source(&plugin, false).await;
        ctx.runtime_host
            .activate(&ctx, "reload_tasks")
            .await
            .unwrap();
        let before = ctx.runtime_host.handle("reload_tasks").await.unwrap();
        assert_eq!(before.tasks.len(), 1);

        ctx.runtime_host.reload(&ctx, "reload_tasks").await.unwrap();

        let after = ctx.runtime_host.handle("reload_tasks").await.unwrap();
        assert_eq!(after.tasks.len(), 1);
        assert_ne!(after.tasks[0].id, before.tasks[0].id);
        assert_eq!(ctx.tasks.active_count(&after.owner).await, 1);
        ctx.tasks
            .cancel_owner(&after.owner, std::time::Duration::from_secs(1))
            .await;
    }

    #[tokio::test]
    async fn conflicting_activation_preserves_previous_snapshot_and_vm() {
        let tmp = TempDir::new().unwrap();
        for (name, body) in [("first", "first"), ("second", "second")] {
            let dir = tmp.path().join("third_party").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("plugin.toml"),
                format!(
                    r#"
schema_version = 1

[plugin]
name = "{name}"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#
                ),
            )
            .unwrap();
            std::fs::write(
                dir.join("init.lua"),
                format!(
                    r#"
sushi.init = function()
    sushi.api.route("GET", "/api/shared", function()
        return "{body}"
    end)
end
"#
                ),
            )
            .unwrap();
        }

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let first = plugins
            .iter()
            .find(|plugin| plugin.name() == "first")
            .unwrap();
        let second = plugins
            .iter()
            .find(|plugin| plugin.name() == "second")
            .unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();

        first.activate_for_test(&ctx).await.unwrap();
        let before = ctx.plugins.capability_snapshot().await;
        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/shared", None)
                .await
                .unwrap()
                .unwrap(),
            "first"
        );

        let err = second.activate_for_test(&ctx).await.unwrap_err();
        assert!(err.to_string().contains("HTTP route conflict"));
        let after = ctx.plugins.capability_snapshot().await;
        assert_eq!(after.as_ref(), before.as_ref());
        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/shared", None)
                .await
                .unwrap()
                .unwrap(),
            "first"
        );
        assert!(ctx.plugins.has_vm("first").await);
        assert!(!ctx.plugins.has_vm("second").await);
    }

    #[tokio::test]
    async fn contract_and_legacy_registration_produce_equivalent_capabilities() {
        async fn load(
            source: &str,
            name: &str,
        ) -> (crate::runtime::CapabilitySnapshot, Vec<String>) {
            let tmp = TempDir::new().unwrap();
            let dir = tmp.path().join("third_party").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("plugin.toml"),
                format!(
                    r#"
schema_version = 1

[plugin]
name = "{name}"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true

[policies]
scopes = ["api.notes.read", "admin.notes.read", "cli.notes.run"]
"#
                ),
            )
            .unwrap();
            std::fs::write(dir.join("init.lua"), source).unwrap();

            let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
            let ctx = test_context().await;
            ctx.db
                .run_migrations(include_str!("../../../../migrations/001_init.sql"))
                .await
                .unwrap();
            ctx.db
                .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
                .await
                .unwrap();
            ctx.db
                .run_migrations(include_str!(
                    "../../../../migrations/006_unified_policy_v2.sql"
                ))
                .await
                .unwrap();
            plugins[0].activate_for_test(&ctx).await.unwrap();
            let snapshot = ctx.plugins.capability_snapshot().await.as_ref().clone();
            let logs = ctx
                .logs
                .list(100)
                .await
                .into_iter()
                .map(|entry| entry.message)
                .collect();
            (snapshot, logs)
        }

        let (legacy, legacy_logs) = load(
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function() return "api" end, {
        policy = "api.notes.read"
    })
    sushi.admin.page("/admin/notes", "Notes", function() return "admin" end, {
        policy = "admin.notes.read"
    })
    sushi.cli.command("notes-run", "Run notes", function() return "cli" end, {
        policy = "cli.notes.run"
    })
    sushi.web.page("/admin/legacy-notes", "plugins/third_party/legacy_contract/notes.html", {
        title = "Legacy Notes"
    })
end
"#,
            "legacy_contract",
        )
        .await;
        let (contract, contract_logs) = load(
            r#"
sushi.init = function()
    sushi.capability.register({
        surface = "api",
        method = "GET",
        path = "/api/notes",
        handler = function() return "api" end,
        policy = "api.notes.read"
    })
    sushi.capability.register({
        surface = "admin",
        path = "/admin/notes",
        title = "Notes",
        handler = function() return "admin" end,
        policy = "admin.notes.read"
    })
    sushi.capability.register({
        surface = "cli",
        name = "notes-run",
        description = "Run notes",
        handler = function() return "cli" end,
        policy = "cli.notes.run"
    })
end
"#,
            "native_contract",
        )
        .await;

        for api in [
            "sushi.api.route",
            "sushi.admin.page",
            "sushi.cli.command",
            "sushi.web.page",
        ] {
            assert_eq!(
                legacy_logs
                    .iter()
                    .filter(|message| message.contains(api))
                    .count(),
                1,
                "expected one diagnostic for {api}: {legacy_logs:?}"
            );
        }
        assert!(contract_logs
            .iter()
            .all(|message| !message.contains("deprecated Lua registration API")));

        let legacy_route = &legacy.http_routes()[0].value;
        let contract_route = &contract.http_routes()[0].value;
        assert_eq!(
            (
                &legacy_route.method,
                &legacy_route.path,
                &legacy_route.policy_key,
                legacy_route.is_public,
            ),
            (
                &contract_route.method,
                &contract_route.path,
                &contract_route.policy_key,
                contract_route.is_public,
            )
        );

        let legacy_page = &legacy
            .admin_pages()
            .iter()
            .find(|registration| registration.value.path == "/admin/notes")
            .unwrap()
            .value;
        let contract_page = &contract
            .admin_pages()
            .iter()
            .find(|registration| registration.value.path == "/admin/notes")
            .unwrap()
            .value;
        assert_eq!(
            (
                &legacy_page.path,
                &legacy_page.title,
                &legacy_page.policy_key,
                &legacy_page.js,
                &legacy_page.css,
            ),
            (
                &contract_page.path,
                &contract_page.title,
                &contract_page.policy_key,
                &contract_page.js,
                &contract_page.css,
            )
        );

        let legacy_command = &legacy.cli_commands()[0].value;
        let contract_command = &contract.cli_commands()[0].value;
        assert_eq!(
            (
                &legacy_command.name,
                &legacy_command.description,
                &legacy_command.policy_key,
            ),
            (
                &contract_command.name,
                &contract_command.description,
                &contract_command.policy_key,
            )
        );
    }

    #[tokio::test]
    async fn policy_transaction_failure_preserves_previous_activation() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_reload");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "policy_reload"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true

[policies]
scopes = ["api.reload.*"]
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/reload", function()
        return "first"
    end, { policy = "api.reload.read" })
end
"#,
        )
        .unwrap();

        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
            .await
            .unwrap();

        let first = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        first.activate_for_test(&ctx).await.unwrap();
        let before = ctx.plugins.capability_snapshot().await;

        ctx.db
            .execute(
                r#"
                CREATE TRIGGER reject_reload_policy
                BEFORE INSERT ON policy_keys
                WHEN NEW.key = 'api.reload.write'
                BEGIN
                    SELECT RAISE(ABORT, 'policy insert rejected');
                END
                "#,
                vec![],
            )
            .await
            .unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/reload", function()
        return "second"
    end, { policy = "api.reload.write" })
end
"#,
        )
        .unwrap();

        let second = LuaPlugin::scan_dir(tmp.path()).await.unwrap().remove(0);
        let err = second.activate_for_test(&ctx).await.unwrap_err();
        assert!(err.to_string().contains("policy insert rejected"));
        assert_eq!(
            ctx.plugins.capability_snapshot().await.as_ref(),
            before.as_ref()
        );
        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/reload", None)
                .await
                .unwrap()
                .unwrap(),
            "first"
        );

        let bindings = ctx
            .db
            .query(
                r#"
                SELECT pk.key
                FROM policy_bindings pb
                JOIN policy_keys pk ON pk.id = pb.policy_key_id
                WHERE pb.owner_type = 'plugin'
                  AND pb.owner_id = 'policy_reload'
                "#,
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0].get("key").and_then(Value::as_str),
            Some("api.reload.read")
        );
    }
}
