use super::manager::{PluginInfo, PluginPermissionsView};
use super::state_repository::PluginStateRepository;
use super::{DatabasePermission, Permissions, PluginKind, PluginManifest};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub(crate) struct PluginRepository {
    plugin_info: Arc<RwLock<HashMap<String, PluginInfo>>>,
    required_plugins: Arc<RwLock<HashSet<String>>>,
    state: Option<Arc<PluginStateRepository>>,
}

impl PluginRepository {
    pub(crate) fn with_storage(storage: Arc<dyn crate::storage::Storage>) -> Self {
        Self {
            state: Some(Arc::new(PluginStateRepository::new(storage))),
            ..Self::default()
        }
    }

    pub(crate) fn with_sqlite_storage(storage: Arc<crate::storage::sqlite::SqliteStorage>) -> Self {
        Self {
            state: Some(Arc::new(PluginStateRepository::new_sqlite(storage))),
            ..Self::default()
        }
    }

    pub(crate) async fn register_builtin(
        &self,
        plugin_id: &str,
        plugin_name: &str,
        version: &str,
        description: &str,
        permissions: &Permissions,
        default_enabled: bool,
        required: bool,
    ) {
        self.set_required(plugin_name, required).await;
        let existing = self.plugin_info.read().await.get(plugin_name).cloned();
        let mut resolved_plugin_id = plugin_id.to_string();
        let mut enabled = existing
            .as_ref()
            .map(|plugin| plugin.enabled)
            .unwrap_or(default_enabled);
        let mut loaded = existing
            .as_ref()
            .map(|plugin| plugin.loaded)
            .unwrap_or(false);
        let mut resolved_version = version.to_string();

        if let Some(state_repository) = &self.state {
            if let Err(error) = state_repository
                .upsert_profile_plugin(
                    plugin_id,
                    plugin_name,
                    "builtin",
                    version,
                    default_enabled,
                    required,
                )
                .await
            {
                tracing::warn!(
                    plugin = plugin_name,
                    error = %error,
                    "failed to upsert builtin runtime state"
                );
            }

            match state_repository.get_by_name(plugin_name).await {
                Ok(Some(state)) => {
                    resolved_plugin_id = state.plugin_id.to_string();
                    enabled = state.enabled;
                    loaded = state.loaded;
                    if !state.version.is_empty() {
                        resolved_version = state.version;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        plugin = plugin_name,
                        error = %error,
                        "failed to read builtin runtime state"
                    );
                }
            }
        }

        self.plugin_info.write().await.insert(
            plugin_name.to_string(),
            PluginInfo {
                plugin_id: resolved_plugin_id,
                source_kind: "builtin".to_string(),
                name: plugin_name.to_string(),
                version: resolved_version,
                description: description.to_string(),
                enabled,
                loaded,
                permissions: permission_view(permissions),
            },
        );
    }

    pub(crate) async fn register_manifest(
        &self,
        manifest: &PluginManifest,
        effective_permissions: &Permissions,
        plugin_id: &str,
        kind: PluginKind,
        default_enabled: bool,
        required: bool,
    ) {
        let plugin_name = manifest.plugin.name.clone();
        self.set_required(&plugin_name, required).await;
        let manifest_version = manifest.plugin.version.clone();
        let source_kind = kind.tier_name().to_string();
        let existing = self.plugin_info.read().await.get(&plugin_name).cloned();
        let mut resolved_plugin_id = plugin_id.to_string();
        let mut resolved_source_kind = source_kind.clone();
        let mut enabled = existing.as_ref().map(|item| item.enabled).unwrap_or(true);
        let mut loaded = existing.as_ref().map(|item| item.loaded).unwrap_or(false);
        let mut resolved_version = manifest_version.clone();

        if let Some(state_repository) = &self.state {
            if let Err(error) = state_repository
                .upsert_profile_plugin(
                    plugin_id,
                    &plugin_name,
                    &source_kind,
                    &manifest_version,
                    default_enabled,
                    required,
                )
                .await
            {
                tracing::warn!(
                    plugin = plugin_name,
                    error = %error,
                    "failed to upsert plugin runtime state during manifest registration"
                );
            }

            match state_repository.get_by_name(&plugin_name).await {
                Ok(Some(state)) => {
                    resolved_plugin_id = state.plugin_id.to_string();
                    resolved_source_kind = state.source_kind;
                    enabled = state.enabled;
                    loaded = state.loaded;
                    if !state.version.is_empty() {
                        resolved_version = state.version;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        plugin = plugin_name,
                        error = %error,
                        "failed to read plugin runtime state during manifest registration"
                    );
                }
            }
        }

        self.plugin_info.write().await.insert(
            plugin_name.clone(),
            PluginInfo {
                plugin_id: resolved_plugin_id,
                source_kind: resolved_source_kind,
                name: plugin_name,
                version: resolved_version,
                description: manifest.plugin.description.clone(),
                enabled,
                loaded,
                permissions: permission_view(effective_permissions),
            },
        );
    }

    pub(crate) async fn mark_loaded(&self, plugin_name: &str, loaded: bool) {
        if let Some(state_repository) = &self.state {
            if let Err(error) = state_repository.set_loaded(plugin_name, loaded).await {
                tracing::warn!(
                    plugin = plugin_name,
                    loaded,
                    error = %error,
                    "failed to persist plugin loaded state"
                );
            }
        }

        let mut plugin_info = self.plugin_info.write().await;
        if let Some(item) = plugin_info.get_mut(plugin_name) {
            item.loaded = loaded;
            return;
        }
        plugin_info.insert(
            plugin_name.to_string(),
            PluginInfo {
                plugin_id: plugin_name.to_string(),
                source_kind: "third_party".to_string(),
                name: plugin_name.to_string(),
                version: String::new(),
                description: String::new(),
                enabled: true,
                loaded,
                permissions: permission_view(&Permissions::default()),
            },
        );
    }

    pub(crate) async fn is_required(&self, plugin_name: &str) -> bool {
        self.required_plugins.read().await.contains(plugin_name)
    }

    async fn set_required(&self, plugin_name: &str, required: bool) {
        let mut required_plugins = self.required_plugins.write().await;
        if required {
            required_plugins.insert(plugin_name.to_string());
        } else {
            required_plugins.remove(plugin_name);
        }
    }

    pub(crate) async fn list(&self) -> Vec<PluginInfo> {
        let mut plugins = self
            .plugin_info
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(state_repository) = &self.state {
            for plugin in &mut plugins {
                match state_repository.get_by_name(&plugin.name).await {
                    Ok(Some(state)) => apply_state(plugin, state),
                    Ok(None) => {}
                    Err(error) => tracing::warn!(
                        plugin = plugin.name,
                        error = %error,
                        "failed to read plugin runtime state while listing plugins"
                    ),
                }
            }
            let mut plugin_info = self.plugin_info.write().await;
            for plugin in &plugins {
                if let Some(item) = plugin_info.get_mut(&plugin.name) {
                    *item = plugin.clone();
                }
            }
        }
        plugins.sort_by(|left, right| left.name.cmp(&right.name));
        plugins
    }

    pub(crate) async fn set_enabled_intent(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<PluginInfo, String> {
        let known_plugin = self
            .plugin_info
            .read()
            .await
            .get(plugin_name)
            .cloned()
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
        if let Some(state_repository) = &self.state {
            state_repository
                .upsert_discovered_plugin(
                    &known_plugin.plugin_id,
                    &known_plugin.name,
                    &known_plugin.source_kind,
                    &known_plugin.version,
                )
                .await?;
            let state = state_repository
                .set_enabled(plugin_name, enabled, actor, reason)
                .await?;
            let mut info = self.plugin_info.write().await;
            let item = info
                .entry(plugin_name.to_string())
                .or_insert_with(|| known_plugin.clone());
            apply_state(item, state);
            return Ok(item.clone());
        }

        let mut info = self.plugin_info.write().await;
        let item = info
            .get_mut(plugin_name)
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
        item.enabled = enabled;
        Ok(item.clone())
    }

    pub(crate) async fn runtime_enabled(&self, plugin_name: &str) -> Result<bool, String> {
        if let Some(state_repository) = &self.state {
            if let Some(state) = state_repository.get_by_name(plugin_name).await? {
                let enabled = state.enabled;
                let mut info = self.plugin_info.write().await;
                let item = info
                    .entry(plugin_name.to_string())
                    .or_insert_with(|| empty_plugin_info(plugin_name));
                apply_state(item, state);
                return Ok(enabled);
            }
        }
        self.plugin_info
            .read()
            .await
            .get(plugin_name)
            .map(|plugin| plugin.enabled)
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))
    }
}

fn permission_view(permissions: &Permissions) -> PluginPermissionsView {
    PluginPermissionsView {
        routes: permissions.routes,
        commands: permissions.commands,
        admin: permissions.admin,
        database: match permissions.database {
            DatabasePermission::None => "none",
            DatabasePermission::ReadOnly => "read",
            DatabasePermission::Write => "write",
            DatabasePermission::Admin => "admin",
        }
        .to_string(),
    }
}

fn empty_plugin_info(plugin_name: &str) -> PluginInfo {
    PluginInfo {
        plugin_id: plugin_name.to_string(),
        source_kind: "third_party".to_string(),
        name: plugin_name.to_string(),
        version: String::new(),
        description: String::new(),
        enabled: true,
        loaded: false,
        permissions: permission_view(&Permissions::default()),
    }
}

fn apply_state(plugin: &mut PluginInfo, state: super::state_repository::StoredPluginState) {
    plugin.plugin_id = state.plugin_id.to_string();
    plugin.source_kind = state.source_kind;
    plugin.enabled = state.enabled;
    plugin.loaded = state.loaded;
    if !state.version.is_empty() {
        plugin.version = state.version;
    }
}
